---
title: Embedding
description: Mount redis-web routing into another Axum service.
---

`redis-web-runtime` exposes router-building APIs for embedding.

```rust
use redis_web_core::config::Config;
use redis_web_runtime::server;

let cfg = Config::new("redis-web.json")?;
let router = server::build_router(&cfg)?;
```

For custom parser/executor wiring, use `build_router_with_dependencies(...)`.
