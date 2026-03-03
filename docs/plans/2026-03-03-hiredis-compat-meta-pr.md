# Hiredis-Compatible Drop-In for redis-web (Meta PR Plan)

## Meta PR
- Branch: `codex/hiredis-compat-meta-plan`
- Title: `plan: hiredis-compatible redis-web bridge (ABI + transport + docs)`

## Summary
Goal:
1. Build a new crate that is a strict `hiredis`-compatible drop-in (ABI target: hiredis 1.3.x, sync API first).
2. Route command traffic to `redis-web` over HTTP/WebSocket with a WS-first fast path and HTTP fallback.
3. Preserve easy integration for existing clients using `redisConnect(host, port)` via env-based overrides.
4. Support Pub/Sub parity even on HTTP fallback, emit warning for HTTP-fallback Pub/Sub, and allow env-based warning mute.
5. Ship shared+static+headers on Linux+macOS, with both `libhiredis`-compatible and `libredisweb_hiredis` naming.
6. Add a dedicated documentation phase for compatibility and functional/perf tests for maintainers and users.

## Grounded Current Baseline
1. Existing `crates/redis-web-compat` only handles naming/config migration helpers.
2. Raw RESP support already exists in runtime:
- HTTP `.raw` handling path.
- WebSocket raw endpoint `/.raw`.
3. Existing HTTP Pub/Sub streams SSE/JSON/JSONP, but not a hiredis session-compatible RESP stream model.

## Branch and PR Topology
1. Meta branch: `codex/hiredis-compat-meta-plan`.
2. Meta PR title: `plan: hiredis-compatible redis-web bridge (ABI + transport + docs)`.
3. Follow-up implementation branches:
- `codex/hiredis-compat-server-session`
- `codex/hiredis-compat-lib-abi-sync`
- `codex/hiredis-compat-pubsub-fallback`
- `codex/hiredis-compat-packaging-symbols`
- `codex/hiredis-compat-docs-tests`

## Meta PR Deliverables
1. Architecture/spec document that is implementation-ready and decision-complete.
2. PR body with complete sub-task checklist.
3. Follow-up PR map with merge order and dependency notes.
4. Risks, rollback strategy, acceptance criteria, and done definition.

## Meta PR Sub-Tasks (Complete)
- [ ] Add new crate `crates/redis-web-hiredis-compat` (keep existing `crates/redis-web-compat` unchanged).
- [ ] Vendor pinned hiredis 1.3.x C sources in-tree.
- [ ] Implement strict sync hiredis ABI surface (`redisContext`, `redisCommand*`, append/getReply, reconnect/timeouts).
- [ ] Export out-of-v1 async/SSL-related symbols as deterministic stubs with explicit runtime errors.
- [ ] Add transport engine with modes: `auto` (WS-first), `ws`, `http`, `direct`.
- [ ] Implement Pub/Sub parity for WS path.
- [ ] Implement Pub/Sub parity for HTTP fallback path.
- [ ] Emit warning when Pub/Sub is established on HTTP fallback.
- [ ] Add env var to mute HTTP-fallback Pub/Sub warning.
- [ ] Add additive server compat endpoints under `/__compat/*`.
- [ ] Ensure per-session dedicated Redis connection semantics and state preservation (`SELECT`, transaction state, subscribe state).
- [ ] Add env-based auth/endpoint/timeout controls with no required app source changes.
- [ ] Enforce ACL/auth parity for compat endpoints.
- [ ] Add config/schema support for `compat_hiredis` namespace.
- [ ] Build and package shared + static + headers for Linux and macOS.
- [ ] Ship both artifact naming modes (`libhiredis*` and `libredisweb_hiredis*`).
- [ ] Add `pkg-config` metadata for both names (`hiredis.pc`, `redisweb-hiredis.pc`).
- [ ] Add C fixture compatibility suite linked against compat library.
- [ ] Add transport matrix tests (`ws`, `http`, `auto`, `direct`).
- [ ] Add Pub/Sub parity test suite (subscribe/pattern/unsubscribe flows and reply shape).
- [ ] Add symbol export parity checks against pinned hiredis 1.3.x expectations.
- [ ] Add struct layout compile checks via C fixtures.
- [ ] Add performance comparison harness and report generation (no CI fail gate).
- [ ] Add full docs phase for compatibility, functional tests, perf tests, migration, and troubleshooting.
- [ ] Update CI/docs commands and release notes/changelog entries.

## Implementation Phases

### Phase 0: Meta PR and Task Tracker
1. Open meta PR on `codex/hiredis-compat-meta-plan`.
2. Include this full plan and checklist.
3. Link all follow-up branches and intended merge order.

### Phase 1: New Compat Server Session Layer (Additive)
1. Add endpoints under configurable prefix (default `/__compat`):
- `POST /__compat/session`
- `GET /__compat/ws/{session_id}`
- `POST /__compat/cmd/{session_id}.raw`
- `GET /__compat/stream/{session_id}.raw`
- `DELETE /__compat/session/{session_id}`
2. Session behavior:
- Create dedicated Redis connection per compat session.
- Preserve connection-scoped behavior across requests.
3. Pub/Sub stream behavior:
- RESP-compatible streamed replies for fallback path.
4. Resource controls:
- Session TTL and max sessions.
5. Security:
- Reuse ACL/auth policy model consistently.

