#!/bin/sh
# Build the WASM engine and install the `md2pdf` Typst package to @local.
#
#   ./build.sh           engine + install
#   ./build.sh engine    only rebuild package/engine.wasm
#   ./build.sh install   only install package/ as @local/md2pdf:0.1.0
set -e
cd "$(dirname "$0")"

build_engine() {
	echo "building engine.wasm ..."
	# The toolchain is pinned in engine/rust-toolchain.toml. Remapping the
	# machine-specific paths makes the build reproducible, so CI can check
	# that the committed engine.wasm matches the source.
	cargo_home="${CARGO_HOME:-$HOME/.cargo}"
	(
		cd engine
		RUSTFLAGS="--remap-path-prefix=$PWD=/engine --remap-path-prefix=$cargo_home=/cargo" \
			cargo build --release --target wasm32-unknown-unknown
	)
	cp engine/target/wasm32-unknown-unknown/release/md2pdf_engine.wasm package/engine.wasm
	echo "  $(du -h package/engine.wasm | cut -f1) -> package/engine.wasm"
}

install_package() {
	pkg="${XDG_DATA_HOME:-$HOME/.local/share}/typst/packages/local/md2pdf/0.1.0"
	echo "installing @local/md2pdf:0.1.0 ..."
	rm -rf "$pkg"
	mkdir -p "$pkg"
	cp -r package/. "$pkg/"
	echo "  -> $pkg"
}

case "${1:-all}" in
engine) build_engine ;;
install) install_package ;;
all)
	build_engine
	install_package
	echo "done. Test:  ./bin/md2pdf tests/sample.md"
	;;
*)
	echo "usage: $0 [engine|install]" >&2
	exit 2
	;;
esac
