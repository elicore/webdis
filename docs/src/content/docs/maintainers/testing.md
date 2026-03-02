---
title: Testing and CI
description: Test tiers and required checks.
---

Test tiers:

- Unit: `cargo test --workspace --lib`
- Functional: non-Redis contract tests in `crates/redis-web/tests`
- Integration: process + Redis socket/HTTP/WS flows

Core commands:

```bash
cargo test -p redis-web --tests --no-run
make test
make test_integration
```

CI also runs docs build/link checks and rename guard checks.
