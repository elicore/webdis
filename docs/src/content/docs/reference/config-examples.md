---
title: Config Examples
description: Canonical and compatibility config examples used in docs and Docker workflows.
---

Canonical files (repo root):

- `redis-web.json`
- `redis-web.prod.json`

Compatibility examples (docs only):

- `docs/examples/config/webdis.json`
- `docs/examples/config/webdis.legacy.json`
- `docs/examples/config/webdis.prod.json`

## `webdis.json`

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

## `webdis.legacy.json`

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

## `webdis.prod.json`

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
