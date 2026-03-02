---
title: Compatibility Test Matrix
description: Coverage map for protocol, config, and runtime compatibility expectations.
---

Compatibility is validated by functional and integration tests in `crates/redis-web/tests`.

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
