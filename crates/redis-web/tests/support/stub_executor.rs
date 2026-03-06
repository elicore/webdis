#![allow(dead_code)]

use redis_web_core::interfaces::{
    CommandExecutionError, CommandExecutor, ExecutableCommand, ExecutionFuture,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct ScriptedStubExecutor {
    values: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    requests: Arc<RwLock<Vec<ExecutableCommand>>>,
}

impl ScriptedStubExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn seen_requests(&self) -> Vec<ExecutableCommand> {
        self.requests.read().await.clone()
    }
}

impl CommandExecutor for ScriptedStubExecutor {
    fn execute<'a>(&'a self, request: &'a ExecutableCommand) -> ExecutionFuture<'a> {
        Box::pin(async move {
            self.requests.write().await.push(request.clone());

            let cmd = request.command_name.to_ascii_uppercase();
            match cmd.as_str() {
                "UNAVAILABLE" => Err(CommandExecutionError::ServiceUnavailable(
                    "stub unavailable".to_string(),
                )),
                "FAIL" => Err(CommandExecutionError::ExecutionFailed(
                    "stub execution failure".to_string(),
                )),
                "SET" => {
                    let key = request
                        .args
                        .first()
                        .map(|value| String::from_utf8_lossy(value).into_owned())
                        .unwrap_or_default();
                    let value = request.args.get(1).cloned().unwrap_or_default();
                    self.values.write().await.insert(key, value);
                    Ok(redis::Value::SimpleString("OK".to_string()))
                }
                "GET" => {
                    let key = request
                        .args
                        .first()
                        .map(|value| String::from_utf8_lossy(value).into_owned())
                        .unwrap_or_default();
                    let value = self
                        .values
                        .read()
                        .await
                        .get(&key)
                        .cloned()
                        .unwrap_or_default();
                    Ok(redis::Value::BulkString(value))
                }
                "STRLEN" => {
                    let key = request
                        .args
                        .first()
                        .map(|value| String::from_utf8_lossy(value).into_owned())
                        .unwrap_or_default();
                    let value = self
                        .values
                        .read()
                        .await
                        .get(&key)
                        .cloned()
                        .unwrap_or_default();
                    Ok(redis::Value::Int(value.len() as i64))
                }
                _ => Ok(redis::Value::SimpleString("OK".to_string())),
            }
        })
    }
}
