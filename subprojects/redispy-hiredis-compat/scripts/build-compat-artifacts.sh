#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

ensure_dist_dirs

"$REPO_ROOT/scripts/build-hiredis-compat.sh" "$DIST_HIREDIS_DIR"

cat > "$DIST_HIREDIS_DIR/env.sh" <<ENV
export REDIS_WEB_HIREDIS_INCLUDE_DIR="$DIST_HIREDIS_DIR/include"
export REDIS_WEB_HIREDIS_LIB_DIR="$DIST_HIREDIS_DIR/lib"
export PKG_CONFIG_PATH="$DIST_HIREDIS_DIR/pkgconfig\${PKG_CONFIG_PATH:+:\$PKG_CONFIG_PATH}"
export CFLAGS="-I$DIST_HIREDIS_DIR/include\${CFLAGS:+ \$CFLAGS}"
export LDFLAGS="-L$DIST_HIREDIS_DIR/lib\${LDFLAGS:+ \$LDFLAGS}"
ENV

if [[ "$(uname -s)" == "Darwin" ]]; then
  cat >> "$DIST_HIREDIS_DIR/env.sh" <<ENV
export DYLD_LIBRARY_PATH="$DIST_HIREDIS_DIR/lib\${DYLD_LIBRARY_PATH:+:\$DYLD_LIBRARY_PATH}"
ENV
else
  cat >> "$DIST_HIREDIS_DIR/env.sh" <<ENV
export LD_LIBRARY_PATH="$DIST_HIREDIS_DIR/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
ENV
fi

echo "Compat artifacts staged at: $DIST_HIREDIS_DIR"
echo "Build environment exports written to: $DIST_HIREDIS_DIR/env.sh"
