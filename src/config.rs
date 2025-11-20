use config::{Config as ConfigLoader, ConfigError, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
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
    #[serde(default)]
    pub database: u8,
    pub pool_size_per_thread: Option<usize>,
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

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LogFsync {
    Mode(LogFsyncMode),
    Millis(u64),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum LogFsyncMode {
    Auto,
    All,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SslConfig {
    pub enabled: bool,
    pub ca_cert_bundle: String,
    pub path_to_certs: Option<String>,
    pub client_cert: String,
    pub client_key: String,
    pub redis_sni: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AclConfig {
    pub disabled: Option<Vec<String>>,
    pub enabled: Option<Vec<String>>,
    pub http_basic_auth: Option<String>,
    pub ip: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
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

        s.try_deserialize()
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
