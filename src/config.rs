use config::{Config as ConfigLoader, ConfigError, File};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const DEFAULT_HTTP_THREADS: usize = 4;
pub const DEFAULT_POOL_SIZE_PER_THREAD: usize = 10;
pub const DEFAULT_HTTP_MAX_REQUEST_SIZE: usize = 128 * 1024 * 1024;
pub const DEFAULT_VERBOSITY: usize = 4;
pub const DEFAULT_DATABASE: u8 = 0;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default = "default_redis_host")]
    pub redis_host: String,
    #[serde(default = "default_redis_port")]
    pub redis_port: u16,
    #[serde(default = "default_http_host")]
    pub http_host: String,
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    pub http_threads: Option<usize>,
    #[serde(default, rename = "threads", skip_serializing, alias = "threads")]
    legacy_http_threads: Option<usize>,
    #[serde(default)]
    pub database: u8,
    pub pool_size_per_thread: Option<usize>,
    #[serde(default, rename = "pool_size", skip_serializing, alias = "pool_size")]
    legacy_pool_size_per_thread: Option<usize>,
    #[serde(default)]
    pub daemonize: bool,
    pub pidfile: Option<String>,
    #[serde(default)]
    pub websockets: bool,
    pub ssl: Option<SslConfig>,
    pub acl: Option<Vec<AclConfig>>,
    pub redis_auth: Option<RedisAuthConfig>,
    pub http_max_request_size: Option<usize>,
    pub user: Option<String>,
    pub group: Option<String>,
    pub default_root: Option<String>,
    pub verbosity: Option<usize>,
    pub logfile: Option<String>,
    pub log_fsync: Option<LogFsync>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum LogFsync {
    Mode(LogFsyncMode),
    Millis(u64),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum LogFsyncMode {
    Auto,
    All,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SslConfig {
    pub enabled: bool,
    pub ca_cert_bundle: String,
    pub path_to_certs: Option<String>,
    pub client_cert: String,
    pub client_key: String,
    pub redis_sni: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AclConfig {
    pub disabled: Option<Vec<String>>,
    pub enabled: Option<Vec<String>>,
    pub http_basic_auth: Option<String>,
    pub ip: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum RedisAuthConfig {
    Legacy(String),
    ACL(Vec<String>),
}

impl Config {
    pub fn new(config_path: &str) -> Result<Self, ConfigError> {
        let s = ConfigLoader::builder()
            .add_source(File::with_name(config_path))
            .build()?;

        let mut config: Self = s.try_deserialize()?;
        config.apply_legacy_aliases();
        Ok(config)
    }

    pub fn get_redis_url(&self) -> String {
        let scheme = if self.ssl.as_ref().map(|s| s.enabled).unwrap_or(false) {
            "rediss"
        } else {
            "redis"
        };

        let mut auth_str = String::new();
        if let Some(auth) = &self.redis_auth {
            match auth {
                RedisAuthConfig::Legacy(password) => {
                    auth_str = format!(":{}@", password);
                }
                RedisAuthConfig::ACL(creds) => {
                    if creds.len() == 2 {
                        auth_str = format!("{}:{}@", creds[0], creds[1]);
                    }
                }
            }
        }

        format!(
            "{}://{}{}:{}/{}",
            scheme, auth_str, self.redis_host, self.redis_port, self.database
        )
    }

    pub fn default_document(schema_ref: &str) -> Value {
        let value = serde_json::to_value(Self::default()).expect("default config is serializable");
        match value {
            Value::Object(map) => Value::Object(decorate_default_map(map, schema_ref)),
            other => other,
        }
    }
    fn apply_legacy_aliases(&mut self) {
        if self.http_threads.is_none() && self.legacy_http_threads.is_some() {
            self.http_threads = self.legacy_http_threads.take();
        } else {
            self.legacy_http_threads = None;
        }

        if self.pool_size_per_thread.is_none() && self.legacy_pool_size_per_thread.is_some() {
            self.pool_size_per_thread = self.legacy_pool_size_per_thread.take();
        } else {
            self.legacy_pool_size_per_thread = None;
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            redis_host: default_redis_host(),
            redis_port: default_redis_port(),
            http_host: default_http_host(),
            http_port: default_http_port(),
            http_threads: Some(DEFAULT_HTTP_THREADS),
            legacy_http_threads: None,
            database: DEFAULT_DATABASE,
            pool_size_per_thread: Some(DEFAULT_POOL_SIZE_PER_THREAD),
            legacy_pool_size_per_thread: None,
            daemonize: false,
            pidfile: None,
            websockets: false,
            ssl: None,
            acl: None,
            redis_auth: None,
            http_max_request_size: Some(DEFAULT_HTTP_MAX_REQUEST_SIZE),
            user: None,
            group: None,
            default_root: None,
            verbosity: Some(DEFAULT_VERBOSITY),
            logfile: None,
            log_fsync: None,
        }
    }
}

fn default_redis_host() -> String {
    "127.0.0.1".to_string()
}

fn default_redis_port() -> u16 {
    6379
}

fn default_http_host() -> String {
    "0.0.0.0".to_string()
}

fn default_http_port() -> u16 {
    7379
}

const DEFAULT_CONFIG_KEY_ORDER: &[&str] = &[
    "$schema",
    "redis_host",
    "redis_port",
    "redis_auth",
    "http_host",
    "http_port",
    "http_threads",
    "pool_size_per_thread",
    "database",
    "daemonize",
    "pidfile",
    "user",
    "group",
    "websockets",
    "default_root",
    "http_max_request_size",
    "verbosity",
    "logfile",
    "log_fsync",
    "ssl",
    "acl",
];

fn decorate_default_map(mut map: Map<String, Value>, schema_ref: &str) -> Map<String, Value> {
    map.insert("$schema".to_string(), Value::String(schema_ref.to_string()));
    map.retain(|_, v| !v.is_null());

    let mut ordered = Map::new();
    let mut remaining: BTreeMap<String, Value> = map.into_iter().collect();

    for &key in DEFAULT_CONFIG_KEY_ORDER {
        if let Some(value) = remaining.remove(key) {
            ordered.insert(key.to_string(), value);
        }
    }

    for (key, value) in remaining {
        ordered.insert(key, value);
    }

    ordered
}
