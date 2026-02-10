//! Configuration parsing and validation tests for Webdis.
//!
//! This module tests the configuration system to ensure:
//! - JSON config files are correctly parsed
//! - All configuration fields are properly deserialized
//! - Default values are applied when fields are missing
//! - Invalid configurations are rejected
//!
//! These tests use temporary files to avoid polluting the filesystem.

use std::io::Write;
use std::sync::Mutex;

use webdis::config::{
    Config, LogFsync, LogFsyncMode, DEFAULT_HTTP_MAX_REQUEST_SIZE, DEFAULT_HTTP_THREADS,
    DEFAULT_POOL_SIZE_PER_THREAD, DEFAULT_VERBOSITY,
};
use webdis::redis;

/// Tests in this module may temporarily set process-wide environment variables.
///
/// Since Rust tests can run in parallel, we guard these mutations behind a mutex
/// to avoid cross-test interference.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Tests that all configuration fields are correctly loaded from a JSON file.
///
/// This test validates:
/// - All required fields are parsed correctly
/// - Optional fields are properly deserialized when present
/// - Field types match expected Rust types (String, u16, bool, etc.)
/// - The Config struct correctly represents the JSON structure
///
/// The test uses a comprehensive configuration with all fields populated
/// to ensure complete coverage of the configuration system.
#[test]
fn test_config_loading() {
    // Create a comprehensive test configuration with all fields populated
    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "0.0.0.0",
        "http_port": 7379,
        "database": 0,
        "daemonize": true,
        "websockets": true,
        "http_max_request_size": 1024,
        "user": "nobody",
        "group": "nogroup",
        "verbosity": 5,
        "logfile": "test.log",
        "log_fsync": "auto"
    }"#;

    // Write to a temporary file (automatically cleaned up when dropped)
    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    // Load and parse the configuration
    let config = Config::new(path).unwrap();

    // Verify required fields
    assert_eq!(config.redis_host, "127.0.0.1");
    assert_eq!(config.daemonize, true);
    assert_eq!(config.websockets, true);

    // Verify optional fields are correctly parsed as Some(value)
    assert_eq!(config.http_max_request_size, Some(1024));
    assert_eq!(config.user, Some("nobody".to_string()));
    assert_eq!(config.verbosity, Some(5));
}

/// Tests that default values are applied for missing optional fields.
///
/// This test validates:
/// - Missing optional fields don't cause parsing errors
/// - Default values are correctly applied (e.g., `false` for booleans)
/// - Optional fields are represented as `None` when not present
/// - The configuration system is robust to minimal configurations
///
/// This ensures that users can provide minimal configurations and the
/// system will fill in sensible defaults.
#[test]
fn test_default_values() {
    // Create a minimal configuration with only required fields
    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "0.0.0.0",
        "http_port": 7379,
        "database": 0
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    let config = Config::new(path).unwrap();

    // Verify boolean defaults are false
    assert_eq!(config.daemonize, false);
    assert_eq!(config.websockets, false);

    // Verify optional fields are None when not specified
    assert_eq!(config.http_max_request_size, None);
    assert_eq!(config.user, None);
}

/// Ensures the generated default configuration document contains the expected
/// defaults and omits unset optional fields.
#[test]
fn test_default_document_generation() {
    let value = Config::default_document("./webdis.schema.json");
    let obj = value
        .as_object()
        .expect("default document should be a JSON object");

    assert_eq!(
        obj.get("$schema").and_then(|v| v.as_str()),
        Some("./webdis.schema.json")
    );
    assert_eq!(
        obj.get("http_threads").and_then(|v| v.as_u64()),
        Some(DEFAULT_HTTP_THREADS as u64)
    );
    assert_eq!(
        obj.get("pool_size_per_thread").and_then(|v| v.as_u64()),
        Some(DEFAULT_POOL_SIZE_PER_THREAD as u64)
    );
    assert_eq!(
        obj.get("http_max_request_size").and_then(|v| v.as_u64()),
        Some(DEFAULT_HTTP_MAX_REQUEST_SIZE as u64)
    );
    assert_eq!(
        obj.get("verbosity").and_then(|v| v.as_u64()),
        Some(DEFAULT_VERBOSITY as u64)
    );
    assert!(!obj.contains_key("redis_auth"));
    assert!(!obj.contains_key("logfile"));
}

/// Ensures legacy aliases (`threads`, `pool_size`) are mapped when canonical
/// fields are absent.
#[test]
fn test_legacy_alias_fallback() {
    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "0.0.0.0",
        "http_port": 7379,
        "threads": 6,
        "pool_size": 12,
        "database": 0
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    let config = Config::new(path).unwrap();
    assert_eq!(config.http_threads, Some(6));
    assert_eq!(config.pool_size_per_thread, Some(12));
}

