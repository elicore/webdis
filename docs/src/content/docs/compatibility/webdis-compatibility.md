---
title: Webdis Compatibility and Migration
description: Scope, migration steps, and how compatibility is tested.
---

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

Legacy forked docs were pruned and replaced by compatibility-focused pages and
tests in this section.

## Migration guide (webdis -> redis-web)

This section gives direct before/after mappings for runtime, config, Docker,
and CI surfaces.

### CLI migration

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

### Config and schema file migration

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

### Rust crate import migration

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

### Docker image migration (GHCR)

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

### Compose service name migration

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

### Script migration

Before:

```bash
./scripts/start-webdis.sh --mode dev
```

After:

```bash
./scripts/start-redis-web.sh --mode dev
```

`start-webdis.sh` remains as a deprecated compatibility wrapper.

### CI migration examples

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

## Compatibility test matrix

Compatibility is validated by functional and integration tests in
`crates/redis-web/tests`.

Key cases:

- config alias keys (`threads`, `pool_size`)
- env-var expansion behavior
- default config precedence (`redis-web.json` then `webdis.json`)
- alias binary deprecation path
- request parsing parity (DB prefix, percent decoding)
- output format and status code mappings

Run focused compatibility tests:

```bash
cargo test -p redis-web --test config_test
cargo test -p redis-web --test functional_interface_mapping_test
cargo test -p redis-web --test integration_process_boot_test
```

## Release and deprecation milestones

See the deprecation timeline in the repository `CHANGELOG.md`.
