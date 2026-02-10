//! Redis connection and pooling helpers.
//!
//! Webdis supports connecting to Redis via:
//! - **TCP/TLS** using `redis_host`, `redis_port`, and `ssl`.
//! - **UNIX-domain sockets** using `redis_socket` (preferred when set).
//!
//! The key design constraint is that UNIX socket connections must *not* be
//! expressed as a URL string. We validate the socket path on startup and then
//! build a typed `redis::ConnectionInfo` so we can use
//! [`redis::ConnectionAddr::Unix`] directly.
//!
//! ## TCP keep-alive parity (`hiredis.keep_alive_sec`)
//!
//! When `hiredis.keep_alive_sec` is set, Webdis configures TCP keep-alive on
//! Redis **TCP/TLS** connections created by the pool:
//!
//! - The keep-alive idle time is set to `keep_alive_sec`.
//! - The probe interval is derived as `max(1, keep_alive_sec / 3)` to match the
//!   legacy Hiredis behavior (`TCP_KEEPINTVL ≈ interval/3`) as closely as the
//!   platform allows.
//!
//! UNIX-domain socket connections (`redis_socket`) are unaffected.

use crate::config::{
    Config as AppConfig, RedisAuthConfig, DEFAULT_HTTP_THREADS, DEFAULT_POOL_SIZE_PER_THREAD,
};
use deadpool::managed::{CreatePoolError, Object, Pool, PoolConfig, RecycleError, RecycleResult};
use deadpool::Runtime;
use redis::aio::ConnectionLike;
use redis::aio::MultiplexedConnection;
use redis::io::tcp::{socket2, TcpSettings};
use redis::{
    ConnectionAddr, ConnectionInfo, ErrorKind, IntoConnectionInfo, Pipeline, ProtocolVersion,
    RedisConnectionInfo, RedisFuture, Value,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;

#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct WebdisRedisManager {
    client: redis::Client,
}

impl WebdisRedisManager {
    fn new(info: ConnectionInfo) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: redis::Client::open(info)?,
        })
    }
}

impl deadpool::managed::Manager for WebdisRedisManager {
    type Type = MultiplexedConnection;
    type Error = redis::RedisError;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        self.client.get_multiplexed_async_connection().await
    }

    async fn recycle(
        &self,
        conn: &mut Self::Type,
        _: &deadpool::managed::Metrics,
    ) -> RecycleResult<Self::Error> {
        // A lightweight health-check to avoid handing out stale connections.
        let pong: String = redis::cmd("PING")
            .query_async(conn)
            .await
            .map_err(RecycleError::Backend)?;
        if pong == "PONG" {
            Ok(())
        } else {
            Err(RecycleError::message(format!(
                "unexpected PING response from Redis: {pong}"
            )))
        }
    }
}

/// A checked-out Redis connection from the pool.
///
/// Deadpool returns a generic `Object<M>` wrapper which does not implement
/// `redis::aio::ConnectionLike`. We wrap it so it can be used directly with
/// `redis::Cmd::query_async`, mirroring the ergonomics of a Redis-specific pool wrapper.
pub struct PooledConnection(Object<WebdisRedisManager>);

impl From<Object<WebdisRedisManager>> for PooledConnection {
    fn from(obj: Object<WebdisRedisManager>) -> Self {
        Self(obj)
    }
}

impl std::ops::Deref for PooledConnection {
    type Target = MultiplexedConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ConnectionLike for PooledConnection {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a redis::Cmd) -> RedisFuture<'a, Value> {
        <MultiplexedConnection as ConnectionLike>::req_packed_command(&mut *self.0, cmd)
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &'a Pipeline,
        offset: usize,
        count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        <MultiplexedConnection as ConnectionLike>::req_packed_commands(
            &mut *self.0,
            cmd,
            offset,
            count,
        )
    }

    fn get_db(&self) -> i64 {
        <MultiplexedConnection as ConnectionLike>::get_db(&*self.0)
    }
}

