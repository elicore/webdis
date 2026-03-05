#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SUBPROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$SUBPROJECT_DIR/../.." && pwd)"

VENDOR_DIR="$SUBPROJECT_DIR/vendor"
REDIS_PY_DIR="$VENDOR_DIR/redis-py"
HIREDIS_PY_DIR="$VENDOR_DIR/hiredis-py"

DIST_DIR="$SUBPROJECT_DIR/.dist"
DIST_HIREDIS_DIR="$DIST_DIR/hiredis"
WHEELS_DIR="$DIST_DIR/wheels"
ARTIFACTS_DIR="$SUBPROJECT_DIR/.artifacts"
WORK_DIR="$SUBPROJECT_DIR/.work"
VENV_DIR="$SUBPROJECT_DIR/.venv"
PATCH_FILE="$SUBPROJECT_DIR/patches/hiredis-py/external-link.patch"
VERSIONS_LOCK="$SUBPROJECT_DIR/versions.lock"

PYTHON_BIN_DEFAULT="python3"
if ! command -v "$PYTHON_BIN_DEFAULT" >/dev/null 2>&1; then
  PYTHON_BIN_DEFAULT="python"
fi

PYTHON_BIN="${PYTHON_BIN:-$PYTHON_BIN_DEFAULT}"

ensure_python() {
  if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
    echo "python interpreter not found: $PYTHON_BIN" >&2
    exit 1
  fi
}

ensure_submodules_present() {
  if [[ ! -d "$REDIS_PY_DIR/.git" && ! -f "$REDIS_PY_DIR/.git" ]]; then
    echo "missing redis-py submodule at $REDIS_PY_DIR" >&2
    exit 1
  fi
  if [[ ! -d "$HIREDIS_PY_DIR/.git" && ! -f "$HIREDIS_PY_DIR/.git" ]]; then
    echo "missing hiredis-py submodule at $HIREDIS_PY_DIR" >&2
    exit 1
  fi
}

ensure_venv() {
  ensure_python
  if [[ ! -x "$VENV_DIR/bin/python" ]]; then
    "$PYTHON_BIN" -m venv "$VENV_DIR"
  fi
}

venv_python() {
  ensure_venv
  echo "$VENV_DIR/bin/python"
}

set_hiredis_build_env() {
  export REDIS_WEB_HIREDIS_INCLUDE_DIR="$DIST_HIREDIS_DIR/include"
  export REDIS_WEB_HIREDIS_LIB_DIR="$DIST_HIREDIS_DIR/lib"

  export PKG_CONFIG_PATH="$DIST_HIREDIS_DIR/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
  export CFLAGS="-I$REDIS_WEB_HIREDIS_INCLUDE_DIR${CFLAGS:+ $CFLAGS}"
  export LDFLAGS="-L$REDIS_WEB_HIREDIS_LIB_DIR${LDFLAGS:+ $LDFLAGS}"

  if [[ "$(uname -s)" == "Darwin" ]]; then
    export DYLD_LIBRARY_PATH="$REDIS_WEB_HIREDIS_LIB_DIR${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
  else
    export LD_LIBRARY_PATH="$REDIS_WEB_HIREDIS_LIB_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  fi
}

reset_hiredis_workdir() {
  local work_hiredis_py="$WORK_DIR/hiredis-py"
  rm -rf "$work_hiredis_py"
  mkdir -p "$WORK_DIR"
  cp -R "$HIREDIS_PY_DIR" "$work_hiredis_py"
  rm -rf "$work_hiredis_py/.git" "$work_hiredis_py/vendor/hiredis"
  echo "$work_hiredis_py"
}

apply_hiredis_patch() {
  local work_hiredis_py="$1"
  patch -p1 -d "$work_hiredis_py" < "$PATCH_FILE"
}

latest_hiredis_wheel() {
  ls -1t "$WHEELS_DIR"/hiredis-*.whl 2>/dev/null | head -n 1
}

ensure_dist_dirs() {
  mkdir -p "$DIST_DIR" "$DIST_HIREDIS_DIR" "$WHEELS_DIR" "$ARTIFACTS_DIR"
}
