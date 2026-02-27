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
use std::process::Stdio;
use std::process::{Child, Command};
use std::sync::Once;
use std::time::Duration;
use tokio::time::sleep;

use base64::{engine::general_purpose, Engine as _};
use std::io::{Seek, SeekFrom, Write};
use tempfile::NamedTempFile;
use tempfile::TempDir;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use redis::aio::MultiplexedConnection;

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

fn pick_unused_local_port() -> u16 {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    listener.local_addr().unwrap().port()
}

/// Parses a JSONP response body in the form `<callback>(<json>)`.
///
/// This intentionally performs minimal validation, matching Webdis' permissive JSONP behavior:
/// we only require a single wrapping `(` ... `)` pair and then parse the enclosed JSON.
fn parse_jsonp_body(body: &str) -> (&str, serde_json::Value) {
    let open = body
        .find('(')
        .unwrap_or_else(|| panic!("Expected '(' in JSONP body, got: {body:?}"));
    let close = body
        .rfind(')')
        .unwrap_or_else(|| panic!("Expected ')' in JSONP body, got: {body:?}"));
    assert_eq!(
        close,
        body.len() - 1,
        "Expected JSONP body to end with ')', got: {body:?}"
    );

    let callback = &body[..open];
    let json_str = &body[open + 1..close];
    let json: serde_json::Value =
        serde_json::from_str(json_str).expect("Expected valid JSON inside JSONP wrapper");

    (callback, json)
}

/// Reads newline-delimited messages from a streaming HTTP response.
///
/// HTTP chunk boundaries are transport-level details and may split logical lines.
/// This helper reassembles lines from streamed chunks until `expected_lines` are read
/// or the per-chunk timeout is reached.
async fn read_stream_lines(
    mut response: reqwest::Response,
    expected_lines: usize,
    per_chunk_timeout: Duration,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(expected_lines);
    let mut buffer = String::new();

    while lines.len() < expected_lines {
        let chunk = tokio::time::timeout(per_chunk_timeout, response.chunk())
            .await
            .expect("timed out waiting for streamed chunk")
            .expect("stream returned an error")
            .expect("stream ended before expected number of lines");

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(idx) = buffer.find('\n') {
            let mut line = buffer[..idx].to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            if !line.is_empty() {
                lines.push(line);
                if lines.len() == expected_lines {
                    break;
                }
            }
            buffer = buffer[idx + 1..].to_string();
        }
    }

    lines
}

/// A Redis server instance that listens on a UNIX-domain socket for the duration of a test.
///
/// Integration tests use this to validate `redis_socket` end-to-end without relying on a
/// preconfigured Redis instance.
#[cfg(unix)]
struct RedisUnixSocketServer {
    socket_path: PathBuf,
    _tempdir: TempDir,
    kind: RedisUnixSocketServerKind,
}

#[cfg(unix)]
enum RedisUnixSocketServerKind {
    Native(Child),
    Docker { container_id: String },
}

