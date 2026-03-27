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
    #[serde(default)]
    pub transport_mode: TransportMode,
    pub http_threads: Option<usize>,
    pub runtime_worker_threads: Option<usize>,
    #[serde(default, rename = "threads", skip_serializing, alias = "threads")]
    legacy_http_threads: Option<usize>,
    #[serde(default)]
    pub database: u8,
    pub pool_size_per_thread: Option<usize>,
    #[serde(default, rename = "pool_size", skip_serializing, alias = "pool_size")]
    legacy_pool_size_per_thread: Option<usize>,
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
    pub hiredis: Option<HiRedisConfig>,
    /// Optional hiredis-compat runtime settings used by the `/__compat/*` bridge when
    /// explicitly enabled.
    #[serde(default)]
    pub compat_hiredis: Option<CompatHiRedisConfig>,
    #[serde(default = "default_grpc")]
    pub grpc: GrpcConfig,
    pub http_max_request_size: Option<usize>,
    pub default_root: Option<String>,
    pub verbosity: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransportMode {
    #[default]
    Rest,
    Grpc,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GrpcConfig {
    #[serde(default = "default_grpc_host")]
    pub host: String,
    #[serde(default = "default_grpc_port")]
    pub port: u16,
    #[serde(default = "default_grpc_health")]
    pub enable_health_service: bool,
    #[serde(default)]
    pub enable_reflection: bool,
    pub max_decoding_message_size: Option<usize>,
    pub max_encoding_message_size: Option<usize>,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            host: default_grpc_host(),
            port: default_grpc_port(),
            enable_health_service: default_grpc_health(),
            enable_reflection: false,
            max_decoding_message_size: None,
            max_encoding_message_size: None,
        }
    }
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
pub struct HiRedisConfig {
    /// When set, enable TCP keep-alive on Redis TCP/TLS connections.
    ///
    /// This value is treated as the keep-alive idle time (in seconds), and is also used
    /// to derive an approximate keep-alive probe interval (\(\mathrm{keepalive\_sec}/3\))
    /// to match Hiredis' `redisEnableKeepAliveWithInterval` behavior as closely as
    /// the platform allows.
    pub keep_alive_sec: Option<u64>,
}

/// Configuration for hiredis-compatible session endpoints.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CompatHiRedisConfig {
    /// Enable/disable the compat session layer.
    pub enabled: bool,
    /// URL prefix mounted for compat endpoints.
    pub path_prefix: String,
    /// Maximum idle time (in seconds) before a compat session expires.
    pub session_ttl_sec: u64,
    /// Maximum number of concurrent compat sessions.
    pub max_sessions: usize,
    /// Hard cap on commands accepted in a single pipelined compat request.
    pub max_pipeline_commands: usize,
}

impl Default for CompatHiRedisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path_prefix: "/__compat".to_string(),
            session_ttl_sec: 300,
            max_sessions: 1024,
            max_pipeline_commands: 256,
        }
    }
}

impl Config {
    /// Create `Config` from a JSON object (e.g. `serde_json::json!({ "redis_host": "localhost" })`).
    ///
    /// Supports `$VARNAME` env-var expansion in string values. Merges the given object with
    /// defaults; only provided keys override.
    pub fn from_value(value: Value) -> Result<Self, ConfigError> {
        let Value::Object(map) = value else {
            return Err(ConfigError::Message(
                "config must be a JSON object".to_string(),
            ));
        };
        Self::from_json_value(Value::Object(map))
    }

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

        let json: Value = loader.try_deserialize()?;
        Self::from_json_value(json)
    }

    fn from_json_value(mut json: Value) -> Result<Self, ConfigError> {
        expand_env_vars_in_json(&mut json, JsonPath::root())?;

        let expanded = serde_json::to_string(&json)
            .map_err(|e| ConfigError::Message(format!("failed to serialize config: {e}")))?;
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
            Value::Object(map) => Value::Object(decorate_config_map(
                map,
                schema_ref,
                DEFAULT_CONFIG_KEY_ORDER,
            )),
            other => other,
        }
    }

    pub fn starter_document(schema_ref: &str) -> Value {
        let mut map = Map::new();
        map.insert(
            "redis_host".to_string(),
            Value::String(default_redis_host()),
        );
        map.insert("redis_port".to_string(), Value::from(default_redis_port()));
        map.insert(
            "http_host".to_string(),
            Value::String(default_http_host_starter()),
        );
        map.insert("http_port".to_string(), Value::from(default_http_port()));
        map.insert("database".to_string(), Value::from(DEFAULT_DATABASE));

        Value::Object(decorate_config_map(
            map,
            schema_ref,
            DEFAULT_CONFIG_KEY_ORDER,
        ))
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
            transport_mode: TransportMode::default(),
            http_threads: Some(DEFAULT_HTTP_THREADS),
            runtime_worker_threads: None,
            legacy_http_threads: None,
            database: DEFAULT_DATABASE,
            pool_size_per_thread: Some(DEFAULT_POOL_SIZE_PER_THREAD),
            legacy_pool_size_per_thread: None,
            websockets: false,
            ssl: None,
            acl: None,
            redis_auth: None,
            hiredis: None,
            compat_hiredis: None,
            grpc: default_grpc(),
            http_max_request_size: Some(DEFAULT_HTTP_MAX_REQUEST_SIZE),
            default_root: None,
            verbosity: Some(DEFAULT_VERBOSITY),
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

fn default_http_host_starter() -> String {
    "127.0.0.1".to_string()
}

fn default_grpc_host() -> String {
    "0.0.0.0".to_string()
}

fn default_grpc_port() -> u16 {
    7379
}

fn default_grpc_health() -> bool {
    true
}

// Generated defaults intentionally omit legacy process-manager keys so the
// canonical config stays small and foreground-oriented.
const DEFAULT_CONFIG_KEY_ORDER: &[&str] = &[
    "$schema",
    "redis_host",
    "redis_port",
    "redis_socket",
    "redis_auth",
    "hiredis",
    "compat_hiredis",
    "transport_mode",
    "grpc",
    "http_host",
    "http_port",
    "http_threads",
    "runtime_worker_threads",
    "pool_size_per_thread",
    "database",
    "websockets",
    "default_root",
    "http_max_request_size",
    "verbosity",
    "ssl",
    "acl",
];

fn default_grpc() -> GrpcConfig {
    GrpcConfig::default()
}

fn decorate_config_map(
    mut map: Map<String, Value>,
    schema_ref: &str,
    key_order: &[&str],
) -> Map<String, Value> {
    map.insert("$schema".to_string(), Value::String(schema_ref.to_string()));
    map.retain(|_, v| !v.is_null());

    let mut ordered = Map::new();
    let mut remaining: BTreeMap<String, Value> = map.into_iter().collect();

    for &key in key_order {
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