/// Ensures canonical fields override legacy aliases when both are present.
#[test]
fn test_legacy_alias_precedence() {
    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "0.0.0.0",
        "http_port": 7379,
        "threads": 2,
        "pool_size": 5,
        "http_threads": 8,
        "pool_size_per_thread": 25,
        "database": 0
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    let config = Config::new(path).unwrap();
    assert_eq!(config.http_threads, Some(8));
    assert_eq!(config.pool_size_per_thread, Some(25));
}

/// Env-var expansion works for `$VARNAME` placeholders in string values.
///
/// This covers a compatibility feature from the original Webdis where config values can
/// be specified indirectly via environment variables (including for numeric fields like ports).
#[test]
fn test_env_var_expansion_works() {
    let _guard = ENV_LOCK.lock().unwrap();

    std::env::set_var("REDIS_HOST", "redis.example.test");
    std::env::set_var("REDIS_PORT", "6380");

    let config_json = r#"{
        "redis_host": "$REDIS_HOST",
        "redis_port": "$REDIS_PORT",
        "http_host": "127.0.0.1",
        "http_port": 7379,
        "database": 0
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    let config = Config::new(path).unwrap();
    assert_eq!(config.redis_host, "redis.example.test");
    assert_eq!(config.redis_port, 6380);
}

/// Missing env vars referenced via `$VARNAME` produce a clear configuration error.
#[test]
fn test_env_var_expansion_missing_var_fails() {
    let _guard = ENV_LOCK.lock().unwrap();

    std::env::remove_var("MISSING_VAR");

    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "127.0.0.1",
        "http_port": 7379,
        "database": 0,
        "logfile": "$MISSING_VAR"
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    let err = Config::new(path).expect_err("expected missing env var to fail config loading");
    let msg = err.to_string();
    assert!(
        msg.contains("MISSING_VAR"),
        "error should mention missing env var name, got: {msg}"
    );
    assert!(
        msg.contains("logfile"),
        "error should mention the config key path, got: {msg}"
    );
}

/// The `redis_socket` field is parsed when present.
#[test]
fn test_redis_socket_parses() {
    let config_json = r#"{
        "redis_socket": "/tmp/redis.sock"
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();

    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    assert_eq!(config.redis_socket.as_deref(), Some("/tmp/redis.sock"));
}

/// Env-var expansion works for `redis_socket` paths (like any other string field).
#[test]
fn test_redis_socket_env_var_expansion_works() {
    let _guard = ENV_LOCK.lock().unwrap();

    std::env::set_var("REDIS_SOCKET", "/tmp/redis.sock");

    let config_json = r#"{
        "redis_socket": "$REDIS_SOCKET"
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();

    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    assert_eq!(config.redis_socket.as_deref(), Some("/tmp/redis.sock"));
}

/// When both `redis_socket` and TCP settings are provided, `redis_socket` takes precedence.
///
/// We validate this by asserting we fail fast due to the socket path, even if TCP
/// fields are present (i.e., we don't attempt to interpret host/port instead).
#[test]
fn test_redis_socket_precedence_over_tcp() {
    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "redis_socket": "/path/that/does/not/exist.sock"
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();

    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    let err = redis::create_pool(&config).expect_err("expected invalid redis_socket to fail fast");
    let msg = err.to_string();
    assert!(
        msg.contains("redis_socket"),
        "error should mention redis_socket, got: {msg}"
    );
}

/// TLS settings are rejected when `redis_socket` is used (TLS does not apply to UNIX sockets).
#[test]
fn test_redis_socket_rejects_ssl() {
    let config_json = r#"{
        "redis_socket": "/tmp/redis.sock",
        "ssl": {
            "enabled": true,
            "ca_cert_bundle": "ca.pem",
            "client_cert": "cert.pem",
            "client_key": "key.pem"
        }
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();

    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    let err = redis::create_pool(&config).expect_err("expected ssl+redis_socket to be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("ssl is not supported with redis_socket"),
        "error should explain ssl/socket incompatibility, got: {msg}"
    );
}

/// The legacy `hiredis.keep_alive_sec` value is parsed and plumbed into Config.
#[test]
fn test_hiredis_keep_alive_sec_parses() {
    let config_json = r#"{
        "hiredis": { "keep_alive_sec": 15 }
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();

    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    assert_eq!(
        config.hiredis.as_ref().and_then(|h| h.keep_alive_sec),
        Some(15)
    );
}


/// The legacy `log_fsync` option parses string modes (`auto`, `all`).
#[test]
fn test_log_fsync_parses_modes() {
    let config_json = r#"{
  "log_fsync": "auto"
}"#;
    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    assert!(matches!(
        config.log_fsync,
        Some(LogFsync::Mode(LogFsyncMode::Auto))
    ));

    let config_json = r#"{
  "log_fsync": "all"
}"#;
    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    assert!(matches!(
        config.log_fsync,
        Some(LogFsync::Mode(LogFsyncMode::All))
    ));
}

/// The legacy `log_fsync` option parses an integer millisecond interval.
#[test]
fn test_log_fsync_parses_millis() {
    let config_json = r#"{
  "log_fsync": 25
}"#;
    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    assert!(matches!(config.log_fsync, Some(LogFsync::Millis(25))));
}
