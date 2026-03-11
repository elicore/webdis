---
title: Hiredis Drop-In Compatibility
description: Compatibility scope, status, and integration guidance for the redis-web hiredis shim.
---

## Scope

The hiredis compatibility track aims to let existing hiredis-based clients relink
against a redis-web-backed library with minimal code changes.

For end-to-end client usage, including redis-py and generic hiredis-linked libraries,
see [compatibility/hiredis-client-integration](./hiredis-client-integration.md).

Target ABI:
- hiredis 1.3.x (sync API first)

Platform scope:
- Linux
- macOS

Artifact scope:
- shared library
- static library
- hiredis-style headers

## Current feature set

- Workspace crate: `crates/redis-web-hiredis-compat`
- `cdylib` + `staticlib` artifact configuration
- Symbol scaffold for `redis-web-hiredis-compat`
- Header scaffold at `crates/redis-web-hiredis-compat/include/hiredis/hiredis.h`
- pkg-config files for both naming modes
- Runtime session bridge in redis-web under `/__compat/*`
- Config namespace `compat_hiredis` with defaults and session controls
- Session lifecycle APIs (`POST /__compat/session`, `DELETE /__compat/session/{id}`)
- Command + stream APIs (`/__compat/cmd/{id}.raw`, `/__compat/stream/{id}.raw`, `/__compat/ws/{id}`)
- Session timeout, max sessions, and pipeline limits
- Pub/Sub flow with fallback warning and opt-out env var
- Integration tests for compat session creation, command roundtrip, and stream pub/sub

Current implementation notes:

- Staged `libhiredis` artifacts provide runtime parity by building upstream hiredis core + async C runtime sources used by this repository harness.
- Strict symbol/header parity against pinned upstream hiredis is validated by `STRICT_UPSTREAM_PARITY=1 make compat_redispy_audit`.
- SSL parity is provided via staged `libhiredis_ssl` using upstream hiredis split-library semantics.

## `compat_hiredis` configuration

The bridge is controlled by the root `compat_hiredis` section in `redis-web.json`.

```json
{
  "compat_hiredis": {
    "enabled": true,
    "path_prefix": "/__compat",
    "session_ttl_sec": 300,
    "max_sessions": 1024,
    "max_pipeline_commands": 256
  }
}
```

## Operational notes

- `REDIS_WEB_COMPAT_MUTE_HTTP_PUBSUB_WARNING=1` disables the one-time warning when a session falls back to HTTP stream mode for pub/sub flows.

## ABI build lifecycle

Use this sequence to produce distribution artifacts locally:

```bash
make build_hiredis_compat
make test_hiredis_compat_fixture
```

`build_hiredis_compat` runs `scripts/build-hiredis-compat.sh`, which:

- builds `redis-web-hiredis-compat` in release mode,
- stages upstream hiredis headers at `target/hiredis-compat-dist/include/hiredis`,
- stages C API artifacts in `target/hiredis-compat-dist/lib`,
- stages both `hiredis.pc` and `redisweb-hiredis.pc` in `target/hiredis-compat-dist/pkgconfig`,
- optionally stages SSL artifacts if `HIREDIS_COMPAT_ENABLE_SSL=1`.

Naming modes are both supported in this same staging layout:

- `libhiredis*` exports for drop-in consumers,
- `libredisweb_hiredis*` canonical `redis-web` exports.

## Naming Modes

The plan supports two naming modes:
- `libhiredis*` compatibility naming for drop-in relink scenarios
- `libredisweb_hiredis*` canonical naming for explicit integrations

## Symbol and header audit checks

For CI-safe ABI maintenance, use these two checks:

```bash
make compat_redispy_audit
make compat_ssl_audit
```

`compat_redispy_audit` validates required symbols against the local `hiredis-py` extension and performs upstream parity reporting.
`compat_ssl_audit` additionally checks SSL-library symbol visibility and staging.

Set `STRICT_UPSTREAM_PARITY=1` when missing upstream parity should hard-fail during release preparation.

## Example workflows

### Create a compat session

```bash
BASE=http://127.0.0.1:7379
session_id=$(curl -sS -X POST "$BASE/__compat/session" | \
  sed -n 's/.*"session_id":"\([^"]*\)".*/\1/p')
echo "session_id=$session_id"
```

### Run a RESP command over the compat bridge

```bash
curl -sS -X POST "$BASE/__compat/cmd/$session_id.raw" \
  --data-binary $'*3\r\n$3\r\nSET\r\n$11\r\ncompat_key\r\n$2\r\nok\r\n'

curl -sS -X POST "$BASE/__compat/cmd/$session_id.raw" \
  --data-binary $'*2\r\n$3\r\nGET\r\n$10\r\ncompat_key\r\n'
```

Expected output shapes:

```text
+OK
$2\r\nok\r\n
```

### Subscribe and consume stream updates

```bash
curl -sS -X POST "$BASE/__compat/cmd/$session_id.raw" \
  --data-binary $'*2\r\n$9\r\nSUBSCRIBE\r\n$13\r\ncompat-channel\r\n' >/dev/null

curl -N "$BASE/__compat/stream/$session_id.raw"
```

The stream returns RESP replies (including `message` frames) as chunks.
