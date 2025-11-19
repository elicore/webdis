use config::{Config as ConfigLoader, ConfigError, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub redis_host: String,
    pub redis_port: u16,
    pub http_host: String,
    pub http_port: u16,
    pub http_threads: Option<usize>,
    pub database: u8,
    pub pool_size_per_thread: Option<usize>,
    pub daemonize: bool,
    pub pidfile: Option<String>,
    pub websockets: bool,
    pub ssl: Option<SslConfig>,
    pub acl: Option<Vec<AclConfig>>,
    pub redis_auth: Option<RedisAuthConfig>,
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
}
