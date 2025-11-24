#!/usr/bin/env bash
set -euo pipefail

# start-webdis.sh
# Simple helper to run a webdis container or start the local dev compose stack
# Usage: ./scripts/start-webdis.sh --mode dev|run --tag webdis:dev

MODE=dev
TAG=webdis:dev
PORT=7379
CONFIG=${CONFIG:-./webdis.json}

while [[ "$#" -gt 0 ]]; do
  case $1 in
    --mode) MODE=$2; shift 2;;
    --tag) TAG=$2; shift 2;;
    --port) PORT=$2; shift 2;;
    --config) CONFIG=$2; shift 2;;
    -h|--help) echo "usage: $0 --mode dev|run --tag image:tag --port 7379 --config ./webdis.json"; exit 0;;
    *) echo "Unknown arg $1"; exit 1;;
  esac
done

if [[ "$MODE" == "dev" ]]; then
  echo "Starting dev compose stack (docker compose -f docker-compose.dev.yml up --build)"
  docker compose -f docker-compose.dev.yml up --build
elif [[ "$MODE" == "run" ]]; then
  echo "Running docker run for $TAG with config $CONFIG"
  docker run --rm -it -p "${PORT}:7379" -v "$(pwd)/${CONFIG}":/etc/webdis.json:ro --name webdis-run "${TAG}"
else
  echo "Unknown mode $MODE"; exit 1
fi
