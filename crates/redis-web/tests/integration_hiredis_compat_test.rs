mod support;

use reqwest::Client;
use std::time::Duration;
use support::process_harness::{redis_publish, TestServer};
use serde_json::json;
use tokio::time::sleep;

fn resp_command(parts: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(format!("*{}\r\n", parts.len()).as_bytes());
    for part in parts {
        out.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        out.extend_from_slice(part.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out
}

async fn create_compat_session(client: &Client, port: u16) -> serde_json::Value {
    let mut last_status = None;
    let mut last_body = None;
    let mut delay = Duration::from_millis(100);

    for _ in 0..20 {
        let resp = client
            .post(format!("http://127.0.0.1:{port}/__compat/session"))
            .send()
            .await
            .expect("create session request failed");

        let status = resp.status();
        let body_bytes = resp.bytes().await.expect("create body read failed");

        if status == reqwest::StatusCode::CREATED {
            return serde_json::from_slice(&body_bytes).expect("create body parse failed");
        }

        last_status = Some(status);
        last_body = Some(String::from_utf8_lossy(&body_bytes).to_string());

        if status != reqwest::StatusCode::SERVICE_UNAVAILABLE {
            break;
        }

        sleep(delay).await;
        delay = std::cmp::min(delay.saturating_mul(2), Duration::from_millis(1_000));
    }

    panic!(
        "failed to create compat session; status={:?}, body={:?}",
        last_status, last_body
    );
}

async fn create_compat_session_at(
    client: &Client,
    port: u16,
    path_prefix: &str,
) -> serde_json::Value {
    let base = format!("http://127.0.0.1:{port}");
    let prefix = path_prefix.trim_end_matches('/');
    let mut last_status = None;
    let mut last_body = None;
    let mut delay = Duration::from_millis(100);

    for _ in 0..20 {
        let resp = client
            .post(format!("{base}{prefix}/session"))
            .send()
            .await
            .expect("create session request failed");

        let status = resp.status();
        let body_bytes = resp.bytes().await.expect("create body read failed");

        if status == reqwest::StatusCode::CREATED {
            return serde_json::from_slice(&body_bytes).expect("create body parse failed");
        }

        last_status = Some(status);
        last_body = Some(String::from_utf8_lossy(&body_bytes).to_string());

        if status != reqwest::StatusCode::SERVICE_UNAVAILABLE {
            break;
        }

        sleep(delay).await;
        delay = std::cmp::min(delay.saturating_mul(2), Duration::from_millis(1_000));
    }

    panic!(
        "failed to create compat session with prefix '{path_prefix}'; status={:?}, body={:?}",
        last_status, last_body
    );
}

async fn spawn_server_with_compat_config(config: serde_json::Value) -> TestServer {
    let config_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("failed to create temp config file");
    TestServer::spawn_with_config_and_env(config_file, config, &[]).await
}

#[tokio::test]
async fn test_compat_session_command_roundtrip() {
    let server = TestServer::new().await;
    let client = Client::new();

    let body = create_compat_session(&client, server.port).await;
    let compat_id = body["id"].as_str().expect("id missing").to_string();

    let set_resp = client
        .post(format!(
            "http://127.0.0.1:{}/__compat/cmd/{}.raw",
            server.port, compat_id
        ))
        .body(resp_command(&["SET", "compat_key", "ok"]))
        .send()
        .await
        .expect("SET request failed");
    assert_eq!(set_resp.status(), reqwest::StatusCode::OK);
    let set_body = set_resp.bytes().await.expect("SET body failed");
    assert_eq!(set_body.as_ref(), b"+OK\r\n");

    let get_resp = client
        .post(format!(
            "http://127.0.0.1:{}/__compat/cmd/{}.raw",
            server.port, compat_id
        ))
        .body(resp_command(&["GET", "compat_key"]))
        .send()
        .await
        .expect("GET request failed");
    assert_eq!(get_resp.status(), reqwest::StatusCode::OK);
    let get_body = get_resp.bytes().await.expect("GET body failed");
    assert_eq!(get_body.as_ref(), b"$2\r\nok\r\n");

    let delete = client
        .delete(format!(
            "http://127.0.0.1:{}/__compat/session/{}",
            server.port, compat_id
        ))
        .send()
        .await
        .expect("delete request failed");
    assert_eq!(delete.status(), reqwest::StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_compat_session_prefix_normalized_mount_path() {
    let server = spawn_server_with_compat_config(json!({
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "compat_hiredis": {
            "enabled": true,
            "session_ttl_sec": 300,
            "max_sessions": 1024,
            "max_pipeline_commands": 256,
            "path_prefix": "compat/"
        }
    }))
    .await;
    let client = Client::new();

    let body = create_compat_session_at(&client, server.port, "/compat").await;
    let compat_id = body["id"].as_str().expect("id missing").to_string();

    let wrong_path = client
        .post(format!("http://127.0.0.1:{}/__compat/session", server.port))
        .send()
        .await
        .expect("wrong prefix request failed");
    assert!(
        wrong_path.status().is_client_error() || wrong_path.status().is_server_error(),
        "expected fallback failure for unmapped legacy prefix, got: {:?}",
        wrong_path.status()
    );

    let delete = client
        .delete(format!(
            "http://127.0.0.1:{}/compat/session/{}",
            server.port, compat_id
        ))
        .send()
        .await
        .expect("delete request failed");
    assert_eq!(delete.status(), reqwest::StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_compat_command_pipeline_limit() {
    let server = spawn_server_with_compat_config(json!({
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "compat_hiredis": {
            "enabled": true,
            "path_prefix": "/__compat",
            "session_ttl_sec": 300,
            "max_sessions": 1024,
            "max_pipeline_commands": 1
        }
    }))
    .await;
    let client = Client::new();
    let body = create_compat_session(&client, server.port).await;
    let compat_id = body["id"].as_str().expect("id missing").to_string();

    let mut pipeline_body = resp_command(&["SET", "compat_pipe", "1"]);
    pipeline_body.extend(resp_command(&["GET", "compat_pipe"]));

    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/__compat/cmd/{}.raw",
            server.port, compat_id
        ))
        .body(pipeline_body)
        .send()
        .await
        .expect("pipelined request failed");
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let body = resp.bytes().await.expect("body read failed");
    assert_eq!(body.as_ref(), b"-ERR Pipelined command limit exceeded\r\n");
}

#[tokio::test]
async fn test_compat_session_limit_and_ttl_cleanup() {
    let server = spawn_server_with_compat_config(json!({
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "compat_hiredis": {
            "enabled": true,
            "path_prefix": "/__compat",
            "max_sessions": 1,
            "max_pipeline_commands": 256,
            "session_ttl_sec": 1
        }
    }))
    .await;
    let client = Client::new();

    let first = create_compat_session(&client, server.port).await;
    let first_id = first["id"].as_str().expect("id missing").to_string();

    let second = client
        .post(format!("http://127.0.0.1:{}/__compat/session", server.port))
        .send()
        .await
        .expect("second session request failed");
    assert_eq!(second.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);

    sleep(Duration::from_secs(2)).await;

    let after_ttl = client
        .post(format!(
            "http://127.0.0.1:{}/__compat/cmd/{}.raw",
            server.port, first_id
        ))
        .body(resp_command(&["GET", "compat_key"]))
        .send()
        .await
        .expect("command after TTL failed");
    assert_eq!(after_ttl.status(), reqwest::StatusCode::NOT_FOUND);

}

#[tokio::test]
async fn test_compat_forbidden_command_and_auth() {
    let server = spawn_server_with_compat_config(json!({
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "127.0.0.1",
        "http_port": 0,
        "database": 0,
        "acl": [
            {"disabled": ["*"]},
            {"http_basic_auth": "user:password", "enabled": ["*"]}
        ],
    }))
    .await;
    let client = Client::new();

    let body = create_compat_session(&client, server.port).await;
    let compat_id = body["id"].as_str().expect("id missing").to_string();

    let blocked = client
        .post(format!(
            "http://127.0.0.1:{}/__compat/cmd/{}.raw",
            server.port, compat_id
        ))
        .body(resp_command(&["GET", "compat_acl"]))
        .send()
        .await
        .expect("forbidden request failed");
    assert_eq!(blocked.status(), reqwest::StatusCode::OK);
    let blocked_body = blocked.bytes().await.expect("blocked body read failed");
    assert_eq!(blocked_body.as_ref(), b"-ERR forbidden\r\n");

    let allowed = client
        .post(format!(
            "http://127.0.0.1:{}/__compat/cmd/{}.raw",
            server.port, compat_id
        ))
        .header("authorization", "Basic dXNlcjpwYXNzd29yZA==")
        .body(resp_command(&["GET", "compat_acl"]))
        .send()
        .await
        .expect("allowed request failed");
    assert_eq!(allowed.status(), reqwest::StatusCode::OK);
    let allowed_body = allowed.bytes().await.expect("allowed body read failed");
    assert_eq!(allowed_body.as_ref(), b"$-1\r\n");
}

#[tokio::test]
async fn test_compat_stream_pubsub_message() {
    let server = TestServer::new().await;
    let client = Client::new();

    let body = create_compat_session(&client, server.port).await;
    let compat_id = body["id"].as_str().expect("id missing").to_string();

    let channel = format!("compat_stream_{}", server.port);
    let subscribe_resp = client
        .post(format!(
            "http://127.0.0.1:{}/__compat/cmd/{}.raw",
            server.port, compat_id
        ))
        .body(resp_command(&["SUBSCRIBE", &channel]))
        .send()
        .await
        .expect("SUBSCRIBE request failed");
    assert_eq!(subscribe_resp.status(), reqwest::StatusCode::OK);
    let subscribe_body = subscribe_resp.bytes().await.expect("SUBSCRIBE body failed");
    assert!(
        String::from_utf8_lossy(&subscribe_body).contains("subscribe"),
        "expected subscribe ack, got: {:?}",
        subscribe_body
    );

    let stream_response = client
        .get(format!(
            "http://127.0.0.1:{}/__compat/stream/{}.raw",
            server.port, compat_id
        ))
        .send()
        .await
        .expect("stream request failed");
    assert_eq!(stream_response.status(), reqwest::StatusCode::OK);

    sleep(Duration::from_millis(150)).await;
    let payload = "stream-payload";
    let _ = redis_publish(&channel, payload).await;

    let mut stream_response = stream_response;
    let chunk = tokio::time::timeout(Duration::from_secs(3), stream_response.chunk())
        .await
        .expect("timed out waiting for stream chunk")
        .expect("chunk read failed")
        .expect("stream ended without chunk");

    let text = String::from_utf8_lossy(&chunk);
    assert!(
        text.contains("message"),
        "expected pubsub message frame, got: {text:?}"
    );
    assert!(
        text.contains(payload),
        "expected payload in stream frame, got: {text:?}"
    );
}