pub type RedisPool = Pool<WebdisRedisManager, PooledConnection>;
pub type RedisCreatePoolError = CreatePoolError<redis::RedisError>;

/// Creates a Redis connection pool configured for the current Webdis config.
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
pub fn create_pool(config: &AppConfig) -> Result<RedisPool, RedisCreatePoolError> {
    let info = pool_connection_info(config).map_err(CreatePoolError::Config)?;
    let manager = WebdisRedisManager::new(info).map_err(CreatePoolError::Config)?;

    let pool_size = config
        .pool_size_per_thread
        .unwrap_or(DEFAULT_POOL_SIZE_PER_THREAD)
        * config.http_threads.unwrap_or(DEFAULT_HTTP_THREADS);

    let pool = Pool::builder(manager)
        .config(PoolConfig::new(pool_size))
        .runtime(Runtime::Tokio1)
        .build()
        .map_err(CreatePoolError::Build)?;

    Ok(pool)
}

/// Creates a dedicated Redis client for Pub/Sub subscriptions.
///
/// This is separate from the pool because Pub/Sub uses long-lived connections.
pub fn create_pubsub_client(config: &AppConfig) -> Result<redis::Client, redis::RedisError> {
    if let Some(socket) = config.redis_socket.as_deref() {
        let info = connection_info_for_unix_socket_redis(config, socket)?;
        redis::Client::open(info)
    } else {
        redis::Client::open(config.get_redis_url())
    }
}

fn ssl_enabled(config: &AppConfig) -> bool {
    config.ssl.as_ref().map(|ssl| ssl.enabled).unwrap_or(false)
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

fn pool_connection_info(config: &AppConfig) -> Result<ConnectionInfo, redis::RedisError> {
    if let Some(socket) = config.redis_socket.as_deref() {
        if ssl_enabled(config) {
            return Err(redis::RedisError::from((
                ErrorKind::InvalidClientConfig,
                "ssl is not supported with redis_socket",
                "Disable ssl.enabled or remove redis_socket. TLS does not apply to UNIX sockets."
                    .to_string(),
            )));
        }

        connection_info_for_unix_socket_redis(config, socket)
    } else {
        let mut info = config.get_redis_url().into_connection_info()?;

        // Webdis assumes RESP2 everywhere else; keep the default explicit.
        let redis_settings = info
            .redis_settings()
            .clone()
            .set_protocol(ProtocolVersion::RESP2);
        info = info.set_redis_settings(redis_settings);

        maybe_apply_tcp_keepalive(config, info)
    }
}

fn connection_info_for_unix_socket_redis(
    config: &AppConfig,
    socket_path: &str,
) -> Result<ConnectionInfo, redis::RedisError> {
    let socket_path = PathBuf::from(socket_path);
    validate_unix_socket_path(&socket_path)?;

    let (username, password) = redis_username_password(config);
    let mut redis_settings = RedisConnectionInfo::default()
        .set_db(i64::from(config.database))
        .set_protocol(ProtocolVersion::RESP2);
    if let Some(u) = username {
        redis_settings = redis_settings.set_username(u);
    }
    if let Some(p) = password {
        redis_settings = redis_settings.set_password(p);
    }

    ConnectionAddr::Unix(socket_path)
        .into_connection_info()
        .map(|info| info.set_redis_settings(redis_settings))
}

fn validate_unix_socket_path(path: &Path) -> Result<(), redis::RedisError> {
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(redis::RedisError::from((
            ErrorKind::InvalidClientConfig,
            "redis_socket is not supported on this platform",
            "UNIX sockets require a Unix-like OS.".to_string(),
        )))
    }

    #[cfg(unix)]
    {
        let meta = std::fs::metadata(path).map_err(|e| {
            redis::RedisError::from((
                ErrorKind::Io,
                "redis_socket path is not accessible",
                format!("{}: {}", path.display(), e),
            ))
        })?;

        if !meta.file_type().is_socket() {
            return Err(redis::RedisError::from((
                ErrorKind::InvalidClientConfig,
                "redis_socket is not a unix socket",
                format!("{} is not a socket file", path.display()),
            )));
        }

        Ok(())
    }
}

