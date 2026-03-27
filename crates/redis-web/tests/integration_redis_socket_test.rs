mod support;

use reqwest::Client;
use support::process_harness::{RedisUnixSocketServer, TestServer};

#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_basic_connectivity() {
    let redis = RedisUnixSocketServer::start().await;

    let config_content = serde_json::json!({
        "redis_socket": redis.socket_path().display().to_string(),
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "websockets": false,
        "verbosity": 4
    });

    let config_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");

    let server = TestServer::spawn_with_config_and_env(config_file, config_content, &[]).await;
    let client = Client::new();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/SET/unix_socket_key/unix_socket_value",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/GET/unix_socket_key",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "unix_socket_value");
}

#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_precedence_over_tcp() {
    let redis = RedisUnixSocketServer::start().await;

    let config_content = serde_json::json!({
        "redis_host": "192.0.2.1",
        "redis_port": 1,
        "redis_socket": redis.socket_path().display().to_string(),
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "websockets": false,
        "verbosity": 4
    });

    let config_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");

    let server = TestServer::spawn_with_config_and_env(config_file, config_content, &[]).await;
    let client = Client::new();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/SET/unix_precedence_key/ok",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
}
