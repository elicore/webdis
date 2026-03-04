mod support;

use reqwest::Client;
use std::time::Duration;
use support::process_harness::{redis_publish, TestServer};
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

#[tokio::test]
async fn test_compat_session_command_roundtrip() {
    let server = TestServer::new().await;
    let client = Client::new();

    let body = create_compat_session(&client, server.port).await;
    let compat_id = body["id"]
        .as_str()
        .expect("id missing")
        .to_string();

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
async fn test_compat_stream_pubsub_message() {
    let server = TestServer::new().await;
    let client = Client::new();

    let body = create_compat_session(&client, server.port).await;
    let compat_id = body["id"]
        .as_str()
        .expect("id missing")
        .to_string();

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
