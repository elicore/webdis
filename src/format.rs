use axum::{
    body::Body,
    http::header,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use rmp_serde;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Returns the JSONP callback function name for this request, if any.
///
/// Webdis supports two query parameters for JSONP:
/// - `jsonp` (preferred)
/// - `callback` (fallback)
///
/// Per the original Webdis semantics, this performs **minimal validation**:
/// any non-empty string is accepted and is passed through unchanged.
pub fn select_jsonp_callback(params: &HashMap<String, String>) -> Option<&str> {
    params
        .get("jsonp")
        .and_then(|s| (!s.is_empty()).then_some(s.as_str()))
        .or_else(|| {
            params
                .get("callback")
                .and_then(|s| (!s.is_empty()).then_some(s.as_str()))
        })
}

/// Formats a JSON value as either plain JSON or JSONP.
///
/// When `jsonp_callback` is set, the payload is wrapped as:
/// `<callback>(<json>)`, and the response `Content-Type` is set to
/// `application/javascript; charset=utf-8`.
pub fn json_value_response(
    status: StatusCode,
    payload: Value,
    jsonp_callback: Option<&str>,
) -> Response {
    if let Some(cb) = jsonp_callback {
        let body = format!("{}({})", cb, payload.to_string());
        Response::builder()
            .status(status)
            .header(
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            )
            .body(Body::from(body))
            .unwrap()
    } else {
        (status, Json(payload)).into_response()
    }
}

/// Maps a request suffix (like `.json` or `.png`) to a `Content-Type` header value.
///
/// This mirrors the original Webdis behavior where certain filename-like extensions
/// control the HTTP `Content-Type` while the underlying Redis payload is returned
/// unchanged for "string" responses.
///
/// Note that `?type=<mime>` can still override the header at runtime (handled by
/// the request handler).
pub fn content_type_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "json" => Some("application/json"),
        "txt" => Some("text/plain"),
        "html" => Some("text/html"),
        "xhtml" => Some("application/xhtml+xml"),
        "xml" => Some("text/xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        // Format extensions also imply a sensible default content type.
        "msg" | "msgpack" => Some("application/x-msgpack"),
        "raw" => Some("text/plain"),
        _ => None,
    }
}

/// Output format for HTTP responses.
///
/// This is intentionally *not* a 1:1 mapping to `Content-Type`:
/// - The output format controls how the Redis reply is serialized into the HTTP body.
/// - The `Content-Type` header can be selected by extension (e.g. `.png`) and/or
///   overridden via `?type=<mime>` without changing the body.
pub enum OutputFormat {
    Json,
    /// Raw Redis Serialization Protocol (RESP) frames.
    ///
    /// This is selected by the `.raw` suffix.
    Raw,
    MessagePack,
    /// Return only string/binary Redis replies as the HTTP body, without wrapping.
    ///
    /// This is selected by suffixes like `.txt`, `.html`, `.xml`, `.png`, `.jpg`, `.jpeg`.
    /// For non-string replies, callers decide how to handle the mismatch.
    Text,
}

impl OutputFormat {
    /// Returns the `OutputFormat` implied by a suffix, if the suffix is recognized.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "json" => Some(OutputFormat::Json),
            "raw" => Some(OutputFormat::Raw),
            "msg" | "msgpack" => Some(OutputFormat::MessagePack),
            "txt" | "html" | "xhtml" | "xml" | "png" | "jpg" | "jpeg" => Some(OutputFormat::Text),
            _ => None,
        }
    }

    /// Formats a Redis response value using the selected output format.
    ///
    /// `jsonp_callback` is only applied to JSON output; callers should pass `None` for
    /// non-JSON formats to preserve parity with the original Webdis behavior.
    pub fn format_response(
        &self,
        command: &str,
        value: Value,
        jsonp_callback: Option<&str>,
    ) -> Response {
        match self {
            OutputFormat::Json => {
                let response = json!({
                    command: value
                });
                json_value_response(StatusCode::OK, response, jsonp_callback)
            }
            OutputFormat::Raw => {
                // Legacy raw formatter (text/plain).
                //
                // Note: the HTTP handler's `.raw` path returns RESP bytes directly. This formatter
                // is kept for parity with previous code and for any future callers that want a
                // plain-text representation of the JSON value.
                let body = match value {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "".to_string(),
                    Value::Array(arr) => {
                        let strings: Vec<String> = arr
                            .iter()
                            .map(|v| match v {
                                Value::String(s) => s.clone(),
                                Value::Number(n) => n.to_string(),
                                Value::Bool(b) => b.to_string(),
                                _ => "".to_string(),
                            })
                            .collect();
                        strings.join("\n")
                    }
                    _ => value.to_string(),
                };
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(body))
                    .unwrap()
            }
            OutputFormat::MessagePack => {
                let response = json!({
                    command: value
                });
                let body = rmp_serde::to_vec(&response).unwrap();
                Response::builder()
                    .header(header::CONTENT_TYPE, "application/x-msgpack")
                    .body(Body::from(body))
                    .unwrap()
            }
            // `Text` responses are built from the raw Redis reply bytes in the handler.
            OutputFormat::Text => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "invalid output format for JSON formatter"})),
            )
                .into_response(),
        }
    }
}
