use config::{Config as ConfigLoader, ConfigError, File, FileFormat};
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
    /// Optional filesystem path to a Redis UNIX-domain socket.
    ///
    /// When set, Webdis will prefer connecting over the socket regardless of
    /// `redis_host` / `redis_port`. TLS (`ssl`) does not apply to UNIX sockets.
    pub redis_socket: Option<String>,
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
    /// Optional Redis TCP keep-alive tuning settings for parity with the legacy Webdis.
    ///
    /// When `hiredis.keep_alive_sec` is set, Webdis configures TCP keep-alive on Redis
    /// **TCP/TLS** connections it opens. This does not apply to UNIX-domain socket
    /// connections (`redis_socket`).
    pub hiredis: Option<HiredisConfig>,
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

/// Legacy Hiredis options kept for compatibility with the original Webdis.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HiredisConfig {
    /// When set, enable TCP keep-alive on Redis TCP/TLS connections.
    ///
    /// This value is treated as the keep-alive idle time (in seconds), and is also used
    /// to derive an approximate keep-alive probe interval (\(\mathrm{keepalive\_sec}/3\))
    /// to match Hiredis' `redisEnableKeepAliveWithInterval` behavior as closely as
    /// the platform allows.
    pub keep_alive_sec: Option<u64>,
}

impl Config {
    pub fn new(config_path: &str) -> Result<Self, ConfigError> {
        // Load the config file as untyped JSON first so we can preserve Webdis' legacy
        // behavior where some values (notably integers) can be specified indirectly via
        // environment variables (e.g. `"redis_port": "$REDIS_PORT"`).
        //
        // We expand `$VARNAME` placeholders before deserializing into the typed `Config`
        // struct so every string field can participate (paths, passwords, etc.).
        let loader = ConfigLoader::builder()
            .add_source(File::with_name(config_path))
            .build()?;

        let mut json: Value = loader.try_deserialize()?;
        expand_env_vars_in_json(&mut json, JsonPath::root())?;

        // Re-parse via the `config` crate so it can apply its value coercions
        // (for example, parsing `"6379"` into a `u16`).
        let expanded = serde_json::to_string(&json).map_err(|e| {
            ConfigError::Message(format!("failed to serialize expanded config: {e}"))
        })?;
        let loader = ConfigLoader::builder()
            .add_source(File::from_str(&expanded, FileFormat::Json))
            .build()?;

        let mut config: Self = loader.try_deserialize()?;
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
            redis_socket: None,
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
            hiredis: None,
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
    "redis_socket",
    "redis_auth",
    "hiredis",
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

/// A JSON path used for error reporting when expanding `$VARNAME` placeholders.
///
/// This is intentionally kept as a lightweight, allocation-friendly helper so the
/// config loader can provide actionable error messages (e.g. `ssl.redis_sni`).
#[derive(Clone, Debug)]
struct JsonPath {
    inner: String,
}

impl JsonPath {
    fn root() -> Self {
        Self {
            inner: String::new(),
        }
    }

    fn push_key(&self, key: &str) -> Self {
        if self.inner.is_empty() {
            Self {
                inner: key.to_string(),
            }
        } else {
            Self {
                inner: format!("{}.{}", self.inner, key),
            }
        }
    }

    fn push_index(&self, idx: usize) -> Self {
        Self {
            inner: format!("{}[{}]", self.inner, idx),
        }
    }

    fn display(&self) -> &str {
        if self.inner.is_empty() {
            "<root>"
        } else {
            &self.inner
        }
    }
}

/// Returns `true` when `name` matches the supported `$VARNAME` syntax.
///
/// Webdis' env-var expansion is intentionally conservative:
/// - The JSON value must be a string that *starts with* `$`.
/// - The remainder must be non-empty and contain only `A–Z`, `0–9`, and `_`.
///
/// This differs from many shells, which also allow lowercase names and restrict the
/// first character; we document this behavior for compatibility and predictability.
fn is_valid_env_var_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Walks a JSON value tree and expands `$VARNAME` placeholders from the process environment.
///
/// Expansion happens before typed deserialization so any string field can reference
/// environment variables. If a referenced environment variable is not set, returns a
/// `ConfigError` with an actionable message that includes both the missing name and
/// the JSON key path where it was referenced.
fn expand_env_vars_in_json(value: &mut Value, path: JsonPath) -> Result<(), ConfigError> {
    match value {
        Value::String(s) => {
            let Some(var_name) = s.strip_prefix('$') else {
                return Ok(());
            };
            if !is_valid_env_var_name(var_name) {
                return Ok(());
            }

            match std::env::var(var_name) {
                Ok(env_value) => {
                    *s = env_value;
                    Ok(())
                }
                Err(_) => Err(ConfigError::Message(format!(
                    "missing environment variable '{var_name}' referenced by config key '{}'",
                    path.display()
                ))),
            }
        }
        Value::Array(items) => {
            for (idx, item) in items.iter_mut().enumerate() {
                expand_env_vars_in_json(item, path.push_index(idx))?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                expand_env_vars_in_json(item, path.push_key(key))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
