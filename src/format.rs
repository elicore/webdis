use axum::{body::Body, http::header, response::Response};
use serde_json::Value;

pub enum OutputFormat {
    Json,
    Raw,
    // Add others as needed
}

impl OutputFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "raw" => OutputFormat::Raw,
            _ => OutputFormat::Json,
        }
    }

    pub fn format_response(&self, command: &str, value: Value) -> Response {
        match self {
            OutputFormat::Json => {
                let body = serde_json::json!({ command: value }).to_string();
                Response::builder()
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
            OutputFormat::Raw => {
                // This is a simplified raw output.
                // Real raw output would need to handle types more carefully.
                let body = match value {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "".to_string(),
                    _ => value.to_string(),
                };
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from(body))
                    .unwrap()
            }
        }
    }
}
