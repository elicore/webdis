# redis-web

`redis-web` is a Rust HTTP/WebSocket gateway for Redis, designed for teams that want an explicit, inspectable protocol boundary with compatibility support for older `webdis` behavior and `hiredis`-based clients.

## Quick start

Run the main `redis-web` binary against the minimal starter config:

```bash
cargo run -p redis-web --bin redis-web -- redis-web.min.json
```

If you want to generate that starter file yourself, use:

```bash
cargo run -p redis-web --bin redis-web -- --write-minimal-config
```

Run the explicit gRPC binary against a gRPC config:

```bash
cargo run -p redis-web --bin redis-web-grpc -- docs/examples/config/redis-web.grpc.json
```

`webdis` compatibility is still available as a temporary alias:

```bash
cargo run -p redis-web --bin webdis -- webdis.json
```

Generate a baseline config file for local development:

```bash
cargo run -p redis-web -- --write-default-config --config ./redis-web.generated.json
```

You can start with a minimal command and test responses over HTTP:

```bash
curl http://127.0.0.1:7379/SET/hello/world
curl http://127.0.0.1:7379/GET/hello
```

The main `redis-web` binary runs in the foreground and logs to stderr. Use your
service manager, container runtime, or shell redirection if you want daemon
behavior or log files.

For the core server path, the simplest build and test loop is:

```bash
cargo build
make test
make test_integration
```

## What you get from redis-web

`redis-web` exposes Redis command execution as URL-driven HTTP endpoints and WebSocket streams, while keeping format selection explicit:

- JSON (`.json`) for general clients and structured tool consumption.
- Raw RESP (`.raw`) for lower-level integrations.
- Optional JSONP support and MIME passthrough behavior for compatibility flows.

It also supports:
- WebSocket command transport through both JSON arrays and raw RESP frames.
- Redis DB selection in the request path.
- Command-level ACL enforcement and optional HTTP basic auth.
- TLS connection options for Redis backends and configurable timeouts/pooling settings.
- Config-comparison performance benchmarking through `redis-web-bench`.
- Foreground-first process behavior with structured stderr logging.

Compatibility is a first-class design goal:
- Legacy `webdis` naming, aliases, and config keys are supported.
- `hiredis` clients (including `redis-py` flows that use `hiredis-py`) can be supported through a staged C ABI compatibility layer (`libhiredis`-style symbols and headers).
- gRPC runs through the separate `redis-web-grpc` binary so the default HTTP path stays small.
- Legacy process-manager config knobs are not supported anymore. Configs that still use `daemonize`, `pidfile`, `user`, `group`, `logfile`, or `log_fsync` must be updated to use a foreground `redis-web` process plus your shell, supervisor, container runtime, or service manager for backgrounding, privilege separation, and log handling.

## Workspace layout and crate responsibilities

This is a Rust workspace with four default build members and two opt-in members.

Default build members:

- `redis-web-core`: shared types and behavior that all other crates depend on.
- `redis-web-runtime`: the actual server runtime layer.
- `redis-web-compat`: compatibility helpers for migration.
- `redis-web`: application entrypoint crate.

Opt-in members:

- `redis-web-hiredis-compat`: C ABI compatibility crate (cdylib/staticlib).
- `redis-web-bench`: informational benchmark runner for comparing config variants.

The default members cover the core server path: config loading, request parsing,
HTTP/WebSocket serving, and the CLI entrypoint. The opt-in members add the
heavier compatibility and benchmarking surfaces.

## Subprojects

- `subprojects/redispy-hiredis-compat`: an integration harness and script collection for exercising redis-py / hiredis compatibility end-to-end.
  - Builds and stages compat artifacts.
  - Provides verification scripts and test orchestration for regression workflows.

## Where to continue reading

- Getting started and install/run flow: [`docs/src/content/docs/getting-started/overview.md`](docs/src/content/docs/getting-started/overview.md), [`docs/src/content/docs/getting-started/run-and-first-requests.md`](docs/src/content/docs/getting-started/run-and-first-requests.md)
- API and protocol details: [`docs/src/content/docs/reference/api.md`](docs/src/content/docs/reference/api.md), [`docs/src/content/docs/reference/configuration.md`](docs/src/content/docs/reference/configuration.md), [`docs/src/content/docs/reference/cli.md`](docs/src/content/docs/reference/cli.md)
- Compatibility references: [`docs/src/content/docs/compatibility/webdis-compatibility.md`](docs/src/content/docs/compatibility/webdis-compatibility.md), [`docs/src/content/docs/compatibility/hiredis-dropin.md`](docs/src/content/docs/compatibility/hiredis-dropin.md), [`docs/src/content/docs/compatibility/hiredis-client-integration.md`](docs/src/content/docs/compatibility/hiredis-client-integration.md), [`subprojects/redispy-hiredis-compat/USAGE.md`](subprojects/redispy-hiredis-compat/USAGE.md)
- Embedding and deploy docs: [`docs/src/content/docs/guides/embedding.md`](docs/src/content/docs/guides/embedding.md), [`docs/src/content/docs/guides/deployment.md`](docs/src/content/docs/guides/deployment.md)
- Maintainer architecture: [`docs/src/content/docs/maintainers/architecture.md`](docs/src/content/docs/maintainers/architecture.md), `redis-web.schema.json`
- Core build/test entrypoints from repo conventions: `cargo build`, `make test`, `make test_integration`, `make clean`, `scripts/compose-smoke.sh`
- Heavier opt-in entrypoints: `make test_grpc`, `make test_compat`, `make test_all`, `make perftest`, `make bench_config_compare SPEC=docs/examples/config/redis-web.bench.yaml`
