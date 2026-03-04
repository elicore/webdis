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

Compatibility examples (docs only):

- `docs/examples/config/webdis.json`
- `docs/examples/config/webdis.legacy.json`
- `docs/examples/config/webdis.prod.json`

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
