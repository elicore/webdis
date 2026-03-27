---
title: Maintainers Guide
description: Architecture, testing, release practices, and changelog notes.
---

## Architecture

Workspace crates:

- `redis-web-core`: config, request/format parsing, ACL, protocol/logging primitives
- `redis-web-runtime`: Redis connectivity, HTTP/WS handlers, router/server wiring
- `redis-web-compat`: naming and migration helpers
- `redis-web-hiredis-compat`: C ABI compatibility library scaffolding for hiredis drop-in usage
- `redis-web`: CLI crate with canonical HTTP + alias binaries, plus an explicit gRPC entrypoint

This split isolates stable protocol/config logic from transport/runtime and
migration concerns.

The default build path targets the core server crates only. Optional surfaces
like gRPC, hiredis ABI compatibility, and benchmark harnesses stay available as
explicit opt-ins.

## Testing and CI

Test tiers:

- Unit: `cargo test --workspace --lib`
- Functional: non-Redis contract tests in `crates/redis-web/tests`
- Integration: process + Redis socket/HTTP/WS flows

Core commands:

```bash
cargo build                                                   # build the default server members
make test                                                     # run fast unit + contract tests
make test_integration                                         # run core HTTP/WS/socket integration tests
make test_grpc                                                # run gRPC contract/integration tests
make test_compat                                              # run hiredis compatibility integration tests
make bench_config_compare SPEC=docs/examples/config/redis-web.bench.yaml         # compare benchmark variants
make bench_config_compare SPEC=docs/examples/config/redis-web.use-cases.bench.yaml # compare use-case variants
```

CI also runs docs build/link checks and rename guard checks.

For realistic deployment-oriented benchmark scenarios, see
[Benchmark Catalog](/maintainers/benchmark-catalog/).

## Release and signing

Canonical image namespace:

- `ghcr.io/elicore/redis-web`

Transition compatibility tags are also published under:

- `ghcr.io/elicore/webdis`

Build and push workflow signs images when cosign secrets are configured.

Verification example:

```bash
./scripts/validate-image.sh --image ghcr.io/elicore/redis-web:latest --method cosign
```

## Changelog

The canonical changelog lives in `CHANGELOG.md` at the repository root.

Changelog generation is automated via the `changelog` GitHub Action using
`git-cliff`.
