mod support;

use reqwest::Client;
use std::sync::Arc;
use support::process_harness::parse_jsonp_body;
use support::router_harness::{functional_config, FunctionalServer};
use support::stub_executor::ScriptedStubExecutor;

#[tokio::test]
async fn test_options_headers_contract() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let server = FunctionalServer::spawn(functional_config(), executor).await;
    let client = Client::new();

    let resp = client
        .request(reqwest::Method::OPTIONS, format!("http://{}/", server.addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    assert!(resp
        .headers()
        .get("Access-Control-Allow-Methods")
        .is_some());
}

#[tokio::test]
async fn test_request_body_size_limit() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let mut cfg = functional_config();
    cfg.http_max_request_size = Some(1024);
    let server = FunctionalServer::spawn(cfg, executor).await;
    let client = Client::new();

    let payload = vec![b'A'; 4096];
    let resp = client
        .put(format!("http://{}/SET/too_big", server.addr))
        .body(payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn test_content_type_override_and_jsonp_raw_exclusion() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let server = FunctionalServer::spawn(functional_config(), executor).await;
    let client = Client::new();

    let _ = client
        .get(format!("http://{}/SET/hello/world", server.addr))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://{}/GET/hello?type=application/pdf", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.headers()[reqwest::header::CONTENT_TYPE], "application/pdf");

    let resp = client
        .get(format!("http://{}/GET/hello?jsonp=myFn", server.addr))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    let (cb, json) = parse_jsonp_body(&body);
    assert_eq!(cb, "myFn");
    assert_eq!(json["GET"], "world");

    let resp = client
        .get(format!("http://{}/GET/hello.raw?jsonp=myFn", server.addr))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert_eq!(body, "$5\r\nworld\r\n");
}

#[tokio::test]
async fn test_etag_positive_and_negative_paths() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let server = FunctionalServer::spawn(functional_config(), executor).await;
    let client = Client::new();

    let _ = client
        .get(format!("http://{}/SET/etag_key/foo", server.addr))
        .send()
        .await
        .unwrap();

    let first = client
        .get(format!("http://{}/GET/etag_key", server.addr))
        .send()
        .await
        .unwrap();
    let etag = first.headers().get("ETag").unwrap().clone();

    let not_modified = client
        .get(format!("http://{}/GET/etag_key", server.addr))
        .header("If-None-Match", etag.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(not_modified.status(), reqwest::StatusCode::NOT_MODIFIED);

    let _ = client
        .get(format!("http://{}/SET/etag_key/bar", server.addr))
        .send()
        .await
        .unwrap();

    let changed = client
        .get(format!("http://{}/GET/etag_key", server.addr))
        .header("If-None-Match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), reqwest::StatusCode::OK);
}
