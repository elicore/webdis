use crate::acl::Acl;
use crate::format::OutputFormat;
use crate::redis::RedisPool;
use axum::extract::ConnectInfo;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use deadpool_redis::redis::{cmd, Value as RedisValue};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::pubsub::PubSubManager;
use sha1::{Digest, Sha1};

pub async fn handle_default_root(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    default_root: String,
) -> Response {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    process_request(
        default_root,
        params,
        None,
        state,
        addr,
        auth_header,
        headers,
    )
    .await
}

pub struct AppState {
    pub pool: RedisPool,
    pub acl: Acl,
    pub pubsub: PubSubManager,
}

use axum::body::Bytes;
// use axum::body::Bytes; // Already imported above
// use axum::http::HeaderMap; // Already imported above

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

use axum::extract::Query;
use std::collections::HashMap;

pub async fn handle_post(
    Path(command): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    process_request(
        command,
        params,
        Some(body.to_vec()),
        state,
        addr,
        auth_header,
        headers,
    )
    .await
}

pub async fn handle_put(
    Path(command): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    process_request(
        command,
        params,
        Some(body.to_vec()),
        state,
        addr,
        auth_header,
        headers,
    )
    .await
}

pub async fn handle_get(
    Path(command): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    process_request(command, params, None, state, addr, auth_header, headers).await
}

async fn process_request(
    command: String,
    params: HashMap<String, String>,
    body: Option<Vec<u8>>,
    state: Arc<AppState>,
    addr: SocketAddr,
    auth_header: Option<String>,
    headers: HeaderMap,
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

    // Check for extension or query param
    let mut format = OutputFormat::Json;

    // 1. Check extension
    if let Some(idx) = cmd_name.rfind('.') {
        let ext = &cmd_name[idx + 1..];
        format = OutputFormat::from_extension(ext);
        cmd_name = &cmd_name[..idx];
    } else if let Some(last_arg) = args.last_mut() {
        if let Some(idx) = last_arg.rfind('.') {
            let ext = &last_arg[idx + 1..].to_string();
            // Only treat as extension if it matches a known format
            let f = OutputFormat::from_extension(ext);
            if !matches!(f, OutputFormat::Json) || ext == "json" {
                format = f;
                *last_arg = last_arg[..idx].to_string();
            }
        }
    }

    // 2. Check query param (overrides extension if present, or maybe fallback? Webdis C seems to prefer extension)
    // Let's allow query param to override for now if extension didn't change it from default,
    // or if we want to support ?type=raw explicitly.
    if let Some(type_param) = params.get("type") {
        // Map type param to format
        // "raw" -> Raw, "json" -> Json, etc.
        // We can reuse from_extension for simple mapping
        format = OutputFormat::from_extension(type_param);
    }

    // Append body as the last argument if present
    if let Some(body_bytes) = body.as_ref() {
        if !body_bytes.is_empty() {
            args.push(String::from_utf8_lossy(body_bytes).to_string());
        }
    }

    // Check ACL
    if !state.acl.check(addr.ip(), cmd_name, auth_header.as_deref()) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Forbidden"}))).into_response();
    }

    let mut redis_cmd = cmd(cmd_name);
    for arg in &args {
        redis_cmd.arg(arg);
    }

    let result: Result<RedisValue, _> = redis_cmd.query_async(&mut conn).await;

    let callback = params.get("callback").cloned();

    let mut response = match result {
        Ok(val) => {
            let mut json_val = redis_value_to_json(val);

            // Special handling for INFO command to return structured JSON
            if (cmd_name.eq_ignore_ascii_case("INFO")
                || (cmd_name.eq_ignore_ascii_case("CLUSTER")
                    && args
                        .get(0)
                        .map_or(false, |a| a.eq_ignore_ascii_case("INFO"))))
                && json_val.is_string()
            {
                if let Some(s) = json_val.as_str() {
                    json_val = parse_info_output(s);
                }
            }

            // Compute ETag for GET requests (body is None)
            let etag = if body.is_none() {
                let mut hasher = Sha1::new();
                hasher.update(cmd_name.as_bytes());
                for arg in &args {
                    hasher.update(arg.as_bytes());
                }
                // Use the string representation of json_val for stable hashing
                hasher.update(json_val.to_string().as_bytes());
                let tag = format!("\"{:x}\"", hasher.finalize());

                if let Some(if_none_match) = headers
                    .get(header::IF_NONE_MATCH)
                    .and_then(|h| h.to_str().ok())
                {
                    if if_none_match == tag {
                        let mut resp = StatusCode::NOT_MODIFIED.into_response();
                        resp.headers_mut()
                            .insert(header::ETAG, tag.parse().unwrap());
                        resp.headers_mut()
                            .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
                        return resp;
                    }
                }
                Some(tag)
            } else {
                None
            };

            let mut resp = format.format_response(cmd_name, json_val, callback);
            if let Some(tag) = etag {
                resp.headers_mut()
                    .insert(header::ETAG, tag.parse().unwrap());
            }
            resp
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

/// Parses the textual output of the Redis INFO command into a structured JSON object.
///
/// It splits the output line by line, ignoring comments (starting with #) and empty lines,
/// and parses "key:value" pairs into a JSON map.
pub fn parse_info_output(text: &str) -> Value {
    let mut map = serde_json::Map::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            map.insert(
                key.trim().to_string(),
                Value::String(value.trim().to_string()),
            );
        }
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_info_output() {
        let input = "
# Server
redis_version:7.2.3
uptime_in_seconds:3600

# Clients
connected_clients:1
";
        let expected = json!({
            "redis_version": "7.2.3",
            "uptime_in_seconds": "3600",
            "connected_clients": "1"
        });
        assert_eq!(parse_info_output(input), expected);
    }

    #[test]
    fn test_parse_info_output_empty() {
        assert_eq!(parse_info_output(""), json!({}));
    }

    #[test]
    fn test_parse_info_output_no_colon() {
        let input = "invalid line\nkey:value";
        let expected = json!({"key": "value"});
        assert_eq!(parse_info_output(input), expected);
    }
}
