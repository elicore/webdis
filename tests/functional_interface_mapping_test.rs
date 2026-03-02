mod support;

use reqwest::Client;
use std::sync::Arc;
use support::router_harness::{functional_config, FunctionalServer};
use support::stub_executor::ScriptedStubExecutor;
use webdis::config::AclConfig;

#[tokio::test]
async fn test_interface_mapping_captures_db_command_and_args() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let mut cfg = functional_config();
    cfg.acl = None;

    let server = FunctionalServer::spawn(cfg, executor.clone()).await;
    let client = Client::new();

    let resp = client
        .get(format!("http://{}/7/SET/mapped/key", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let seen = executor.seen_requests().await;
    assert!(!seen.is_empty());
    let last = seen.last().unwrap();
    assert_eq!(last.target_database, 7);
    assert_eq!(last.command_name, "SET");
    assert_eq!(last.args, vec!["mapped", "key"]);
}

#[tokio::test]
async fn test_error_mapping_for_parser_and_executor() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let server = FunctionalServer::spawn(functional_config(), executor).await;
    let client = Client::new();

    let resp = client
        .get(format!("http://{}/9999/GET/key", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let resp = client
        .get(format!("http://{}/UNAVAILABLE", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);

    let resp = client
        .get(format!("http://{}/FAIL", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_acl_denied_before_executor() {
    let executor = Arc::new(ScriptedStubExecutor::new());
    let mut cfg = functional_config();
    cfg.acl = Some(vec![AclConfig {
        http_basic_auth: None,
        ip: None,
        enabled: None,
        disabled: Some(vec!["PING".to_string()]),
    }]);

    let server = FunctionalServer::spawn(cfg, executor).await;
    let client = Client::new();

    let resp = client
        .get(format!("http://{}/PING", server.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);
}
