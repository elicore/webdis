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

(
  cd "$work_hiredis_py"
  "$PYTHON" setup.py build_ext --inplace
)

EXT_PATH="$(find "$work_hiredis_py/hiredis" -maxdepth 1 -type f \( -name '*.so' -o -name '*.dylib' \) | head -n 1)"
if [[ -z "$EXT_PATH" ]]; then
  echo "failed to locate built hiredis extension artifact" >&2
  exit 1
fi

LIBHIREDIS_PATH=""
if [[ -f "$DIST_HIREDIS_DIR/lib/libhiredis.dylib" ]]; then
  LIBHIREDIS_PATH="$DIST_HIREDIS_DIR/lib/libhiredis.dylib"
elif [[ -f "$DIST_HIREDIS_DIR/lib/libhiredis.so" ]]; then
  LIBHIREDIS_PATH="$DIST_HIREDIS_DIR/lib/libhiredis.so"
else
  echo "failed to locate compat libhiredis shared library in $DIST_HIREDIS_DIR/lib" >&2
  exit 1
fi

REQUIRED_SYMS_FILE="$ARTIFACTS_DIR/hiredis-required-symbols.txt"
PROVIDED_SYMS_FILE="$ARTIFACTS_DIR/hiredis-provided-symbols.txt"
MISSING_SYMS_FILE="$ARTIFACTS_DIR/hiredis-missing-symbols.txt"
REPORT_FILE="$ARTIFACTS_DIR/symbol-audit.txt"

if [[ "$(uname -s)" == "Darwin" ]]; then
  nm -gU "$EXT_PATH" | awk '/ U /{print $2}' | sed 's/^_//' | sort -u > "$REQUIRED_SYMS_FILE"
  nm -gU "$LIBHIREDIS_PATH" | awk '{print $3}' | sed 's/^_//' | sed '/^$/d' | sort -u > "$PROVIDED_SYMS_FILE"
else
  nm -D --undefined-only "$EXT_PATH" | awk '{print $2}' | sed 's/^_//' | sort -u > "$REQUIRED_SYMS_FILE"
  nm -D --defined-only "$LIBHIREDIS_PATH" | awk '{print $3}' | sed 's/^_//' | sed '/^$/d' | sort -u > "$PROVIDED_SYMS_FILE"
fi

grep -E '^(redis|sds|hi_|hiredis)' "$REQUIRED_SYMS_FILE" > "$REQUIRED_SYMS_FILE.filtered" || true
mv "$REQUIRED_SYMS_FILE.filtered" "$REQUIRED_SYMS_FILE"

comm -23 "$REQUIRED_SYMS_FILE" "$PROVIDED_SYMS_FILE" > "$MISSING_SYMS_FILE"

{
  echo "compat library: $LIBHIREDIS_PATH"
  echo "extension: $EXT_PATH"
  echo "required symbols file: $REQUIRED_SYMS_FILE"
  echo "provided symbols file: $PROVIDED_SYMS_FILE"
  echo "missing symbols file: $MISSING_SYMS_FILE"
} > "$REPORT_FILE"

if [[ -s "$MISSING_SYMS_FILE" ]]; then
  cat "$REPORT_FILE"
  echo
  echo "Missing compat symbols:"
  cat "$MISSING_SYMS_FILE"
  exit 1
fi

echo "Symbol audit passed"
cat "$REPORT_FILE"
