#!/usr/bin/env sh
set -euo pipefail

# Build a reproducible libtorrent bundle (headers + shared libs) for packaging/CI caches.
# Uses pkg-config to locate libtorrent and its dependencies, then assembles a bundle
# under ./artifacts/libtorrent-bundle-${ARCH} by default. This script is also used
# by the Dockerfile build stage to keep bundle contents consistent.

ARCH="$(uname -m)"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_DIR="${1:-${LIBTORRENT_BUNDLE_DIR:-"$ROOT_DIR/artifacts/libtorrent-bundle-${ARCH}"}}"
LIBTORRENT_PKG="${LIBTORRENT_PKG:-libtorrent-rasterbar}"

mkdir -p "$BUNDLE_DIR"/{include,lib}

include_dir="$(pkg-config --variable=includedir "$LIBTORRENT_PKG")"
lib_dir="$(pkg-config --variable=libdir "$LIBTORRENT_PKG")"

cp -a "$include_dir"/libtorrent "$BUNDLE_DIR/include/"

copy_lib() {
    name="$1"
    if [ -f "$lib_dir/$name" ]; then
        cp -a "$lib_dir"/"$name"* "$BUNDLE_DIR/lib/"
    fi
}

copy_lib "libtorrent-rasterbar.so"
copy_lib "libboost_system.so"
copy_lib "libssl.so"
copy_lib "libcrypto.so"
copy_lib "libstdc++.so"
copy_lib "libgcc_s.so"

tar -czf "${BUNDLE_DIR}.tar.gz" -C "$BUNDLE_DIR" .
echo "Bundle written to ${BUNDLE_DIR}.tar.gz"
