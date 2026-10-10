#!/bin/sh
set -eu
prefix="${HOME}/.local"
if [ "${1:-}" = "--prefix" ]; then
    prefix="$2"
fi
unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
autostart_dir="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user disable --now vrc-expression-hotkeys.service >/dev/null 2>&1 || true
fi
rm -f "$prefix/bin/vrc-expression-hotkeys" \
    "$prefix/bin/vrc-expression-hotkeys-gui" \
    "$prefix/share/applications/vrc-expression-hotkeys.desktop" \
    "$prefix/share/icons/hicolor/64x64/apps/vrc-expression-hotkeys.png" \
    "$unit_dir/vrc-expression-hotkeys.service" \
    "$autostart_dir/vrc-expression-hotkeys.desktop"
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload >/dev/null 2>&1 || true
fi
echo "Removed the user install under $prefix"
echo "A system package, if you installed one, is removed with the package manager."
