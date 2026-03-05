#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"

ensure_submodules_present

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required to start managed redis test env" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not running" >&2
  exit 1
fi

compose_running=0
if docker ps --format '{{.Names}}' | rg -qx 'redis-standalone' && \
   docker ps --format '{{.Names}}' | rg -qx 'redis-replica'; then
  compose_running=1
fi

if [[ $compose_running -eq 0 ]] && command -v lsof >/dev/null 2>&1; then
  for port in 6379 6380; do
    if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
      listener="$(lsof -nP -iTCP:"$port" -sTCP:LISTEN | awk 'NR==2{print $1, $2, $9}')"
      echo "port $port is already in use (${listener:-unknown listener})" >&2
      echo "free ports 6379 and 6380, or run with MANAGED_REDIS=0 to use your existing redis setup" >&2
      exit 1
    fi
  done
fi

COMPAT_REDIS_IMAGE_TAG="${COMPAT_REDIS_IMAGE_TAG:-8.4.0}"
COMPAT_REDIS_DOCKER_PLATFORM="${COMPAT_REDIS_DOCKER_PLATFORM:-}"
if [[ -z "$COMPAT_REDIS_DOCKER_PLATFORM" ]]; then
  case "$(uname -m)" in
    arm64|aarch64)
      COMPAT_REDIS_DOCKER_PLATFORM="linux/amd64"
      ;;
    *)
      COMPAT_REDIS_DOCKER_PLATFORM=""
      ;;
  esac
fi

export CLIENT_LIBS_TEST_IMAGE_TAG="$COMPAT_REDIS_IMAGE_TAG"
if [[ -n "$COMPAT_REDIS_DOCKER_PLATFORM" ]]; then
  export DOCKER_DEFAULT_PLATFORM="$COMPAT_REDIS_DOCKER_PLATFORM"
  docker pull --platform "$COMPAT_REDIS_DOCKER_PLATFORM" "redislabs/client-libs-test:$COMPAT_REDIS_IMAGE_TAG" >/dev/null
fi

(
  cd "$REDIS_PY_DIR"
  docker compose --profile replica up -d --pull always redis replica
)

wait_for_redis() {
  local host="$1"
  local port="$2"
  local attempts=60
  for ((i=1; i<=attempts; i++)); do
    if redis-cli -h "$host" -p "$port" PING >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

if ! command -v redis-cli >/dev/null 2>&1; then
  echo "redis-cli is required for readiness checks" >&2
  exit 1
fi

wait_for_redis 127.0.0.1 6379 || {
  echo "redis master failed to become ready on 6379" >&2
  exit 1
}
wait_for_redis 127.0.0.1 6380 || {
  echo "redis replica failed to become ready on 6380" >&2
  exit 1
}

replication_info="$(redis-cli -p 6380 INFO replication | tr -d '\r')"
if ! grep -Eq '^role:(slave|replica)$' <<<"$replication_info"; then
  echo "redis on 6380 is not in replica role" >&2
  echo "$replication_info" >&2
  exit 1
fi

echo "Managed redis test env ready (master:6379 replica:6380 image:$COMPAT_REDIS_IMAGE_TAG platform:${COMPAT_REDIS_DOCKER_PLATFORM:-native})"
