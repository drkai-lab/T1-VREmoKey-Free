#!/bin/sh
# Stage a Linux tarball from the release binaries in rust/target/release.
set -eu
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
name="vrc-expression-hotkeys-linux-${version}"
stage=$(mktemp -d)
dest="$stage/$name"
mkdir -p "$dest/icons"
install -m 755 target/release/vrc-expression-hotkeys "$dest/vrc-expression-hotkeys"
install -m 755 target/release/vrc-expression-hotkeys-gui "$dest/vrc-expression-hotkeys-gui"
install -m 755 packaging/install.sh packaging/uninstall.sh "$dest/"
install -m 644 packaging/vrc-expression-hotkeys.desktop "$dest/"
install -m 644 packaging/vrc-expression-hotkeys.service "$dest/"
install -m 644 packaging/99-vrc-expression-hotkeys.rules "$dest/"
install -m 644 assets/app-64.png "$dest/icons/app-64.png"
install -m 644 ../LICENSE "$dest/LICENSE"
install -m 644 assets/OFL "$dest/OFL"
mkdir -p ../dist
tar -C "$stage" -czf "../dist/${name}.tar.gz" "$name"
(cd ../dist && sha256sum "${name}.tar.gz" > "${name}.tar.gz.sha256")
rm -rf "$stage"
echo "../dist/${name}.tar.gz"
