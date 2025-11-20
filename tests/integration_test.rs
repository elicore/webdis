//! Integration tests for Webdis
//!
//! This module contains end-to-end integration tests that validate the complete
//! functionality of the Webdis server, including:
//! - HTTP-to-Redis command translation
//! - WebSocket support and command execution
//! - Pub/Sub functionality over WebSockets
//! - ACL (Access Control List) enforcement
//! - Request size limits and DoS protection
//!
//! Tests use a real Webdis instance with a temporary configuration file and
//! dynamically allocated ports to avoid conflicts.

use reqwest::Client;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

use std::io::Write;
use tempfile::NamedTempFile;

/// Test server instance that manages a Webdis process for integration testing.
///
/// This struct handles:
/// - Building the Webdis binary
/// - Creating temporary configuration files
/// - Allocating a free port dynamically
/// - Starting the Webdis process
/// - Automatic cleanup on drop
struct TestServer {
    /// The running Webdis process
    process: Child,
    /// Temporary config file (kept alive for the duration of the test)
    /// The underscore prefix indicates it's kept for RAII cleanup
    _config_file: NamedTempFile,
    /// The port on which Webdis is listening
    pub port: u16,
}

impl TestServer {
    /// Creates a new test server with default configuration.
    ///
    /// This is a convenience wrapper around `new_with_limit(None)`.
    async fn new() -> Self {
        Self::new_with_limit(None).await
    }

    /// Creates a new test server with an optional request size limit.
    ///
    /// # Arguments
    /// * `limit` - Optional maximum request body size in bytes. If `None`, uses server defaults.
    ///
    /// # Process
    /// 1. Builds the Webdis binary to ensure it's up-to-date
    /// 2. Creates a temporary JSON configuration file
    /// 3. Finds a free port by binding to port 0 and reading the assigned port
    /// 4. Generates configuration with ACL rules for testing
    /// 5. Spawns the Webdis process
    /// 6. Waits 2 seconds for the server to start
    ///
    /// # Panics
    /// Panics if:
    /// - The build fails
    /// - Cannot create a temporary file
    /// - Cannot bind to a port
    /// - Cannot start the Webdis process
    async fn new_with_limit(limit: Option<usize>) -> Self {
        // Build the project first to ensure binary is up to date
        let status = Command::new("cargo")
            .arg("build")
            .status()
            .expect("Failed to build project");
        assert!(status.success());

        // Create a temporary config file that will be automatically deleted when dropped
        let mut config_file = tempfile::Builder::new()
            .suffix(".json")
            .tempfile()
            .expect("Failed to create temp config file");

        // Find a free port by binding to port 0, which lets the OS assign an available port
        // We immediately drop the listener so Webdis can bind to it
        let port = {
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
            listener.local_addr().unwrap().port()
        };

        // Generate test configuration with ACL rules:
        // - DEBUG command is disabled by default (for ACL testing)
        // - DEBUG command is enabled when authenticated with "user:password"
        // - WebSockets are enabled for WebSocket tests
        // - http_max_request_size is configurable for limit testing
        let config_content = serde_json::json!({
            "redis_host": "127.0.0.1",
            "redis_port": 6379,
            "http_host": "127.0.0.1",
            "http_port": port,
            "database": 0,
            "websockets": true,
            "daemonize": false,
            "verbosity": 5,
            "logfile": "webdis.log",
            "http_max_request_size": limit,
            "acl": [
                {
                    "disabled": ["DEBUG"]
                },
                {
                    "http_basic_auth": "user:password",
                    "enabled": ["DEBUG"]
                }
            ]
        });

        write!(config_file, "{}", config_content.to_string()).expect("Failed to write config");

        let config_path = config_file.path().to_str().unwrap().to_string();

        // Spawn the Webdis process with the temporary config
        let process = Command::new("target/debug/webdis")
            .arg(&config_path)
            .spawn()
            .expect("Failed to start webdis");

        // Give the server time to start up and bind to the port
        // This is a simple approach; production code might poll the port instead
        sleep(Duration::from_secs(2)).await;

        Self {
            process,
            _config_file: config_file,
            port,
        }
    }
}

