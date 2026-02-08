use axum::{
    body::Body,
    http::header,
    response::{IntoResponse, Json, Response},
};
use rmp_serde;
use serde_json::{json, Value};

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

    pub fn format_response(&self, command: &str, value: Value, callback: Option<String>) -> Response {
        match self {
            OutputFormat::Json => {
                let response = json!({
                    command: value
                });
                if let Some(cb) = callback {
                    let body = format!("{}({})", cb, response.to_string());
                    Response::builder()
                        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
                        .body(Body::from(body))
                        .unwrap()
                } else {
                    Json(response).into_response()
                }
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