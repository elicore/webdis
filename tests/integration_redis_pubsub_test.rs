mod support;

use reqwest::Client;
use std::time::Duration;
use support::process_harness::{read_stream_lines, redis_publish, TestServer};
use tokio::time::sleep;

#[tokio::test]
async fn test_subscribe_chunked_json_stream() {
    let server = TestServer::new().await;
    let client = Client::new();
    let channel = format!("comet_json_{}", server.port);

    let response = client
        .get(format!("http://127.0.0.1:{}/SUBSCRIBE/{}", server.port, channel))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .unwrap();

    sleep(Duration::from_millis(150)).await;
    let _ = redis_publish(&channel, "hello").await;
    let _ = redis_publish(&channel, "world").await;

    let lines = read_stream_lines(response, 2, Duration::from_secs(3)).await;
    assert_eq!(lines.len(), 2);
}

#[tokio::test]
async fn test_subscribe_jsonp_comet_stream() {
    let server = TestServer::new().await;
    let client = Client::new();
    let channel = format!("comet_jsonp_{}", server.port);

    let response = client
        .get(format!(
            "http://127.0.0.1:{}/SUBSCRIBE/{}?jsonp=myFn",
            server.port, channel
        ))
        .send()
        .await
        .unwrap();

    sleep(Duration::from_millis(150)).await;
    let _ = redis_publish(&channel, "one").await;

    let lines = read_stream_lines(response, 1, Duration::from_secs(3)).await;
    assert!(lines[0].starts_with("myFn("));
}

#[tokio::test]
async fn test_subscribe_sse_default_compatibility() {
    let server = TestServer::new().await;
    let client = Client::new();
    let channel = format!("sse_default_{}", server.port);

    let response = client
        .get(format!("http://127.0.0.1:{}/SUBSCRIBE/{}", server.port, channel))
        .send()
        .await
        .unwrap();

    sleep(Duration::from_millis(150)).await;
    let expected_payload = "sse-payload";
    let _ = redis_publish(&channel, expected_payload).await;

    let lines = read_stream_lines(response, 1, Duration::from_secs(3)).await;
    assert!(lines[0].contains("data:"));
}
