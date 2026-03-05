#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

"$SCRIPT_DIR/setup-test-env.sh"

if [[ "${MANAGED_REDIS:-1}" == "1" ]]; then
  "$SCRIPT_DIR/start-redis-test-env.sh"
  trap '"$SCRIPT_DIR/stop-redis-test-env.sh"' EXIT
fi

PYTHON="$(venv_python)"
REDIS_PY_DIR="${REDIS_PY_DIR:-$VENDOR_DIR/redis-py}"

export REDIS_TEST_URL="${REDIS_TEST_URL:-redis://127.0.0.1:6379/0}"
export TEST_RESP_VERSION=2
export TMPDIR="${TMPDIR:-/tmp}"

"$PYTHON" "$SCRIPT_DIR/verify-hiredis-runtime-behavior.py" --db 0

(
  cd "$REDIS_PY_DIR"
  "$PYTHON" -m pytest \
    tests/test_connect.py \
    tests/test_connection.py \
    tests/test_pipeline.py \
    tests/test_pubsub.py \
    -m "not onlycluster" \
    -k "not uds and not unix and not shard and not msetex" \
    -q
)

echo "redis-py runtime matrix passed"
