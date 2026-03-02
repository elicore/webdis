---
title: Configuration
description: Canonical config files, schema, and compatibility keys.
---

Canonical files:

- `redis-web.json`
- `redis-web.prod.json`
- `redis-web.schema.json`

Compatibility files kept during transition:

- `webdis.json`
- `webdis.prod.json`
- `webdis.schema.json`
- `webdis.legacy.json`

Compatibility keys still accepted:

- `threads` alias for `http_threads`
- `pool_size` alias for `pool_size_per_thread`

Environment variable expansion supports exact `$VARNAME` string values.
