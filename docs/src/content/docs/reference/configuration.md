---
title: Configuration
description: Canonical config files, schema, compatibility keys, and examples.
---

## Canonical files

- `redis-web.json`
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

## Transport Selection

Use `transport_mode` to select which public surface redis-web exposes at
startup.

```json
{
  "transport_mode": "rest"
}
```

Supported values:

- `rest`: enable the existing HTTP and optional WebSocket surface.
- `grpc`: enable the gRPC surface instead of REST/WS.

When `transport_mode` is `grpc`, REST-only settings such as `websockets`,
`default_root`, and `compat_hiredis` remain in the config for compatibility but
are inactive.

## gRPC Surface

Use the `grpc` block to configure the gRPC listener and optional helper
services.

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
  `grpc`.
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

Use `compat_hiredis` to configure session endpoints used by hiredis-compatible
clients.

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

- The bridge is enabled by default (`"enabled": true`) so existing deployments do not need extra config to expose `/__compat/*`.
- `path_prefix` is normalized to a leading slash and used for session and stream routes.
- `session_ttl_sec` controls idle cleanup and keeps stale sessions from leaking resources.
- `max_sessions` limits concurrent active sessions.
- `max_pipeline_commands` rejects oversized pipelined payloads in one request.

For v1, transport mode selection is handled by redis-web only; direct/HTTP/ws mode
switching and richer auth controls are currently not configured through `compat_hiredis`.

## Examples

Canonical files (repo root):

- `redis-web.json`
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
  "daemonize": false,
  "websockets": false,
  "http_max_request_size": 134217728,
  "verbosity": 4,
  "logfile": "webdis.log",
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
  "daemonize": false,
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
  "verbosity": 4,
  "logfile": "webdis.log"
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
  "daemonize": true,
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
  "verbosity": 3,
  "logfile": "/var/log/webdis.log"
}
```
