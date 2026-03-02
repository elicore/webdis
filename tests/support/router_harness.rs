#![allow(dead_code)]

use crate::support::stub_executor::ScriptedStubExecutor;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use webdis::config::Config;
use webdis::request::WebdisRequestParser;
use webdis::server::{self, ServerDependencies};

pub struct FunctionalServer {
    pub addr: SocketAddr,
    _task: JoinHandle<()>,
}

impl FunctionalServer {
    pub async fn spawn(config: Config, executor: Arc<ScriptedStubExecutor>) -> Self {
        let pool = webdis::redis::create_pool(&config).expect("pool config should be valid");
        let pools = Arc::new(webdis::redis::DatabasePoolRegistry::new(config.clone(), pool));
        let pubsub_client =
            webdis::redis::create_pubsub_client(&config).expect("pubsub client config should be valid");
        let pubsub = webdis::pubsub::PubSubManager::new(pubsub_client);

        let app = server::build_router_with_dependencies(
            &config,
            ServerDependencies {
                request_parser: Arc::new(WebdisRequestParser),
                command_executor: executor,
            },
            pools,
            pubsub,
        );

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("bind failed");
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

pub fn functional_config() -> Config {
    let mut cfg = Config::default();
    cfg.redis_host = "127.0.0.1".to_string();
    cfg.redis_port = 6379;
    cfg.http_host = "127.0.0.1".to_string();
    cfg.http_port = 0;
    cfg.http_threads = Some(1);
    cfg.pool_size_per_thread = Some(1);
    cfg.daemonize = false;
    cfg.websockets = true;
    cfg.database = 0;
    cfg.acl = None;
    cfg.verbosity = Some(0);
    cfg.http_max_request_size = None;
    cfg
}
