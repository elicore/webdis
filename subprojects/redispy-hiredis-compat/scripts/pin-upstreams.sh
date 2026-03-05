#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

ensure_submodules_present

REDIS_PY_TAG="${REDIS_PY_TAG:-v7.2.1}"
HIREDIS_PY_TAG="${HIREDIS_PY_TAG:-v3.3.0}"

pin_repo() {
  local repo_dir="$1"
  local tag="$2"

  git -C "$repo_dir" fetch --tags
  git -C "$repo_dir" checkout -q "$tag"
}

pin_repo "$REDIS_PY_DIR" "$REDIS_PY_TAG"
pin_repo "$HIREDIS_PY_DIR" "$HIREDIS_PY_TAG"

REDIS_PY_SHA="$(git -C "$REDIS_PY_DIR" rev-parse HEAD)"
HIREDIS_PY_SHA="$(git -C "$HIREDIS_PY_DIR" rev-parse HEAD)"
UPDATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat > "$VERSIONS_LOCK" <<LOCK
updated_at = "$UPDATED_AT"

[redis_py]
repo = "https://github.com/redis/redis-py.git"
tag = "$REDIS_PY_TAG"
sha = "$REDIS_PY_SHA"

[hiredis_py]
repo = "https://github.com/redis/hiredis-py.git"
tag = "$HIREDIS_PY_TAG"
sha = "$HIREDIS_PY_SHA"
LOCK

echo "Pinned redis-py:    $REDIS_PY_TAG ($REDIS_PY_SHA)"
echo "Pinned hiredis-py: $HIREDIS_PY_TAG ($HIREDIS_PY_SHA)"
echo "Wrote lockfile: $VERSIONS_LOCK"
