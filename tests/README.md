# Webdis Test Suite

## Test tiers

The suite is split into three layers:

1. `unit` (`cargo test --lib`)
   - Pure component behavior in `src/*`.
   - No network and no Redis dependency.

2. `functional` (non-Redis contract tests)
   - `config_test`, `handler_test`, `logging_fsync_test`
   - `functional_http_contract_test`
   - `functional_interface_mapping_test`
   - `functional_ws_contract_test`
   - Uses dependency injection and scripted executors to validate parser, ACL, formatter, status mapping, ETag, content-type, body limits, and WS contract behavior without Redis command execution.

3. `integration` (real Redis/process/socket)
   - `integration_process_boot_test`
   - `integration_redis_http_test`
   - `integration_redis_pubsub_test`
   - `integration_redis_socket_test`
   - `websocket_raw_test`
   - Uses real Webdis process startup and Redis interactions.

## Quick commands

```bash
# Unit only
cargo test --lib

# Functional only (non-Redis)
cargo test --test config_test --test handler_test --test logging_fsync_test \
  --test functional_interface_mapping_test --test functional_http_contract_test --test functional_ws_contract_test

# Integration only (Redis/process/socket)
cargo test --test integration_process_boot_test --test integration_redis_http_test \
  --test integration_redis_pubsub_test --test integration_redis_socket_test --test websocket_raw_test

# Compile regression guard for all integration-test crates
cargo test --tests --no-run

# Run the Linux CI workflow locally (both matrix entries) with act
make ci_local
```

`make ci_local` requires Docker and `act` installed locally.

## Shared harnesses

- `tests/support/process_harness.rs`: process lifecycle, raw HTTP helpers, Redis helpers, stream helpers.
- `tests/support/router_harness.rs`: in-process injected router for non-Redis functional tests.
- `tests/support/stub_executor.rs`: deterministic scripted executor and request capture.
- `tests/support/redis_fixtures.rs`: deterministic key helpers.

## CI policy

- PR default: `unit + functional`.
- Push to `main` and scheduled runs: `unit + functional + integration`.
- Manual runs can enable integration tier explicitly.

## Requirements

- Functional tier: no Redis required.
- Integration tier: Redis on `127.0.0.1:6379`.
- For UNIX socket integration tests: `redis-server` on `PATH` (preferred) or Docker available.
