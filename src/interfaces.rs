use crate::request::{ParsedRequest, RequestParseError};
use redis::Value as RedisValue;
use std::future::Future;
use std::pin::Pin;

/// Input passed from an HTTP adapter to a request parser.
///
/// This keeps transport-specific extraction outside the parser while preserving
/// all request components needed to build a Redis command request.
pub struct ParseRequestInput<'a> {
    pub command_path: &'a str,
    pub params: &'a std::collections::HashMap<String, String>,
    pub default_database: u8,
    pub body: Option<&'a [u8]>,
    pub etag_enabled: bool,
}

pub type ExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<RedisValue, CommandExecutionError>> + Send + 'a>>;

/// Parser interface that turns transport-level input into a normalized request.
pub trait RequestParser: Send + Sync {
    fn parse(&self, input: ParseRequestInput<'_>) -> Result<ParsedRequest, RequestParseError>;
}

/// Executor interface that runs a normalized request against a backend.
pub trait CommandExecutor: Send + Sync {
    fn execute<'a>(&'a self, request: &'a ParsedRequest) -> ExecutionFuture<'a>;
}

#[derive(Debug)]
pub enum CommandExecutionError {
    ServiceUnavailable(String),
    ExecutionFailed(String),
}

impl std::fmt::Display for CommandExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandExecutionError::ServiceUnavailable(msg) => write!(f, "{msg}"),
            CommandExecutionError::ExecutionFailed(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CommandExecutionError {}
