---
title: Embedding
description: Mount redis-web routing into another Axum service.
---

`redis-web-runtime` exposes router-building APIs for embedding.

Use embedding when you already run an Axum service and want to expose
`redis-web` endpoints alongside your own routes. The runtime returns an
`axum::Router`, so you can `merge` or `nest` it under whatever path makes sense
for your app.

Typical flow:

- Load configuration the same way you would for the standalone binary.
- Build the router from that config.
- Combine it with your existing router.

```rust
use redis_web_core::config::Config;
use redis_web_runtime::server;

let cfg = Config::new("redis-web.json")?;
let router = server::build_router(&cfg)?;
```

For custom parser/executor wiring, use `build_router_with_dependencies(...)`.
That hook is useful when you want to inject your own Redis client, attach
metrics, or wrap the executor with extra authorization logic.
