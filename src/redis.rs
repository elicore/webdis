//! Redis connection and pooling helpers.
//!
//! Webdis supports connecting to Redis via:
//! - **TCP/TLS** using `redis_host`, `redis_port`, and `ssl`.
//! - **UNIX-domain sockets** using `redis_socket` (preferred when set).
//!
//! The key design constraint is that UNIX socket connections must *not* be
//! expressed as a URL string. Instead we build a `deadpool_redis::ConnectionInfo`
//! so we can use `ConnectionAddr::Unix(PathBuf)` directly.

use crate::config::{
    Config as AppConfig, RedisAuthConfig, DEFAULT_HTTP_THREADS, DEFAULT_POOL_SIZE_PER_THREAD,
};
use deadpool_redis::{Config, ConnectionAddr, ConnectionInfo, Pool, ProtocolVersion, RedisConnectionInfo, Runtime};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;

pub type RedisPool = Pool;

/// Creates a [`deadpool_redis::Pool`] configured for the current Webdis config.
///
/// # Precedence
///
/// If `config.redis_socket` is set, the pool connects via the UNIX socket and
/// ignores `redis_host` / `redis_port`. Otherwise the pool uses the Redis URL
/// built from the TCP/TLS settings.
///
/// # Fail-fast behavior
///
/// For UNIX sockets, we validate that the configured path exists and is a
/// socket before returning successfully. This makes misconfiguration fail on
/// startup rather than on the first request.
pub fn create_pool(config: &AppConfig) -> Result<RedisPool, deadpool_redis::CreatePoolError> {
    let mut cfg = deadpool_config(config)?;

    // Configure pool size
    let pool_size = config
        .pool_size_per_thread
        .unwrap_or(DEFAULT_POOL_SIZE_PER_THREAD)
        * config.http_threads.unwrap_or(DEFAULT_HTTP_THREADS);
    cfg.pool = Some(deadpool_redis::PoolConfig::new(pool_size));

    cfg.create_pool(Some(Runtime::Tokio1))
}

/// Creates a dedicated Redis client for Pub/Sub subscriptions.
///
/// This is separate from the pool because Pub/Sub uses long-lived connections.
pub fn create_pubsub_client(
    config: &AppConfig,
) -> Result<deadpool_redis::redis::Client, deadpool_redis::redis::RedisError> {
    if let Some(socket) = config.redis_socket.as_deref() {
        let info = connection_info_for_unix_socket(config, socket)?;
        deadpool_redis::redis::Client::open(info)
    } else {
        deadpool_redis::redis::Client::open(config.get_redis_url())
    }
}

fn deadpool_config(config: &AppConfig) -> Result<Config, deadpool_redis::CreatePoolError> {
    if let Some(socket) = config.redis_socket.as_deref() {
        if ssl_enabled(config) {
            return Err(pool_config_error(
                deadpool_redis::redis::ErrorKind::InvalidClientConfig,
                "ssl is not supported with redis_socket",
                "Disable ssl.enabled or remove redis_socket. TLS does not apply to UNIX sockets.",
            ));
        }

        let info = connection_info_for_unix_socket(config, socket).map_err(|e| {
            deadpool_redis::CreatePoolError::Config(deadpool_redis::ConfigError::Redis(e))
        })?;
        Ok(Config::from_connection_info(info))
    } else {
        Ok(Config::from_url(config.get_redis_url()))
    }
}

fn ssl_enabled(config: &AppConfig) -> bool {
    config.ssl.as_ref().map(|ssl| ssl.enabled).unwrap_or(false)
}

fn connection_info_for_unix_socket(
    config: &AppConfig,
    socket_path: &str,
) -> Result<ConnectionInfo, deadpool_redis::redis::RedisError> {
    let socket_path = PathBuf::from(socket_path);
    validate_unix_socket_path(&socket_path)?;

    let (username, password) = redis_username_password(config);
    Ok(ConnectionInfo {
        addr: ConnectionAddr::Unix(socket_path),
        redis: RedisConnectionInfo {
            db: i64::from(config.database),
            username,
            password,
            // Webdis assumes RESP2 everywhere else; keep the default explicit.
            protocol: ProtocolVersion::RESP2,
        },
    })
}

fn validate_unix_socket_path(path: &Path) -> Result<(), deadpool_redis::redis::RedisError> {
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(deadpool_redis::redis::RedisError::from((
            deadpool_redis::redis::ErrorKind::InvalidClientConfig,
            "redis_socket is not supported on this platform",
            "UNIX sockets require a Unix-like OS.".to_string(),
        )))
    }

    #[cfg(unix)]
    {
        let meta = std::fs::metadata(path).map_err(|e| {
            deadpool_redis::redis::RedisError::from((
                deadpool_redis::redis::ErrorKind::IoError,
                "redis_socket path is not accessible",
                format!("{}: {}", path.display(), e),
            ))
        })?;

        if !meta.file_type().is_socket() {
            return Err(deadpool_redis::redis::RedisError::from((
                deadpool_redis::redis::ErrorKind::InvalidClientConfig,
                "redis_socket is not a unix socket",
                format!("{} is not a socket file", path.display()),
            )));
        }

        Ok(())
    }
}

fn redis_username_password(config: &AppConfig) -> (Option<String>, Option<String>) {
    let Some(auth) = &config.redis_auth else {
        return (None, None);
    };

    match auth {
        RedisAuthConfig::Legacy(password) => (None, Some(password.clone())),
        RedisAuthConfig::ACL(creds) => {
            if creds.len() == 2 {
                (Some(creds[0].clone()), Some(creds[1].clone()))
            } else {
                (None, None)
            }
        }
    }
}

fn pool_config_error(
    kind: deadpool_redis::redis::ErrorKind,
    desc: &'static str,
    detail: &'static str,
) -> deadpool_redis::CreatePoolError {
    let err = deadpool_redis::redis::RedisError::from((kind, desc, detail.to_string()));
    deadpool_redis::CreatePoolError::Config(deadpool_redis::ConfigError::Redis(err))
}
