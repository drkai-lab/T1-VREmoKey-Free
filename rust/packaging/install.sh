#!/bin/sh
# Install the free-edition binaries from this directory.
# --autostart      headless listener at login (systemd user service)
# --autostart-gui  window at login (XDG autostart)
# Those two options cannot be combined.
set -eu

prefix="${HOME}/.local"
autostart=""
autostart_gui=""

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            prefix="$2"
            shift 2
            ;;
        --autostart)
            autostart=1
            shift
            ;;
        --autostart-gui)
            autostart_gui=1
            shift
            ;;
        *)
            echo "unknown option: $1" >&2
            exit 1
            ;;
    esac
done

if [ -n "$autostart" ] && [ -n "$autostart_gui" ]; then
    echo "choose either --autostart or --autostart-gui" >&2
    exit 1
fi

here=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
install -d "$prefix/bin" "$prefix/share/applications" \
    "$prefix/share/icons/hicolor/64x64/apps"
install -m 755 "$here/vrc-expression-hotkeys" "$prefix/bin/vrc-expression-hotkeys"
if [ -f "$here/vrc-expression-hotkeys-gui" ]; then
    install -m 755 "$here/vrc-expression-hotkeys-gui" "$prefix/bin/vrc-expression-hotkeys-gui"
else
    install -m 755 "$here/vrc-expression-hotkeys" "$prefix/bin/vrc-expression-hotkeys-gui"
fi
sed "s|^Exec=.*|Exec=$prefix/bin/vrc-expression-hotkeys-gui|" \
    "$here/vrc-expression-hotkeys.desktop" > "$prefix/share/applications/vrc-expression-hotkeys.desktop"
if [ -f "$here/icons/app-64.png" ]; then
    install -m 644 "$here/icons/app-64.png" \
        "$prefix/share/icons/hicolor/64x64/apps/vrc-expression-hotkeys.png"
fi

if [ "$(id -u)" -eq 0 ] && [ -f "$here/99-vrc-expression-hotkeys.rules" ]; then
    install -d /usr/lib/udev/rules.d
    install -m 644 "$here/99-vrc-expression-hotkeys.rules" \
        /usr/lib/udev/rules.d/99-vrc-expression-hotkeys.rules
    if command -v udevadm >/dev/null 2>&1; then
        udevadm control --reload-rules || true
        udevadm trigger --subsystem-match=input || true
    fi
elif [ -f "$here/99-vrc-expression-hotkeys.rules" ]; then
    echo "To let your user read keyboards, install the bundled rules as root:"
    echo "  sudo install -m 644 $here/99-vrc-expression-hotkeys.rules /usr/lib/udev/rules.d/"
    echo "  sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=input"
    echo "Then add your user to the input group and log in again."
fi

unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
autostart_dir="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
if [ -n "$autostart" ]; then
    install -d "$unit_dir"
    rm -f "$autostart_dir/vrc-expression-hotkeys.desktop"
    cat > "$unit_dir/vrc-expression-hotkeys.service" <<EOF
[Unit]
Description=VRChat Expression Hotkeys (Shift+F1..F4)

[Service]
ExecStart=$prefix/bin/vrc-expression-hotkeys --listen
Restart=on-failure

[Install]
WantedBy=default.target
EOF
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user daemon-reload
        systemctl --user enable --now vrc-expression-hotkeys.service
    fi
fi
if [ -n "$autostart_gui" ]; then
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user disable --now vrc-expression-hotkeys.service >/dev/null 2>&1 || true
    fi
    install -d "$autostart_dir"
    sed "s|^Exec=.*|Exec=$prefix/bin/vrc-expression-hotkeys-gui|" \
        "$here/vrc-expression-hotkeys.desktop" > "$autostart_dir/vrc-expression-hotkeys.desktop"
fi

echo "Installed into $prefix/bin"