#[cfg(unix)]
impl RedisUnixSocketServer {
    async fn start() -> Self {
        let tempdir = tempfile::tempdir().expect("failed to create temp dir for unix socket");
        let socket_path = tempdir.path().join("redis.sock");

        if command_is_available("redis-server", &["--version"]) {
            let child = Command::new("redis-server")
                .arg("--port")
                .arg("0")
                .arg("--unixsocket")
                .arg(&socket_path)
                .arg("--unixsocketperm")
                .arg("700")
                .arg("--save")
                .arg("")
                .arg("--appendonly")
                .arg("no")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("failed to start redis-server");

            wait_for_unix_socket(&socket_path).await;

            return Self {
                socket_path,
                _tempdir: tempdir,
                kind: RedisUnixSocketServerKind::Native(child),
            };
        }

        if command_is_available("docker", &["version"]) {
            let output = Command::new("docker")
                .arg("run")
                .arg("--rm")
                .arg("-d")
                .arg("-v")
                .arg(format!("{}:/data", tempdir.path().display()))
                .arg("redis:8.2-alpine")
                .arg("redis-server")
                .arg("--port")
                .arg("0")
                .arg("--unixsocket")
                .arg("/data/redis.sock")
                .arg("--unixsocketperm")
                .arg("700")
                .arg("--save")
                .arg("")
                .arg("--appendonly")
                .arg("no")
                .output()
                .expect("failed to start redis via docker");

            assert!(
                output.status.success(),
                "docker run failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );

            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            wait_for_unix_socket(&socket_path).await;

            return Self {
                socket_path,
                _tempdir: tempdir,
                kind: RedisUnixSocketServerKind::Docker { container_id },
            };
        }

        panic!("need either redis-server or docker available to run unix socket integration tests");
    }

    fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

#[cfg(unix)]
impl Drop for RedisUnixSocketServer {
    fn drop(&mut self) {
        match &mut self.kind {
            RedisUnixSocketServerKind::Native(child) => {
                let _ = child.kill();
            }
            RedisUnixSocketServerKind::Docker { container_id } => {
                let _ = Command::new("docker")
                    .arg("stop")
                    .arg(container_id)
                    .output();
            }
        }
    }
}

#[cfg(unix)]
fn command_is_available(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
async fn wait_for_unix_socket(path: &Path) {
    for _ in 0..50 {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.file_type().is_socket() {
                return;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out waiting for unix socket {}", path.display());
}

static BUILD_WEBDIS_DEBUG_ONCE: Once = Once::new();

fn ensure_webdis_debug_binary() {
    BUILD_WEBDIS_DEBUG_ONCE.call_once(|| {
        let status = Command::new("cargo")
            .arg("build")
            .status()
            .expect("Failed to build project");
        assert!(status.success());
    });
}

async fn redis_connect_local() -> MultiplexedConnection {
    // Integration tests assume a locally reachable Redis at the default address,
    // matching the default `TestServer` configuration.
    let client =
        redis::Client::open("redis://127.0.0.1:6379/").expect("failed to create Redis client");
    client
        .get_multiplexed_async_connection()
        .await
        .expect("failed to connect to Redis at 127.0.0.1:6379")
}

/// Opens a direct Redis connection to a specific logical database.
///
/// This bypasses Webdis to validate that DB-prefixed HTTP requests hit the
/// intended database and do not leak state across requests.
async fn redis_connect_local_db(database: u8) -> MultiplexedConnection {
    let client = redis::Client::open(format!("redis://127.0.0.1:6379/{database}"))
        .expect("failed to create Redis client");
    client
        .get_multiplexed_async_connection()
        .await
        .expect("failed to connect to Redis")
}

async fn redis_get_string(key: &str) -> Option<String> {
    let mut conn = redis_connect_local().await;
    redis::cmd("GET")
        .arg(key)
        .query_async(&mut conn)
        .await
        .expect("failed to GET from Redis")
}

async fn redis_publish(channel: &str, payload: &str) -> i64 {
    let mut conn = redis_connect_local().await;
    redis::cmd("PUBLISH")
        .arg(channel)
        .arg(payload)
        .query_async(&mut conn)
        .await
        .expect("failed to PUBLISH to Redis")
}

async fn raw_http_get(port: u16, request_target: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("failed to connect to webdis test server");

    let req = format!(
        "GET {request_target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .await
        .expect("failed to write HTTP request");

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .expect("failed to read HTTP response");

    let text = String::from_utf8_lossy(&buf).to_string();
    let mut header_body = text.splitn(2, "\r\n\r\n");
    let headers = header_body
        .next()
        .expect("response missing headers section");
    let body = header_body.next().unwrap_or("").to_string();

    let status_line = headers
        .lines()
        .next()
        .expect("response missing status line");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or_else(|| panic!("could not parse status line {status_line:?}"));

    (status, body)
}

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
        // Create a temporary config file that will be automatically deleted when dropped
        let config_file = tempfile::Builder::new()
            .suffix(".json")
            .tempfile()
            .expect("Failed to create temp config file");

        // Generate test configuration with ACL rules:
        // - DEBUG command is disabled by default (for ACL testing)
        // - DEBUG command is enabled when authenticated with "user:password"
        // - WebSockets are enabled for WebSocket tests
        // - http_max_request_size is configurable for limit testing
        let config_content = serde_json::json!({
            "redis_host": "127.0.0.1",
            "redis_port": 6379,
            "http_host": "127.0.0.1",
            // The test harness will allocate a free port per spawned process to avoid
            // racy "port already in use" failures when tests run in parallel.
            "http_port": 0,
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

        Self::spawn_with_config_and_env(config_file, config_content, &[]).await
    }

    /// Spawns a Webdis process using an explicit JSON config and optional env vars.
    ///
    /// This is used by integration tests that need to validate config loader behavior
    /// end-to-end (for example, `$VARNAME` environment variable expansion).
    async fn spawn_with_config_and_env(
        mut config_file: NamedTempFile,
        mut config_content: serde_json::Value,
        env: &[(&str, &str)],
    ) -> Self {
        ensure_webdis_debug_binary();

        let config_path = config_file.path().to_str().unwrap().to_string();

        // Starting a separate Webdis process is inherently racy when choosing a port ahead of
        // time. When integration tests run in parallel, it is possible for another process to
        // claim a port after we pick it but before Webdis binds.
        //
        // To keep the suite stable, we retry with a fresh port until the server is reachable.
        for attempt in 0..20 {
            let port = pick_unused_local_port();
            if let Some(obj) = config_content.as_object_mut() {
                obj.insert("http_port".to_string(), serde_json::Value::from(port));
            }

            // Rewrite the config file in-place.
            let file = config_file.as_file_mut();
            file.set_len(0).expect("Failed to truncate config file");
            file.seek(SeekFrom::Start(0))
                .expect("Failed to seek config file");
            write!(config_file, "{}", config_content.to_string()).expect("Failed to write config");

            // Spawn the Webdis process with the temporary config.
            // Note: we inject per-process env vars via Command to avoid mutating
            // the process-global test environment (tests can run in parallel).
            let mut cmd = Command::new("target/debug/webdis");
            cmd.arg(&config_path);
            for (k, v) in env {
                cmd.env(k, v);
            }
            let mut process = cmd.spawn().expect("Failed to start webdis");

            // Poll for readiness by attempting to connect to the bound port.
            let mut ready = false;
            for _ in 0..40 {
                if let Ok(Ok(_)) = tokio::time::timeout(
                    Duration::from_millis(100),
                    TcpStream::connect(("127.0.0.1", port)),
                )
                .await
                {
                    ready = true;
                    break;
                }

                if let Ok(Some(_)) = process.try_wait() {
                    break;
                }
                sleep(Duration::from_millis(50)).await;
            }

            if ready {
                return Self {
                    process,
                    _config_file: config_file,
                    port,
                };
            }

            let _ = process.kill();
            if attempt == 19 {
                panic!("failed to start webdis after retries");
            }
        }

        unreachable!("retry loop returns or panics")
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

/// Supports DB-prefixed paths so the same key can store distinct values per DB.
#[tokio::test]
async fn test_database_prefix_separate_values_per_db() {
    let server = TestServer::new().await;
    let client = Client::new();
    let key = format!("db_prefix_key_{}", server.port);

    // Cleanup any stale values from previous runs.
    let mut db0 = redis_connect_local_db(0).await;
    let mut db7 = redis_connect_local_db(7).await;
    let _: i64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut db0)
        .await
        .expect("failed to cleanup DB 0 key");
    let _: i64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut db7)
        .await
        .expect("failed to cleanup DB 7 key");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/{}/value0",
            server.port, key
        ))
        .send()
        .await
        .expect("Failed to write DB 0 value");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/7/SET/{}/value7",
            server.port, key
        ))
        .send()
        .await
        .expect("Failed to write DB 7 value");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/{}", server.port, key))
        .send()
        .await
        .expect("Failed to read default DB value");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "value0");

    let resp = client
        .get(&format!("http://127.0.0.1:{}/7/GET/{}", server.port, key))
        .send()
        .await
        .expect("Failed to read DB 7 value");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "value7");
}

