#!/usr/bin/env bash
# build-tool.sh <tool-name>
#
# Build one workspace member's release binary against the musl target.
# Honours $CARGO and $RUSTC if set (Makefile uses these to pick a rustup
# toolchain instead of a nix shim).
set -euo pipefail

tool=${1:?usage: build-tool.sh <tool-name>}
root=$(cd "$(dirname "$0")/.." && pwd)
cargo=${CARGO:-cargo}

cd "$root"
exec "$cargo" build --release -p "$tool"
