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

pub enum OutputFormat {
    Json,
    Raw,
    MessagePack,
}

impl OutputFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "raw" => OutputFormat::Raw,
            "msg" | "msgpack" => OutputFormat::MessagePack,
            _ => OutputFormat::Json,
        }
    }

    /// Formats a Redis response value using the selected output format.
    ///
    /// `jsonp_callback` is only applied to JSON output; callers should pass `None`
    /// for non-JSON formats to preserve parity with the original Webdis behavior.
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
        }
    }
}
