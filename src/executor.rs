use crate::interfaces::{CommandExecutionError, CommandExecutor, ExecutionFuture};
use crate::redis::DatabasePoolRegistry;
use crate::request::ParsedRequest;
use redis::cmd;
use std::sync::Arc;

/// Default Redis-backed executor for parsed Webdis requests.
pub struct RedisCommandExecutor {
    redis_pools: Arc<DatabasePoolRegistry>,
}

impl RedisCommandExecutor {
    pub fn new(redis_pools: Arc<DatabasePoolRegistry>) -> Self {
        Self { redis_pools }
    }
}

impl CommandExecutor for RedisCommandExecutor {
    fn execute<'a>(&'a self, request: &'a ParsedRequest) -> ExecutionFuture<'a> {
        Box::pin(async move {
            let pool = self
                .redis_pools
                .pool_for_database(request.target_database)
                .await
                .map_err(|error| CommandExecutionError::ServiceUnavailable(error.to_string()))?;

            let mut connection = pool
                .get()
                .await
                .map_err(|error| CommandExecutionError::ServiceUnavailable(error.to_string()))?;

            let mut redis_command = cmd(request.command_name.as_str());
            for arg in &request.args {
                redis_command.arg(arg);
            }
            if let Some(body_arg) = request.body_arg.as_deref() {
                redis_command.arg(body_arg);
            }

            redis_command
                .query_async(&mut connection)
                .await
                .map_err(|error| CommandExecutionError::ExecutionFailed(error.to_string()))
        })
    }
}
