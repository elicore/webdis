#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

ensure_submodules_present
ensure_dist_dirs

"$SCRIPT_DIR/build-compat-artifacts.sh"

work_hiredis_py="$(reset_hiredis_workdir)"
apply_hiredis_patch "$work_hiredis_py"

PYTHON="$(venv_python)"
"$PYTHON" -m pip install --upgrade pip setuptools wheel build

set_hiredis_build_env

"$PYTHON" -m pip wheel --no-deps --no-build-isolation -w "$WHEELS_DIR" "$work_hiredis_py"

WHEEL_PATH="$(latest_hiredis_wheel)"
if [[ -z "$WHEEL_PATH" ]]; then
  echo "failed to locate built hiredis wheel in $WHEELS_DIR" >&2
  exit 1
fi

echo "$WHEEL_PATH" > "$WHEELS_DIR/latest-hiredis-wheel.txt"
echo "Built hiredis wheel: $WHEEL_PATH"
