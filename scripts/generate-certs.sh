#!/usr/bin/env bash
set -euo pipefail

# Simple idempotent cert generation for Redis <-> Webdis TLS for dev/test
# Usage: ./scripts/generate-certs.sh --outdir ./certs --cn redis.local

OUT_DIR=./certs
CN=${CN:-redis.local}
SAN=${SAN:-"DNS:redis,IP:127.0.0.1"}

while [[ "$#" -gt 0 ]]; do
  case $1 in
    --outdir) OUT_DIR=$2; shift 2;;
    --cn) CN=$2; shift 2;;
    --san) SAN=$2; shift 2;;
    -h|--help) echo "usage: $0 [--outdir ./certs] [--cn redis.local] [--san 'DNS:redis,IP:127.0.0.1']"; exit 0;;
    *) echo "Unknown arg $1"; exit 1;;
  esac
done

mkdir -p "$OUT_DIR"
pushd "$OUT_DIR" >/dev/null

if [[ ! -f ca.key || ! -f ca.crt ]]; then
  echo "Generating CA..."
  openssl genrsa -out ca.key 4096
  openssl req -x509 -new -nodes -key ca.key -sha256 -days 3650 -subj "/CN=webdis CA" -out ca.crt
fi

echo "Generating Redis server key and cert..."
if [[ ! -f redis.key || ! -f redis.csr || ! -f redis.crt ]]; then
  openssl genrsa -out redis.key 2048
  openssl req -new -key redis.key -subj "/CN=$CN" -out redis.csr
  openssl x509 -req -in redis.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out redis.crt -days 365 -sha256 -extfile <(printf "subjectAltName=%s" "$SAN")
fi

echo "Generating client key and cert..."
if [[ ! -f client.key || ! -f client.csr || ! -f client.crt ]]; then
  openssl genrsa -out client.key 2048
  openssl req -new -key client.key -subj "/CN=webdis-client" -out client.csr
  openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out client.crt -days 365 -sha256
fi

chmod 0444 *.crt || true
chmod 0400 *.key || true

popd >/dev/null
echo "Certificates generated in $OUT_DIR"
