#!/usr/bin/env bash
set -euo pipefail

# Import a local dump.rdb into the docker volume used by docker-compose.rdb.yml
# Usage: ./scripts/import-rdb.sh /path/to/dump.rdb

RDB_SRC=${1:-}
VOLUME_DIR=./redis-data

if [[ -z "$RDB_SRC" ]]; then
  echo "Usage: $0 /path/to/dump.rdb"; exit 1
fi

if [[ ! -f "$RDB_SRC" ]]; then
  echo "Provided RDB file does not exist: $RDB_SRC"; exit 2
fi

mkdir -p "$VOLUME_DIR"
cp -v "$RDB_SRC" "$VOLUME_DIR/dump.rdb"
chmod 0644 "$VOLUME_DIR/dump.rdb"
echo "dump.rdb copied to $VOLUME_DIR/dump.rdb"

echo "Starting stack with docker-compose.rdb.yml"
docker compose -f docker-compose.rdb.yml up --build
