use crate::format::{content_type_for_extension, select_jsonp_callback, OutputFormat};
use crate::interfaces::{ParseRequestInput, RequestParser};

#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub target_database: u8,
    pub command_name: String,
    pub args: Vec<String>,
    pub body_arg: Option<Vec<u8>>,
    pub output_format: OutputFormat,
    pub jsonp_callback: Option<String>,
    pub content_type_override: Option<String>,
    pub extension_content_type: Option<&'static str>,
    pub etag_enabled: bool,
}

#[derive(Debug)]
pub enum RequestParseError {
    EmptyCommand,
    InvalidDatabaseIndex,
    MissingCommandAfterDatabasePrefix,
    InvalidCommand(String),
}

/// Default parser that implements Webdis URL and output-format semantics.
#[derive(Default)]
pub struct WebdisRequestParser;

impl RequestParser for WebdisRequestParser {
    fn parse(&self, input: ParseRequestInput<'_>) -> Result<ParsedRequest, RequestParseError> {
        parse_http_request(input)
    }
}

fn parse_http_request(input: ParseRequestInput<'_>) -> Result<ParsedRequest, RequestParseError> {
    let parts: Vec<&str> = input.command_path.split('/').collect();
    if parts.is_empty() {
        return Err(RequestParseError::EmptyCommand);
    }

    fn is_decimal_segment(segment: &str) -> bool {
        !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit())
    }

    let mut target_database = input.default_database;
    let mut command_segment_index = 0usize;
    if is_decimal_segment(parts[0]) {
        target_database = parts[0]
            .parse::<u8>()
            .map_err(|_| RequestParseError::InvalidDatabaseIndex)?;

        if parts.len() < 2 || parts[1].is_empty() {
            return Err(RequestParseError::MissingCommandAfterDatabasePrefix);
        }

        command_segment_index = 1;
    }

    let mut raw_cmd_name = parts[command_segment_index].to_string();
    let mut raw_args: Vec<String> = parts[command_segment_index + 1..]
        .iter()
        .map(|segment| segment.to_string())
        .collect();

    let mut extension: Option<String> = None;
    if let Some(last_arg) = raw_args.last_mut() {
        if let Some(idx) = last_arg.rfind('.') {
            let candidate = last_arg[idx + 1..].to_ascii_lowercase();
            let known = OutputFormat::from_extension(candidate.as_str()).is_some()
                || content_type_for_extension(candidate.as_str()).is_some();
            if known {
                extension = Some(candidate);
                last_arg.truncate(idx);
            }
        }
    } else if let Some(idx) = raw_cmd_name.rfind('.') {
        let candidate = raw_cmd_name[idx + 1..].to_ascii_lowercase();
        let known = OutputFormat::from_extension(candidate.as_str()).is_some()
            || content_type_for_extension(candidate.as_str()).is_some();
        if known {
            extension = Some(candidate);
            raw_cmd_name.truncate(idx);
        }
    }

    let command_name = percent_decode_segment_lossy(&raw_cmd_name);
    let args: Vec<String> = raw_args
        .into_iter()
        .map(|segment| percent_decode_segment_lossy(&segment))
        .collect();

    let mut output_format = OutputFormat::Json;
    if let Some(ext) = extension.as_deref() {
        if let Some(format) = OutputFormat::from_extension(ext) {
            output_format = format;
        }
    }

    let jsonp_callback = if output_format == OutputFormat::Json {
        select_jsonp_callback(input.params).map(|cb| cb.to_string())
    } else {
        None
    };

    let content_type_override = input
        .params
        .get("type")
        .and_then(|value| (!value.is_empty()).then_some(value.clone()));
    let extension_content_type = extension
        .as_deref()
        .and_then(|ext| content_type_for_extension(ext));
    let body_arg = input
        .body
        .filter(|body| !body.is_empty())
        .map(|b| b.to_vec());

    Ok(ParsedRequest {
        target_database,
        command_name,
        args,
        body_arg,
        output_format,
        jsonp_callback,
        content_type_override,
        extension_content_type,
        etag_enabled: input.etag_enabled,
    })
}

/// Percent-decodes a single URL path segment while preserving slash splitting.
fn percent_decode_segment_lossy(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output: Vec<u8> = Vec::with_capacity(bytes.len());

    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = bytes[index + 1] as char;
            let lo = bytes[index + 2] as char;
            if let (Some(hi), Some(lo)) = (hi.to_digit(16), lo.to_digit(16)) {
                output.push(((hi << 4) | lo) as u8);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::ParseRequestInput;
    use std::collections::HashMap;

    #[test]
    fn parser_supports_db_prefix_and_suffix() {
        let params = HashMap::new();
        let parsed = parse_http_request(ParseRequestInput {
            command_path: "7/GET/key.raw",
            params: &params,
            default_database: 0,
            body: None,
            etag_enabled: true,
        })
        .unwrap();

        assert_eq!(parsed.target_database, 7);
        assert_eq!(parsed.command_name, "GET");
        assert_eq!(parsed.args, vec!["key"]);
        assert_eq!(parsed.output_format, OutputFormat::Raw);
        assert!(parsed.extension_content_type.is_some());
    }

    #[test]
    fn parser_decodes_percent_escapes_per_segment() {
        let params = HashMap::new();
        let parsed = parse_http_request(ParseRequestInput {
            command_path: "GET/a%2Fb%2Eraw",
            params: &params,
            default_database: 0,
            body: None,
            etag_enabled: true,
        })
        .unwrap();

        assert_eq!(parsed.command_name, "GET");
        assert_eq!(parsed.args, vec!["a/b.raw"]);
        assert_eq!(parsed.output_format, OutputFormat::Json);
    }

    #[test]
    fn parser_rejects_invalid_database_index() {
        let params = HashMap::new();
        let err = parse_http_request(ParseRequestInput {
            command_path: "9999/GET/key",
            params: &params,
            default_database: 0,
            body: None,
            etag_enabled: true,
        })
        .expect_err("invalid DB prefix should fail");

        assert!(matches!(err, RequestParseError::InvalidDatabaseIndex));
    }

    #[test]
    fn parser_rejects_db_prefix_without_command() {
        let params = HashMap::new();
        let err = parse_http_request(ParseRequestInput {
            command_path: "7",
            params: &params,
            default_database: 0,
            body: None,
            etag_enabled: true,
        })
        .expect_err("db prefix without command should fail");

        assert!(matches!(err, RequestParseError::MissingCommandAfterDatabasePrefix));
    }

    #[test]
    fn parser_disables_jsonp_for_raw_output() {
        let mut params = HashMap::new();
        params.insert("jsonp".to_string(), "cb".to_string());

        let parsed = parse_http_request(ParseRequestInput {
            command_path: "GET/key.raw",
            params: &params,
            default_database: 0,
            body: None,
            etag_enabled: true,
        })
        .expect("raw request should parse");

        assert_eq!(parsed.output_format, OutputFormat::Raw);
        assert!(parsed.jsonp_callback.is_none());
    }
}
