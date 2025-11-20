use reqwest::Client;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::sleep;

use std::io::Write;
use tempfile::NamedTempFile;

struct TestServer {
    process: Child,
    _config_file: NamedTempFile,
    pub port: u16,
}

impl TestServer {
    async fn new() -> Self {
        // Build the project first to ensure binary is up to date
        let status = Command::new("cargo")
            .arg("build")
            .status()
            .expect("Failed to build project");
        assert!(status.success());

        // Create a temporary config file
        let mut config_file = tempfile::Builder::new()
            .suffix(".json")
            .tempfile()
            .expect("Failed to create temp config file");

        // Find a free port
        let port = {
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
            listener.local_addr().unwrap().port()
        };

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

        let process = Command::new("target/debug/webdis")
            .arg(&config_path)
            .spawn()
            .expect("Failed to start webdis");

        // Give it a moment to start
        sleep(Duration::from_secs(2)).await;

        Self {
            process,
            _config_file: config_file,
            port,
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

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
    // Redis stores it as a string, Webdis returns it as a string unless we parse it?
    // Original Webdis returns string for GET.
    assert_eq!(body["GET"], json_val);
}

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

    // Should be 403 Forbidden
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

    // Should be allowed (200 OK or 500/400 from Redis if command fails, but not 403)
    // DEBUG OBJECT might fail if key doesn't exist, but it shouldn't be 403.
    assert_ne!(resp.status(), reqwest::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_websocket_commands() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    let server = TestServer::new().await;
    let url = format!("ws://127.0.0.1:{}/.json", server.port);
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    // Send SET command
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

    // Give it a moment to subscribe
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Publish using HTTP client
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
