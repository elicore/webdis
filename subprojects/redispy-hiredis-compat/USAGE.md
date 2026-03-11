# hiredis compat integration guide

This guide documents the `redis-web-hiredis-compat` feature and how to consume it from:

1. `redis-py` (via patched `hiredis-py`)
2. Other libraries that dynamically link to `libhiredis`

## What this feature is

`redis-web-hiredis-compat` exports a hiredis-compatible C ABI surface (`libhiredis`) so existing consumers can link without changing their source code.

Current scope in this repository:

- Full upstream hiredis exported symbol parity for the pinned upstream version used by the harness.
- Full upstream public header API name parity for `hiredis.h`, `read.h`, `alloc.h`, and `sds.h`.
- Runtime behavior parity is provided by building and staging upstream hiredis core + async C sources in `scripts/build-hiredis-compat.sh`.
- Parser/reader and transport/async behavior are validated through redis-py compatibility runs and runtime fixtures.

## Architecture

```mermaid
flowchart LR
  A["Client Library"] --> B["hiredis ABI (headers + libhiredis)"]
  B --> C["redis-web-hiredis-compat"]
  C --> D["Parser + compat symbols"]
```

## redis-py usage path

`redis-py` does not link to `libhiredis` directly. It uses `hiredis-py`, whose C extension links to `libhiredis`.

```mermaid
flowchart TD
  RP["redis-py"] --> HP["hiredis-py extension"]
  HP -->|"-lhiredis"| HC["compat libhiredis"]
  HC --> RESP["RESP parser + callbacks"]
```

### End-to-end commands

From repository root:

```bash
make compat_redispy_bootstrap
make compat_redispy_build_hiredis
make compat_redispy_audit
make compat_ssl_audit
make compat_redispy_regression
make compat_redispy_test
```

Equivalent script-level flow:

```bash
subprojects/redispy-hiredis-compat/scripts/pin-upstreams.sh
subprojects/redispy-hiredis-compat/scripts/build-compat-artifacts.sh
subprojects/redispy-hiredis-compat/scripts/build-hiredis-wheel.sh
subprojects/redispy-hiredis-compat/scripts/setup-test-env.sh
subprojects/redispy-hiredis-compat/scripts/run-redispy-tests.sh
```

### Runtime verification for redis-py

```bash
subprojects/redispy-hiredis-compat/scripts/verify-hiredis-active.py --db 0
```

This validates:

- `hiredis` Python module imports successfully.
- `redis-py` selected a hiredis-based parser.
- Redis `PING` succeeds with the configured endpoint.

`setup-test-env.sh` no longer requires a live Redis server by default.

- Default: `VERIFY_HIREDIS_ACTIVE=0` (skip runtime ping during environment setup).
- Optional runtime check: `VERIFY_HIREDIS_ACTIVE=1`.
- Optional verify script override (for regression testing): `VERIFY_HIREDIS_SCRIPT=/path/to/script.py`.

### Expected artifact and audit outputs

During the audit flow, script outputs are written into `.artifacts/` and include:

- `hiredis-required-symbols.txt`
- `hiredis-provided-symbols.txt`
- `hiredis-ssl-provided-symbols.txt`
- `hiredis-provided-all-symbols.txt`
- `hiredis-missing-symbols.txt`
- `symbol-audit.txt`
- `hiredis-upstream-provided-symbols.txt`
- `hiredis-missing-vs-upstream-symbols.txt`
- `hiredis-ssl-missing-vs-upstream-symbols.txt`
- `hiredis-upstream-header-api.txt`
- `hiredis-compat-header-api.txt`
- `hiredis-missing-header-api.txt`

`hiredis-missing-symbols.txt` is a hard gate for the target hiredis extension, while
`STRICT_UPSTREAM_PARITY=1` additionally treats upstream parity gaps in upstream symbol and header APIs as blocking failures.

### Redis test topology

The harness can manage Redis automatically:

