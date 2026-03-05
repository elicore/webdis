# redispy-hiredis-compat subproject

Local harness for validating `redis-py` against `redis-web-hiredis-compat` as a `libhiredis` replacement.

See also: `subprojects/redispy-hiredis-compat/USAGE.md` for architecture diagrams and integration recipes for redis-py and other hiredis consumers.

## What this subproject does

1. Pins `redis-py` and `hiredis-py` as git submodules at release tags.
2. Builds/stages compat `libhiredis` artifacts from this repository.
3. Rebuilds `hiredis-py` against compat artifacts (no vendored hiredis C sources).
4. Runs standalone `redis-py` tests with hiredis enabled.

## Layout

- `vendor/redis-py`: pinned upstream submodule.
- `vendor/hiredis-py`: pinned upstream submodule.
- `versions.lock`: release tags + resolved SHAs.
- `patches/hiredis-py/external-link.patch`: build patch forcing external `libhiredis` linkage.
- `scripts/`: bootstrap/build/audit/setup/test scripts.
- `.dist/`: built artifacts (compat libs + wheels).
- `.artifacts/`: test and audit outputs.

## Prerequisites

- Rust toolchain (for `cargo build`).
- Python 3 with `venv`.
- Docker (default path: managed Redis master+replica test env).
- `redis-cli` for readiness checks.

## Quick start

```bash
# from repo root
make compat_redispy_bootstrap
make compat_redispy_test
```

## Script workflow

```bash
# refresh release-tag pins and lockfile
subprojects/redispy-hiredis-compat/scripts/pin-upstreams.sh

# build compat headers/libs into .dist/hiredis/
subprojects/redispy-hiredis-compat/scripts/build-compat-artifacts.sh

# rebuild hiredis wheel linked to compat libhiredis
subprojects/redispy-hiredis-compat/scripts/build-hiredis-wheel.sh

# enforce symbol coverage needed by hiredis-py extension
subprojects/redispy-hiredis-compat/scripts/audit-hiredis-symbols.sh

# create venv + install deps + install patched hiredis and editable redis-py
subprojects/redispy-hiredis-compat/scripts/setup-test-env.sh

# run standalone redis-py tests and write junit/log summaries
subprojects/redispy-hiredis-compat/scripts/run-redispy-tests.sh

# optional: manage the redis test topology explicitly
subprojects/redispy-hiredis-compat/scripts/start-redis-test-env.sh
subprojects/redispy-hiredis-compat/scripts/stop-redis-test-env.sh
```

## Environment knobs

- `REDIS_PY_TAG` and `HIREDIS_PY_TAG` override release tags for `pin-upstreams.sh`.
- `MANAGED_REDIS` controls managed test redis lifecycle in `run-redispy-tests.sh`:
  - `1` (default): start/stop managed Redis via docker compose (`redis` + `replica`).
  - `0`: use an already-running Redis topology.
- `COMPAT_REDIS_IMAGE_TAG` selects redis image tag for managed mode (default `8.4.0`).
- `COMPAT_REDIS_DOCKER_PLATFORM` overrides docker platform for managed mode.
  - Default is `linux/amd64` on `arm64/aarch64` hosts to match redis-py GEO fixture precision.
- `REDIS_TEST_URL` controls Redis test endpoint (default `redis://127.0.0.1:6379/0`).
- `PYTEST_MARK_EXPR` overrides test marker selection.
- `PYTEST_K_EXPR` overrides default name filter (`not test_uds_connect`).
- `PYTEST_EXTRA_ARGS` appends raw extra pytest arguments.
- `PYTEST_TARGETS` overrides pytest target path(s) (default `tests`).
- `PYTHON_BIN` selects the Python executable used to create/use the venv.

## Outputs

- Symbol audit:
  - `.artifacts/symbol-audit.txt`
  - `.artifacts/hiredis-required-symbols.txt`
  - `.artifacts/hiredis-provided-symbols.txt`
  - `.artifacts/hiredis-missing-symbols.txt`
- Test run:
  - `.artifacts/junit.xml`
  - `.artifacts/pytest.log`
  - `.artifacts/environment-summary.txt`

## Troubleshooting

- Missing hiredis symbols:
  - Run `scripts/audit-hiredis-symbols.sh` and inspect `.artifacts/hiredis-missing-symbols.txt`.
  - Add missing exports to `crates/redis-web-hiredis-compat/src/lib.rs` and corresponding headers.
- Dynamic linker errors at import time:
  - Ensure `DYLD_LIBRARY_PATH` (macOS) or `LD_LIBRARY_PATH` (Linux) includes `.dist/hiredis/lib`.
- Patch apply failures:
  - Upstream `hiredis-py` changed; refresh/rework `patches/hiredis-py/external-link.patch`.

## Safety note

`run-redispy-tests.sh` calls `FLUSHDB` on the DB selected by `REDIS_TEST_URL` before running tests.
Use a disposable database for this harness.

When `MANAGED_REDIS=1`, ports `6379` and `6380` must be free before startup.

By default the standalone run excludes known external-infra suites:
- `tests/maint_notifications`
- `tests/test_sentinel.py`
- `tests/test_asyncio/test_sentinel.py`
