use crate::acl::Acl;
use crate::format::{
    content_type_for_extension, json_value_response, select_jsonp_callback, OutputFormat,
};
use crate::redis::RedisPool;
use crate::resp; // Added resp module
use axum::body::Body; // Added Body
use axum::extract::ConnectInfo;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
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
    // Parse the command path (e.g., "GET/hello")
    let parts: Vec<&str> = command.split('/').collect();
    if parts.is_empty() {
        // No meaningful command means we can't select a non-JSON output format.
        // Still honor JSONP on the default JSON response when requested.
        let jsonp = select_jsonp_callback(&params);
        return json_value_response(
            StatusCode::BAD_REQUEST,
            json!({"error": "Empty command"}),
            jsonp,
        );
    }

    let mut cmd_name = parts[0];
    let mut args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    // Parse an optional suffix from the *final path segment* (or the command name when there
    // are no args). In Webdis, `/COMMAND/.../argN.ext` selects the output format and/or default
    // content type, while the `argN` used for Redis is `argN` without the suffix.
    let mut ext: Option<String> = None;
    if let Some(last_arg) = args.last_mut() {
        if let Some(idx) = last_arg.rfind('.') {
            let candidate = last_arg[idx + 1..].to_ascii_lowercase();
            let known = OutputFormat::from_extension(candidate.as_str()).is_some()
                || content_type_for_extension(candidate.as_str()).is_some();
            if known {
                ext = Some(candidate);
                last_arg.truncate(idx);
            }
        }
    } else if let Some(idx) = cmd_name.rfind('.') {
        let candidate = cmd_name[idx + 1..].to_ascii_lowercase();
        let known = OutputFormat::from_extension(candidate.as_str()).is_some()
            || content_type_for_extension(candidate.as_str()).is_some();
        if known {
            ext = Some(candidate);
            cmd_name = &cmd_name[..idx];
        }
    }

    // The output format controls the response body. `?type=<mime>` is *header-only*.
    let mut format = OutputFormat::Json;
    if let Some(ext) = ext.as_deref() {
        if let Some(f) = OutputFormat::from_extension(ext) {
            format = f;
        }
    }

    // Optional content-type override (header only; does not affect payload format).
    let content_type_override = params
        .get("type")
        .and_then(|s| (!s.is_empty()).then_some(s.clone()));
    let ext_content_type = ext.as_deref().and_then(|e| content_type_for_extension(e));

    // JSONP is HTTP-only and applies only to JSON output.
    // For non-JSON formats (.raw, .msg/.msgpack, .txt/.html/.xml/.png, etc.), ignore `jsonp`/`callback`.
    let jsonp_callback = matches!(format, OutputFormat::Json)
        .then(|| select_jsonp_callback(&params))
        .flatten();

    // For PUT/POST requests, the HTTP body is appended as the last Redis argument.
    // This must be passed as raw bytes to preserve binary payloads (images, etc.).
    let body_arg = body.as_deref().filter(|b| !b.is_empty());

    // Check ACL
    if !state.acl.check(addr.ip(), cmd_name, auth_header.as_deref()) {
        return json_value_response(
            StatusCode::FORBIDDEN,
            json!({"error": "Forbidden"}),
            jsonp_callback,
        );
    }

    let mut conn = match state.pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            // Preserve status codes, but wrap the JSON error payload when JSONP is requested.
            return json_value_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error": e.to_string()}),
                jsonp_callback,
            );
        }
    };

    let mut redis_cmd = cmd(cmd_name);
    for arg in &args {
        redis_cmd.arg(arg);
    }
    if let Some(bytes) = body_arg {
        redis_cmd.arg(bytes);
    }

    let result: Result<RedisValue, _> = redis_cmd.query_async(&mut conn).await;

    let mut response = match result {
        Ok(val) => {
            if matches!(format, OutputFormat::Raw) {
                // Raw mode: convert directly to RESP (Redis Serialization Protocol) bytes.
                // This bypasses the JSON conversion and returns the exact wire protocol representation
                // expected by Redis clients.
                let bytes = resp::value_to_resp(&val);
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(bytes))
                    .unwrap()
            } else if matches!(format, OutputFormat::Text) {
                // Text/binary mode: return only the string value bytes with a MIME type
                // implied by the suffix (.txt, .html, .png, etc.).
                let bytes = match redis_value_to_bytes(val) {
                    Some(b) => b,
                    None => {
                        return json_value_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({"error": "Text output supports only string/binary Redis replies"}),
                            None,
                        );
                    }
                };

                // Compute ETag for GET requests (body is None).
                let etag = if body.is_none() {
                    let mut hasher = Sha1::new();
                    hasher.update(cmd_name.as_bytes());
                    for arg in &args {
                        hasher.update(arg.as_bytes());
                    }
                    hasher.update(&bytes);
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

                let mut resp = Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(bytes))
                    .unwrap();
                if let Some(tag) = etag {
                    resp.headers_mut()
                        .insert(header::ETAG, tag.parse().unwrap());
                }
                resp
            } else {
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
                    if let Some(cb) = jsonp_callback {
                        hasher.update(cb.as_bytes());
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

                // Note: ETag must vary by JSONP callback, since the response body changes.
                let mut resp = format.format_response(cmd_name, json_val, jsonp_callback);
                if let Some(tag) = etag {
                    resp.headers_mut()
                        .insert(header::ETAG, tag.parse().unwrap());
                }
                resp
            }
        }
        Err(e) => {
            if matches!(format, OutputFormat::Raw) {
                // Raw Error: -ERR message
                let err_msg = format!("-ERR {}\r\n", e);
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(err_msg))
                    .unwrap()
            } else if matches!(format, OutputFormat::Text) {
                // Text errors mirror the original Webdis behavior: errors are plain text.
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(e.to_string()))
                    .unwrap()
            } else {
                json_value_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": e.to_string()}),
                    jsonp_callback,
                )
            }
        }
    };

    // Add CORS headers to every response
    response
        .headers_mut()
        .insert("Access-Control-Allow-Origin", "*".parse().unwrap());

    // Apply suffix-selected content types and/or `?type=<mime>` overrides.
    //
    // Precedence:
    // 1) `?type` override (always wins)
    // 2) JSONP content type (unless overridden)
    // 3) Extension mapping (e.g. `.png` => `image/png`)
    if let Some(ct) = content_type_override.as_deref() {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, ct.parse().unwrap());
    } else if jsonp_callback.is_none() {
        // Only apply extension-derived content types when the response didn't already
        // define its own `Content-Type`. This avoids incorrectly overwriting error
        // responses (e.g. JSON error bodies) with image/* content types.
        if response.headers().get(header::CONTENT_TYPE).is_none() {
            if let Some(ct) = ext_content_type {
                response
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, ct.parse().unwrap());
            }
        }
    }

    response
}

