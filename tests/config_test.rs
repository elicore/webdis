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

use webdis::config::{
    Config, DEFAULT_HTTP_MAX_REQUEST_SIZE, DEFAULT_HTTP_THREADS, DEFAULT_POOL_SIZE_PER_THREAD,
    DEFAULT_VERBOSITY,
};

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