fn maybe_apply_tcp_keepalive(
    config: &AppConfig,
    mut info: ConnectionInfo,
) -> Result<ConnectionInfo, redis::RedisError> {
    let keep_alive_sec = config
        .hiredis
        .as_ref()
        .and_then(|h| h.keep_alive_sec)
        .filter(|&secs| secs > 0);

    let Some(keep_alive_sec) = keep_alive_sec else {
        return Ok(info);
    };

    match info.addr() {
        ConnectionAddr::Tcp(_, _) | ConnectionAddr::TcpTls { .. } => {
            let (time, interval) = keepalive_time_and_interval(keep_alive_sec);
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(time)
                .with_interval(interval);

            let tcp_settings: TcpSettings = info.tcp_settings().clone().set_keepalive(keepalive);
            info = info.set_tcp_settings(tcp_settings);
            Ok(info)
        }
        ConnectionAddr::Unix(_) => Ok(info),
        // Non-exhaustive enum: preserve defaults for future variants.
        _ => Ok(info),
    }
}

/// Computes the keep-alive idle time and probe interval used for Redis TCP/TLS connections.
///
/// This mirrors the legacy Hiredis behavior (interval divided by ~3) while preserving
/// a minimum interval of 1 second.
fn keepalive_time_and_interval(keep_alive_sec: u64) -> (Duration, Duration) {
    let time = Duration::from_secs(keep_alive_sec);
    let interval_sec = std::cmp::max(1, keep_alive_sec / 3);
    let interval = Duration::from_secs(interval_sec);
    (time, interval)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HiredisConfig;

    #[test]
    fn test_keepalive_time_and_interval_derivation() {
        assert_eq!(
            keepalive_time_and_interval(15),
            (Duration::from_secs(15), Duration::from_secs(5))
        );
        assert_eq!(
            keepalive_time_and_interval(1),
            (Duration::from_secs(1), Duration::from_secs(1))
        );
        assert_eq!(
            keepalive_time_and_interval(2),
            (Duration::from_secs(2), Duration::from_secs(1))
        );
        assert_eq!(
            keepalive_time_and_interval(3),
            (Duration::from_secs(3), Duration::from_secs(1))
        );
        assert_eq!(
            keepalive_time_and_interval(4),
            (Duration::from_secs(4), Duration::from_secs(1))
        );
        assert_eq!(
            keepalive_time_and_interval(6),
            (Duration::from_secs(6), Duration::from_secs(2))
        );
    }

    #[test]
    fn test_maybe_apply_tcp_keepalive_only_for_tcp_addrs() {
        let mut config = AppConfig::default();
        config.hiredis = Some(HiredisConfig {
            keep_alive_sec: Some(15),
        });

        // TCP address gets keepalive settings attached.
        let tcp_info = "redis://127.0.0.1:6379/0".into_connection_info().unwrap();
        let tcp_info = maybe_apply_tcp_keepalive(&config, tcp_info).unwrap();
        assert!(
            tcp_info.tcp_settings().keepalive().is_some(),
            "expected TCP keepalive settings to be present for TCP/TLS connections"
        );

        // UNIX sockets should bypass keepalive tuning.
        let unix_info = "redis://127.0.0.1:6379/0".into_connection_info().unwrap();
        let unix_info = unix_info.set_addr(ConnectionAddr::Unix(PathBuf::from("/tmp/redis.sock")));
        let unix_info = maybe_apply_tcp_keepalive(&config, unix_info).unwrap();
        assert!(
            unix_info.tcp_settings().keepalive().is_none(),
            "expected keepalive tuning to be skipped for UNIX sockets"
        );
    }
}
