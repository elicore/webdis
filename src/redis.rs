use crate::config::{Config as AppConfig, DEFAULT_HTTP_THREADS, DEFAULT_POOL_SIZE_PER_THREAD};
use deadpool_redis::{Config, Pool, Runtime};

pub type RedisPool = Pool;

pub fn create_pool(config: &AppConfig) -> Result<RedisPool, deadpool_redis::CreatePoolError> {
    let mut cfg = Config::from_url(config.get_redis_url());

    // Configure pool size
    let pool_size = config
        .pool_size_per_thread
        .unwrap_or(DEFAULT_POOL_SIZE_PER_THREAD)
        * config.http_threads.unwrap_or(DEFAULT_HTTP_THREADS);
    cfg.pool = Some(deadpool_redis::PoolConfig::new(pool_size));

    cfg.create_pool(Some(Runtime::Tokio1))
}
