mod support;

use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use support::router_harness::{functional_config, FunctionalServer};
use support::stub_executor::ScriptedStubExecutor;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[tokio::test]
async fn test_raw_ws_invalid_resp_returns_err_without_redis() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let mut cfg = functional_config();
    cfg.websockets = true;
    let server = FunctionalServer::spawn(cfg, executor).await;

    let url = format!("ws://{}/.raw", server.addr);
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    ws_stream
        .send(Message::Text("NOT_RESP\r\n".into()))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    let data = msg.into_data();
    assert!(
        data.starts_with(b"-ERR"),
        "expected RESP error, got: {:?}",
        data
    );
}
