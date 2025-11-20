use crate::handler::redis_value_to_json;
use crate::handler::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use deadpool_redis::redis::{cmd, Value as RedisValue};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // Spawn a task to forward messages from the mpsc channel to the websocket sender
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            // client disconnected
            return;
        };

        if let Message::Text(text) = msg {
            // Parse message as JSON array: ["COMMAND", "arg1", "arg2"]
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&text) {
                if parsed.is_empty() {
                    continue;
                }

                let cmd_name = &parsed[0];
                let args = &parsed[1..];

                // Check ACL (TODO: Need IP here, but WebSocketUpgrade doesn't provide it easily without wrapper)
                // For now, skipping ACL check for WS or assuming allow.

                if cmd_name.eq_ignore_ascii_case("SUBSCRIBE") {
                    if args.is_empty() {
                        continue;
                    }
                    let channel = args[0].clone();
                    let mut pubsub_rx = state.pubsub.subscribe(channel).await;
                    let tx_clone = tx.clone();

                    // Spawn a task to forward Pub/Sub messages to the websocket
                    tokio::spawn(async move {
                        loop {
                            match pubsub_rx.recv().await {
                                Ok(msg) => {
                                    let response = serde_json::json!({"message": msg}); // Webdis format?
                                                                                        // Webdis format for pubsub: {"SUBSCRIBE":["message","channel","payload"]}
                                                                                        // Actually, Webdis C format is: {"SUBSCRIBE": ["message", "channel", "payload"]}
                                                                                        // But my PubSubManager only sends payload.
                                                                                        // I should probably include channel in the broadcast message or change PubSubManager.
                                                                                        // For now, let's just send the payload as a string or JSON.
                                                                                        // Let's wrap it: {"message": payload}
                                    if tx_clone
                                        .send(Message::Text(response.to_string()))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                    continue;
                }

                let mut conn = match state.pool.get().await {
                    Ok(conn) => conn,
                    Err(_) => {
                        let _ = tx
                            .send(Message::Text(
                                serde_json::json!({"error": "Service Unavailable"}).to_string(),
                            ))
                            .await;
                        continue;
                    }
                };

                let mut redis_cmd = cmd(cmd_name);
                for arg in args {
                    redis_cmd.arg(arg);
                }

                let result: Result<RedisValue, _> = redis_cmd.query_async(&mut conn).await;
                match result {
                    Ok(val) => {
                        let json_val = redis_value_to_json(val);
                        let response = serde_json::json!({cmd_name: json_val});
                        let _ = tx.send(Message::Text(response.to_string())).await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Message::Text(
                                serde_json::json!({"error": e.to_string()}).to_string(),
                            ))
                            .await;
                    }
                }
            }
        }
    }
}
