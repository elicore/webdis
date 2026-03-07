# redis-web

`redis-web` is a Rust HTTP/WebSocket gateway for Redis, designed for teams that want an explicit, inspectable protocol boundary with compatibility support for older `webdis` behavior and `hiredis`-based clients.

## Quick start

Run the canonical binary against a JSON config:

```bash
cargo run -p redis-web --bin redis-web -- redis-web.json
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

## What you get from redis-web

`redis-web` exposes Redis command execution as URL-driven HTTP endpoints and WebSocket streams, while keeping format selection explicit:

- JSON (`.json`) for general clients and structured tool consumption.
- MessagePack (`.msg`) for compact binary responses.
- Raw RESP (`.raw`) for lower-level integrations.
- Optional JSONP support and MIME passthrough behavior for compatibility flows.

It also supports:
- WebSocket command transport through both JSON arrays and raw RESP frames.
- Redis DB selection in the request path.
- Command-level ACL enforcement and optional HTTP basic auth.
- TLS connection options for Redis backends and configurable timeouts/pooling settings.
- Config-comparison performance benchmarking through `redis-web-bench`.
- Process controls you usually need in service environments: daemonization, privilege dropping, and structured tracing.

Compatibility is a first-class design goal:
- Legacy `webdis` naming, aliases, and config keys are supported.
- `hiredis` clients (including `redis-py` flows that use `hiredis-py`) can be supported through a staged C ABI compatibility layer (`libhiredis`-style symbols and headers).

## Workspace layout and crate responsibilities

This is a Rust workspace with five crates:

- `redis-web-core`: shared types and behavior that all other crates depend on.
  - Configuration loading and validation (`config` + schema compatibility helpers).
  - Request parsing, output format negotiation, ACL primitives, and response rendering.
- `redis-web-runtime`: the actual server runtime layer.
  - Axum routes, HTTP handlers, command execution bridge, WebSocket handlers, and Redis pool management.
  - Re-exported router/server functions let you embed the same runtime behavior into other Axum apps.
- `redis-web-compat`: compatibility helpers for migration.
  - Canonical vs legacy invocation/config naming.
  - Friendly deprecation behavior and alias handling during transition.
- `redis-web-hiredis-compat`: C ABI compatibility crate (cdylib/staticlib).
  - Builds `libhiredis`-compatible symbols and headers for relinking C clients.
  - Used when you need drop-in behavior for existing `hiredis` integration paths.
- `redis-web`: application entrypoint crate.
  - CLI parsing (`--write-default-config`, legacy/canonical entrypoint behavior).
  - Logging setup, daemonization, privilege dropping, and startup orchestration.
- `redis-web-bench`: informational benchmark runner for comparing config variants.
  - Loads a base config plus named override variants from YAML/JSON.
  - Boots isolated `redis-web` processes, runs benchmark suites, and writes JSON/Markdown artifacts.

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
- Useful build/test entrypoints from repo conventions: `make test`, `make test_all`, `make clean`, `scripts/compose-smoke.sh`
- Config benchmark comparison entrypoint: `make bench_config_compare SPEC=docs/examples/config/redis-web.bench.yaml`
