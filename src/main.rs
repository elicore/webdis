use webdis::{acl, config, handler, pubsub, redis, websocket};
use webdis::logging::FsyncWriter;

use axum::{
    routing::{get, options},
    Router,
};
use clap::Parser;
use config::{Config, DEFAULT_HTTP_MAX_REQUEST_SIZE, DEFAULT_VERBOSITY};
use handler::AppState;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::process;
use std::sync::Arc;
use tracing::{error, info};

use axum::extract::DefaultBodyLimit;
use daemonize::Daemonize;
use nix::unistd::{Group, User};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(default_value = "webdis.json")]
    config: String,

    /// Write the default configuration to --config (or webdis.json) and exit
    #[arg(long)]
    write_default_config: bool,
}

fn main() {
    let args = Args::parse();

    if args.write_default_config {
        match write_default_config(&args.config) {
            Ok(_) => {
                println!("Default configuration written to {}", args.config);
                return;
            }
            Err(e) => {
                eprintln!("Failed to write default configuration: {}", e);
                process::exit(1);
            }
        }
    }

    let config = match Config::new(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    // Configure logging
    let log_level = match config.verbosity.unwrap_or(DEFAULT_VERBOSITY) {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::WARN,
        2 => tracing::Level::INFO,
        3 => tracing::Level::INFO,
        4 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    let file_writer: Option<Box<dyn std::io::Write + Send + 'static>> = if let Some(logfile) = &config.logfile {
        let path = std::path::Path::new(logfile);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!("Failed to create log directory {:?}: {}", parent, e);
                    process::exit(1);
                }
            }
        }

        let file = match OpenOptions::new().create(true).append(true).open(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open logfile {}: {}", logfile, e);
                process::exit(1);
            }
        };

        let writer: Box<dyn std::io::Write + Send + 'static> = if config.log_fsync.is_some() {
            Box::new(FsyncWriter::new(file, config.log_fsync.as_ref()))
        } else {
            Box::new(file)
        };

        Some(writer)
    } else {
        None
    };

    let (non_blocking, _guard) = if let Some(writer) = file_writer {
        let (non_blocking, guard) = tracing_appender::non_blocking(writer);
        (Some(non_blocking), Some(guard))
    } else {
        (None, None)
    };

    let registry = tracing_subscriber::registry().with(
        tracing_subscriber::filter::LevelFilter::from_level(log_level),
    );

    if let Some(writer) = non_blocking {
        registry
            .with(tracing_subscriber::fmt::layer().with_writer(writer))
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }

    info!("Starting Webdis...");
    info!("Configuration loaded successfully: {:?}", config);

    // Daemonize
    if config.daemonize {
        let daemonize = Daemonize::new()
            .pid_file(config.pidfile.as_deref().unwrap_or("webdis.pid"))
            .working_directory(".");

        match daemonize.start() {
            Ok(_) => info!("Success, daemonized"),
            Err(e) => {
                error!("Error, {}", e);
                process::exit(1);
            }
        }
    }

    // Drop privileges
    if let Some(user) = &config.user {
        if let Ok(Some(u)) = User::from_name(user) {
            if let Err(e) = nix::unistd::setuid(u.uid) {
                error!("Failed to set user to {}: {}", user, e);
                process::exit(1);
            }
            info!("Dropped privileges to user {}", user);
        } else {
            error!("User {} not found", user);
            process::exit(1);
        }
    }

    if let Some(group) = &config.group {
        if let Ok(Some(g)) = Group::from_name(group) {
            if let Err(e) = nix::unistd::setgid(g.gid) {
                error!("Failed to set group to {}: {}", group, e);
                process::exit(1);
            }
            info!("Dropped privileges to group {}", group);
        } else {
            error!("Group {} not found", group);
            process::exit(1);
        }
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main(config));
}

async fn async_main(config: Config) {
    let pool = match redis::create_pool(&config) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to create Redis pool: {}", e);
            process::exit(1);
        }
    };

    // Create a dedicated Redis client for Pub/Sub
    let pubsub_client = match redis::create_pubsub_client(&config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Redis client for Pub/Sub: {}", e);
            process::exit(1);
        }
    };
    let pubsub_manager = pubsub::PubSubManager::new(pubsub_client);

    // State management: shared pool, ACLs, and the Pub/Sub manager.
    // The Pub/Sub manager is separate from the pool as it requires dedicated long-lived connections.
    let app_state = Arc::new(AppState {
        pool,
        acl: acl::Acl::new(config.acl),
        pubsub: pubsub_manager,
    });

    let mut app = Router::new()
        // API Route handling: uses Axum 0.8 wildcard syntax {*wildcard}
        // to capture all segments after the base path for command parsing.
        .route(
            "/{*command}",
            get(handler::handle_get)
                .post(handler::handle_post)
                .put(handler::handle_put)
                .options(handler::handle_options),
        )
        // Dedicated Pub/Sub endpoint for SSE and WebSocket subscriptions.
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
        // Bi-directional JSON WebSocket endpoint
        app = app.route("/.json", get(websocket::ws_handler));
        // Bi-directional Raw RESP WebSocket endpoint
        app = app.route("/.raw", get(websocket::ws_handler_raw));
    }

    let app = app
        .layer(DefaultBodyLimit::max(
            config
                .http_max_request_size
                .unwrap_or(DEFAULT_HTTP_MAX_REQUEST_SIZE),
        ))
        .with_state(app_state);

    let ip: std::net::IpAddr = config.http_host.parse().expect("Invalid HTTP host");
    let addr = SocketAddr::from((ip, config.http_port));
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

const DEFAULT_SCHEMA_PATH: &str = "./webdis.schema.json";

fn write_default_config(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path_ref = Path::new(path);
    if path_ref.exists() {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path),
        )));
    }

    let value = Config::default_document(DEFAULT_SCHEMA_PATH);
    let json = serde_json::to_string_pretty(&value)?;
    fs::write(path_ref, format!("{json}\n"))?;
    Ok(())
}
