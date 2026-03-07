use clap::Parser;
use daemonize::Daemonize;
use nix::unistd::{Group, User};
use redis_web_compat::{
    legacy_alias_notice, resolve_default_config, InvocationKind, LEGACY_CONFIG_NAME,
};
use redis_web_core::config::{Config, TransportMode, DEFAULT_VERBOSITY};
use redis_web_core::logging::FsyncWriter;
use redis_web_runtime::{grpc, server};
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::process;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
}

pub fn run(kind: InvocationKind) {
    if matches!(kind, InvocationKind::LegacyAlias) {
        eprintln!("{}", legacy_alias_notice());
    }

    let args = Args::parse();
    let explicit_config = args.config_path.or(args.config);
    let config_path = explicit_config
        .clone()
        .unwrap_or_else(|| resolve_default_config(kind));
    if explicit_config.is_none()
        && matches!(kind, InvocationKind::Canonical)
        && config_path == LEGACY_CONFIG_NAME
    {
        eprintln!(
            "[compat] No `redis-web.json` found. Falling back to legacy `{}`.",
            LEGACY_CONFIG_NAME
        );
    }

    if args.write_default_config {
        match write_default_config(&config_path, kind) {
            Ok(_) => {
                println!("Default configuration written to {}", config_path);
                return;
            }
            Err(e) => {
                eprintln!("Failed to write default configuration: {}", e);
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

    let log_level = match config.verbosity.unwrap_or(DEFAULT_VERBOSITY) {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::WARN,
        2 => tracing::Level::INFO,
        3 => tracing::Level::INFO,
        4 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    let file_writer: Option<Box<dyn std::io::Write + Send + 'static>> =
        if let Some(logfile) = &config.logfile {
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

    info!("Starting redis-web");
    info!("Using configuration file: {}", config_path);
    info!("Configuration loaded successfully: {:?}", config);
    info!(
        "Logging initialized at level {:?}, destination: {}",
        log_level,
        config.logfile.as_deref().unwrap_or("stderr")
    );

    if config.daemonize {
        let daemonize = Daemonize::new()
            .pid_file(config.pidfile.as_deref().unwrap_or("redis-web.pid"))
            .working_directory(".");

        match daemonize.start() {
            Ok(_) => info!("Success, daemonized"),
            Err(e) => {
                error!("Error, {}", e);
                process::exit(1);
            }
        }
    }

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

    info!("Building Tokio runtime");
    let mut runtime = tokio::runtime::Builder::new_multi_thread();
    runtime.enable_all();
    if let Some(worker_threads) = config.runtime_worker_threads {
        runtime.worker_threads(worker_threads);
    }
    runtime.build().unwrap().block_on(async_main(config));
}

async fn async_main(config: Config) {
    let components = match server::build_runtime(&config) {
        Ok(components) => components,
        Err(error) => {
            error!("Server startup failed during runtime build: {error}");
            process::exit(1);
        }
    };

    match config.transport_mode {
        TransportMode::Rest => {
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
        TransportMode::Grpc => {
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