/// Using a DB-prefixed request must not affect subsequent default-DB requests.
#[tokio::test]
async fn test_database_prefix_no_bleed_between_requests() {
    let server = TestServer::new().await;
    let client = Client::new();
    let key = format!("db_prefix_bleed_key_{}", server.port);

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/{}/default_db_value",
            server.port, key
        ))
        .send()
        .await
        .expect("Failed to write default DB value");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/7/SET/{}/db7_value",
            server.port, key
        ))
        .send()
        .await
        .expect("Failed to write DB 7 value");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let resp = client
        .get(&format!("http://127.0.0.1:{}/7/GET/{}", server.port, key))
        .send()
        .await
        .expect("Failed to read DB 7 value");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "db7_value");

    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/{}", server.port, key))
        .send()
        .await
        .expect("Failed to read default DB value after DB 7 request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "default_db_value");
}

/// Numeric DB prefixes outside the supported range return a 400 error.
#[tokio::test]
async fn test_database_prefix_invalid_db_index_returns_400() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(&format!("http://127.0.0.1:{}/9999/GET/key", server.port))
        .send()
        .await
        .expect("Failed to send invalid DB prefix request");

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert!(
        body["error"]
            .as_str()
            .map(|s| s.contains("Invalid database index"))
            .unwrap_or(false),
        "Expected clear invalid DB index error, got: {body:?}"
    );
}

/// A DB-only path must return a clear 400 instead of trying to execute an empty command.
#[tokio::test]
async fn test_database_prefix_missing_command_returns_400() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(&format!("http://127.0.0.1:{}/7", server.port))
        .send()
        .await
        .expect("Failed to send DB-only path request");

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["error"], "Missing command after database prefix");
}

/// JSONP must wrap DB-prefix validation errors the same way as command execution errors.
#[tokio::test]
async fn test_database_prefix_invalid_db_index_jsonp_wraps_error() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/9999/GET/key?jsonp=myFn",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send invalid DB prefix JSONP request");

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("application/javascript"),
        "Expected JSONP content type, got: {content_type}"
    );

    let body = resp.text().await.expect("Failed to read JSONP body");
    let (callback, payload) = parse_jsonp_body(&body);
    assert_eq!(callback, "myFn");
    assert!(
        payload["error"]
            .as_str()
            .map(|s| s.contains("Invalid database index"))
            .unwrap_or(false),
        "Expected invalid database index error in JSONP payload, got: {payload:?}"
    );
}

