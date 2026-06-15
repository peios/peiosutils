#!/usr/bin/env bash
set -euo pipefail

here="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
mkuki_dir="$(CDPATH= cd -- "$here/.." && pwd)"
stubs_dir="$mkuki_dir/stubs"

package_name="systemd-boot-efi"
package_version="257.13-1~deb13u1"
arch="amd64"
deb_name="${package_name}_${package_version}_${arch}.deb"
deb_url="https://deb.debian.org/debian/pool/main/s/systemd/${deb_name}"
deb_sha256="390ecdcef9bbb753f51bb6d8f696bdae2f1dbedde7239c49ccb2512f137a8933"
stub_path="./usr/lib/systemd/boot/efi/linuxx64.efi.stub"
copyright_path="./usr/share/doc/systemd-boot-efi/copyright"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$stubs_dir"

echo "fetching $deb_url"
curl --fail --location --show-error --silent --output "$tmp/$deb_name" "$deb_url"
printf '%s  %s\n' "$deb_sha256" "$tmp/$deb_name" | sha256sum --check --status

(
    cd "$tmp"
    ar x "$deb_name"
    data_member="$(printf '%s\n' data.tar.* | head -n 1)"
    case "$data_member" in
        *.zst) zstd -dc "$data_member" | tar -x -f - "$stub_path" "$copyright_path" ;;
        *.xz) xz -dc "$data_member" | tar -x -f - "$stub_path" "$copyright_path" ;;
        *.gz) tar -x -z -f "$data_member" "$stub_path" "$copyright_path" ;;
        *) echo "unsupported Debian data archive: $data_member" >&2; exit 1 ;;
    esac
)

install -m 0644 "$tmp/$stub_path" "$stubs_dir/linuxx64.efi.stub"
install -m 0644 "$tmp/$copyright_path" "$stubs_dir/COPYRIGHT.debian-systemd"

stub_sha256="$(sha256sum "$stubs_dir/linuxx64.efi.stub" | awk '{print $1}')"
cat > "$stubs_dir/SOURCE.systemd-stub" <<EOF
default stub: systemd-stub linuxx64.efi.stub
upstream: systemd
upstream project: https://github.com/systemd/systemd
debian package: ${package_name}
debian version: ${package_version}
debian architecture: ${arch}
debian package url: ${deb_url}
debian package sha256: ${deb_sha256}
embedded stub sha256: ${stub_sha256}
license: LGPL-2.1-or-later
license/copyright details: src/uu/mkuki/stubs/COPYRIGHT.debian-systemd
refresh script: src/uu/mkuki/tools/update-systemd-stub.sh
EOF

cat > "$stubs_dir/SHA256SUMS" <<EOF
${stub_sha256}  linuxx64.efi.stub
EOF

echo "wrote $stubs_dir/linuxx64.efi.stub"
echo "stub sha256: $stub_sha256"
