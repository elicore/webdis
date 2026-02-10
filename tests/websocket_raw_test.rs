//! Integration tests for the raw RESP WebSocket endpoint (`/.raw`).
//!
//! These tests verify that the server correctly handles raw RESP frames,
//! supporting various Redis commands, complex data types, streaming inputs,
//! and error conditions.

use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use webdis::{acl, config, handler, pubsub, redis, websocket};

async fn setup_test_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let mut config = config::Config::default();
    config.websockets = true;
    config.http_port = 0; // OS assigns port
    config.http_host = "127.0.0.1".to_string();
    config.redis_host = "127.0.0.1".to_string();
    config.redis_port = 6379;

    let pool = redis::create_pool(&config).unwrap();
    let pubsub_client = redis::create_pubsub_client(&config).unwrap();
    let pubsub_manager = pubsub::PubSubManager::new(pubsub_client);

    let app_state = Arc::new(handler::AppState {
        pool,
        acl: acl::Acl::new(config.acl.clone()),
        pubsub: pubsub_manager,
    });

    let app = axum::Router::new()
        .route("/.raw", axum::routing::get(websocket::ws_handler_raw))
        .with_state(app_state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    });

    (addr, handle)
}

#[tokio::test]
async fn test_websocket_raw_basic() {
    let (addr, _handle) = setup_test_server().await;
    let url = format!("ws://{}/.raw", addr);

    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Test SET
    let set_cmd = b"*3\r\n$3\r\nSET\r\n$10\r\ntest_key_r\r\n$7\r\nval_raw\r\n";
    ws_stream
        .send(Message::Binary(set_cmd.to_vec().into()))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    assert_eq!(&msg.into_data()[..], b"+OK\r\n");

    // Test GET
    let get_cmd = b"*2\r\n$3\r\nGET\r\n$10\r\ntest_key_r\r\n";
    ws_stream
        .send(Message::Binary(get_cmd.to_vec().into()))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    assert_eq!(&msg.into_data()[..], b"$7\r\nval_raw\r\n");
}

#[tokio::test]
async fn test_websocket_raw_complex_types() {
    let (addr, _handle) = setup_test_server().await;
    let url = format!("ws://{}/.raw", addr);
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Test LPUSH/LRANGE (List)
    let key = "test_list_raw";
    ws_stream
        .send(Message::Binary(
            format!("*2\r\n$3\r\nDEL\r\n${}\r\n{}\r\n", key.len(), key)
                .into_bytes()
                .into(),
        ))
        .await
        .unwrap();
    ws_stream.next().await.unwrap().unwrap(); // DEL result

    let lpush_cmd = format!(
        "*3\r\n$5\r\nLPUSH\r\n${}\r\n{}\r\n$5\r\nitem1\r\n",
        key.len(),
        key
    );
    ws_stream
        .send(Message::Binary(lpush_cmd.into_bytes().into()))
        .await
        .unwrap();
    ws_stream.next().await.unwrap().unwrap();

    let lrange_cmd = format!(
        "*4\r\n$6\r\nLRANGE\r\n${}\r\n{}\r\n$1\r\n0\r\n$2\r\n-1\r\n",
        key.len(),
        key
    );
    ws_stream
        .send(Message::Binary(lrange_cmd.into_bytes().into()))
        .await
        .unwrap();
    let msg = ws_stream.next().await.unwrap().unwrap();
    assert_eq!(&msg.into_data()[..], b"*1\r\n$5\r\nitem1\r\n");

    // Test Binary Data
    let binary_val = vec![0, 1, 2, 3, 255];
    let mut set_bin = format!(
        "*3\r\n$3\r\nSET\r\n$8\r\ntest_bin\r\n${}\r\n",
        binary_val.len()
    )
    .into_bytes();
    set_bin.extend_from_slice(&binary_val);
    set_bin.extend_from_slice(b"\r\n");
    ws_stream
        .send(Message::Binary(set_bin.into()))
        .await
        .unwrap();
    ws_stream.next().await.unwrap().unwrap();

    ws_stream
        .send(Message::Binary(
            b"*2\r\n$3\r\nGET\r\n$8\r\ntest_bin\r\n".to_vec().into(),
        ))
        .await
        .unwrap();
    let msg = ws_stream.next().await.unwrap().unwrap();
    let expected_data = {
        let mut d = b"$5\r\n".to_vec();
        d.extend_from_slice(&binary_val);
        d.extend_from_slice(b"\r\n");
        d
    };
    assert_eq!(&msg.into_data()[..], expected_data);
}

#[tokio::test]
async fn test_websocket_raw_streaming() {
    let (addr, _handle) = setup_test_server().await;
    let url = format!("ws://{}/.raw", addr);
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Send command in two parts
    let part1 = b"*2\r\n$4\r\nECHO\r\n";
    let part2 = b"$5\r\nhello\r\n";

    ws_stream
        .send(Message::Binary(part1.to_vec().into()))
        .await
        .unwrap();
    // Should NOT receive anything yet
    // But how to test "not receiving"? We can't easily without a timeout.

    ws_stream
        .send(Message::Binary(part2.to_vec().into()))
        .await
        .unwrap();
    let msg = ws_stream.next().await.unwrap().unwrap();
    assert_eq!(&msg.into_data()[..], b"$5\r\nhello\r\n");
}

#[tokio::test]
async fn test_websocket_raw_error() {
    let (addr, _handle) = setup_test_server().await;
    let url = format!("ws://{}/.raw", addr);
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    // Invalid RESP
    ws_stream
        .send(Message::Text("NOT_RESP\r\n".into()))
        .await
        .unwrap();
    let msg = ws_stream.next().await.unwrap().unwrap();
    assert!(msg.into_data().starts_with(b"-ERR"));

    // Unknown command
    ws_stream
        .send(Message::Binary(b"*1\r\n$7\r\nNON_CMD\r\n".to_vec().into()))
        .await
        .unwrap();
    let msg = ws_stream.next().await.unwrap().unwrap();
    assert!(msg.into_data().starts_with(b"-ERR"));
}
