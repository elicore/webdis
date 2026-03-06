mod support;

use redis_web_runtime::grpc::proto::{self, redis_gateway_client::RedisGatewayClient};
use support::process_harness::{redis_publish, GrpcTestServer};
use tempfile::Builder;
use tokio_stream::StreamExt;
use tonic::transport::Endpoint;
use tonic::Request;
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use tonic_reflection::pb::v1::{
    server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
    ServerReflectionRequest,
};

#[tokio::test]
async fn test_grpc_execute_and_health_service() {
    let server = GrpcTestServer::new().await;
    let endpoint = format!("http://127.0.0.1:{}", server.port);

    let mut client = RedisGatewayClient::connect(endpoint.clone()).await.unwrap();
    client
        .execute(proto::CommandRequest {
            command: "SET".to_string(),
            database: None,
            args: vec![b"grpc:key".to_vec(), b"grpc:value".to_vec()],
        })
        .await
        .unwrap();

    let reply = client
        .execute(proto::CommandRequest {
            command: "GET".to_string(),
            database: None,
            args: vec![b"grpc:key".to_vec()],
        })
        .await
        .unwrap()
        .into_inner();
    match reply.value.unwrap().kind.unwrap() {
        proto::redis_value::Kind::BulkBytes(bytes) => assert_eq!(bytes, b"grpc:value".to_vec()),
        other => panic!("expected bulk bytes reply, got {:?}", other),
    }

    let channel = Endpoint::new(endpoint).unwrap().connect().await.unwrap();
    let mut health = HealthClient::new(channel);
    let response = health
        .check(HealthCheckRequest {
            service: "redis_web.v1.RedisGateway".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.status, tonic_health::ServingStatus::Serving as i32);
}

#[tokio::test]
async fn test_grpc_subscribe_stream_receives_pubsub_messages() {
    let server = GrpcTestServer::new().await;
    let endpoint = format!("http://127.0.0.1:{}", server.port);
    let mut client = RedisGatewayClient::connect(endpoint).await.unwrap();

    let mut stream = client
        .subscribe(Request::new(proto::SubscribeRequest {
            channel: "grpc-subscribe".to_string(),
        }))
        .await
        .unwrap()
        .into_inner();

    tokio::spawn(async {
        for _ in 0..10 {
            redis_publish("grpc-subscribe", "hello-stream").await;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
        .await
        .expect("timed out waiting for subscribe event")
        .expect("stream ended unexpectedly")
        .expect("subscribe event should succeed");

    assert_eq!(event.channel, b"grpc-subscribe".to_vec());
    assert_eq!(event.payload, b"hello-stream".to_vec());
}

#[tokio::test]
async fn test_grpc_reflection_is_disabled_by_default() {
    let server = GrpcTestServer::new().await;
    let endpoint = format!("http://127.0.0.1:{}", server.port);
    let channel = Endpoint::new(endpoint).unwrap().connect().await.unwrap();
    let mut client = ServerReflectionClient::new(channel);
    let request = Request::new(tokio_stream::once(ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    }));

    let error = client
        .server_reflection_info(request)
        .await
        .expect_err("reflection should be disabled by default");
    assert_eq!(error.code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn test_grpc_reflection_can_be_enabled() {
    let config_file = Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("temp config should exist");
    let config_content = serde_json::json!({
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "transport_mode": "grpc",
        "grpc": {
            "host": "127.0.0.1",
            "port": 0,
            "enable_health_service": true,
            "enable_reflection": true
        },
        "database": 0,
        "daemonize": false,
        "verbosity": 5
    });

    let server = GrpcTestServer::spawn_with_config_and_env(config_file, config_content, &[]).await;
    let endpoint = format!("http://127.0.0.1:{}", server.port);
    let channel = Endpoint::new(endpoint).unwrap().connect().await.unwrap();
    let mut client = ServerReflectionClient::new(channel);
    let request = Request::new(tokio_stream::once(ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    }));

    let mut stream = client
        .server_reflection_info(request)
        .await
        .unwrap()
        .into_inner();
    let response = stream
        .message()
        .await
        .unwrap()
        .expect("reflection response should exist");
    let services = match response.message_response.expect("message response should exist") {
        tonic_reflection::pb::v1::server_reflection_response::MessageResponse::ListServicesResponse(
            services,
        ) => services.service,
        other => panic!("expected list services response, got {:?}", other),
    };
    assert!(services
        .iter()
        .any(|service| service.name == "redis_web.v1.RedisGateway"));
}
