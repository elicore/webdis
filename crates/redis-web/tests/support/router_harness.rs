#![allow(dead_code)]

use crate::support::stub_executor::ScriptedStubExecutor;
use redis_web_core::config::Config;
use redis_web_core::request::WebdisRequestParser;
use redis_web_runtime::grpc;
use redis_web_runtime::server::{self, ServerDependencies};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub struct FunctionalServer {
    pub addr: SocketAddr,
    _task: JoinHandle<()>,
}

pub struct GrpcFunctionalServer {
    pub addr: SocketAddr,
    _task: JoinHandle<()>,
}

impl FunctionalServer {
    pub async fn spawn(config: Config, executor: Arc<ScriptedStubExecutor>) -> Self {
        let pool =
            redis_web_runtime::redis::create_pool(&config).expect("pool config should be valid");
        let pools = Arc::new(redis_web_runtime::redis::DatabasePoolRegistry::new(
            config.clone(),
            pool,
        ));
        let pubsub_client = redis_web_runtime::redis::create_pubsub_client(&config)
            .expect("pubsub client config should be valid");
        let pubsub = redis_web_runtime::pubsub::PubSubManager::new(pubsub_client);

        let app = server::build_router_with_dependencies(
            &config,
            ServerDependencies {
                request_parser: Arc::new(WebdisRequestParser),
                command_executor: executor,
            },
            pools,
            pubsub,
            None,
        );

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind failed");
        let addr = listener.local_addr().expect("addr missing");

        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .expect("functional server crashed");
        });

        Self { addr, _task: task }
    }
}

impl GrpcFunctionalServer {
    pub async fn spawn(config: Config, executor: Arc<ScriptedStubExecutor>) -> Self {
        let pool =
            redis_web_runtime::redis::create_pool(&config).expect("pool config should be valid");
        let pools = Arc::new(redis_web_runtime::redis::DatabasePoolRegistry::new(
            config.clone(),
            pool,
        ));
        let pubsub_client = redis_web_runtime::redis::create_pubsub_client(&config)
            .expect("pubsub client config should be valid");
        let pubsub = redis_web_runtime::pubsub::PubSubManager::new(pubsub_client);

        let components = server::build_runtime_with_dependencies(
            &config,
            ServerDependencies {
                request_parser: Arc::new(WebdisRequestParser),
                command_executor: executor,
            },
            pools,
            pubsub,
            None,
        );

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind failed");
        let addr = listener.local_addr().expect("addr missing");

        let task = tokio::spawn(async move {
            grpc::serve_with_listener(&config, components.app_state, listener)
                .await
                .expect("functional gRPC server crashed");
        });

        Self { addr, _task: task }
    }
}

pub fn functional_config() -> Config {
    let mut cfg = Config::default();
    cfg.redis_host = "127.0.0.1".to_string();
    cfg.redis_port = 6379;
    cfg.http_host = "127.0.0.1".to_string();
    cfg.http_port = 0;
    cfg.http_threads = Some(1);
    cfg.pool_size_per_thread = Some(1);
    cfg.websockets = true;
    cfg.database = 0;
    cfg.acl = None;
    cfg.verbosity = Some(0);
    cfg.http_max_request_size = None;
    cfg
}
