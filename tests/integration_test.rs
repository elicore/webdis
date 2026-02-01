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
    let json_val = r#"{"a":1,"b":"c"}"
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
/// - Commands can be sent as JSON arrays: `[