# Webdis Test Suite

## Quick Start

```bash
# Run all tests (requires Redis on localhost:6379)
cargo test

# Run specific test
cargo test test_websocket_pubsub

# Run only integration tests
cargo test --test integration_test

# Run benchmarks (requires ab and curl)
cd tests && ./bench.sh
```

## Test Structure

### Rust Tests

#### `integration_test.rs` - End-to-end server tests

Spins up a real Webdis instance with temporary config and dynamic port allocation.

| Test                      | What It Validates                                |
| ------------------------- | ------------------------------------------------ |
| `test_basic_get_set`      | HTTP GET/SET operations, JSON responses          |
| `test_json_output`        | JSON value storage and retrieval                 |
| `test_acl_restrictions`   | ACL enforcement, HTTP Basic Auth                 |
| `test_websocket_commands` | WebSocket connection and command execution       |
| `test_websocket_pubsub`   | Pub/Sub over WebSocket, cross-protocol messaging |
| `test_huge_url`           | URI size limits (DoS protection)                 |
| `test_huge_upload`        | Request body limits, `Expect: 100-continue`      |

#### `config_test.rs` - Configuration parsing

- `test_config_loading` - All fields parsed correctly
- `test_default_values` - Missing fields use defaults

### Shell Scripts

#### `bench.sh` - Performance benchmarks

Uses ApacheBench to test PING, SET, GET, INCR, LPUSH, LRANGE throughput.

- Requires: `ab`, `curl`
- Config: 100 clients, 100K requests per test

#### `curl-tests.sh` - Edge case validation

- Large PUT uploads with `Connection: close` (issue #194)
- OPTIONS request header validation (issue #217)

## Test Infrastructure

### `TestServer` Helper

Manages Webdis lifecycle for integration tests:

```rust
// Default config
let server = TestServer::new().await;

// With custom request size limit
let server = TestServer::new_with_limit(Some(1024 * 1024)).await;
```

**Features:**

- Auto-builds binary before each test
- Generates temporary config files
- Allocates free ports dynamically (no conflicts)
- Auto-cleanup on drop (even if test panics)

## Adding Tests

### New Integration Test

```rust
#[tokio::test]
async fn test_my_feature() {
    let server = TestServer::new().await;
    let client = Client::new();

    let resp = client
        .get(&format!("http://127.0.0.1:{}/COMMAND/args", server.port))
        .send()
        .await
        .expect("Request failed");

    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["COMMAND"], expected_value);
}
```

### New Config Test

```rust
#[test]
fn test_my_config_field() {
    let config_json = r#"{"redis_host": "127.0.0.1", ...}"#;
    let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
    write!(file, "{}", config_json).unwrap();

    let config = Config::new(file.path().to_str().unwrap()).unwrap();
    assert_eq!(config.my_field, expected);
}
```

### New Shell Test

Add to `curl-tests.sh`:

```bash
function test_my_feature() {
    echo -n 'Test: My feature... '
    # Test logic here
    echo 'OK'
}

# Call at end of script
test_my_feature
```

## Coverage

| Area                        | Status |
| --------------------------- | ------ |
| HTTP methods (GET/POST/PUT) | ✅     |
| WebSocket connections       | ✅     |
| Pub/Sub over WebSocket      | ✅     |
| ACL enforcement             | ✅     |
| Request/URI size limits     | ✅     |
| Configuration parsing       | ✅     |
| Large uploads               | ✅     |

## Requirements

**Rust tests:**

- Redis running on `localhost:6379`
- Rust dependencies (auto-installed by cargo)

**Shell tests:**

- `curl` - HTTP client
- `ab` (ApacheBench) - For benchmarks only
- `uuidgen` (macOS) or `uuid` (Linux) - For unique keys

## CI/CD Notes

Tests are CI-friendly:

- No hardcoded ports (dynamic allocation)
- Automatic cleanup (RAII)
- Temporary files (auto-deleted)
- Parallel execution safe

## Troubleshooting

**Tests hang:** Check Redis is running on port 6379
**Port conflicts:** Tests use dynamic ports, but ensure no firewall blocks
**WebSocket flaky:** May timeout on slow systems (increase sleep durations)
