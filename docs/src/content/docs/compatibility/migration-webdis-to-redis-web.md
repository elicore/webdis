---
title: Migration Guide (webdis -> redis-web)
description: Command-by-command migration from legacy naming to canonical redis-web naming.
---

This page gives direct before/after mappings for runtime, config, Docker, and CI surfaces.

## Scope

- Protocol behavior compatibility remains covered during transition.
- Naming and packaging move to `redis-web`.
- Legacy names remain available temporarily for migration safety.

## CLI migration

Before:

```bash
webdis webdis.json
webdis --write-default-config --config ./webdis.generated.json
```

After:

```bash
redis-web redis-web.json
redis-web --write-default-config --config ./redis-web.generated.json
```

Compatibility fallback still works:

```bash
redis-web   # loads redis-web.json, then falls back to webdis.json
```

## Config and schema file migration

Before:

```text
webdis.json
webdis.prod.json
webdis.schema.json
```

After:

```text
redis-web.json
redis-web.prod.json
redis-web.schema.json
```

Recommended update in config documents:

Before:

```json
"$schema": "./webdis.schema.json"
```

After:

```json
"$schema": "./redis-web.schema.json"
```

## Rust crate import migration

Before:

```rust
use webdis::server;
use webdis::config::Config;
```

After:

```rust
use redis_web_runtime::server;
use redis_web_core::config::Config;
```

## Docker image migration (GHCR)

Before:

```yaml
image: ghcr.io/elicore/webdis:latest
```

After:

```yaml
image: ghcr.io/elicore/redis-web:latest
```

Pinned production tag example:

```yaml
image: ghcr.io/elicore/redis-web:1.0.0
```

## Compose service name migration

Before:

```yaml
services:
  webdis:
    image: ghcr.io/elicore/webdis:latest
```

After:

```yaml
services:
  redis-web:
    image: ghcr.io/elicore/redis-web:latest
```

## Script migration

Before:

```bash
./scripts/start-webdis.sh --mode dev
```

After:

```bash
./scripts/start-redis-web.sh --mode dev
```

`start-webdis.sh` remains as a deprecated compatibility wrapper.

## CI migration examples

Before:

```bash
cargo test --test config_test
cargo test --test integration_process_boot_test
```

After:

```bash
cargo test -p redis-web --test config_test
cargo test -p redis-web --test integration_process_boot_test
```

## Release and deprecation milestones

See the deprecation timeline in the repository `CHANGELOG.md` and compatibility commitments in [Webdis Compatibility Scope](/compatibility/webdis-compatibility/).
