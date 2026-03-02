//! Integration tests for the raw RESP WebSocket endpoint (`/.raw`).

mod support;

use futures_util::{SinkExt, StreamExt};
use support::process_harness::TestServer;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[tokio::test]
async fn test_websocket_raw_basic() {
    let server = TestServer::new().await;
    let url = format!("ws://127.0.0.1:{}/.raw", server.port);

    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    let set_cmd = b"*3\r\n$3\r\nSET\r\n$10\r\ntest_key_r\r\n$7\r\nval_raw\r\n";
    ws_stream
        .send(Message::Binary(set_cmd.to_vec().into()))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    assert_eq!(&msg.into_data()[..], b"+OK\r\n");

    let get_cmd = b"*2\r\n$3\r\nGET\r\n$10\r\ntest_key_r\r\n";
    ws_stream
        .send(Message::Binary(get_cmd.to_vec().into()))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    assert_eq!(&msg.into_data()[..], b"$7\r\nval_raw\r\n");
}

#[tokio::test]
async fn test_websocket_raw_streaming_and_error() {
    let server = TestServer::new().await;
    let url = format!("ws://127.0.0.1:{}/.raw", server.port);
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    ws_stream
        .send(Message::Binary(b"*2\r\n$4\r\nECHO\r\n".to_vec().into()))
        .await
        .unwrap();
    ws_stream
        .send(Message::Binary(b"$5\r\nhello\r\n".to_vec().into()))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    assert_eq!(&msg.into_data()[..], b"$5\r\nhello\r\n");

    ws_stream
        .send(Message::Text("NOT_RESP\r\n".into()))
        .await
        .unwrap();
    let msg = ws_stream.next().await.unwrap().unwrap();
    assert!(msg.into_data().starts_with(b"-ERR"));
}