/// Percent-decoding parity: `%2f` is decoded into `/` *inside* an argument, never as a separator.
#[tokio::test]
async fn test_percent_decoding_slash_in_key_roundtrip() {
    let server = TestServer::new().await;

    let key = format!("percent_slash_key_{}/b", server.port);

    let (status, body) = raw_http_get(
        server.port,
        &format!("/SET/percent_slash_key_{}%2Fb/value", server.port),
    )
    .await;
    assert_eq!(status, 200, "Expected SET to succeed, got: {body:?}");
    let json: serde_json::Value = serde_json::from_str(&body).expect("Expected JSON body from SET");
    assert_eq!(json["SET"], "OK");

    let (status, body) = raw_http_get(
        server.port,
        &format!("/GET/percent_slash_key_{}%2Fb", server.port),
    )
    .await;
    assert_eq!(status, 200, "Expected GET to succeed, got: {body:?}");
    let json: serde_json::Value = serde_json::from_str(&body).expect("Expected JSON body from GET");
    assert_eq!(json["GET"], "value");

    // Confirm the key stored in Redis is literally `.../b`, not `...%2Fb`.
    let direct = redis_get_string(&key).await;
    assert_eq!(direct.as_deref(), Some("value"));
}

/// Percent-decoding parity: uppercase `%2F` must match lowercase `%2f` behavior.
#[tokio::test]
async fn test_percent_decoding_uppercase_slash_in_key_roundtrip() {
    let server = TestServer::new().await;

    let key = format!("percent_slash_upper_key_{}/b", server.port);

    let (status, body) = raw_http_get(
        server.port,
        &format!("/SET/percent_slash_upper_key_{}%2Fb/value", server.port),
    )
    .await;
    assert_eq!(status, 200, "Expected SET to succeed, got: {body:?}");

    let (status, body) = raw_http_get(
        server.port,
        &format!("/GET/percent_slash_upper_key_{}%2Fb", server.port),
    )
    .await;
    assert_eq!(status, 200, "Expected GET to succeed, got: {body:?}");
    let json: serde_json::Value = serde_json::from_str(&body).expect("Expected JSON body from GET");
    assert_eq!(json["GET"], "value");

    // Confirm Redis key contains `/` (decoded), not literal `%2F`.
    let direct = redis_get_string(&key).await;
    assert_eq!(direct.as_deref(), Some("value"));
}

/// Percent-decoding parity: `%2e` is decoded into `.` inside an argument without triggering
/// output-format parsing.
#[tokio::test]
async fn test_percent_decoding_dot_does_not_trigger_format_suffix() {
    let server = TestServer::new().await;

    // This path contains no literal dot. If the server incorrectly performs `.ext` parsing
    // after decoding, it would interpret the decoded `.raw` as a suffix and change output mode.
    let encoded_key = format!("percent_dot_key_{}%2Eraw", server.port);
    let decoded_key = format!("percent_dot_key_{}.raw", server.port);

    let (status, body) = raw_http_get(server.port, &format!("/SET/{}/world", encoded_key)).await;
    assert_eq!(status, 200, "Expected SET to succeed, got: {body:?}");

    let (status, body) = raw_http_get(server.port, &format!("/GET/{}", encoded_key)).await;
    assert_eq!(status, 200, "Expected GET to succeed, got: {body:?}");

    let json: serde_json::Value =
        serde_json::from_str(&body).expect("Expected JSON output (not .raw RESP)");
    assert_eq!(json["GET"], "world");

    // Confirm the decoded key exists in Redis under the literal dotted name.
    let direct = redis_get_string(&decoded_key).await;
    assert_eq!(direct.as_deref(), Some("world"));
}

/// Percent-decoding parity: uppercase `%2E` in args must not trigger suffix parsing.
#[tokio::test]
async fn test_percent_decoding_uppercase_dot_does_not_trigger_format_suffix() {
    let server = TestServer::new().await;

    let encoded_key = format!("percent_dot_upper_key_{}%2Eraw", server.port);
    let decoded_key = format!("percent_dot_upper_key_{}.raw", server.port);

    let (status, body) = raw_http_get(server.port, &format!("/SET/{}/world", encoded_key)).await;
    assert_eq!(status, 200, "Expected SET to succeed, got: {body:?}");

    let (status, body) = raw_http_get(server.port, &format!("/GET/{}", encoded_key)).await;
    assert_eq!(status, 200, "Expected GET to succeed, got: {body:?}");

    let json: serde_json::Value =
        serde_json::from_str(&body).expect("Expected JSON output (not .raw RESP)");
    assert_eq!(json["GET"], "world");

    let direct = redis_get_string(&decoded_key).await;
    assert_eq!(direct.as_deref(), Some("world"));
}

