use crate::config::Config as AppConfig;
use deadpool_redis::{Config, Pool, Runtime};

pub type RedisPool = Pool;

pub fn create_pool(config: &AppConfig) -> Result<RedisPool, deadpool_redis::CreatePoolError> {
    let scheme = if config.ssl.as_ref().map(|s| s.enabled).unwrap_or(false) {
        "rediss"
    } else {
        "redis"
    };

    let mut cfg = Config::from_url(format!(
        "{}://{}:{}/{}",
        scheme, config.redis_host, config.redis_port, config.database
    ));

    // Configure pool size
    let pool_size = config.pool_size_per_thread.unwrap_or(10) * config.http_threads.unwrap_or(4);
    cfg.pool = Some(deadpool_redis::PoolConfig::new(pool_size));

    cfg.create_pool(Some(Runtime::Tokio1))
}
