mod support;

use base64::{engine::general_purpose, Engine as _};
use redis_web_core::config::{AclConfig, TransportMode};
use redis_web_runtime::grpc::proto::{
    self, redis_gateway_client::RedisGatewayClient, stream_command_reply,
};
use std::sync::Arc;
use support::router_harness::{functional_config, GrpcFunctionalServer};
use support::stub_executor::ScriptedStubExecutor;
use tokio_stream::iter;
use tonic::metadata::MetadataValue;
use tonic::{Code, Request};

#[tokio::test]
async fn test_grpc_execute_and_binary_args() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let mut cfg = functional_config();
    cfg.transport_mode = TransportMode::Grpc;

    let server = GrpcFunctionalServer::spawn(cfg, executor).await;
    let mut client = RedisGatewayClient::connect(format!("http://{}", server.addr))
        .await
        .unwrap();

    client
        .execute(proto::CommandRequest {
            command: "SET".to_string(),
            database: None,
            args: vec![b"hello".to_vec(), b"world".to_vec()],
        })
        .await
        .unwrap();

    let response = client
        .execute(proto::CommandRequest {
            command: "GET".to_string(),
            database: None,
            args: vec![b"hello".to_vec()],
        })
        .await
        .unwrap()
        .into_inner();

    let value = response.value.unwrap().kind.unwrap();
    match value {
        proto::redis_value::Kind::BulkBytes(bytes) => assert_eq!(bytes, b"world".to_vec()),
        other => panic!("expected bulk bytes reply, got {:?}", other),
    }
}

#[tokio::test]
async fn test_grpc_execute_stream_keeps_per_message_errors_in_payload() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let mut cfg = functional_config();
    cfg.transport_mode = TransportMode::Grpc;

    let server = GrpcFunctionalServer::spawn(cfg, executor).await;
    let mut client = RedisGatewayClient::connect(format!("http://{}", server.addr))
        .await
        .unwrap();

    let stream = iter(vec![
        proto::StreamCommandRequest {
            correlation_id: "ok".to_string(),
            command: Some(proto::CommandRequest {
                command: "SET".to_string(),
                database: None,
                args: vec![b"stream-key".to_vec(), b"stream-value".to_vec()],
            }),
        },
        proto::StreamCommandRequest {
            correlation_id: "err".to_string(),
            command: Some(proto::CommandRequest {
                command: "FAIL".to_string(),
                database: None,
                args: Vec::new(),
            }),
        },
    ]);

    let mut replies = client
        .execute_stream(Request::new(stream))
        .await
        .unwrap()
        .into_inner();

    let first = replies.message().await.unwrap().unwrap();
    assert_eq!(first.correlation_id, "ok");
    assert!(matches!(
        first.result,
        Some(stream_command_reply::Result::Value(_))
    ));

    let second = replies.message().await.unwrap().unwrap();
    assert_eq!(second.correlation_id, "err");
    match second.result {
        Some(stream_command_reply::Result::Error(error)) => {
            assert_eq!(error.kind, proto::ErrorKind::ExecutionFailed as i32);
            assert!(error.message.contains("stub execution failure"));
        }
        other => panic!("expected stream error payload, got {:?}", other),
    }
}

#[tokio::test]
async fn test_grpc_acl_uses_authorization_metadata() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let mut cfg = functional_config();
    cfg.transport_mode = TransportMode::Grpc;
    cfg.acl = Some(vec![
        AclConfig {
            http_basic_auth: None,
            ip: None,
            enabled: None,
            disabled: Some(vec!["DEBUG".to_string()]),
        },
        AclConfig {
            http_basic_auth: Some("user:password".to_string()),
            ip: None,
            enabled: Some(vec!["DEBUG".to_string()]),
            disabled: None,
        },
    ]);

    let server = GrpcFunctionalServer::spawn(cfg, executor).await;
    let mut client = RedisGatewayClient::connect(format!("http://{}", server.addr))
        .await
        .unwrap();

    let denied = client
        .execute(proto::CommandRequest {
            command: "DEBUG".to_string(),
            database: None,
            args: vec![b"OBJECT".to_vec(), b"key".to_vec()],
        })
        .await
        .expect_err("missing auth should be denied");
    assert_eq!(denied.code(), Code::PermissionDenied);

    let mut request = Request::new(proto::CommandRequest {
        command: "DEBUG".to_string(),
        database: None,
        args: vec![b"OBJECT".to_vec(), b"key".to_vec()],
    });
    let header = format!(
        "Basic {}",
        general_purpose::STANDARD.encode("user:password")
    );
    request.metadata_mut().insert(
        "authorization",
        MetadataValue::try_from(header.as_str()).unwrap(),
    );

    client.execute(request).await.unwrap();
}
