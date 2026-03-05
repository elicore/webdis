#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

ensure_submodules_present
ensure_dist_dirs

"$SCRIPT_DIR/build-hiredis-wheel.sh"

PYTHON="$(venv_python)"
"$PYTHON" -m pip install --upgrade pip
"$PYTHON" -m pip install -r "$REDIS_PY_DIR/dev_requirements.txt"

WHEEL_PATH="$(latest_hiredis_wheel)"
if [[ -z "$WHEEL_PATH" ]]; then
  echo "no hiredis wheel found in $WHEELS_DIR" >&2
  exit 1
fi

"$PYTHON" -m pip install --force-reinstall "$WHEEL_PATH"
"$PYTHON" -m pip install -e "$REDIS_PY_DIR"

set_hiredis_build_env

"$PYTHON" "$SCRIPT_DIR/verify-hiredis-active.py"

echo "Test environment ready: $VENV_DIR"