impl Drop for TestServer {
    /// Automatically kills the Webdis process when the TestServer is dropped.
    ///
    /// This ensures cleanup even if a test panics or fails.
    /// The `let _ =` ignores errors (e.g., if the process already exited).
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

/// Tests basic HTTP GET/SET operations via Webdis.
///
/// This test validates:
/// - HTTP GET requests are translated to Redis SET commands
/// - HTTP GET requests are translated to Redis GET commands
/// - Responses are properly formatted as JSON
/// - The command name is used as the JSON key
/// - Values are correctly stored and retrieved
#[tokio::test]
async fn test_basic_get_set() {
    let server = TestServer::new().await;
    let client = Client::new();

    // SET
    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/test_key/test_value",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["SET"], "OK");

    // GET
    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/test_key", server.port))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "test_value");
}

/// Tests JSON value handling through Webdis.
///
/// This test validates:
/// - Complex JSON strings can be stored in Redis via Webdis
/// - Retrieved JSON values match what was stored
/// - Redis stores JSON as a string (not parsed)
/// - Webdis returns the JSON string without modification
#[tokio::test]
async fn test_json_output() {
    let server = TestServer::new().await;
    let client = Client::new();

    // SET JSON
    let json_val = r#"{"a":1,"b":"c"}"#;
    let _ = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/json_key/{}",
            server.port, json_val
        ))
        .send()
        .await
        .expect("Failed to send request");

    // GET JSON
    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/json_key", server.port))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    // Redis stores JSON as a string, so Webdis returns it as-is
    // This matches the behavior of the original C implementation
    assert_eq!(body["GET"], json_val);
}

/// Tests Access Control List (ACL) enforcement.
///
/// This test validates:
/// - Commands disabled in ACL return 403 Forbidden
/// - HTTP Basic Authentication is properly validated
/// - Authenticated requests can access restricted commands
/// - ACL rules are evaluated in order
///
/// The test configuration has:
/// - DEBUG command disabled by default
/// - DEBUG command enabled for "user:password" authentication
#[tokio::test]
async fn test_acl_restrictions() {
    let server = TestServer::new().await;
    let client = Client::new();

    // DEBUG is disabled by default in webdis.json
    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/DEBUG/OBJECT/test_key",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    // Unauthenticated request should be denied
    assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);

    // Authenticated request should be allowed (if configured)
    // In webdis.json: "http_basic_auth": "user:password", "enabled": ["DEBUG"]
    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/DEBUG/OBJECT/test_key",
            server.port
        ))
        .basic_auth("user", Some("password"))
        .send()
        .await
        .expect("Failed to send request");

    // Authenticated request should be allowed
    // The command might fail (400/500) if the key doesn't exist,
    // but it should NOT be forbidden (403)
    assert_ne!(resp.status(), reqwest::StatusCode::FORBIDDEN);
}

