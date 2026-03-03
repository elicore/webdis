#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CRATE_DIR="$ROOT_DIR/crates/redis-web-hiredis-compat"
DIST_DIR="$ROOT_DIR/target/hiredis-compat-dist"

"$ROOT_DIR/scripts/build-hiredis-compat.sh" "$DIST_DIR"

cc \
  -I"$DIST_DIR/include" \
  "$CRATE_DIR/tests/fixtures/smoke.c" \
  -L"$DIST_DIR/lib" \
  -lhiredis \
  -o "$DIST_DIR/smoke"

"$DIST_DIR/smoke" || true

echo "C fixture compiled successfully against hiredis compatibility artifacts"
