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
use std::sync::Arc;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    while let Some(msg) = socket.recv().await {
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

                let mut conn = match state.pool.get().await {
                    Ok(conn) => conn,
                    Err(_) => {
                        let _ = socket
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
                        if socket
                            .send(Message::Text(response.to_string()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = socket
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
