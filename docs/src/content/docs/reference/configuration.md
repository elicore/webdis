---
title: Configuration
description: Canonical config files, schema, compatibility keys, and examples.
---

## Canonical files

- `redis-web.json`
- `redis-web.min.json`
- `redis-web.prod.json`
- `redis-web.schema.json`

## Compatibility files (transition)

- `webdis.json`
- `webdis.prod.json`
- `webdis.schema.json`
- `webdis.legacy.json`

Compatibility keys still accepted:

- `threads` alias for `http_threads`
- `pool_size` alias for `pool_size_per_thread`

Environment variable expansion supports exact `$VARNAME` string values.

## Minimal starter config

For the first run, use `redis-web.min.json`:

```json
{
  "$schema": "./redis-web.schema.json",
  "redis_host": "127.0.0.1",
  "redis_port": 6379,
  "http_host": "127.0.0.1",
  "http_port": 7379,
  "database": 0
}
```

This starter config keeps the server local-only and leaves the advanced knobs
unset. Generate the same file with:

```bash
redis-web --write-minimal-config
```

## Transport Selection

Use `transport_mode` to match the binary you are starting.

```json
{
  "transport_mode": "rest"
}
```

Supported values:

- `rest`: use `redis-web` for the HTTP, WebSocket, and compat surface.
- `grpc`: use `redis-web-grpc` for the gRPC surface.

The transport choice now lives in the binary name, and `transport_mode` keeps
the config aligned with that choice.

## Worker and Pool Sizing

Use `runtime_worker_threads` to override the Tokio runtime worker count for
either transport mode.

```json
{
  "runtime_worker_threads": 8
}
```

Attribute reference:

- `runtime_worker_threads`
  Default: unset
  When set, redis-web passes this value to Tokio's multi-thread runtime builder.
  This affects both REST and gRPC startup paths.
- `http_threads`
  Default: `4`
  This remains the HTTP-side concurrency setting used by redis-web for sizing
  Redis pool capacity and related HTTP runtime behavior. It is not the Tokio
  runtime worker-thread count.
- `pool_size_per_thread`
  Default: `10`
  redis-web multiplies this by `http_threads` to derive total Redis pool
  capacity.

When `transport_mode` is `grpc`, REST-only settings such as `websockets`,
`default_root`, and `compat_hiredis` remain in the config for compatibility but
are inactive in `redis-web-grpc`.

## Foreground-first Startup

The main `redis-web` binary now runs in the foreground and logs to stderr by
default. Use your service manager, container runtime, or shell redirection to
daemonize or capture output if you need those behaviors.

These legacy process-manager keys are no longer accepted in config files:

- `daemonize`
- `pidfile`
- `user`
- `group`
- `logfile`
- `log_fsync`

If an older config still uses them, remove them and move that behavior into the
surrounding runtime environment instead.

## gRPC Surface

Use the `grpc` block to configure the gRPC listener and optional helper
services for `redis-web-grpc`.

```json
{
  "transport_mode": "grpc",
  "grpc": {
    "host": "0.0.0.0",
    "port": 7379,
    "enable_health_service": true,
    "enable_reflection": false,
    "max_decoding_message_size": 134217728
  }
}
```

Attribute reference:

- `host`
  Default: `0.0.0.0`
  This is the bind address for the gRPC listener. The current runtime parses it
  as an IP address, not a hostname, so use values such as `127.0.0.1`,
  `0.0.0.0`, or `::1`.
- `port`
  Default: `7379`
  This is the listening port for the gRPC server when `transport_mode` is
  `grpc` and the `redis-web-grpc` binary is in use.
- `enable_health_service`
  Default: `true`
  Exposes the standard `grpc.health.v1.Health` service. Leave this enabled if
  you want load balancers, smoke tests, or `grpcurl` health checks to verify
  that the server is ready.
- `enable_reflection`
  Default: `false`
  Exposes gRPC server reflection. This is mainly a developer convenience for
  tools such as `grpcurl`, IDE plugins, or schema explorers. Keep it disabled
  unless you actively need service discovery.
- `max_decoding_message_size`
  Default: inherit `http_max_request_size`, then fall back to `134217728`
  bytes (128 MiB)
  Caps the size of inbound gRPC messages. Raise this when clients send large
  binary arguments or large command batches through `ExecuteStream`.
- `max_encoding_message_size`
  Default: tonic's server default
  Caps the size of outbound gRPC messages. Set this when replies can be large,
  such as `GET` of large values or array replies with many elements.

Practical guidance:

- If you only want local development access, use `127.0.0.1` for `grpc.host`.
- If you enable reflection in development, pair it with `grpcurl` for fast
  manual testing.
- If you raise `max_decoding_message_size`, consider whether large replies also
  require `max_encoding_message_size`.
- In gRPC mode, `http_host`, `http_port`, `websockets`, `default_root`, and
  `compat_hiredis` stay in the config for compatibility but are not used.

## Hiredis Compat Bridge

