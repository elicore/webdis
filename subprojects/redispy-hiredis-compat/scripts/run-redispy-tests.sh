#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

ensure_dist_dirs
"$SCRIPT_DIR/setup-test-env.sh"

PYTHON="$(venv_python)"
set_hiredis_build_env

MANAGED_REDIS="${MANAGED_REDIS:-1}"
if [[ "$MANAGED_REDIS" == "1" ]]; then
  "$SCRIPT_DIR/start-redis-test-env.sh"
  trap "$SCRIPT_DIR/stop-redis-test-env.sh" EXIT
fi

REDIS_TEST_URL="${REDIS_TEST_URL:-redis://127.0.0.1:6379/0}"
REDIS_PROTOCOL="${REDIS_PROTOCOL:-2}"
PYTEST_MARK_EXPR="${PYTEST_MARK_EXPR:-not onlycluster and not redismod and not ssl and not cp_integration and not experimental}"
PYTEST_K_EXPR="${PYTEST_K_EXPR:-not test_uds_connect}"
PYTEST_EXTRA_ARGS="${PYTEST_EXTRA_ARGS:-}"
PYTEST_TARGETS="${PYTEST_TARGETS:-tests}"

python_summary="$ARTIFACTS_DIR/environment-summary.txt"
junit_xml="$ARTIFACTS_DIR/junit.xml"
pytest_log="$ARTIFACTS_DIR/pytest.log"

mkdir -p "$ARTIFACTS_DIR"

{
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "redis_test_url=$REDIS_TEST_URL"
  echo "redis_protocol=$REDIS_PROTOCOL"
  echo "pytest_mark_expr=$PYTEST_MARK_EXPR"
  echo "redis_py_sha=$(git -C "$REDIS_PY_DIR" rev-parse HEAD)"
  echo "hiredis_py_sha=$(git -C "$HIREDIS_PY_DIR" rev-parse HEAD)"
  echo "python=$($PYTHON --version 2>&1)"
  echo "platform=$(uname -a)"
} > "$python_summary"

echo "WARNING: this test harness flushes DB selected by REDIS_TEST_URL before test run"
"$PYTHON" - <<PY
import redis
from urllib.parse import urlparse

url = "$REDIS_TEST_URL"
r = redis.Redis.from_url("$REDIS_TEST_URL")
r.ping()
r.flushdb()
db = urlparse(url).path.lstrip("/") or "0"
print("Redis reachable and test DB flushed")
print(f"Selected DB: {db}")
PY

test_db="$(
  REDIS_TEST_URL="$REDIS_TEST_URL" "$PYTHON" -c \
    'import os; from urllib.parse import urlparse; p=urlparse(os.environ["REDIS_TEST_URL"]); path=p.path.lstrip("/"); print(int(path) if path else 0)'
)"

"$PYTHON" "$SCRIPT_DIR/verify-hiredis-active.py" --db "$test_db"

set +e
(
  cd "$REDIS_PY_DIR"
  "$PYTHON" -m pytest \
    --redis-url="$REDIS_TEST_URL" \
    --protocol="$REDIS_PROTOCOL" \
    -k "$PYTEST_K_EXPR" \
    --ignore=tests/test_scenario \
    --ignore=tests/test_asyncio/test_scenario \
    --ignore=tests/maint_notifications \
    --ignore=tests/test_sentinel.py \
    --ignore=tests/test_asyncio/test_sentinel.py \
    -m "$PYTEST_MARK_EXPR" \
    --junit-xml="$junit_xml" \
    $PYTEST_EXTRA_ARGS \
    $PYTEST_TARGETS
) | tee "$pytest_log"
pytest_status=${PIPESTATUS[0]}
set -e

if [[ $pytest_status -ne 0 ]]; then
  echo "redis-py test run failed (exit=$pytest_status). See: $pytest_log" >&2
  exit $pytest_status
fi

echo "redis-py standalone compatibility tests passed"
echo "Artifacts:"
echo "- $junit_xml"
echo "- $pytest_log"
echo "- $python_summary"
