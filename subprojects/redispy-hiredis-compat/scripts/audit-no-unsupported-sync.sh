#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

ROOT_DIR="$REPO_ROOT"
LIB_RS="$ROOT_DIR/crates/redis-web-hiredis-compat/src/lib.rs"

if rg -n "ERR_UNSUPPORTED" "$LIB_RS" >/dev/null; then
  echo "found unsupported sync markers in $LIB_RS" >&2
  exit 1
fi

"$SCRIPT_DIR/setup-test-env.sh"

if [[ "${MANAGED_REDIS:-1}" == "1" ]]; then
  "$SCRIPT_DIR/start-redis-test-env.sh"
  trap "$SCRIPT_DIR/stop-redis-test-env.sh" EXIT
fi

MANAGED_REDIS=0 "$ROOT_DIR/crates/redis-web-hiredis-compat/tests/compile-runtime-roundtrip.sh"

"$(venv_python)" "$SCRIPT_DIR/verify-hiredis-runtime-behavior.py" --db 0

echo "no unsupported sync markers and runtime checks passed"
