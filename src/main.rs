mod acl;
mod config;
mod format;
mod handler;
mod pubsub;
mod redis;
mod websocket;

use axum::{
    routing::{get, options},
    Router,
};
use clap::Parser;
use config::Config;
use handler::AppState;
use std::net::SocketAddr;
use std::process;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(default_value = "webdis.json")]
    config: String,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("Starting Webdis...");

    let config = match Config::new(&args.config) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    info!("Configuration loaded successfully: {:?}", config);

    let pool = match redis::create_pool(&config) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to create Redis pool: {}", e);
            process::exit(1);
        }
    };

    info!("Redis pool initialized");

    let acl = acl::Acl::new(config.acl.clone());
    let app_state = Arc::new(AppState { pool, acl });

    let app = Router::new()
        .route(
            "/*command",
            get(handler::handle_get)
                .post(handler::handle_post)
                .put(handler::handle_put)
                .options(handler::handle_options),
        )
        .route("/", options(handler::handle_options))
        .route("/.json", get(websocket::ws_handler))
        .route("/SUBSCRIBE/*channel", get(pubsub::handle_subscribe))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.http_port));
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
