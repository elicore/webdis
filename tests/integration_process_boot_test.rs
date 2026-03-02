mod support;

use reqwest::Client;
use support::process_harness::{ensure_webdis_debug_binary, TestServer};
use tempfile::Builder;

#[tokio::test]
async fn test_basic_get_set() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/SET/test_key/test_value",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/test_key", server.port))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "test_value");
}

#[tokio::test]
async fn test_env_var_expansion_end_to_end() {
    let config_content = serde_json::json!({
        "redis_host": "$REDIS_HOST",
        "redis_port": "$REDIS_PORT",
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "websockets": false,
        "daemonize": false,
        "verbosity": 4
    });

    let config_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");

    let server = TestServer::spawn_with_config_and_env(
        config_file,
        config_content,
        &[("REDIS_HOST", "127.0.0.1"), ("REDIS_PORT", "6379")],
    )
    .await;

    let client = Client::new();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/SET/env_expand_key/env_expand_value",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_acl_restrictions() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/DEBUG/OBJECT/test_key",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/DEBUG/OBJECT/test_key",
            server.port
        ))
        .basic_auth("user", Some("password"))
        .send()
        .await
        .expect("Failed to send request");
    assert_ne!(resp.status(), reqwest::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_invalid_unix_socket_path_fails_fast() {
    let config_content = serde_json::json!({
        "redis_socket": "/path/that/does/not/exist.sock",
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "websockets": false,
        "daemonize": false,
        "verbosity": 4
    });

    let mut config_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");
    use std::io::Write;
    write!(config_file, "{}", config_content).expect("Failed to write config");

    ensure_webdis_debug_binary();
    let output = std::process::Command::new("target/debug/webdis")
        .arg(config_file.path())
        .output()
        .expect("Failed to run webdis");

    assert!(!output.status.success());
}
