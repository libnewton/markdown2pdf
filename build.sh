#!/bin/sh
# Build the WASM engine and install the `md2pdf` Typst package to @local.
set -e
cd "$(dirname "$0")"

echo "[1/3] building engine.wasm ..."
( cd engine && cargo build --release --target wasm32-unknown-unknown )
cp engine/target/wasm32-unknown-unknown/release/md2pdf_engine.wasm package/engine.wasm
echo "      $(du -h package/engine.wasm | cut -f1) -> package/engine.wasm"

echo "[2/3] installing @local/md2pdf:0.1.0 ..."
PKG="${XDG_DATA_HOME:-$HOME/.local/share}/typst/packages/local/md2pdf/0.1.0"
rm -rf "$PKG"
mkdir -p "$PKG"
cp -r package/. "$PKG/"
echo "      -> $PKG"

echo "[3/3] done. Test:  ./bin/md2pdf tests/sample.md"
