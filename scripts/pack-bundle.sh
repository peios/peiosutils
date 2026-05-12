#!/usr/bin/env bash
# pack-bundle.sh
#
# Produce dist/<pkg>_<version>-1_<arch>.peipkg containing every workspace
# member's release binary, each at the install_path declared in its
# [package.metadata.peios] table.
#
# Package-level metadata (name, version, description) comes from
# [workspace.metadata.peios.package] at the workspace root, read via
# `cargo metadata`'s top-level `.metadata` field.
#
# Honours $CARGO and $PEIPKG_BUILD.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cargo=${CARGO:-cargo}
peipkg_build=${PEIPKG_BUILD:-$root/build/bin/peipkg-build}

[[ -x $peipkg_build ]] || { echo "pack-bundle: peipkg-build missing at $peipkg_build (run 'make peipkg-build')"; exit 1; }
command -v jq >/dev/null || { echo "pack-bundle: jq is required"; exit 1; }

meta=$("$cargo" metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml")

pkg_name=$(jq -r '.metadata.peios.package.name // empty' <<<"$meta")
pkg_version=$(jq -r '.metadata.peios.package.version // empty' <<<"$meta")
pkg_description=$(jq -r '.metadata.peios.package.description // ""' <<<"$meta")
pkg_license=$(jq -r '.metadata.peios.package.license // "MIT"' <<<"$meta")
[[ -n $pkg_name && -n $pkg_version ]] || {
    echo "pack-bundle: [workspace.metadata.peios.package] missing name/version"; exit 1; }

stage=$root/build/stage/$pkg_name
manifest_json=$root/build/manifest/$pkg_name.json
out=$root/dist/${pkg_name}_${pkg_version}-1_x86_64.peipkg
mkdir -p "$(dirname "$out")" "$(dirname "$manifest_json")"

rm -rf "$stage"
mkdir -p "$stage"

# Walk every workspace member, install each binary to its declared path.
# `cargo metadata` lists every workspace member in .packages with their
# manifest_path + metadata.peios + targets.
mapfile -t members < <(jq -c '.packages[] | select(.id as $i | (.id | startswith("path+file://")) and (.metadata.peios != null))' <<<"$meta")

installed=()
for entry in "${members[@]}"; do
    tool_name=$(jq -r '.name' <<<"$entry")
    install_path=$(jq -r '.metadata.peios.install_path // ("bin/" + .name)' <<<"$entry")
    bin_name=$(jq -r '.targets[] | select(.kind[] == "bin") | .name' <<<"$entry" | head -1)

    bin=$root/target/x86_64-unknown-linux-musl/release/$bin_name
    [[ -x $bin ]] || { echo "pack-bundle: binary not built: $bin (run 'make all' first)"; exit 1; }

    dest=$stage/${install_path#/}
    mkdir -p "$(dirname "$dest")"
    install -m 0755 "$bin" "$dest"
    installed+=("$tool_name -> $install_path")
done

echo "staged into $stage:"
printf '  %s\n' "${installed[@]}"

jq -n \
    --arg name "$pkg_name" \
    --arg version "${pkg_version}-1" \
    --arg description "$pkg_description" \
    --arg license "$pkg_license" \
    '{
        schema_version: 1,
        name: $name,
        version: $version,
        architecture: "x86_64",
        description: $description,
        license: $license,
        homepage: "",
        dependencies: [],
        optional_dependencies: [],
        conflicts: [],
        provides: [],
        replaces: [],
        side_effects: [],
        size_installed: 0,
        sd_overrides: [],
        build: {
            timestamp: "2026-05-12T00:00:00Z",
            farm_id: "local-dev",
            source_ref: "local:peiosutils"
        }
    }' > "$manifest_json"

"$peipkg_build" pack \
    --manifest "$manifest_json" \
    --staged "$stage" \
    --out "$out"

echo "packed: $out ($(stat -c%s "$out") bytes)"
