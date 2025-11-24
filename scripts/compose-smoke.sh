#!/usr/bin/env bash
set -euo pipefail

echo "Starting dev compose stack (smoke test)..."
docker compose -f docker-compose.dev.yml up --build -d

echo "Waiting for webdis to be healthy..."
for i in $(seq 1 30); do
  if curl -sSf http://127.0.0.1:7379/PING >/dev/null 2>&1; then
    echo "Webdis responded to PING"
    break
  fi
  if [ "$i" -eq 30 ]; then
    echo "Timed out waiting for Webdis; showing logs for debugging..."
    docker compose -f docker-compose.dev.yml logs --no-color --tail 100
    docker compose -f docker-compose.dev.yml down -v
    exit 1
  fi
  sleep 1
done

echo "Stopping compose..."
docker compose -f docker-compose.dev.yml down -v

echo "Smoke test completed"
