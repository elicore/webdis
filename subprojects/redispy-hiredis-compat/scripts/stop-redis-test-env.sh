#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

if ! command -v docker >/dev/null 2>&1; then
  exit 0
fi

if ! docker info >/dev/null 2>&1; then
  exit 0
fi

(
  cd "$REDIS_PY_DIR"
  docker compose --profile replica down -v --remove-orphans
)

echo "Managed redis test env stopped"
