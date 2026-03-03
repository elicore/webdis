#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/crates/redis-web-hiredis-compat"
TARGET_DIR="$ROOT_DIR/target/release"
OUT_DIR="${1:-$ROOT_DIR/target/hiredis-compat-dist}"

mkdir -p "$OUT_DIR/lib" "$OUT_DIR/include/hiredis" "$OUT_DIR/pkgconfig"

cargo build -p redis-web-hiredis-compat --release

cp "$CRATE_DIR/include/hiredis/hiredis.h" "$OUT_DIR/include/hiredis/hiredis.h"
cp "$CRATE_DIR/pkgconfig/hiredis.pc" "$OUT_DIR/pkgconfig/hiredis.pc"
cp "$CRATE_DIR/pkgconfig/redisweb-hiredis.pc" "$OUT_DIR/pkgconfig/redisweb-hiredis.pc"

if [[ -f "$TARGET_DIR/libredisweb_hiredis.a" ]]; then
  cp "$TARGET_DIR/libredisweb_hiredis.a" "$OUT_DIR/lib/"
  cp "$TARGET_DIR/libredisweb_hiredis.a" "$OUT_DIR/lib/libhiredis.a"
fi

if [[ "$OSTYPE" == darwin* ]]; then
  if [[ -f "$TARGET_DIR/libredisweb_hiredis.dylib" ]]; then
    cp "$TARGET_DIR/libredisweb_hiredis.dylib" "$OUT_DIR/lib/"
    cp "$TARGET_DIR/libredisweb_hiredis.dylib" "$OUT_DIR/lib/libhiredis.dylib"
  fi
else
  if [[ -f "$TARGET_DIR/libredisweb_hiredis.so" ]]; then
    cp "$TARGET_DIR/libredisweb_hiredis.so" "$OUT_DIR/lib/"
    cp "$TARGET_DIR/libredisweb_hiredis.so" "$OUT_DIR/lib/libhiredis.so"
  fi
fi

echo "Built hiredis compatibility artifacts in: $OUT_DIR"