/// Tests WebSocket command execution.
///
/// This test validates:
/// - WebSocket connections can be established to `/.json` endpoint
/// - Commands can be sent as JSON arrays: `["COMMAND", "arg1", "arg2"]`
/// - Responses are received as JSON objects: `{"COMMAND": result}`
/// - Multiple commands can be executed over the same connection
/// - SET and GET operations work correctly over WebSocket
#[tokio::test]
async fn test_websocket_commands() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    let server = TestServer::new().await;
    let url = format!("ws://127.0.0.1:{}/.json", server.port);
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    // Send SET command as JSON array
    let cmd = serde_json::json!(["SET", "ws_key", "ws_value"]).to_string();
    ws_stream
        .send(Message::Text(cmd.into()))
        .await
        .expect("Failed to send SET");

    // Receive response
    let msg = ws_stream
        .next()
        .await
        .expect("Stream ended")
        .expect("Failed to receive");
    if let Message::Text(text) = msg {
        let body: serde_json::Value = serde_json::from_str(&text).expect("Failed to parse JSON");
        assert_eq!(body["SET"], "OK");
    } else {
        panic!("Expected text message");
    }

    // Send GET command
    let cmd = serde_json::json!(["GET", "ws_key"]).to_string();
    ws_stream
        .send(Message::Text(cmd.into()))
        .await
        .expect("Failed to send GET");

    // Receive response
    let msg = ws_stream
        .next()
        .await
        .expect("Stream ended")
        .expect("Failed to receive");
    if let Message::Text(text) = msg {
        let body: serde_json::Value = serde_json::from_str(&text).expect("Failed to parse JSON");
        assert_eq!(body["GET"], "ws_value");
    } else {
        panic!("Expected text message");
    }
}

/// Tests Redis Pub/Sub functionality over WebSocket.
///
/// This test validates:
/// - SUBSCRIBE command works over WebSocket
/// - Messages published via HTTP are delivered to WebSocket subscribers
/// - Message format matches Redis Pub/Sub protocol
/// - Cross-protocol communication (HTTP publish → WebSocket receive)
///
/// This is a critical test for real-time messaging applications.
#[tokio::test]
async fn test_websocket_pubsub() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    let server = TestServer::new().await;
    let url = format!("ws://127.0.0.1:{}/.json", server.port);
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    // Subscribe
    let cmd = serde_json::json!(["SUBSCRIBE", "ws_channel"]).to_string();
    ws_stream
        .send(Message::Text(cmd.into()))
        .await
        .expect("Failed to send SUBSCRIBE");

    // Wait for subscription to be processed by Redis
    // This is necessary because SUBSCRIBE is asynchronous
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Publish a message via HTTP (different protocol than subscriber)
    let client = Client::new();
    client
        .post(&format!(
            "http://127.0.0.1:{}/PUBLISH/ws_channel",
            server.port
        ))
        .body("ws_message")
        .send()
        .await
        .expect("Failed to publish");

    // Receive message
    let msg = ws_stream
        .next()
        .await
        .expect("Stream ended")
        .expect("Failed to receive");
    if let Message::Text(text) = msg {
        let body: serde_json::Value = serde_json::from_str(&text).expect("Failed to parse JSON");
        assert_eq!(body["message"], "ws_message");
    } else {
        panic!("Expected text message");
    }
}

/// Tests server protection against oversized URIs.
///
/// This test validates:
/// - Server rejects URIs that exceed reasonable size limits
/// - Returns appropriate HTTP error codes (414, 431, or 400)
/// - Connection is properly closed after rejection
/// - Server doesn't crash or hang on huge requests
///
/// This is a security test to prevent DoS attacks via large URIs.
/// The test sends a 1MB URI, which should be rejected by Hyper's default limits.
#[tokio::test]
async fn test_huge_url() {
    // Set a 1MB body limit (though this test focuses on URI size)
    let limit = 1024 * 1024;
    let server = TestServer::new_with_limit(Some(limit)).await;

    // Use raw TCP to send malformed/oversized requests
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", server.port))
        .await
        .expect("Failed to connect");

    // Construct an oversized URI (1MB path)
    // Hyper's default max_uri_size is ~65KB, so this should be rejected
    // The original limits.py tested a 1GB query string, but 1MB is sufficient
    // to trigger the protection without consuming excessive memory
    let huge_path = "A".repeat(1024 * 1024); // 1MB of 'A' characters
    let request = format!("GET /GET/{} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n", huge_path);

    // Attempt to send the huge request
    // The server may close the connection before we finish writing
    let _ = stream.write_all(request.as_bytes()).await;

    // Read the server's response
    let mut buffer = [0; 1024];
    let n = stream
        .read(&mut buffer)
        .await
        .expect("Failed to read response");
    let response = String::from_utf8_lossy(&buffer[..n]);

    // Verify the server rejected the request with an appropriate error code:
    // - 414 URI Too Long: Standard HTTP error for oversized URIs
    // - 431 Request Header Fields Too Large: Alternative error for large headers
    // - 400 Bad Request: Generic error for malformed requests
    assert!(
        response.contains("414 URI Too Long")
            || response.contains("400 Bad Request")
            || response.contains("431 Request Header Fields Too Large"),
        "Unexpected response: {}",
        response
    );
}

