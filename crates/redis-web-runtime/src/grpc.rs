use crate::handler::AppState;
use futures::Stream;
use redis::Value as RedisValue;
use redis_web_core::config::{Config, DEFAULT_HTTP_MAX_REQUEST_SIZE};
use redis_web_core::interfaces::{AuthContext, CommandExecutionError, ExecutableCommand};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_stream::StreamExt;
use tonic::metadata::MetadataMap;
use tonic::{Request, Response, Status};
use tracing::{error, info};

pub mod proto {
    tonic::include_proto!("redis_web.v1");
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("redis_web_descriptor");
}

type ResponseStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

pub struct RedisGatewayService {
    state: Arc<AppState>,
}

impl RedisGatewayService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::redis_gateway_server::RedisGateway for RedisGatewayService {
    async fn execute(
        &self,
        request: Request<proto::CommandRequest>,
    ) -> Result<Response<proto::CommandReply>, Status> {
        let auth = auth_context(&request);
        let command = command_from_proto(self.state.default_database, request.into_inner())?;

        authorize(&self.state, &auth, command.command_name.as_str())?;
        let value = self
            .state
            .command_executor
            .execute(&command)
            .await
            .map_err(command_error_to_status)?;

        Ok(Response::new(proto::CommandReply {
            value: Some(redis_value_to_proto(value)?),
        }))
    }

    type ExecuteStreamStream = ResponseStream<proto::StreamCommandReply>;

    async fn execute_stream(
        &self,
        request: Request<tonic::Streaming<proto::StreamCommandRequest>>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let auth = auth_context(&request);
        let default_database = self.state.default_database;
        let state = self.state.clone();
        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                let item = match item {
                    Ok(item) => item,
                    Err(status) => {
                        let _ = tx.send(Err(status)).await;
                        break;
                    }
                };

                let correlation_id = item.correlation_id.clone();
                let reply = match item.command {
                    Some(command_request) => {
                        match command_from_proto(default_database, command_request) {
                            Ok(command) => {
                                if let Err(status) =
                                    authorize(&state, &auth, command.command_name.as_str())
                                {
                                    proto::StreamCommandReply {
                                        correlation_id,
                                        result: Some(proto::stream_command_reply::Result::Error(
                                            status_to_stream_error(&status),
                                        )),
                                    }
                                } else {
                                    match state.command_executor.execute(&command).await {
                                        Ok(value) => match redis_value_to_proto(value) {
                                            Ok(value) => proto::StreamCommandReply {
                                                correlation_id,
                                                result: Some(
                                                    proto::stream_command_reply::Result::Value(
                                                        value,
                                                    ),
                                                ),
                                            },
                                            Err(status) => proto::StreamCommandReply {
                                                correlation_id,
                                                result: Some(
                                                    proto::stream_command_reply::Result::Error(
                                                        status_to_stream_error(&status),
                                                    ),
                                                ),
                                            },
                                        },
                                        Err(error) => proto::StreamCommandReply {
                                            correlation_id,
                                            result: Some(
                                                proto::stream_command_reply::Result::Error(
                                                    command_error_to_stream_error(error),
                                                ),
                                            ),
                                        },
                                    }
                                }
                            }
                            Err(status) => proto::StreamCommandReply {
                                correlation_id,
                                result: Some(proto::stream_command_reply::Result::Error(
                                    status_to_stream_error(&status),
                                )),
                            },
                        }
                    }
                    None => proto::StreamCommandReply {
                        correlation_id,
                        result: Some(proto::stream_command_reply::Result::Error(
                            status_to_stream_error(&Status::invalid_argument(
                                "stream request is missing command",
                            )),
                        )),
                    },
                };

                if tx.send(Ok(reply)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::ExecuteStreamStream
        ))
    }

    type SubscribeStream = ResponseStream<proto::SubscribeEvent>;

    async fn subscribe(
        &self,
        request: Request<proto::SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let auth = auth_context(&request);
        let inner = request.into_inner();
        if inner.channel.is_empty() {
            return Err(Status::invalid_argument("channel must not be empty"));
        }

        authorize(&self.state, &auth, "SUBSCRIBE")?;

        let channel = inner.channel;
        let mut rx = self.state.pubsub.subscribe(channel.clone()).await;

        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(payload) => yield Ok(proto::SubscribeEvent {
                        channel: channel.as_bytes().to_vec(),
                        payload: payload.into_bytes(),
                    }),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(stream) as Self::SubscribeStream))
    }
}

pub async fn serve(config: &Config, state: Arc<AppState>) -> Result<(), std::io::Error> {
    let ip: IpAddr = config.grpc.host.parse().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid gRPC host {}", config.grpc.host),
        )
    })?;
    let addr = SocketAddr::from((ip, config.grpc.port));
    let listener = TcpListener::bind(addr).await?;
    info!("gRPC listener bound to {}", listener.local_addr()?);
    serve_with_listener(config, state, listener).await
}

