---
title: Webdis Compatibility and Migration
description: Scope, migration steps, and how compatibility is tested.
---

## Feature tracks

This surface tracks three active compatibility areas:

- `webdis` migration and bootstrap behavior
- `compat_hiredis` REST endpoint compatibility
- `redis-web-hiredis-compat` ABI/harness compatibility

Use this page as the owner mapping and go-to validation index for Track 1 and Track 2.

This project intentionally preserves compatibility where migration safety
matters.

Guaranteed during the transition cycle:

- `webdis` alias binary
- Legacy config filenames and key aliases
- Webdis-compatible request parsing and response behavior under existing tests

Intentionally shifted:

- Canonical package, crate, binary, image, and docs naming is now `redis-web`
- Documentation is rewritten for the current implementation instead of forked
  historical docs

The compatibility surface no longer includes legacy process-manager behavior.
Configs must drop `daemonize`, `pidfile`, `user`, `group`, `logfile`, and
`log_fsync` in favor of a foreground process managed by the surrounding shell,
container, or service supervisor.

Legacy forked docs were replaced by compatibility-focused pages and tests in this
section.

## Track 1: webdis migration

### CLI and startup behavior

#### Before

```bash
webdis webdis.json
webdis --write-default-config --config ./webdis.generated.json
```

#### After

```bash
redis-web redis-web.min.json
redis-web --write-default-config --config ./redis-web.generated.json
```

Compatibility behavior is implemented in `redis-web/src/lib.rs` and
`redis-web-compat/src/lib.rs`:

- `redis-web` resolves default config in this order:
  - prefer `redis-web.json`
  - then `redis-web.min.json`
  - fallback to `webdis.json`
  - default to `redis-web.json` if neither exists
- `redis-web` prints a fallback notice only when it resolves to legacy `webdis.json`.
- `webdis` prints the deprecation notice before startup.
- `--write-default-config` writes schema paths tied to the invoked binary name:
  - `./redis-web.schema.json` for the main `redis-web` binary
  - `./webdis.schema.json` for legacy binary name
- `--write-minimal-config` writes a compact starter file:
  - `./redis-web.min.json` for canonical binary
  - `./webdis.min.json` for legacy binary name

### Config and schema migration

Legacy names and schema:

```text
webdis.json
webdis.prod.json
webdis.schema.json
```

Canonical names and schema:

```text
redis-web.json
redis-web.prod.json
redis-web.schema.json
```

Typical legacy-to-canonical schema migration in config docs:

```json
"$schema": "./webdis.schema.json"
```

becomes

```json
"$schema": "./redis-web.schema.json"
```

### Runtime surface migration notes

- `start-webdis.sh` remains as a compatibility wrapper.
- CI command names should target package-qualified tests:

```bash
cargo test -p redis-web --test config_test
cargo test -p redis-web --test integration_process_boot_test
```

- Docker image name changed to `ghcr.io/elicore/redis-web`.
- Service naming should migrate from `webdis` service keys to `redis-web` in compose.

## Track 2: compat endpoint compatibility

The `compat_hiredis` bridge is opt-in in REST mode and is not mounted for gRPC
mode.

- Runtime source: `crates/redis-web-runtime/src/server.rs`
- Endpoint behavior source: `crates/redis-web-runtime/src/compat.rs`

Key runtime behavior:

- `compat_hiredis.path_prefix` changes all mounted endpoints when the bridge is
  enabled.
- `/session` creates a new session and returns session metadata.
- `/cmd/{session_id}.raw` executes one or more RESP command frames in order.
- `/stream/{session_id}.raw` keeps a pub/sub stream open and sends RESP messages.
- `/ws/{session_id}` provides the websocket transport variant.

`path_prefix` defaults to `/__compat` and is normalized to a leading slash and
without trailing slash in settings.

Operational limits:

- `max_pipeline_commands` rejects oversized pipelined command payloads in a single request.
- `max_sessions` limits concurrent sessions.
- `session_ttl_sec` controls idle timeout cleanup of sessions.
- ACL checks reuse the server auth pipeline; forbidden commands are returned as
  `-ERR forbidden` frames with HTTP `200`.

## Track 3: ABI bridge and external compatibility

The ABI bridge documentation lives in:

- `docs/compatibility/hiredis-dropin.md`
- `docs/compatibility/hiredis-client-integration.md`

Keep it aligned with:

- `crates/redis-web-hiredis-compat`
- `scripts/build-hiredis-compat.sh`
- `subprojects/redispy-hiredis-compat`

## Compatibility coverage matrix

This matrix is the one-to-one mapping between feature claims and test owners:

| Track | Feature owner | Test owner | Test target |
|---|---|---|---|
| Webdis migration | `redis-web/src/lib.rs`, `redis-web-compat/src/lib.rs` | bootstrap/integration test maintainer | `crates/redis-web/tests/integration_process_boot_test.rs`, `crates/redis-web/tests/config_test.rs` |
| Compat endpoints | `crates/redis-web-runtime/src/server.rs`, `crates/redis-web-runtime/src/compat.rs` | runtime maintainer | `crates/redis-web/tests/integration_hiredis_compat_test.rs`, `crates/redis-web/tests/config_test.rs` |
| ABI bridge | `crates/redis-web-hiredis-compat`, `scripts/build-hiredis-compat.sh`, `subprojects/redispy-hiredis-compat` | compatibility harness maintainer | `make compat_redispy_bootstrap`, `make compat_redispy_audit`, `make compat_ssl_audit`, `make test_hiredis_compat_fixture` |

Suggested focused commands:

```bash
cargo test -p redis-web --test config_test
cargo test -p redis-web --test integration_process_boot_test
cargo test -p redis-web --test integration_hiredis_compat_test
make compat_redispy_bootstrap
make compat_redispy_audit
make compat_ssl_audit
make test_hiredis_compat_fixture
```

## Verification sequence for publication

If these commands are not green, compatibility docs must not be treated as final.

```bash
cargo test -p redis-web --test config_test
cargo test -p redis-web --test integration_process_boot_test
cargo test -p redis-web --test integration_hiredis_compat_test
make compat_redispy_bootstrap
make compat_redispy_audit
make compat_ssl_audit
make test_hiredis_compat_fixture
```

## Release and deprecation milestones

See the deprecation timeline in the repository `CHANGELOG.md`.