/// Explicit `.raw` suffix still works when the key contains an encoded dot.
#[tokio::test]
async fn test_percent_decoding_dot_with_raw_suffix_still_selects_raw() {
    let server = TestServer::new().await;

    let encoded_key = format!("percent_dot_key_{}%2Eb", server.port);
    let decoded_key = format!("percent_dot_key_{}.b", server.port);

    let (status, body) = raw_http_get(server.port, &format!("/SET/{}/hello", encoded_key)).await;
    assert_eq!(status, 200, "Expected SET to succeed, got: {body:?}");

    let (status, body) = raw_http_get(server.port, &format!("/GET/{}.raw", encoded_key)).await;
    assert_eq!(status, 200, "Expected GET to succeed, got: {body:?}");
    assert_eq!(body, "$5\r\nhello\r\n");

    let direct = redis_get_string(&decoded_key).await;
    assert_eq!(direct.as_deref(), Some("hello"));
}

/// Invalid percent-encodings are preserved (left untouched).
///
/// We send a raw HTTP request (bypassing client-side URL validation) so the server can
/// observe `%` sequences that are not valid percent-escapes.
#[tokio::test]
async fn test_percent_decoding_invalid_encodings_left_untouched() {
    let server = TestServer::new().await;

    let key = format!("percent_invalid_{}%2Xkey", server.port);

    // SET via raw HTTP to allow the invalid `%2X` sequence.
    let (status, body) = raw_http_get(server.port, &format!("/SET/{}/ok", key)).await;
    assert_eq!(status, 200, "Expected SET to succeed, got body: {body:?}");

    // GET via raw HTTP as well.
    let (status, body) = raw_http_get(server.port, &format!("/GET/{key}")).await;
    assert_eq!(status, 200, "Expected GET to succeed, got body: {body:?}");

    let json: serde_json::Value = serde_json::from_str(&body).expect("Expected JSON body from GET");
    assert_eq!(json["GET"], "ok");

    // Confirm the literal key was stored in Redis unchanged (contains `%2X`).
    let direct = redis_get_string(&key).await;
    assert_eq!(direct.as_deref(), Some("ok"));
}

/// Connects to Redis over a UNIX-domain socket when `redis_socket` is configured.
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_basic_connectivity() {
    let redis = RedisUnixSocketServer::start().await;

    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener.local_addr().unwrap().port()
    };

    let config_content = serde_json::json!({
        "redis_socket": redis.socket_path().display().to_string(),
        "http_host": "127.0.0.1",
        "http_port": port,
        "database": 0,
        "websockets": false,
        "daemonize": false,
        "verbosity": 4
    });

    let config_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");

    let server = TestServer::spawn_with_config_and_env(config_file, config_content, &[]).await;
    let client = Client::new();

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/unix_socket_key/unix_socket_value",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["SET"], "OK");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/GET/unix_socket_key",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "unix_socket_value");
}

/// `redis_socket` takes precedence over `redis_host` / `redis_port`.
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_precedence_over_tcp() {
    let redis = RedisUnixSocketServer::start().await;

    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener.local_addr().unwrap().port()
    };

    // Intentionally bogus TCP settings: if Webdis uses these, the test will fail.
    let config_content = serde_json::json!({
        "redis_host": "192.0.2.1",
        "redis_port": 1,
        "redis_socket": redis.socket_path().display().to_string(),
        "http_host": "127.0.0.1",
        "http_port": port,
        "database": 0,
        "websockets": false,
        "daemonize": false,
        "verbosity": 4
    });

    let config_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");

    let server = TestServer::spawn_with_config_and_env(config_file, config_content, &[]).await;
    let client = Client::new();

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/unix_precedence_key/ok",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["SET"], "OK");
}

/// Invalid socket paths fail fast with a clear startup error.
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_invalid_path_fails_fast() {
    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener.local_addr().unwrap().port()
    };

    let config_content = serde_json::json!({
        "redis_socket": "/path/that/does/not/exist.sock",
        "http_host": "127.0.0.1",
        "http_port": port,
        "database": 0,
        "websockets": false,
        "daemonize": false,
        "verbosity": 4
    });

    let mut config_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp config file");
    write!(config_file, "{}", config_content.to_string()).expect("Failed to write config");

    ensure_webdis_debug_binary();

    let output = Command::new("target/debug/webdis")
        .arg(config_file.path())
        .output()
        .expect("Failed to run webdis");

    assert!(
        !output.status.success(),
        "expected webdis to exit non-zero for invalid socket path"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("redis_socket"),
        "expected error output to mention redis_socket, got:\n{combined}"
    );
}

