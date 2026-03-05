#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CRATE_DIR="$ROOT_DIR/crates/redis-web-hiredis-compat"
DIST_DIR="$ROOT_DIR/target/hiredis-compat-dist"
MANAGED_REDIS="${MANAGED_REDIS:-1}"

"$ROOT_DIR/scripts/build-hiredis-compat.sh" "$DIST_DIR"

cc \
  -I"$DIST_DIR/include" \
  "$CRATE_DIR/tests/fixtures/runtime_command_roundtrip.c" \
  -L"$DIST_DIR/lib" \
  -lhiredis \
  -o "$DIST_DIR/runtime_roundtrip"

if [[ "$(uname -s)" == "Darwin" ]]; then
  export DYLD_LIBRARY_PATH="$DIST_DIR/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
else
  export LD_LIBRARY_PATH="$DIST_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

if [[ "$MANAGED_REDIS" == "1" ]]; then
  "$ROOT_DIR/subprojects/redispy-hiredis-compat/scripts/start-redis-test-env.sh"
  trap "$ROOT_DIR/subprojects/redispy-hiredis-compat/scripts/stop-redis-test-env.sh" EXIT
fi

REDIS_HOST="${REDIS_HOST:-127.0.0.1}" \
REDIS_PORT="${REDIS_PORT:-6379}" \
  "$DIST_DIR/runtime_roundtrip"

echo "Runtime command roundtrip fixture passed"
