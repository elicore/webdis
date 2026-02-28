use crate::acl;
use crate::config::{Config, DEFAULT_HTTP_MAX_REQUEST_SIZE};
use crate::executor::RedisCommandExecutor;
use crate::handler::{self, AppState};
use crate::interfaces::{CommandExecutor, RequestParser};
use crate::pubsub::{self, PubSubManager};
use crate::redis::{self, DatabasePoolRegistry};
use crate::request::WebdisRequestParser;
use crate::websocket;
use axum::extract::DefaultBodyLimit;
use axum::{
    routing::{get, options},
    Router,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

/// Injectable dependencies for embedding Webdis with custom parser/executor implementations.
pub struct ServerDependencies {
    pub request_parser: Arc<dyn RequestParser>,
    pub command_executor: Arc<dyn CommandExecutor>,
}

/// Builds a server state and router from config and injected dependencies.
///
/// This function is suitable for embedding in another process that already has
/// its own Tokio runtime and needs to host Webdis beside other services.
pub fn build_router_with_dependencies(
    config: &Config,
    dependencies: ServerDependencies,
    redis_pools: Arc<DatabasePoolRegistry>,
    pubsub_manager: PubSubManager,
) -> Router {
    let app_state = Arc::new(AppState {
        redis_pools,
        default_database: config.database,
        request_parser: dependencies.request_parser,
        command_executor: dependencies.command_executor,
        acl: acl::Acl::new(config.acl.clone()),
        pubsub: pubsub_manager,
    });

    let mut app = Router::new()
        .route(
            "/{*command}",
            get(handler::handle_get)
                .post(handler::handle_post)
                .put(handler::handle_put)
                .options(handler::handle_options),
        )
        .route("/SUBSCRIBE/{*channel}", get(pubsub::handle_subscribe));

    if let Some(default_root) = config.default_root.clone() {
        app = app.route(
            "/",
            get(move |state, addr, headers, query| {
                handler::handle_default_root(state, addr, headers, query, default_root)
            })
            .options(handler::handle_options),
        );
    } else {
        app = app.route("/", options(handler::handle_options));
    }

    if config.websockets {
        app = app.route("/.json", get(websocket::ws_handler));
        app = app.route("/.raw", get(websocket::ws_handler_raw));
    }

    app.layer(DefaultBodyLimit::max(
        config
            .http_max_request_size
            .unwrap_or(DEFAULT_HTTP_MAX_REQUEST_SIZE),
    ))
    .with_state(app_state)
}

/// Builds the default Webdis router using the built-in parser and Redis executor.
pub fn build_router(config: &Config) -> Result<Router, ServerBuildError> {
    let redis_pool = redis::create_pool(config).map_err(ServerBuildError::RedisPool)?;
    let redis_pools = DatabasePoolRegistry::new(config.clone(), redis_pool);
    let redis_pools_shared = Arc::new(redis_pools);

    let pubsub_client = redis::create_pubsub_client(config).map_err(ServerBuildError::PubSub)?;
    let pubsub_manager = pubsub::PubSubManager::new(pubsub_client);

    let dependencies = ServerDependencies {
        request_parser: Arc::new(WebdisRequestParser),
        command_executor: Arc::new(RedisCommandExecutor::new(redis_pools_shared.clone())),
    };

    Ok(build_router_with_dependencies(
        config,
        dependencies,
        redis_pools_shared,
        pubsub_manager,
    ))
}

/// Serves a pre-built Axum router on the configured host/port.
pub async fn serve(config: &Config, app: Router) -> Result<(), std::io::Error> {
    let ip: IpAddr = config.http_host.parse().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid HTTP host {}", config.http_host),
        )
    })?;
    let addr = SocketAddr::from((ip, config.http_port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

#[derive(Debug)]
pub enum ServerBuildError {
    RedisPool(redis::RedisCreatePoolError),
    PubSub(::redis::RedisError),
}

impl std::fmt::Display for ServerBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerBuildError::RedisPool(error) => write!(f, "failed to create Redis pool: {error}"),
            ServerBuildError::PubSub(error) => {
                write!(f, "failed to create Redis pub/sub client: {error}")
            }
        }
    }
}

impl std::error::Error for ServerBuildError {}
