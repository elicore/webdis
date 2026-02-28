use webdis::logging::FsyncWriter;
use webdis::{config, server};

use clap::Parser;
use config::{Config, DEFAULT_VERBOSITY};
use daemonize::Daemonize;
use nix::unistd::{Group, User};
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
    let app = match server::build_router(&config) {
        Ok(app) => app,
        Err(error) => {
            error!("{error}");
            process::exit(1);
        }
    };

    if let Err(error) = server::serve(&config, app).await {
        error!("Failed to serve HTTP traffic: {}", error);
        process::exit(1);
    }
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
