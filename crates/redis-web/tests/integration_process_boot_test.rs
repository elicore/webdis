mod support;

use reqwest::Client;
use std::fs;
use support::process_harness::{
    ensure_redis_web_debug_binaries, redis_web_binary_path, webdis_binary_path, TestServer,
};
use tempfile::{Builder, TempDir};

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
        "verbosity": 4
    });

    let mut config_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");
    use std::io::Write;
    write!(config_file, "{}", config_content).expect("Failed to write config");

    ensure_redis_web_debug_binaries();
    let output = std::process::Command::new(redis_web_binary_path())
        .arg(config_file.path())
        .output()
        .expect("Failed to run redis-web");

    assert!(!output.status.success());
}

#[test]
fn test_redis_web_uses_redis_web_json_by_default() {
    ensure_redis_web_debug_binaries();
    let tmp = TempDir::new().expect("temp dir should be created");
    fs::write(
        tmp.path().join("redis-web.json"),
        r#"{"redis_host":"$REDIS_WEB_SELECTED"}"#,
    )
    .expect("redis-web config should be written");
    fs::write(
        tmp.path().join("webdis.json"),
        r#"{"redis_host":"$WEBDIS_SELECTED"}"#,
    )
    .expect("webdis config should be written");

    let output = std::process::Command::new(redis_web_binary_path())
        .current_dir(tmp.path())
        .output()
        .expect("redis-web should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("REDIS_WEB_SELECTED"),
        "expected canonical default config selection, got stderr: {stderr}"
    );
}

#[test]
fn test_redis_web_falls_back_to_webdis_json() {
    ensure_redis_web_debug_binaries();
    let tmp = TempDir::new().expect("temp dir should be created");
    fs::write(
        tmp.path().join("webdis.json"),
        r#"{"redis_host":"$WEBDIS_FALLBACK"}"#,
    )
    .expect("webdis config should be written");

    let output = std::process::Command::new(redis_web_binary_path())
        .current_dir(tmp.path())
        .output()
        .expect("redis-web should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WEBDIS_FALLBACK"),
        "expected fallback config selection, got stderr: {stderr}"
    );
}

#[test]
fn test_redis_web_prefers_redis_web_min_json_when_present() {
    ensure_redis_web_debug_binaries();
    let tmp = TempDir::new().expect("temp dir should be created");
    fs::write(
        tmp.path().join("redis-web.min.json"),
        r#"{"redis_host":"$REDIS_WEB_MIN_SELECTED"}"#,
    )
    .expect("redis-web.min config should be written");
    fs::write(
        tmp.path().join("webdis.json"),
        r#"{"redis_host":"$WEBDIS_SELECTED"}"#,
    )
    .expect("webdis config should be written");

    let output = std::process::Command::new(redis_web_binary_path())
        .current_dir(tmp.path())
        .output()
        .expect("redis-web should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("REDIS_WEB_MIN_SELECTED"),
        "expected minimal starter config selection, got stderr: {stderr}"
    );
}

#[test]
fn test_webdis_alias_prints_deprecation_notice() {
    ensure_redis_web_debug_binaries();
    let tmp = TempDir::new().expect("temp dir should be created");
    fs::write(
        tmp.path().join("webdis.json"),
        r#"{"redis_host":"$WEBDIS_ALIAS"}"#,
    )
    .expect("webdis config should be written");

    let output = std::process::Command::new(webdis_binary_path())
        .current_dir(tmp.path())
        .output()
        .expect("webdis alias should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[deprecated] `webdis` is an alias for `redis-web`"),
        "expected deprecation notice, got stderr: {stderr}"
    );
}

#[test]
fn test_write_default_config_uses_binary_specific_schema() {
    ensure_redis_web_debug_binaries();
    let tmp = TempDir::new().expect("temp dir should be created");

    let redis_web = std::process::Command::new(redis_web_binary_path())
        .current_dir(tmp.path())
        .arg("--write-default-config")
        .output()
        .expect("redis-web should write default config");
    assert!(
        redis_web.status.success(),
        "redis-web write failed: {}",
        String::from_utf8_lossy(&redis_web.stderr)
    );
    let canonical = fs::read_to_string(tmp.path().join("redis-web.json"))
        .expect("canonical default config should exist");
    assert!(canonical.contains("\"$schema\": \"./redis-web.schema.json\""));

    let alias_path = tmp.path().join("webdis.generated.json");
    let webdis = std::process::Command::new(webdis_binary_path())
        .current_dir(tmp.path())
        .arg("--write-default-config")
        .arg("--config")
        .arg(alias_path.as_os_str())
        .output()
        .expect("webdis alias should write default config");
    assert!(
        webdis.status.success(),
        "webdis write failed: {}",
        String::from_utf8_lossy(&webdis.stderr)
    );
    let legacy = fs::read_to_string(alias_path).expect("legacy default config should exist");
    assert!(legacy.contains("\"$schema\": \"./webdis.schema.json\""));
}

#[test]
fn test_write_minimal_config_uses_binary_specific_schema() {
    ensure_redis_web_debug_binaries();
    let tmp = TempDir::new().expect("temp dir should be created");

    let redis_web = std::process::Command::new(redis_web_binary_path())
        .current_dir(tmp.path())
        .arg("--write-minimal-config")
        .output()
        .expect("redis-web should write minimal config");
    assert!(
        redis_web.status.success(),
        "redis-web write failed: {}",
        String::from_utf8_lossy(&redis_web.stderr)
    );
    let canonical = fs::read_to_string(tmp.path().join("redis-web.min.json"))
        .expect("canonical minimal config should exist");
    assert!(canonical.contains("\"$schema\": \"./redis-web.schema.json\""));
    assert!(canonical.contains("\"http_host\": \"127.0.0.1\""));

    let alias_path = tmp.path().join("webdis.min.generated.json");
    let webdis = std::process::Command::new(webdis_binary_path())
        .current_dir(tmp.path())
        .arg("--write-minimal-config")
        .arg("--config")
        .arg(alias_path.as_os_str())
        .output()
        .expect("webdis alias should write minimal config");
    assert!(
        webdis.status.success(),
        "webdis write failed: {}",
        String::from_utf8_lossy(&webdis.stderr)
    );
    let legacy = fs::read_to_string(alias_path).expect("legacy minimal config should exist");
    assert!(legacy.contains("\"$schema\": \"./webdis.schema.json\""));
}
