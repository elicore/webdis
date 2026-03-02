#![allow(dead_code)]

use redis::aio::MultiplexedConnection;
use reqwest::Client;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::process::{Child, Command};
use std::sync::Once;
use std::time::Duration;
use tempfile::NamedTempFile;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;

static BUILD_WEBDIS_DEBUG_ONCE: Once = Once::new();

pub fn parse_jsonp_body(body: &str) -> (&str, serde_json::Value) {
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

pub async fn read_stream_lines(
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

pub fn ensure_webdis_debug_binary() {
    BUILD_WEBDIS_DEBUG_ONCE.call_once(|| {
        let status = Command::new("cargo")
            .arg("build")
            .status()
            .expect("Failed to build project");
        assert!(status.success());
    });
}

pub async fn redis_connect_local() -> MultiplexedConnection {
    let client =
        redis::Client::open("redis://127.0.0.1:6379/").expect("failed to create Redis client");
    client
        .get_multiplexed_async_connection()
        .await
        .expect("failed to connect to Redis at 127.0.0.1:6379")
}

pub async fn redis_connect_local_db(database: u8) -> MultiplexedConnection {
    let client = redis::Client::open(format!("redis://127.0.0.1:6379/{database}"))
        .expect("failed to create Redis client");
    client
        .get_multiplexed_async_connection()
        .await
        .expect("failed to connect to Redis")
}

pub async fn redis_get_string(key: &str) -> Option<String> {
    let mut conn = redis_connect_local().await;
    redis::cmd("GET")
        .arg(key)
        .query_async(&mut conn)
        .await
        .expect("failed to GET from Redis")
}

pub async fn redis_publish(channel: &str, payload: &str) -> i64 {
    let mut conn = redis_connect_local().await;
    redis::cmd("PUBLISH")
        .arg(channel)
        .arg(payload)
        .query_async(&mut conn)
        .await
        .expect("failed to PUBLISH to Redis")
}

pub async fn raw_http_get(port: u16, request_target: &str) -> (u16, String) {
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

fn pick_unused_local_port() -> u16 {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    listener.local_addr().unwrap().port()
}

pub struct TestServer {
    process: Child,
    _config_file: NamedTempFile,
    pub port: u16,
}

impl TestServer {
    pub async fn new() -> Self {
        Self::new_with_limit(None).await
    }

    pub async fn new_with_limit(limit: Option<usize>) -> Self {
        let config_file = tempfile::Builder::new()
            .suffix(".json")
            .tempfile()
            .expect("Failed to create temp config file");

        let config_content = serde_json::json!({
            "redis_host": "127.0.0.1",
            "redis_port": 6379,
            "http_host": "127.0.0.1",
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

    pub async fn spawn_with_config_and_env(
        mut config_file: NamedTempFile,
        mut config_content: serde_json::Value,
        env: &[(&str, &str)],
    ) -> Self {
        ensure_webdis_debug_binary();

        let config_path = config_file.path().to_str().unwrap().to_string();

        for attempt in 0..20 {
            let port = pick_unused_local_port();
            if let Some(obj) = config_content.as_object_mut() {
                obj.insert("http_port".to_string(), serde_json::Value::from(port));
            }

            let file = config_file.as_file_mut();
            file.set_len(0).expect("Failed to truncate config file");
            file.seek(SeekFrom::Start(0))
                .expect("Failed to seek config file");
            write!(config_file, "{}", config_content).expect("Failed to write config");

            let mut cmd = Command::new("target/debug/webdis");
            cmd.arg(&config_path);
            for (k, v) in env {
                cmd.env(k, v);
            }
            let mut process = cmd.spawn().expect("Failed to start webdis");

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
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

#[cfg(unix)]
pub struct RedisUnixSocketServer {
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
    pub async fn start() -> Self {
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

    pub fn socket_path(&self) -> &Path {
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
                let _ = Command::new("docker").arg("stop").arg(container_id).output();
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

pub async fn http_get_json(client: &Client, port: u16, path: &str) -> serde_json::Value {
    let resp = client
        .get(format!("http://127.0.0.1:{port}/{path}"))
        .send()
        .await
        .expect("request failed");
    assert!(resp.status().is_success());
    resp.json().await.expect("json body expected")
}
