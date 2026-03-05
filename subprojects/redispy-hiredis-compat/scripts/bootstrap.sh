#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

"$SCRIPT_DIR/pin-upstreams.sh"
"$SCRIPT_DIR/audit-hiredis-symbols.sh"
"$SCRIPT_DIR/setup-test-env.sh"

echo "Bootstrap complete"
