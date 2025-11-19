use crate::acl::Acl;
use crate::format::OutputFormat;
use crate::redis::RedisPool;
use axum::extract::ConnectInfo;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use deadpool_redis::redis::{cmd, Value as RedisValue};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

pub struct AppState {
    pub pool: RedisPool,
    pub acl: Acl,
}

use axum::body::Bytes;
use axum::http::HeaderMap;

pub async fn handle_options() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    headers.insert(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, OPTIONS".parse().unwrap(),
    );
    headers.insert("Access-Control-Allow-Headers", "*".parse().unwrap());
    (StatusCode::OK, headers).into_response()
}

pub async fn handle_post(
    Path(command): Path<String>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Bytes,
) -> Response {
    process_request(command, Some(body.to_vec()), state, addr).await
}

pub async fn handle_put(
    Path(command): Path<String>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Bytes,
) -> Response {
    process_request(command, Some(body.to_vec()), state, addr).await
}

pub async fn handle_get(
    Path(command): Path<String>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    process_request(command, None, state, addr).await
}

async fn process_request(
    command: String,
    body: Option<Vec<u8>>,
    state: Arc<AppState>,
    addr: SocketAddr,
) -> Response {
    let mut conn = match state.pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Parse the command path (e.g., "GET/hello")
    let parts: Vec<&str> = command.split('/').collect();
    if parts.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Empty command"})),
        )
            .into_response();
    }

    let mut cmd_name = parts[0];
    let mut args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    // Check for extension
    let mut format = OutputFormat::Json;
    if let Some(idx) = cmd_name.rfind('.') {
        let ext = &cmd_name[idx + 1..];
        format = OutputFormat::from_extension(ext);
        cmd_name = &cmd_name[..idx];
    } else if let Some(last_arg) = args.last() {
        if let Some(_idx) = last_arg.rfind('.') {
            // Handle extension on last argument if needed
        }
    }

    // Append body as the last argument if present
    if let Some(body_bytes) = body {
        if !body_bytes.is_empty() {
            // We need to handle binary data.
            // For now, assuming UTF-8 for simplicity in args, but Redis args are binary.
            // The redis crate's `arg` method takes `ToRedisArgs`.
            // We should probably keep args as Vec<Vec<u8>> to support binary.
            // But `parts` comes from URL which is string.
            // So we convert URL parts to bytes, and append body bytes.
            args.push(String::from_utf8_lossy(&body_bytes).to_string());
        }
    }

    // Check ACL
    if !state.acl.check(addr.ip(), cmd_name) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Forbidden"}))).into_response();
    }

    let mut redis_cmd = cmd(cmd_name);
    for arg in args {
        redis_cmd.arg(arg);
    }

    let result: Result<RedisValue, _> = redis_cmd.query_async(&mut conn).await;

    let mut response = match result {
        Ok(val) => {
            let json_val = redis_value_to_json(val);
            format.format_response(cmd_name, json_val)
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    };

    // Add CORS headers to every response
    response
        .headers_mut()
        .insert("Access-Control-Allow-Origin", "*".parse().unwrap());

    response
}

pub fn redis_value_to_json(v: RedisValue) -> Value {
    match v {
        RedisValue::Nil => Value::Null,
        RedisValue::Int(i) => Value::Number(i.into()),
        RedisValue::Data(bytes) => Value::String(String::from_utf8_lossy(&bytes).to_string()),
        RedisValue::Bulk(items) => {
            Value::Array(items.into_iter().map(redis_value_to_json).collect())
        }
        RedisValue::Status(s) => Value::String(s),
        RedisValue::Okay => Value::String("OK".to_string()),
    }
}
