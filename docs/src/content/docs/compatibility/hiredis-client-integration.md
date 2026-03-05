---
title: Hiredis Client Integration
description: How redis-web-hiredis-compat is used from redis-py and other hiredis-linked clients.
---

This page documents the hiredis compatibility feature and integration patterns for:

- `redis-py` (through `hiredis-py`)
- Other libraries that link to `libhiredis`

## Feature model

`redis-web-hiredis-compat` exports a hiredis-compatible C ABI (`libhiredis`) so existing consumers can relink without source-level changes.

Current implementation focus:

- Parser/reader ABI required by `hiredis-py`
- SDS, allocator, and command formatting symbols used by `hiredis-py`
- Explicit exported stubs for unsupported command-execution paths

## Integration architecture

```mermaid
flowchart LR
  C["Client library"] --> H["hiredis headers + libhiredis ABI"]
  H --> RW["redis-web-hiredis-compat"]
  RW --> P["RESP parser and callback surface"]
```

## redis-py path

`redis-py` uses `hiredis-py` when available. The `hiredis-py` extension links against `libhiredis`.

```mermaid
flowchart TD
  RP["redis-py"] --> HP["hiredis-py C extension"]
  HP -->|"dynamic link: -lhiredis"| HC["compat libhiredis"]
  HC --> RESP["reader callbacks + parsed replies"]
```

The local harness in this repository automates this flow:

1. Build compat artifacts from `crates/redis-web-hiredis-compat`.
2. Patch and rebuild `hiredis-py` to link externally.
3. Install patched wheel into an isolated venv.
4. Run redis-py standalone tests.

### Commands

```bash
make compat_redispy_bootstrap
make compat_redispy_build_hiredis
make compat_redispy_test
```

Manual script flow:

```bash
subprojects/redispy-hiredis-compat/scripts/build-compat-artifacts.sh
subprojects/redispy-hiredis-compat/scripts/build-hiredis-wheel.sh
subprojects/redispy-hiredis-compat/scripts/setup-test-env.sh
subprojects/redispy-hiredis-compat/scripts/run-redispy-tests.sh
```

### Runtime verification

```bash
subprojects/redispy-hiredis-compat/scripts/verify-hiredis-active.py --db 0
```

This checks that `redis-py` is actively using a hiredis parser.

## Redis test topology used by harness

```mermaid
flowchart LR
  T["run-redispy-tests.sh"] --> M["redis:6379 master"]
  T --> R["redis:6380 replica"]
  M --> R
```

Notes:

- Managed mode is on by default (`MANAGED_REDIS=1`).
- On ARM hosts, the harness defaults docker platform to `linux/amd64` to match redis-py GEO precision fixtures.

## Using with other hiredis-linked libraries

For any library that expects hiredis headers and `-lhiredis`:

1. Build compat artifacts:

```bash
subprojects/redispy-hiredis-compat/scripts/build-compat-artifacts.sh
```

2. Export compiler/linker paths:

```bash
source subprojects/redispy-hiredis-compat/.dist/hiredis/env.sh
```

3. Build your consumer normally (pkg-config or explicit include/lib flags).

4. Set runtime loader paths as needed:

```bash
# macOS
export DYLD_LIBRARY_PATH="subprojects/redispy-hiredis-compat/.dist/hiredis/lib:$DYLD_LIBRARY_PATH"

# Linux
export LD_LIBRARY_PATH="subprojects/redispy-hiredis-compat/.dist/hiredis/lib:$LD_LIBRARY_PATH"
```

5. Validate with the consumer's own tests.

## Compatibility limits

This is an ABI compatibility effort, not full hiredis feature parity yet.

- Supported: symbol/link compatibility required by the current redis-py + hiredis-py integration path.
- In progress: broader hiredis command/transport behavior beyond parser-focused paths.

For implementation-level details and local harness usage, see:

- `subprojects/redispy-hiredis-compat/USAGE.md`
