---
title: Run
description: Start redis-web locally with canonical and compatibility entrypoints.
---

## Canonical CLI

```bash
cargo run -p redis-web --bin redis-web -- redis-web.json
```

If no config path is passed, `redis-web` loads `redis-web.json` by default, then falls back to `webdis.json`.

## Compatibility alias

```bash
cargo run -p redis-web --bin webdis -- webdis.json
```

The `webdis` binary works as a transition alias and prints a deprecation notice.

## Write a default config

```bash
redis-web --write-default-config
```

This writes `redis-web.json` with `$schema` set to `./redis-web.schema.json`.
