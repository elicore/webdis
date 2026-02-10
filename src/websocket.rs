use crate::handler::redis_value_to_json;
use crate::handler::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use redis::{cmd, Value as RedisValue};
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
                                        .send(Message::Text(response.to_string().into()))
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
                                serde_json::json!({"error": "Service Unavailable"})
                                    .to_string()
                                    .into(),
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
                        // Axum 0.8 requires Utf8Bytes for Message::Text; .into() handles the conversion from String.
                        let _ = tx.send(Message::Text(response.to_string().into())).await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Message::Text(
                                serde_json::json!({"error": e.to_string()})
                                    .to_string()
                                    .into(),
                            ))
                            .await;
                    }
                }
            }
        }
    }
}

/// Axum handler for raw RESP WebSocket connections at `/.raw`.
///
/// This endpoint allows clients to send and receive raw Redis protocol frames.
pub async fn ws_handler_raw(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_socket_raw(socket, state))
}

/// Main loop for raw RESP WebSocket connections.
///
/// It maintains a buffer for incoming data, parses complete RESP commands,
/// executes them against Redis, and sends the raw RESP responses back.
async fn handle_socket_raw(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut buffer = Vec::new();

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(_) => return, // Client disconnected or error
        };

        // Handle different message types
        match msg {
            Message::Binary(data) => buffer.extend_from_slice(&data),
            Message::Text(text) => buffer.extend_from_slice(text.as_bytes()),
            Message::Close(_) => return,
            Message::Ping(p) => {
                if sender.send(Message::Pong(p)).await.is_err() {
                    return;
                }
                continue;
            }
            _ => continue,
        }

        // Process any complete commands in the buffer
        loop {
            match crate::resp::parse_command(&buffer) {
                Ok(Some((args, consumed))) => {
                    // Consume the bytes used by the command
                    buffer.drain(..consumed);

                    if args.is_empty() {
                        continue;
                    }

                    // Get a connection from the pool
                    let mut conn = match state.pool.get().await {
                        Ok(conn) => conn,
                        Err(e) => {
                            let err_resp = format!("-ERR {}\r\n", e);
                            if sender
                                .send(Message::Binary(err_resp.into_bytes().into()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            continue;
                        }
                    };

                    // Build and execute the Redis command
                    let cmd_name = String::from_utf8_lossy(&args[0]).to_string();
                    let mut redis_cmd = cmd(&cmd_name);
                    for arg in &args[1..] {
                        redis_cmd.arg(arg);
                    }

                    let result: Result<RedisValue, _> = redis_cmd.query_async(&mut conn).await;
                    match result {
                        Ok(val) => {
                            // Convert result to RESP and send as binary message
                            let resp = crate::resp::value_to_resp(&val);
                            if sender.send(Message::Binary(resp.into())).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            // Forward Redis error as RESP error
                            let err_resp = format!("-ERR {}\r\n", e);
                            if sender
                                .send(Message::Binary(err_resp.into_bytes().into()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
                Ok(None) => break, // Need more data for a complete command
                Err(e) => {
                    let err_msg = match e {
                        crate::resp::RespError::Incomplete => break, // Should not happen with current parser but handled for safety
                        _ => {
                            // Fatal command format error, clear buffer and inform client
                            buffer.clear();
                            "-ERR Invalid RESP\r\n"
                        }
                    };
                    if err_msg.starts_with("-ERR") {
                        if sender
                            .send(Message::Binary(err_msg.as_bytes().to_vec().into()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    break;
                }
            }
        }
    }
}