/// Tests env-var expansion end-to-end by starting Webdis with `$REDIS_HOST` / `$REDIS_PORT`.
///
/// This validates that:
/// - The config loader expands `$VARNAME` placeholders before deserialization.
/// - The expanded values are honored by the running server process.
/// - Numeric fields like ports continue to work when configured via env vars.
#[tokio::test]
async fn test_env_var_expansion_end_to_end() {
    let port = {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
        listener.local_addr().unwrap().port()
    };

    let config_content = serde_json::json!({
        "redis_host": "$REDIS_HOST",
        "redis_port": "$REDIS_PORT",
        "http_host": "127.0.0.1",
        "http_port": port,
        "database": 0,
        "websockets": false,
        "daemonize": false,
        "verbosity": 4
    });

    let config_file = tempfile::Builder::new()
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
        .get(&format!(
            "http://127.0.0.1:{}/SET/env_expand_key/env_expand_value",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["SET"], "OK");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/GET/env_expand_key",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "env_expand_value");
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

/// Tests extension-based content types for string replies.
///
/// Webdis supports suffixes like `.txt`, `.html`, `.xml` which:
/// - return the Redis string value as the HTTP body (no JSON envelope), and
/// - set `Content-Type` based on the suffix.
#[tokio::test]
async fn test_extension_based_text_content_types() {
    let server = TestServer::new().await;
    let client = Client::new();

    // SET hello -> world
    let _ = client
        .get(&format!("http://127.0.0.1:{}/SET/hello/world", server.port))
        .send()
        .await
        .expect("Failed to send request");

    for (path, expected_content_type) in [
        ("GET/hello.txt", "text/plain"),
        ("GET/hello.html", "text/html"),
        ("GET/hello.xml", "text/xml"),
    ] {
        let resp = client
            .get(&format!("http://127.0.0.1:{}/{path}", server.port))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .expect("Content-Type header missing")
            .to_str()
            .expect("Invalid Content-Type header value");
        assert_eq!(
            content_type, expected_content_type,
            "Expected {expected_content_type}, got: {content_type}"
        );

        let body = resp.text().await.expect("Failed to read body");
        assert_eq!(body, "world");
    }
}

/// Tests binary upload and retrieval with an image extension.
///
/// This validates that PUT preserves bytes and `GET/key.png`:
/// - returns the stored bytes unchanged, and
/// - sets `Content-Type: image/png`.
#[tokio::test]
async fn test_binary_content_type_png_roundtrip() {
    let server = TestServer::new().await;
    let client = Client::new();

    // A tiny 1x1 transparent PNG.
    let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO0X6b8AAAAASUVORK5CYII=";
    let png_bytes = general_purpose::STANDARD
        .decode(png_b64)
        .expect("Failed to decode base64 PNG");

    // Upload PNG bytes as the last argument of SET.
    let resp = client
        .put(&format!("http://127.0.0.1:{}/SET/logo", server.port))
        .body(png_bytes.clone())
        .send()
        .await
        .expect("Failed to upload PNG bytes");
    assert!(resp.status().is_success());

    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/logo.png", server.port))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert_eq!(content_type, "image/png");

    let body = resp.bytes().await.expect("Failed to read body bytes");
    assert_eq!(
        &body[..],
        &png_bytes[..],
        "PNG bytes must roundtrip unchanged"
    );
}

/// Tests `?type=<mime>` override behavior.
///
/// `?type` overrides the `Content-Type` header while leaving the response body
/// unchanged (JSON envelope by default).
#[tokio::test]
async fn test_type_query_param_overrides_content_type_only() {
    let server = TestServer::new().await;
    let client = Client::new();

    // SET hello -> world
    let _ = client
        .get(&format!("http://127.0.0.1:{}/SET/hello/world", server.port))
        .send()
        .await
        .expect("Failed to send request");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/GET/hello?type=application/pdf",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert_eq!(content_type, "application/pdf");

    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");
    assert_eq!(body["GET"], "world");
}

/// Tests JSONP support via the `jsonp` query parameter for JSON responses.
#[tokio::test]
async fn test_jsonp_simple_get() {
    let server = TestServer::new().await;
    let client = Client::new();

    // SET hello -> world
    let _ = client
        .get(&format!("http://127.0.0.1:{}/SET/hello/world", server.port))
        .send()
        .await
        .expect("Failed to send request");

    // GET hello as JSONP
    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/GET/hello?jsonp=myFn",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert!(
        content_type.starts_with("application/javascript"),
        "Expected application/javascript, got: {content_type}"
    );

    let body = resp.text().await.expect("Failed to read body");
    let (cb, json) = parse_jsonp_body(&body);
    assert_eq!(cb, "myFn");
    assert_eq!(json["GET"], "world");
}

/// Tests JSONP support via the `callback` query parameter (fallback).
#[tokio::test]
async fn test_jsonp_callback_fallback() {
    let server = TestServer::new().await;
    let client = Client::new();

    // SET hello -> world
    let _ = client
        .get(&format!("http://127.0.0.1:{}/SET/hello/world", server.port))
        .send()
        .await
        .expect("Failed to send request");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/GET/hello?callback=cb",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert!(
        content_type.starts_with("application/javascript"),
        "Expected application/javascript, got: {content_type}"
    );

    let body = resp.text().await.expect("Failed to read body");
    let (cb, json) = parse_jsonp_body(&body);
    assert_eq!(cb, "cb");
    assert_eq!(json["GET"], "world");
}