/// Tests request body size limit enforcement.
///
/// This test validates:
/// - Server respects `http_max_request_size` configuration
/// - `Expect: 100-continue` header is handled correctly per HTTP/1.1 spec
/// - Server sends 100 Continue before client sends body
/// - Uploads exceeding the limit are rejected or fail gracefully
/// - Server doesn't crash or hang on oversized uploads
///
/// This is a security test to prevent DoS attacks via large request bodies.
/// The test attempts to upload 10MB when the limit is 1MB.
#[tokio::test]
async fn test_huge_upload() {
    // Configure server with 1MB request body limit
    let limit = 1024 * 1024;
    let server = TestServer::new_with_limit(Some(limit)).await;

    // Use raw TCP to control the upload process
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", server.port))
        .await
        .expect("Failed to connect");

    // Attempt to upload 10MB (10x the limit)
    let content_length = 10 * 1024 * 1024;

    // Use Expect: 100-continue to test HTTP/1.1 compliance
    // The server should send "100 Continue" before we send the body
    let headers = format!(
        "PUT /SET/huge_key HTTP/1.1\r\n\
         Host: 127.0.0.1\r\n\
         Content-Length: {}\r\n\
         Expect: 100-continue\r\n\
         \r\n",
        content_length
    );

    stream
        .write_all(headers.as_bytes())
        .await
        .expect("Failed to write headers");

    // Wait for "100 Continue" response from server
    // This confirms the server is willing to accept the request body
    let mut buffer = [0; 1024];
    let n = stream
        .read(&mut buffer)
        .await
        .expect("Failed to read 100 continue");
    let response = String::from_utf8_lossy(&buffer[..n]);

    assert!(
        response.contains("100 Continue"),
        "Expected 100 Continue, got: {}",
        response
    );

    // Attempt to send the 10MB body in chunks
    // The server should either:
    // 1. Close the connection when the limit is exceeded
    // 2. Accept the data but return an error response
    let chunk_size = 64 * 1024; // 64KB chunks
    let chunk = vec![b'A'; chunk_size];
    let mut sent = 0;
    let mut failed = false;

    // Keep sending until we hit the limit or the write fails
    while sent < content_length {
        match stream.write_all(&chunk).await {
            Ok(_) => {
                sent += chunk_size;
            }
            Err(_) => {
                failed = true;
                break;
            }
        }
    }

    // Validate that the upload was rejected:
    // - Either the write failed (connection closed)
    // - Or we got an error response (413 Payload Too Large or 400 Bad Request)
    //
    // Note: The exact behavior depends on how Axum/Hyper handles the limit:
    // - It might close the connection immediately
    // - It might buffer up to the limit, then return an error
    // - OS socket buffers might allow some writes even after the server stops reading

    if !failed {
        // Write succeeded (possibly due to OS buffering)
        // Check if we got an error response
        let n = stream.read(&mut buffer).await.unwrap_or(0);
        if n > 0 {
            let response = String::from_utf8_lossy(&buffer[..n]);
            assert!(
                response.contains("413 Payload Too Large") || response.contains("400 Bad Request"),
                "Expected error response, got: {}",
                response
            );
        } else {
            // Connection closed without response - this is acceptable
            // The server enforced the limit by closing the connection
        }
    } else {
        // Write failed - this is the expected behavior
        // The server closed the connection when the limit was exceeded
    }
}
