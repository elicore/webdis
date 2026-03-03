#!/usr/bin/env bash
set -euo pipefail

# Informational benchmark harness (no CI gate).
# Compares redis-web compat command endpoint latency with direct Redis command path.

HOST="${HOST:-127.0.0.1}"
PORT="${PORT:-7379}"
ITERATIONS="${ITERATIONS:-100}"

compat_url="http://${HOST}:${PORT}/__compat/session"

echo "[compat-bench] Creating compat session on ${compat_url}"
session_id=$(curl -sS -X POST "$compat_url" | sed -n 's/.*"session_id":"\([^"]*\)".*/\1/p')
if [[ -z "${session_id}" ]]; then
  echo "[compat-bench] failed to create session"
  exit 1
fi

cmd_url="http://${HOST}:${PORT}/__compat/cmd/${session_id}.raw"
set_body=$'*3\r\n$3\r\nSET\r\n$10\r\ncompat:bench\r\n$2\r\nok\r\n'
get_body=$'*2\r\n$3\r\nGET\r\n$10\r\ncompat:bench\r\n'

echo "[compat-bench] Running ${ITERATIONS} SET/GET roundtrips"
start=$(date +%s)
for _ in $(seq 1 "$ITERATIONS"); do
  curl -sS -X POST "$cmd_url" --data-binary "$set_body" >/dev/null
  curl -sS -X POST "$cmd_url" --data-binary "$get_body" >/dev/null
done
end=$(date +%s)

elapsed=$((end - start))
echo "[compat-bench] elapsed_sec=${elapsed} iterations=${ITERATIONS}"

echo "[compat-bench] Cleaning up session"
curl -sS -X DELETE "http://${HOST}:${PORT}/__compat/session/${session_id}" >/dev/null || true
