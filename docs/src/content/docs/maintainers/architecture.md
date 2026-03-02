---
title: Architecture
description: Workspace and crate boundaries.
---

Workspace crates:

- `redis-web-core`: config, request/format parsing, ACL, protocol/logging primitives
- `redis-web-runtime`: Redis connectivity, HTTP/WS handlers, router/server wiring
- `redis-web-compat`: naming and migration helpers
- `redis-web`: CLI crate with canonical + alias binaries

This split isolates stable protocol/config logic from transport/runtime and migration concerns.
