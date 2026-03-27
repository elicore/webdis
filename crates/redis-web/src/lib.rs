use clap::Parser;
use redis_web_compat::{
    legacy_alias_notice, resolve_default_config, InvocationKind, LEGACY_CONFIG_NAME,
};
use redis_web_core::config::{Config, TransportMode, DEFAULT_VERBOSITY};
use redis_web_runtime::{grpc, server};
use std::fs;
use std::io;
use std::path::Path;
use std::process;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const HTTP_APP_NAME: &str = "redis-web";
const GRPC_APP_NAME: &str = "redis-web-grpc";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file (positional form)
    config: Option<String>,

    /// Path to configuration file
    #[arg(long = "config")]
    config_path: Option<String>,

    /// Write the default configuration to --config (or default file) and exit
    #[arg(long)]
    write_default_config: bool,

    /// Write the minimal starter configuration to --config (or starter file) and exit
    #[arg(long)]
    write_minimal_config: bool,
}

pub fn run(kind: InvocationKind) {
    if matches!(kind, InvocationKind::LegacyAlias) {
        eprintln!("{}", legacy_alias_notice());
    }

    let LoadedConfig {
        config,
        config_path,
    } = load_config(kind);
    init_logging(&config, &config_path, HTTP_APP_NAME);
    if config.transport_mode != TransportMode::Rest {
        eprintln!(
            "{} only serves REST/WebSocket traffic. Use `redis-web-grpc` for gRPC configs.",
            HTTP_APP_NAME
        );
        process::exit(1);
    }

    start_http_runtime(config);
}

pub fn run_grpc(kind: InvocationKind) {
    if matches!(kind, InvocationKind::LegacyAlias) {
        eprintln!("{}", legacy_alias_notice());
    }

    let LoadedConfig {
        config,
        config_path,
    } = load_config(kind);
    init_logging(&config, &config_path, GRPC_APP_NAME);
    if config.transport_mode != TransportMode::Grpc {
        eprintln!(
            "{} requires `transport_mode: \"grpc\"` in the config file.",
            GRPC_APP_NAME
        );
        process::exit(1);
    }

    start_grpc_runtime(config);
}

struct LoadedConfig {
    config: Config,
    config_path: String,
}

fn load_config(kind: InvocationKind) -> LoadedConfig {
    let args = Args::parse();
    let explicit_config = args.config_path.or(args.config);
    let has_explicit_config = explicit_config.is_some();
    if args.write_default_config && args.write_minimal_config {
        eprintln!("`--write-default-config` and `--write-minimal-config` cannot be combined.");
        process::exit(1);
    }

    let config_path = explicit_config.clone().unwrap_or_else(|| {
        if args.write_minimal_config {
            kind.default_minimal_config_name().to_string()
        } else if args.write_default_config {
            kind.default_config_name().to_string()
        } else {
            resolve_default_config(kind)
        }
    });

    if !has_explicit_config
        && matches!(kind, InvocationKind::Canonical)
        && config_path == LEGACY_CONFIG_NAME
    {
        eprintln!(
            "[compat] No `redis-web.json` or `redis-web.min.json` found. Falling back to legacy `{}`.",
            LEGACY_CONFIG_NAME
        );
    }

    if args.write_default_config {
        match write_default_config(&config_path, kind) {
            Ok(_) => {
                println!("Default configuration written to {}", config_path);
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Failed to write default configuration: {}", e);
                process::exit(1);
            }
        }
    }

    if args.write_minimal_config {
        match write_minimal_config(&config_path, kind) {
            Ok(_) => {
                println!("Minimal configuration written to {}", config_path);
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Failed to write minimal configuration: {}", e);
                process::exit(1);
            }
        }
    }

    let config = match Config::new(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    LoadedConfig {
        config,
        config_path,
    }
}

fn init_logging(config: &Config, config_path: &str, app_name: &str) {
    let log_level = match config.verbosity.unwrap_or(DEFAULT_VERBOSITY) {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::WARN,
        2 => tracing::Level::INFO,
        3 => tracing::Level::INFO,
        4 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::from_level(
            log_level,
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting {}", app_name);
    info!("Using configuration file: {}", config_path);
    info!("Configuration loaded successfully: {:?}", config);
    info!(
        "Logging initialized at level {:?}, destination: stderr",
        log_level
    );
}

fn start_http_runtime(config: Config) {
    info!("Building Tokio runtime");
    let mut runtime = tokio::runtime::Builder::new_multi_thread();
    runtime.enable_all();
    if let Some(worker_threads) = config.runtime_worker_threads {
        runtime.worker_threads(worker_threads);
    }
    runtime.build().unwrap().block_on(async_main_http(config));
}

fn start_grpc_runtime(config: Config) {
    info!("Building Tokio runtime");
    let mut runtime = tokio::runtime::Builder::new_multi_thread();
    runtime.enable_all();
    if let Some(worker_threads) = config.runtime_worker_threads {
        runtime.worker_threads(worker_threads);
    }
    runtime.build().unwrap().block_on(async_main_grpc(config));
}

async fn async_main_http(config: Config) {
    let components = match server::build_runtime(&config) {
        Ok(components) => components,
        Err(error) => {
            error!("Server startup failed during runtime build: {error}");
            process::exit(1);
        }
    };

    let app = server::build_router_from_components(&config, components);

    info!(
        "Starting HTTP server on {}:{}",
        config.http_host, config.http_port
    );
    if let Err(error) = server::serve(&config, app).await {
        error!("Failed to serve HTTP traffic: {}", error);
        process::exit(1);
    }
}

async fn async_main_grpc(config: Config) {
    let components = match server::build_runtime(&config) {
        Ok(components) => components,
        Err(error) => {
            error!("Server startup failed during runtime build: {error}");
            process::exit(1);
        }
    };

    log_ignored_rest_settings(&config);
    info!(
        "Starting gRPC server on {}:{}",
        config.grpc.host, config.grpc.port
    );
    if let Err(error) = grpc::serve(&config, components.app_state).await {
        error!("Failed to serve gRPC traffic: {}", error);
        process::exit(1);
    }
}

fn log_ignored_rest_settings(config: &Config) {
    let mut ignored = Vec::new();
    if config.websockets {
        ignored.push("websockets");
    }
    if config.default_root.is_some() {
        ignored.push("default_root");
    }
    if config
        .compat_hiredis
        .as_ref()
        .is_some_and(|cfg| cfg.enabled)
    {
        ignored.push("compat_hiredis");
    }
    if config.http_host != "0.0.0.0" || config.http_port != 7379 {
        ignored.push("http_host/http_port");
    }

    if !ignored.is_empty() {
        info!(
            "Ignoring REST-only settings in gRPC mode: {}",
            ignored.join(", ")
        );
    }
}

fn write_default_config(
    path: &str,
    kind: InvocationKind,
) -> Result<(), Box<dyn std::error::Error>> {
    let path_ref = Path::new(path);
    if path_ref.exists() {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path),
        )));
    }

    let value = Config::default_document(kind.default_schema_path());
    let json = serde_json::to_string_pretty(&value)?;
    fs::write(path_ref, format!("{json}\n"))?;
    Ok(())
}

fn write_minimal_config(
    path: &str,
    kind: InvocationKind,
) -> Result<(), Box<dyn std::error::Error>> {
    let path_ref = Path::new(path);
    if path_ref.exists() {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path),
        )));
    }

    let value = Config::starter_document(kind.default_schema_path());
    let json = serde_json::to_string_pretty(&value)?;
    fs::write(path_ref, format!("{json}\n"))?;
    Ok(())
}