/// Converts a Redis response value (RedisValue) into a JSON Value.
/// This mapping accounts for Redis 0.32+ variant names and prepares for RESP3 types.
pub fn redis_value_to_json(v: RedisValue) -> Value {
    match v {
        RedisValue::Nil => Value::Null,
        RedisValue::Int(i) => Value::Number(i.into()),
        // BulkString replaces the older 'Data' variant in modern redis-rs.
        RedisValue::BulkString(bytes) => Value::String(String::from_utf8_lossy(&bytes).to_string()),
        // Array replaces the older 'Bulk' variant.
        RedisValue::Array(items) => {
            Value::Array(items.into_iter().map(redis_value_to_json).collect())
        }
        // SimpleString replaces 'Status'.
        RedisValue::SimpleString(s) => Value::String(s),
        RedisValue::Okay => Value::String("OK".to_string()),
        // Handle new RESP3 types (Map, Set, Attribute, etc.) by defaulting to Null for now.
        // As Webdis evolves, these can be mapped to more specific JSON structures.
        _ => Value::Null,
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

/// Converts a Redis reply into raw bytes suitable for "text/binary" HTTP responses.
///
/// This is used for suffixes like `.txt`, `.html`, `.xml`, `.png`, `.jpg`, `.jpeg` where
/// the response body should be the stored Redis string bytes *without* JSON wrapping.
fn redis_value_to_bytes(v: RedisValue) -> Option<Vec<u8>> {
    match v {
        RedisValue::Nil => Some(Vec::new()),
        RedisValue::Int(i) => Some(i.to_string().into_bytes()),
        RedisValue::BulkString(bytes) => Some(bytes),
        RedisValue::SimpleString(s) => Some(s.into_bytes()),
        RedisValue::Okay => Some(b"OK".to_vec()),
        _ => None,
    }
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
