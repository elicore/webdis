---
title: Webdis Compatibility Scope
description: What compatibility is guaranteed and what has shifted.
---

This project intentionally preserves compatibility where migration safety matters.

Guaranteed during transition cycle:

- `webdis` alias binary
- Legacy config filenames and key aliases
- Webdis-compatible request parsing and response behavior under existing tests

Intentionally shifted:

- Canonical package, crate, binary, image, and docs naming is now `redis-web`
- Documentation is rewritten for current implementation instead of forked historical docs

Legacy forked docs were pruned and replaced by compatibility-focused pages and tests in this section.

For concrete command/file/image migration steps, use [Migration Guide (webdis -> redis-web)](/compatibility/migration-webdis-to-redis-web/).