/// Tests that JSON error payloads are wrapped in JSONP when requested.
#[tokio::test]
async fn test_jsonp_with_errors() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Make a non-numeric key and then INCR it to trigger a Redis error.
    let _ = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/non_numeric_key/not_a_number",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/INCR/non_numeric_key?jsonp=myFn",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(
        resp.status(),
        reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        "Expected Redis INCR error to map to 500"
    );

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert!(
        content_type.starts_with("application/javascript"),
        "Expected application/javascript, got: {content_type}"
    );

    let body = resp.text().await.expect("Failed to read body");
    let (cb, json) = parse_jsonp_body(&body);
    assert_eq!(cb, "myFn");
    assert!(
        json.get("error").and_then(|v| v.as_str()).is_some(),
        "Expected an error string payload, got: {json:?}"
    );
}

/// Tests that JSONP is ignored for non-JSON formats like `.raw`.
#[tokio::test]
async fn test_jsonp_ignored_on_raw() {
    let server = TestServer::new().await;
    let client = Client::new();

    // SET hello -> world
    let _ = client
        .get(&format!("http://127.0.0.1:{}/SET/hello/world", server.port))
        .send()
        .await
        .expect("Failed to send request");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/GET/hello.raw?jsonp=myFn",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert!(
        content_type.starts_with("text/plain"),
        "Expected text/plain, got: {content_type}"
    );

    let body = resp.text().await.expect("Failed to read body");
    assert_eq!(body, "$5\r\nworld\r\n");
    assert!(
        !body.starts_with("myFn("),
        "Raw response must not be JSONP-wrapped"
    );
}

/// `/SUBSCRIBE/*channel` supports chunked JSON streaming when JSON is explicitly negotiated.
#[tokio::test]
async fn test_subscribe_chunked_json_stream() {
    let server = TestServer::new().await;
    let client = Client::new();
    let channel = format!("comet_json_{}", server.port);

    let response = client
        .get(&format!(
            "http://127.0.0.1:{}/SUBSCRIBE/{}",
            server.port, channel
        ))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .expect("failed to open SUBSCRIBE stream");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert!(
        content_type.starts_with("application/json"),
        "Expected application/json content type, got: {content_type}"
    );

    // Give the subscribe loop a moment to attach before publishing.
    sleep(Duration::from_millis(150)).await;
    let _ = redis_publish(&channel, "hello").await;
    let _ = redis_publish(&channel, "world").await;

    let lines = read_stream_lines(response, 2, Duration::from_secs(3)).await;
    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("Expected valid JSON chunk"))
        .collect();

    let payloads: Vec<String> = parsed
        .iter()
        .map(|v| {
            v["SUBSCRIBE"][2]
                .as_str()
                .expect("Expected SUBSCRIBE payload string")
                .to_string()
        })
        .collect();
    assert!(payloads.contains(&"hello".to_string()));
    assert!(payloads.contains(&"world".to_string()));
}

/// `/SUBSCRIBE/*channel?jsonp=...` streams JSONP-wrapped Comet chunks.
#[tokio::test]
async fn test_subscribe_jsonp_comet_stream() {
    let server = TestServer::new().await;
    let client = Client::new();
    let channel = format!("comet_jsonp_{}", server.port);

    let response = client
        .get(&format!(
            "http://127.0.0.1:{}/SUBSCRIBE/{}?jsonp=myFn",
            server.port, channel
        ))
        .send()
        .await
        .expect("failed to open JSONP SUBSCRIBE stream");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert!(
        content_type.starts_with("application/javascript"),
        "Expected application/javascript content type, got: {content_type}"
    );

    sleep(Duration::from_millis(150)).await;
    let _ = redis_publish(&channel, "one").await;
    let _ = redis_publish(&channel, "two").await;

    let lines = read_stream_lines(response, 2, Duration::from_secs(3)).await;
    let mut payloads = Vec::new();
    for line in lines {
        assert!(line.ends_with(';'), "Expected JSONP chunk to end with ';'");
        let (callback, json) = parse_jsonp_body(line.trim_end_matches(';'));
        assert_eq!(callback, "myFn");
        payloads.push(
            json["SUBSCRIBE"][2]
                .as_str()
                .expect("Expected SUBSCRIBE payload string")
                .to_string(),
        );
    }

    assert!(payloads.contains(&"one".to_string()));
    assert!(payloads.contains(&"two".to_string()));
}

