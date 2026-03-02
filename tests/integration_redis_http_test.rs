mod support;

use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use support::process_harness::{
    parse_jsonp_body, raw_http_get, redis_connect_local_db, redis_get_string, TestServer,
};

#[tokio::test]
async fn test_database_prefix_separate_values_per_db() {
    let server = TestServer::new().await;
    let client = Client::new();
    let key = format!("db_prefix_key_{}", server.port);

    let mut db0 = redis_connect_local_db(0).await;
    let mut db7 = redis_connect_local_db(7).await;
    let _: i64 = redis::cmd("DEL").arg(&key).query_async(&mut db0).await.unwrap();
    let _: i64 = redis::cmd("DEL").arg(&key).query_async(&mut db7).await.unwrap();

    let resp = client
        .get(format!("http://127.0.0.1:{}/SET/{}/value0", server.port, key))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let resp = client
        .get(format!("http://127.0.0.1:{}/7/SET/{}/value7", server.port, key))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/{}", server.port, key))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["GET"], "value0");

    let resp = client
        .get(format!("http://127.0.0.1:{}/7/GET/{}", server.port, key))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["GET"], "value7");
}

#[tokio::test]
async fn test_percent_decoding_slash_in_key_roundtrip() {
    let server = TestServer::new().await;
    let key = format!("percent_slash_key_{}/b", server.port);

    let (status, _) = raw_http_get(
        server.port,
        &format!("/SET/percent_slash_key_{}%2Fb/value", server.port),
    )
    .await;
    assert_eq!(status, 200);

    let (status, body) =
        raw_http_get(server.port, &format!("/GET/percent_slash_key_{}%2Fb", server.port)).await;
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["GET"], "value");

    let direct = redis_get_string(&key).await;
    assert_eq!(direct.as_deref(), Some("value"));
}

#[tokio::test]
async fn test_jsonp_and_raw_parity() {
    let server = TestServer::new().await;
    let client = Client::new();

    let _ = client
        .get(format!("http://127.0.0.1:{}/SET/hello/world", server.port))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/hello?jsonp=myFn", server.port))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    let (cb, json) = parse_jsonp_body(&body);
    assert_eq!(cb, "myFn");
    assert_eq!(json["GET"], "world");

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/hello.raw?jsonp=myFn", server.port))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert_eq!(body, "$5\r\nworld\r\n");
}

#[tokio::test]
async fn test_extension_content_types_and_override() {
    let server = TestServer::new().await;
    let client = Client::new();

    let _ = client
        .get(format!("http://127.0.0.1:{}/SET/hello/world", server.port))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/hello.txt", server.port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.headers()[reqwest::header::CONTENT_TYPE], "text/plain");

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/GET/hello?type=application/pdf",
            server.port
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.headers()[reqwest::header::CONTENT_TYPE], "application/pdf");
}

#[tokio::test]
async fn test_binary_content_type_png_roundtrip() {
    let server = TestServer::new().await;
    let client = Client::new();

    let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO0X6b8AAAAASUVORK5CYII=";
    let png_bytes = general_purpose::STANDARD.decode(png_b64).unwrap();

    let resp = client
        .put(format!("http://127.0.0.1:{}/SET/logo", server.port))
        .body(png_bytes.clone())
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/logo.png", server.port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.headers()[reqwest::header::CONTENT_TYPE], "image/png");
    let body = resp.bytes().await.unwrap();
    assert_eq!(&body[..], &png_bytes[..]);
}

#[tokio::test]
async fn test_etag_support() {
    let server = TestServer::new().await;
    let client = Client::new();

    let _ = client
        .get(format!("http://127.0.0.1:{}/SET/etag_key/foo", server.port))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/etag_key", server.port))
        .send()
        .await
        .unwrap();

    let etag = resp.headers().get("ETag").unwrap().clone();

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/etag_key", server.port))
        .header("If-None-Match", etag.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_MODIFIED);

    let _ = client
        .get(format!("http://127.0.0.1:{}/SET/etag_key/bar", server.port))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://127.0.0.1:{}/GET/etag_key", server.port))
        .header("If-None-Match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn test_info_command_returns_structured_json() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(format!("http://127.0.0.1:{}/INFO", server.port))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["INFO"].is_object());
}
