#!/bin/sh
# Build an Arch .pkg.tar.zst from the release binaries. makepkg is not required.
set -eu
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
pkg="vrc-expression-hotkeys-${version}-1-x86_64"
stage=$(mktemp -d)
root="$stage/root"
mkdir -p "$root/usr/bin" \
    "$root/usr/share/applications" \
    "$root/usr/lib/systemd/user" \
    "$root/usr/lib/udev/rules.d" \
    "$root/usr/share/licenses/vrc-expression-hotkeys" \
    "$root/usr/share/icons/hicolor/64x64/apps"
install -m 755 target/release/vrc-expression-hotkeys "$root/usr/bin/"
install -m 755 target/release/vrc-expression-hotkeys-gui "$root/usr/bin/"
install -m 644 packaging/vrc-expression-hotkeys.desktop "$root/usr/share/applications/"
install -m 644 packaging/vrc-expression-hotkeys.service "$root/usr/lib/systemd/user/"
install -m 644 packaging/99-vrc-expression-hotkeys.rules "$root/usr/lib/udev/rules.d/"
install -m 644 assets/app-64.png "$root/usr/share/icons/hicolor/64x64/apps/vrc-expression-hotkeys.png"
install -m 644 ../LICENSE "$root/usr/share/licenses/vrc-expression-hotkeys/LICENSE"
size=$(du -sb "$root" | awk '{print $1}')
cat > "$root/.PKGINFO" <<EOF
pkgname = vrc-expression-hotkeys
pkgbase = vrc-expression-hotkeys
pkgver = ${version}-1
pkgdesc = Free VRChat expression hotkeys (Shift+F1 through F4)
url = https://github.com/drkai-lab/T1-VREmoKey-Free
builddate = $(date +%s)
packager = drkai-lab <drkai-lab@users.noreply.github.com>
size = ${size}
arch = x86_64
license = MIT
EOF
mkdir -p ../dist
tar --zstd -C "$root" -cf "../dist/${pkg}.pkg.tar.zst" .PKGINFO usr
rm -rf "$stage"
echo "../dist/${pkg}.pkg.tar.zst"