/// Default `/SUBSCRIBE/*channel` behavior remains SSE-compatible.
#[tokio::test]
async fn test_subscribe_sse_default_compatibility() {
    let server = TestServer::new().await;
    let client = Client::new();
    let channel = format!("sse_default_{}", server.port);

    let response = client
        .get(&format!(
            "http://127.0.0.1:{}/SUBSCRIBE/{}",
            server.port, channel
        ))
        .send()
        .await
        .expect("failed to open SSE SUBSCRIBE stream");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("Content-Type header missing")
        .to_str()
        .expect("Invalid Content-Type header value");
    assert!(
        content_type.starts_with("text/event-stream"),
        "Expected text/event-stream content type, got: {content_type}"
    );

    sleep(Duration::from_millis(150)).await;
    let expected_payload = "sse-payload";
    let _ = redis_publish(&channel, expected_payload).await;

    let lines = read_stream_lines(response, 1, Duration::from_secs(3)).await;
    assert!(
        lines
            .iter()
            .any(|line| line.contains(&format!("data: {expected_payload}"))),
        "Expected at least one SSE data line for payload {expected_payload}, got: {lines:?}"
    );
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

/// Tests ETag support for efficient client-side caching.
#[tokio::test]
async fn test_etag_support() {
    let server = TestServer::new().await;
    let client = Client::new();

    // 1. SET a value
    let _ = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/etag_key/foo",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    // 2. GET the value and check for ETag
    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/etag_key", server.port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(resp.status().is_success());
    let etag = resp
        .headers()
        .get("ETag")
        .expect("ETag header missing")
        .clone();

    // 3. GET again with If-None-Match
    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/etag_key", server.port))
        .header("If-None-Match", etag.clone())
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), reqwest::StatusCode::NOT_MODIFIED);
    let body_text = resp.text().await.unwrap();
    assert!(body_text.is_empty());

    // 4. Update the value
    let _ = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/etag_key/bar",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    // 5. GET again, ETag should be different
    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/etag_key", server.port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(resp.status().is_success());
    let new_etag = resp.headers().get("ETag").expect("ETag header missing");
    assert_ne!(etag, new_etag);

    // 6. Old ETag should return 200
    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/etag_key", server.port))
        .header("If-None-Match", etag)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

/// Tests the raw output mode (.raw extension) for full RESP compliance.
///
/// This test validates:
/// 1. Simple strings (SET -> OK) return `+OK\r\n`
/// 2. Integers (INCR) return `:123\r\n`
/// 3. Bulk strings (GET) return `$len\r\nval\r\n`
/// 4. Arrays (LRANGE) return `*count\r\n...`
/// 5. Errors (invalid command) return `-ERR ...\r\n`
#[tokio::test]
async fn test_raw_mode_parity() {
    let server = TestServer::new().await;
    let client = Client::new();

    // 1. Raw string response (SET -> +OK)
    let _ = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/raw_key/raw_value",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/SET/raw_key/raw_value.raw",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body = resp.text().await.expect("Failed to read body");
    // Expecting RESP simple string for status OK
    assert_eq!(body, "+OK\r\n");

    // 2. Raw bulk string response (GET -> $len\r\nval\r\n)
    let resp = client
        .get(&format!("http://127.0.0.1:{}/GET/raw_key.raw", server.port))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body = resp.text().await.expect("Failed to read body");
    // "raw_value" is 9 bytes
    assert_eq!(body, "$9\r\nraw_value\r\n");

    // 3. Raw integer response (INCR -> :123)
    let _ = client
        .get(&format!("http://127.0.0.1:{}/SET/counter/10", server.port))
        .send()
        .await
        .expect("Failed to send request");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/INCR/counter.raw",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body = resp.text().await.expect("Failed to read body");
    assert_eq!(body, ":11\r\n");

    // 4. Raw array response (LRANGE)
    // Clean up key first
    let _ = client
        .get(&format!("http://127.0.0.1:{}/DEL/list", server.port))
        .send()
        .await
        .expect("Failed to send request");

    let _ = client
        .get(&format!("http://127.0.0.1:{}/RPUSH/list/a", server.port))
        .send()
        .await
        .expect("Failed to send request");
    let _ = client
        .get(&format!("http://127.0.0.1:{}/RPUSH/list/b", server.port))
        .send()
        .await
        .expect("Failed to send request");

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/LRANGE/list/0/-1.raw",
            server.port
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert!(resp.status().is_success());
    let body = resp.text().await.expect("Failed to read body");
    // Expecting array of 2 bulk strings
    // *2\r\n$1\r\na\r\n$1\r\nb\r\n
    assert_eq!(body, "*2\r\n$1\r\na\r\n$1\r\nb\r\n");

    // 5. Raw error response (Unknown Command)
    let resp = client
        .get(&format!("http://127.0.0.1:{}/UNKNOWN_CMD.raw", server.port))
        .send()
        .await
        .expect("Failed to send request");

    let body = resp.text().await.expect("Failed to read body");
    assert!(
        body.starts_with("-ERR"),
        "Body should start with -ERR, got: {}",
        body
    );
}

/// Tests that the INFO command returns a structured JSON object.
///
/// This test validates:
/// - The response for INFO is a JSON object, not a string.
/// - The object contains expected Redis performance metrics and version info.
#[tokio::test]
async fn test_info_command() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(&format!("http://127.0.0.1:{}/INFO", server.port))
        .send()
        .await
        .expect("Failed to send request");

    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON");

    // INFO response should now be a structured object
    assert!(body["INFO"].is_object());

    // Check for some common keys in INFO output
    let info = body["INFO"].as_object().unwrap();
    assert!(info.contains_key("redis_version"));
    assert!(info.contains_key("uptime_in_seconds"));
    assert!(info.contains_key("used_memory"));
}