pub async fn serve_with_listener(
    config: &Config,
    state: Arc<AppState>,
    listener: TcpListener,
) -> Result<(), std::io::Error> {
    let mut gateway =
        proto::redis_gateway_server::RedisGatewayServer::new(RedisGatewayService::new(state));
    gateway = gateway.max_decoding_message_size(
        config
            .grpc
            .max_decoding_message_size
            .or(config.http_max_request_size)
            .unwrap_or(DEFAULT_HTTP_MAX_REQUEST_SIZE),
    );
    if let Some(limit) = config.grpc.max_encoding_message_size {
        gateway = gateway.max_encoding_message_size(limit);
    }

    let mut builder = tonic::transport::Server::builder();

    let local_addr = listener.local_addr()?;
    info!("Binding gRPC listener to {}", local_addr);
    let incoming = TcpListenerStream::new(listener);

    if config.grpc.enable_health_service {
        let (mut reporter, health_service) = tonic_health::server::health_reporter();
        reporter
            .set_serving::<proto::redis_gateway_server::RedisGatewayServer<RedisGatewayService>>()
            .await;

        if config.grpc.enable_reflection {
            let reflection = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
                .build_v1()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            builder
                .add_service(gateway)
                .add_service(health_service)
                .add_service(reflection)
                .serve_with_incoming(incoming)
                .await
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        } else {
            builder
                .add_service(gateway)
                .add_service(health_service)
                .serve_with_incoming(incoming)
                .await
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }
    } else if config.grpc.enable_reflection {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
            .build_v1()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        builder
            .add_service(gateway)
            .add_service(reflection)
            .serve_with_incoming(incoming)
            .await
            .map_err(|error| std::io::Error::other(error.to_string()))?;
    } else {
        builder
            .add_service(gateway)
            .serve_with_incoming(incoming)
            .await
            .map_err(|error| std::io::Error::other(error.to_string()))?;
    }

    Ok(())
}

fn authorize(state: &AppState, auth: &AuthContext, command: &str) -> Result<(), Status> {
    if state.acl.check_auth(auth, command) {
        Ok(())
    } else {
        Err(Status::permission_denied("Forbidden"))
    }
}

fn auth_context<T>(request: &Request<T>) -> AuthContext {
    AuthContext {
        client_ip: request
            .remote_addr()
            .map(|addr| addr.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        authorization: authorization_header(request.metadata()),
    }
}

fn authorization_header(metadata: &MetadataMap) -> Option<String> {
    metadata
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

fn command_from_proto(
    default_database: u8,
    request: proto::CommandRequest,
) -> Result<ExecutableCommand, Status> {
    if request.command.is_empty() {
        return Err(Status::invalid_argument("command must not be empty"));
    }
    if request.database.unwrap_or(default_database as u32) > u8::MAX as u32 {
        return Err(Status::invalid_argument(
            "database must be between 0 and 255",
        ));
    }

    Ok(ExecutableCommand {
        target_database: request.database.unwrap_or(default_database as u32) as u8,
        command_name: request.command,
        args: request.args,
    })
}

pub fn redis_value_to_proto(value: RedisValue) -> Result<proto::RedisValue, Status> {
    use proto::redis_value::Kind;

    let kind = match value {
        RedisValue::Nil => Kind::Nil(proto::Nil {}),
        RedisValue::Int(integer) => Kind::Integer(integer),
        RedisValue::BulkString(bytes) => Kind::BulkBytes(bytes),
        RedisValue::Array(values) => Kind::Array(proto::RedisArray {
            values: values
                .into_iter()
                .map(redis_value_to_proto)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        RedisValue::SimpleString(value) => Kind::SimpleString(value),
        RedisValue::Okay => Kind::SimpleString("OK".to_string()),
        unsupported => {
            error!(
                "unsupported Redis value for gRPC mapping: {:?}",
                unsupported
            );
            return Err(Status::internal(
                "unsupported Redis reply type for gRPC response",
            ));
        }
    };

    Ok(proto::RedisValue { kind: Some(kind) })
}

fn command_error_to_status(error: CommandExecutionError) -> Status {
    match error {
        CommandExecutionError::ServiceUnavailable(message) => Status::unavailable(message),
        CommandExecutionError::ExecutionFailed(message) => Status::internal(message),
    }
}

fn command_error_to_stream_error(error: CommandExecutionError) -> proto::CommandError {
    match error {
        CommandExecutionError::ServiceUnavailable(message) => proto::CommandError {
            kind: proto::ErrorKind::ServiceUnavailable as i32,
            message,
        },
        CommandExecutionError::ExecutionFailed(message) => proto::CommandError {
            kind: proto::ErrorKind::ExecutionFailed as i32,
            message,
        },
    }
}

fn status_to_stream_error(status: &Status) -> proto::CommandError {
    proto::CommandError {
        kind: 0,
        message: status.message().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_value_mapping_supports_core_types() {
        let mapped = redis_value_to_proto(RedisValue::Array(vec![
            RedisValue::Nil,
            RedisValue::Int(42),
            RedisValue::BulkString(b"hello".to_vec()),
            RedisValue::Okay,
        ]))
        .expect("mapping should succeed");

        let proto::redis_value::Kind::Array(array) = mapped.kind.expect("array kind expected")
        else {
            panic!("expected array kind");
        };
        assert_eq!(array.values.len(), 4);
    }

    #[test]
    fn redis_value_mapping_rejects_unsupported_variants() {
        let error = redis_value_to_proto(RedisValue::Boolean(true))
            .expect_err("RESP3 boolean should be unsupported in v1");
        assert_eq!(error.code(), tonic::Code::Internal);
    }
}
