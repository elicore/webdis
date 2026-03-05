#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="$ROOT_DIR/crates/redis-web-hiredis-compat"
TARGET_DIR="$ROOT_DIR/target/release"
OUT_DIR="${1:-$ROOT_DIR/target/hiredis-compat-dist}"
UPSTREAM_DIR="$ROOT_DIR/subprojects/redispy-hiredis-compat/vendor/hiredis-py/vendor/hiredis"
BUILD_DIR="$OUT_DIR/.build"
CC_BIN="${CC:-cc}"

mkdir -p "$OUT_DIR/lib" "$OUT_DIR/include/hiredis" "$OUT_DIR/pkgconfig" "$BUILD_DIR"

cargo build -p redis-web-hiredis-compat --release

cp "$CRATE_DIR/include/hiredis/"*.h "$OUT_DIR/include/hiredis/"
cp "$CRATE_DIR/pkgconfig/hiredis.pc" "$OUT_DIR/pkgconfig/hiredis.pc"
cp "$CRATE_DIR/pkgconfig/redisweb-hiredis.pc" "$OUT_DIR/pkgconfig/redisweb-hiredis.pc"

if [[ ! -d "$UPSTREAM_DIR" ]]; then
  echo "missing upstream hiredis sources: $UPSTREAM_DIR" >&2
  echo "run: make compat_redispy_bootstrap" >&2
  exit 1
fi

# Build upstream hiredis core + async runtime as the staged compat artifacts.
sources=(
  alloc.c
  dict.c
  hiredis.c
  net.c
  read.c
  sds.c
  sockcompat.c
  async.c
)
objects=()
for src in "${sources[@]}"; do
  obj="$BUILD_DIR/${src%.c}.o"
  "$CC_BIN" -O2 -fPIC -I"$UPSTREAM_DIR" -c "$UPSTREAM_DIR/$src" -o "$obj"
  objects+=("$obj")
done

ar rcs "$OUT_DIR/lib/libhiredis.a" "${objects[@]}"
cp "$OUT_DIR/lib/libhiredis.a" "$OUT_DIR/lib/libredisweb_hiredis.a"

if [[ "$OSTYPE" == darwin* ]]; then
  "$CC_BIN" -dynamiclib -o "$OUT_DIR/lib/libhiredis.dylib" "${objects[@]}"
  cp "$OUT_DIR/lib/libhiredis.dylib" "$OUT_DIR/lib/libredisweb_hiredis.dylib"
else
  "$CC_BIN" -shared -Wl,-soname,libhiredis.so.1 -o "$OUT_DIR/lib/libhiredis.so" "${objects[@]}"
  cp "$OUT_DIR/lib/libhiredis.so" "$OUT_DIR/lib/libredisweb_hiredis.so"
fi

echo "Built hiredis compatibility artifacts in: $OUT_DIR"