### Phase 2: New Drop-In ABI Crate
1. Create `crates/redis-web-hiredis-compat`.
2. Vendor hiredis 1.3.x source and preserve ABI layouts.
3. Implement sync API behavior to match existing hiredis client expectations.
4. Keep existing host/port call sites unchanged and route behavior via env-based runtime config.
5. Stub non-v1 exports with explicit error text and `REDIS_ERR`.

### Phase 3: Transport and Fallback Semantics
1. Default mode `auto`: WS raw first, fallback to HTTP compat command/stream endpoints.
2. Mode `ws`: force WebSocket raw.
3. Mode `http`: force HTTP compat endpoints.
4. Mode `direct`: bypass redis-web and connect Redis TCP directly for A/B and emergency fallback.
5. Pub/Sub warning:
- On HTTP-fallback Pub/Sub establishment, emit one warning.
- Allow env-based mute.

### Phase 4: Config, Schema, and Runtime Controls
1. Add config block in core config and schema:
- `compat_hiredis.enabled`
- `compat_hiredis.path_prefix`
- `compat_hiredis.session_ttl_sec`
- `compat_hiredis.max_sessions`
- `compat_hiredis.max_pipeline_commands`
2. Defaults:
- Feature enabled by default for integration simplicity.
- Conservative TTL/session/pipeline limits.
3. Env controls in library:
- transport mode
- compat prefix
- auth header/basic/bearer data
- timeout tuning
- warning mute switch

### Phase 5: Packaging and Integration Artifacts
1. Produce artifacts:
- Shared library (`.so/.dylib`)
- Static library (`.a`)
- Hiredis-compatible headers under `include/hiredis/`
2. Naming outputs:
- `libhiredis*` compatibility artifacts
- `libredisweb_hiredis*` canonical artifacts
3. `pkg-config` outputs:
- `hiredis.pc`
- `redisweb-hiredis.pc`
4. Add install/dist assembly scripts and usage examples.

### Phase 6: Testing and Validation (Functional + ABI + Perf)
1. C compatibility fixtures:
- Build and run sample client programs with no source changes.
2. Functional behavior:
- basic commands
- binary payloads
- pipelining
- transactions
- reconnect/error mapping
- DB switching and state preservation
3. Pub/Sub parity:
- SUBSCRIBE/PSUBSCRIBE/UNSUBSCRIBE/PUNSUBSCRIBE flow and payload shape
4. Transport matrix:
- forced `ws`
- forced `http`
- `auto` fallback
- `direct`
5. Async/SSL stub behavior:
- deterministic unsupported responses and no crash behavior
6. Server integration tests:
- new runtime tests for compat session endpoint correctness
7. ABI conformance:
- exported symbol list checks against pinned hiredis 1.3.x sync target
- struct layout compile assertions via C fixtures
8. Performance:
- add harness comparing direct hiredis TCP vs compat WS vs compat HTTP
- generate report artifacts for future optimization tracking
- no CI fail gate (measurement only)

### Phase 7: Documentation for Client Maintainers and Users
1. Compatibility support matrix doc:
- what is fully compatible now
- what is stubbed in v1
- platform/artifact matrix
2. Integration guide for maintainers:
- relinking with `libhiredis` compatibility name
- optional canonical `libredisweb_hiredis` usage
- static vs dynamic linking notes
- pkg-config/CMake/Make examples
3. Functional test guide:
- test categories, commands, expected outputs, failure interpretation
4. Performance test guide:
- harness methodology
- baseline reading
- caveats and future optimization workflow
5. Troubleshooting:
- transport fallback diagnostics
- auth/env misconfiguration checks
- Pub/Sub fallback warning and mute behavior
6. Release documentation:
- migration notes and known limitations

## Public API / Interface / Type Changes
1. New workspace crate: `crates/redis-web-hiredis-compat`.
2. New additive server endpoint family: `/__compat/*`.
3. New config namespace: `compat_hiredis` in config + schema.
4. New environment-variable contract for compat library behavior.
5. New distributed artifact set with dual naming and pkg-config files.

## Explicit Assumptions and Defaults
1. ABI target is hiredis 1.3.x.
2. v1 prioritizes sync hiredis API compatibility.
3. Async/SSL extras are exported stubs with explicit runtime errors in v1.
4. Platform scope is Linux + macOS.
5. Artifact scope is shared + static + headers.
6. Naming scope ships both `libhiredis*` and `libredisweb_hiredis*`.
7. Default transport is WS-first with HTTP fallback.
8. Pub/Sub must function on HTTP fallback with warning + env mute support.
9. Direct TCP mode is included as optional runtime bypass.
10. Existing `crates/redis-web-compat` remains dedicated to naming/config migration helpers.
11. Meta PR is tracking/spec; implementation lands in follow-up PRs.
