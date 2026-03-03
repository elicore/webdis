use crate::redis::DatabasePoolRegistry;
use axum::body::Body; // Added Body
use axum::extract::{ConnectInfo, OriginalUri};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use redis::Value as RedisValue;
use redis_web_core::acl::Acl;
use redis_web_core::format::{json_value_response, select_jsonp_callback, OutputFormat};
use redis_web_core::interfaces::{
    CommandExecutionError, CommandExecutor, ParseRequestInput, RequestParser,
};
use redis_web_core::request::RequestParseError;
use redis_web_core::resp;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;

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

/// Shared application state injected into HTTP and WebSocket handlers.
///
/// `redis_pools` serves regular Redis command traffic, while `pubsub` owns separate
/// long-lived Redis subscription machinery. Keeping these separate avoids
/// mixing blocking Pub/Sub loops with pooled command connections.
pub struct AppState {
    /// Lazily created Redis pools keyed by logical database index.
    pub redis_pools: Arc<DatabasePoolRegistry>,
    /// Default logical database configured in the active runtime config.
    pub default_database: u8,
    /// Parser used by HTTP handlers to normalize input into executable requests.
    pub request_parser: Arc<dyn RequestParser>,
    /// Executor used to run normalized requests against Redis or another backend.
    pub command_executor: Arc<dyn CommandExecutor>,
    pub acl: Acl,
    pub pubsub: PubSubManager,
    /// Optional hiredis-compat session manager (mounted under `/__compat/*`).
    pub compat_hiredis: Option<Arc<crate::compat::CompatSessionManager>>,
}

use axum::body::Bytes;
// use axum::body::Bytes; // Already imported above
// use axum::http::HeaderMap; // Already imported above

/// Handles CORS preflight requests for all command routes.
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

