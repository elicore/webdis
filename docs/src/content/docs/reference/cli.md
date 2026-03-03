---
title: CLI
description: Canonical and compatibility binaries.
---

## Binaries

- `redis-web` (canonical)
- `webdis` (compatibility alias)

Both binaries accept the same flags and config file format. Prefer `redis-web`
for new deployments and scripts.

## Common commands

```bash
redis-web redis-web.json
redis-web --config redis-web.json
redis-web --write-default-config
```

Alias binary:

```bash
webdis webdis.json
```

The alias is temporary and emits a deprecation message.