```mermaid
flowchart LR
  T["run-redispy-tests.sh"] --> M["redis:6379 (master)"]
  T --> R["redis:6380 (replica)"]
  M --> R
```

Defaults:

- `MANAGED_REDIS=1`
- `COMPAT_REDIS_IMAGE_TAG=8.4.0`
- On ARM hosts, default docker platform is `linux/amd64` to match redis-py GEO precision fixtures.

## Using compat libhiredis with other libraries

Any consumer that links against hiredis can be pointed at compat artifacts.

### 1) Build and stage artifacts

```bash
subprojects/redispy-hiredis-compat/scripts/build-compat-artifacts.sh
```

Artifacts are staged at:

- `subprojects/redispy-hiredis-compat/.dist/hiredis/include`
- `subprojects/redispy-hiredis-compat/.dist/hiredis/lib`
- `subprojects/redispy-hiredis-compat/.dist/hiredis/pkgconfig`

### 2) Export build environment

```bash
source subprojects/redispy-hiredis-compat/.dist/hiredis/env.sh
```

Or manually:

```bash
export PKG_CONFIG_PATH="subprojects/redispy-hiredis-compat/.dist/hiredis/pkgconfig:$PKG_CONFIG_PATH"
export CFLAGS="-Isubprojects/redispy-hiredis-compat/.dist/hiredis/include $CFLAGS"
export LDFLAGS="-Lsubprojects/redispy-hiredis-compat/.dist/hiredis/lib $LDFLAGS"
```

### 3) Build your hiredis consumer

Typical examples:

```bash
# pkg-config driven
cc app.c $(pkg-config --cflags --libs hiredis)

# explicit link
cc app.c -Isubprojects/redispy-hiredis-compat/.dist/hiredis/include \
  -Lsubprojects/redispy-hiredis-compat/.dist/hiredis/lib -lhiredis
```

### 4) Set runtime loader path

```bash
# macOS
export DYLD_LIBRARY_PATH="subprojects/redispy-hiredis-compat/.dist/hiredis/lib:$DYLD_LIBRARY_PATH"

# Linux
export LD_LIBRARY_PATH="subprojects/redispy-hiredis-compat/.dist/hiredis/lib:$LD_LIBRARY_PATH"
```

### 5) Audit symbol coverage (recommended)

For consumers similar to `hiredis-py`, run:

```bash
subprojects/redispy-hiredis-compat/scripts/audit-hiredis-symbols.sh
```

This catches missing symbols before runtime and produces two parity tiers:

- Hard gate: symbols required by the built `hiredis-py` extension must exist in compat `libhiredis`.
- Upstream parity: diff vs upstream hiredis exported symbols and header API names.

Artifacts are written to `subprojects/redispy-hiredis-compat/.artifacts/`, including:

- `hiredis-missing-symbols.txt` (hard fail if non-empty).
- `hiredis-missing-vs-upstream-symbols.txt` (should be empty for full symbol parity).
- `hiredis-missing-header-api.txt` (should be empty for full header API parity).

To make upstream parity diffs fail the audit, set `STRICT_UPSTREAM_PARITY=1`.

## Compatibility contract and limits

- Header surface:
  - `crates/redis-web-hiredis-compat/include/hiredis/hiredis.h`
  - `crates/redis-web-hiredis-compat/include/hiredis/read.h`
  - `crates/redis-web-hiredis-compat/include/hiredis/alloc.h`
  - `crates/redis-web-hiredis-compat/include/hiredis/sds.h`
  - `crates/redis-web-hiredis-compat/include/hiredis/async.h`
- Library names are ABI-compatible (`libhiredis`).
- Symbol and header API parity is full for the pinned upstream version in this harness.
- Runtime behavior parity is provided by upstream hiredis C runtime in staged artifacts.
- SSL parity is provided via staged `libhiredis_ssl` (upstream split-library model) when `HIREDIS_COMPAT_ENABLE_SSL=1` (default).

If integrating a new hiredis-based consumer, treat parser/symbol linkage as the first gate, then validate behavior with that library's native test suite.