/// Handles `POST` command requests where the URI encodes command parts and the
/// HTTP body is appended as the final Redis argument.
pub async fn handle_post(
    OriginalUri(uri): OriginalUri,
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
    // Use the raw request URI path (percent-encoded) to preserve `%2f` and `%2e` semantics.
    // `Path<String>` would decode many percent-escapes before we can apply Webdis-compatible
    // segment decoding rules.
    let command = uri.path().trim_start_matches('/').to_string();
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

/// Handles `PUT` command requests where the URI encodes command parts and the
/// HTTP body is appended as the final Redis argument.
pub async fn handle_put(
    OriginalUri(uri): OriginalUri,
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
    let command = uri.path().trim_start_matches('/').to_string();
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

/// Handles `GET` command requests where the full Redis command is encoded in
/// the request path.
pub async fn handle_get(
    OriginalUri(uri): OriginalUri,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let command = uri.path().trim_start_matches('/').to_string();
    process_request(command, params, None, state, addr, auth_header, headers).await
}

/// Normalizes an HTTP request into a Redis command and emits a formatted HTTP response.
///
/// Non-trivial behavior in this path includes:
/// - optional `/<db>/` prefix parsing with strict numeric range validation,
/// - per-request DB pool selection without connection state bleed,
/// - extension-driven response formatting and content type negotiation,
/// - ACL checks and conditional ETag handling.
async fn process_request(
    command: String,
    params: HashMap<String, String>,
    body: Option<Vec<u8>>,
    state: Arc<AppState>,
    addr: SocketAddr,
    auth_header: Option<String>,
    headers: HeaderMap,
) -> Response {
    let parsed = match state.request_parser.parse(ParseRequestInput {
        command_path: command.as_str(),
        params: &params,
        default_database: state.default_database,
        body: body.as_deref(),
        etag_enabled: body.is_none(),
    }) {
        Ok(parsed) => parsed,
        Err(error) => {
            let jsonp = select_jsonp_callback(&params);
            return json_value_response(
                StatusCode::BAD_REQUEST,
                json!({"error": request_parse_error_message(&error)}),
                jsonp,
            );
        }
    };

    // Check ACL
    if !state.acl.check(
        addr.ip(),
        parsed.command_name.as_str(),
        auth_header.as_deref(),
    ) {
        return json_value_response(
            StatusCode::FORBIDDEN,
            json!({"error": "Forbidden"}),
            parsed.jsonp_callback.as_deref(),
        );
    }

    let execution = state.command_executor.execute(&parsed).await;

    let mut response = match execution {
        Ok(val) => {
            if matches!(parsed.output_format, OutputFormat::Raw) {
                // Raw mode: convert directly to RESP (Redis Serialization Protocol) bytes.
                // This bypasses the JSON conversion and returns the exact wire protocol representation
                // expected by Redis clients.
                let bytes = resp::value_to_resp(&val);
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(bytes))
                    .unwrap()
            } else if matches!(parsed.output_format, OutputFormat::Text) {
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
                let etag = if parsed.etag_enabled {
                    let mut hasher = Sha1::new();
                    hasher.update(parsed.command_name.as_bytes());
                    for arg in &parsed.args {
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
                if (parsed.command_name.eq_ignore_ascii_case("INFO")
                    || (parsed.command_name.eq_ignore_ascii_case("CLUSTER")
                        && parsed
                            .args
                            .get(0)
                            .map_or(false, |a| a.eq_ignore_ascii_case("INFO"))))
                    && json_val.is_string()
                {
                    if let Some(s) = json_val.as_str() {
                        json_val = parse_info_output(s);
                    }
                }

                // Compute ETag for GET requests (body is None)
                let etag = if parsed.etag_enabled {
                    let mut hasher = Sha1::new();
                    hasher.update(parsed.command_name.as_bytes());
                    for arg in &parsed.args {
                        hasher.update(arg.as_bytes());
                    }
                    if let Some(cb) = parsed.jsonp_callback.as_deref() {
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
                let mut resp = parsed.output_format.format_response(
                    parsed.command_name.as_str(),
                    json_val,
                    parsed.jsonp_callback.as_deref(),
                );
                if let Some(tag) = etag {
                    resp.headers_mut()
                        .insert(header::ETAG, tag.parse().unwrap());
                }
                resp
            }
        }
        Err(error) => {
            error!(
                "Redis command execution failed: command={} db={} client={} error={}",
                parsed.command_name, parsed.target_database, addr, error
            );
            if matches!(parsed.output_format, OutputFormat::Raw) {
                // Raw Error: -ERR message
                let err_msg = format!("-ERR {}\r\n", error);
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(err_msg))
                    .unwrap()
            } else if matches!(parsed.output_format, OutputFormat::Text) {
                // Text errors mirror the original Webdis behavior: errors are plain text.
                let status = match error {
                    CommandExecutionError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
                    CommandExecutionError::ExecutionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                };
                Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(error.to_string()))
                    .unwrap()
            } else {
                let status = match error {
                    CommandExecutionError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
                    CommandExecutionError::ExecutionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                };
                json_value_response(
                    status,
                    json!({"error": error.to_string()}),
                    parsed.jsonp_callback.as_deref(),
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
    if let Some(ct) = parsed.content_type_override.as_deref() {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, ct.parse().unwrap());
    } else if parsed.jsonp_callback.is_none() {
        // Only apply extension-derived content types when the response didn't already
        // define its own `Content-Type`. This avoids incorrectly overwriting error
        // responses (e.g. JSON error bodies) with image/* content types.
        if response.headers().get(header::CONTENT_TYPE).is_none() {
            if let Some(ct) = parsed.extension_content_type {
                response
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, ct.parse().unwrap());
            }
        }
    }

    response
}

fn request_parse_error_message(error: &RequestParseError) -> String {
    match error {
        RequestParseError::EmptyCommand => "Empty command".to_string(),
        RequestParseError::InvalidDatabaseIndex => {
            "Invalid database index in path. Expected 0-255 for /<db>/<COMMAND>/...".to_string()
        }
        RequestParseError::MissingCommandAfterDatabasePrefix => {
            "Missing command after database prefix".to_string()
        }
        RequestParseError::InvalidCommand(message) => message.clone(),
    }
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