Use `compat_hiredis` to opt in to the session endpoints used by
hiredis-compatible clients.

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

Current behavior:

- The bridge is disabled unless you add an explicit `compat_hiredis` section and set `"enabled": true`.
- `path_prefix` is normalized by the runtime to a leading slash and used for all compat routes.
- `session_ttl_sec` controls idle cleanup and keeps stale sessions from leaking resources.
- `max_sessions` limits concurrent active sessions.
- `max_pipeline_commands` rejects oversized pipelined payloads in one request.

For v1, transport mode selection is handled by redis-web only; direct/HTTP/ws mode
switching and richer auth controls are currently not configured through `compat_hiredis`.

## Practical `compat_hiredis` runbook

### Mount route prefix

The prefix is normalized before route registration. This means these values are all
equivalent in config:

```json
{ "compat_hiredis": { "path_prefix": "__compat" } }
```

```json
{ "compat_hiredis": { "path_prefix": "compat" } }
```

```json
{ "compat_hiredis": { "path_prefix": "/compat/" } }
```

All normalize to `/compat`.

```bash
# verify mounted session endpoint
curl -i -X POST http://127.0.0.1:7379/compat/session
```

### Hardening knobs

Use these defaults when a hard limit is desired for embedded clients.

```json
{
  "compat_hiredis": {
    "enabled": true,
    "path_prefix": "/__compat",
    "session_ttl_sec": 30,
    "max_sessions": 64,
    "max_pipeline_commands": 4
  }
}
```

- `session_ttl_sec`: session cleanup happens on access, not per command heartbeat.
- `max_sessions`: create will return `429` when the pool is exhausted.
- `max_pipeline_commands`: command endpoint returns `400` when the request frame count
  exceeds this limit.

### Troubleshooting map

If commands are unexpectedly failing with `-ERR forbidden`:

- Check `acl` policy for the command name.
- Confirm client auth header is what the server expects.
- Compare to REST mode behavior; compat endpoint and regular HTTP route both share the
  same ACL engine but return command-level RESP error text.

### Test ownership

- Static field defaults and config decoding coverage: `crates/redis-web/tests/config_test.rs`
- Runtime compat route behavior: `crates/redis-web/tests/integration_hiredis_compat_test.rs`

## Examples

Canonical files (repo root):

- `redis-web.json`
- `redis-web.min.json`
- `redis-web.prod.json`

gRPC example:

- `docs/examples/config/redis-web.grpc.json`

Compatibility examples (docs only):

- `docs/examples/config/webdis.json`
- `docs/examples/config/webdis.legacy.json`
- `docs/examples/config/webdis.prod.json`

Developer workflow:

- [gRPC Development](/guides/grpc-development/)

### `webdis.json`

```json
{
  "$schema": "https://raw.githubusercontent.com/elicore/redis-web/main/webdis.schema.json",
  "redis_host": "127.0.0.1",
  "redis_port": 6379,
  "http_host": "0.0.0.0",
  "http_port": 7379,
  "http_threads": 4,
  "pool_size_per_thread": 10,
  "database": 0,
  "websockets": false,
  "http_max_request_size": 134217728,
  "verbosity": 4,
  "acl": [
    {
      "disabled": [
        "DEBUG"
      ]
    },
    {
      "http_basic_auth": "user:password",
      "enabled": [
        "DEBUG"
      ]
    }
  ]
}
```

### `webdis.legacy.json`

```json
{
  "$schema": "https://raw.githubusercontent.com/elicore/redis-web/main/webdis.schema.json",
  "redis_host": "127.0.0.1",
  "redis_port": 6379,
  "redis_auth": null,
  "http_host": "0.0.0.0",
  "http_port": 7379,
  "threads": 5,
  "pool_size": 20,
  "websockets": false,
  "database": 0,
  "acl": [
    {
      "disabled": [
        "DEBUG"
      ]
    },
    {
      "http_basic_auth": "user:password",
      "enabled": [
        "DEBUG"
      ]
    }
  ],
  "hiredis": {
    "keep_alive_sec": 15
  },
  "verbosity": 4
}
```

Runtime troubleshooting example:

```text
# mute fallback pub/sub warning from HTTP stream mode
REDIS_WEB_COMPAT_MUTE_HTTP_PUBSUB_WARNING=1
```

### `webdis.prod.json`

```json
{
  "$schema": "https://raw.githubusercontent.com/elicore/redis-web/main/webdis.schema.json",
  "redis_host": "127.0.0.1",
  "redis_port": 6379,
  "redis_auth": [
    "user",
    "password"
  ],
  "http_host": "0.0.0.0",
  "http_port": 7379,
  "http_threads": 4,
  "database": 0,
  "acl": [
    {
      "disabled": [
        "DEBUG"
      ]
    },
    {
      "http_basic_auth": "user:password",
      "enabled": [
        "DEBUG"
      ]
    }
  ],
  "hiredis": {
    "keep_alive_sec": 15
  },
  "verbosity": 3
}
```
