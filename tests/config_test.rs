use std::io::Write;

use webdis::config::Config;

#[test]
fn test_config_loading() {
    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "0.0.0.0",
        "http_port": 7379,
        "database": 0,
        "daemonize": true,
        "websockets": true,
        "http_max_request_size": 1024,
        "user": "nobody",
        "group": "nogroup",
        "verbosity": 5,
        "logfile": "test.log",
        "log_fsync": "auto"
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    let config = Config::new(path).unwrap();

    assert_eq!(config.redis_host, "127.0.0.1");
    assert_eq!(config.daemonize, true);
    assert_eq!(config.websockets, true);
    assert_eq!(config.http_max_request_size, Some(1024));
    assert_eq!(config.user, Some("nobody".to_string()));
    assert_eq!(config.verbosity, Some(5));
}

#[test]
fn test_default_values() {
    let config_json = r#"{
        "redis_host": "127.0.0.1",
        "redis_port": 6379,
        "http_host": "0.0.0.0",
        "http_port": 7379,
        "database": 0
    }"#;

    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();
    let path = file.path().to_str().unwrap();

    let config = Config::new(path).unwrap();

    assert_eq!(config.daemonize, false);
    assert_eq!(config.websockets, false);
    assert_eq!(config.http_max_request_size, None);
    assert_eq!(config.user, None);
}
