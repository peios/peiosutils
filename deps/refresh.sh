#!/bin/sh
# Re-vendor libpeios's headers + shared object into deps/ (a temporary build
# bridge — see deps/README.md). Builds libpeios release first if needed.
#
# Usage:
#   LIBPEIOS=../libpeios PKM=../pkm ./deps/refresh.sh
# Defaults assume the standard peios workspace layout (siblings of peiosutils).
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
LIBPEIOS="${LIBPEIOS:-$here/../../libpeios}"
PKM="${PKM:-$here/../../pkm}"

[ -d "$LIBPEIOS" ] || { echo "libpeios not found at $LIBPEIOS (set LIBPEIOS=)" >&2; exit 1; }
[ -d "$PKM/uapi/pkm" ] || { echo "pkm uapi not found at $PKM/uapi/pkm (set PKM=)" >&2; exit 1; }

echo "building libpeios (release) ..."
( cd "$LIBPEIOS" && cargo build --release --locked )

so="$LIBPEIOS/target/release/libpeios.so"
[ -f "$so" ] || { echo "missing $so after build" >&2; exit 1; }

echo "vendoring into $here ..."
rm -rf "$here/include" "$here/lib"
mkdir -p "$here/include/peios" "$here/include/pkm" "$here/lib"
cp "$LIBPEIOS/include/peios.h"   "$here/include/"
cp "$LIBPEIOS/include/peios/"*.h "$here/include/peios/"
cp "$PKM/uapi/pkm/"*.h           "$here/include/pkm/"
cp "$so"                          "$here/lib/libpeios.so.0"
ln -sf libpeios.so.0             "$here/lib/libpeios.so"
cp "$LIBPEIOS/target/release/libpeios.a" "$here/lib/"

echo "done. libpeios $(grep -m1 '^version = ' "$LIBPEIOS/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/') vendored."
