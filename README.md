# T1-VREmoKey-Free

VRChat expression hotkeys (free edition of **T1-VREmoKey**).

English | [日本語](README.ja.md)

> 日本語の説明は [README.ja.md](README.ja.md) にあります。

Free edition. Hold Shift and press F1 through F4 to toggle a VRChat
expression. This edition has four expressions. Each avatar can have its
own names and preview images. This repository does not talk to keyboard
hardware and does not write firmware.

The full edition, **T1-VREmoKey**, is for the T1 mini keyboard. It supports
12 expressions, assigning them to the keyboard's own buttons, cycling them
with the dial, and writing the keyboard's settings. It is a separate project.

The command name stays `vrc-expression-hotkeys`, and settings stay in the same directories, so an existing setup keeps working. Version 2.0.1 is the Rust app. Its window title and tray name are **T1-VREmoKey-Free**. Release builds strip symbols and use link-time optimization. This public build is not obfuscated.

## What it does

1. In Unity, open `Tools > VRChat Expression Hotkeys > Avatar Profile`. Name
   each expression, generate 256×256 face previews (the avatar must be open
   in the scene), and export a folder of `profile.json` plus `thumbs/*.png`.
   The editor tools use reflection, so a project without the VRChat SDK
   still compiles. Zip that folder if you want a single file.
2. In this app, import the pack. Pick the avatar in the dropdown, or let
   VRChat switch it. VRChat sends `/avatar/change` to `127.0.0.1:9001` by
   default. The listen port is configurable. If that port is already taken
   the app keeps running and says so; forward `/avatar/change` with any OSC
   router onto the port you chose.
3. Assign each of F1..F4 to an expression. The row shows the expression
   number, the avatar's name, and the thumbnail. A missing PNG shows a
   generic icon and `Expression N`. Shift+F5..F12 do nothing.
4. Leave the app running. Shift+Fn sends that expression's OSC bool: TRUE
   on press, FALSE on release. Repeats are ignored. By default this happens
   only while VRChat is the foreground app.

The built-in Default profile is Expression1..4. An imported profile may
list more than four expressions. Indexes above 4 are ignored, and the
window says so.

The chord listener runs on Windows (a low-level keyboard hook that does not
swallow keys) and Linux (evdev, without grabbing the device). macOS can edit
profiles and assignments; it does not listen for keys.

On Linux, if no keyboard is readable, add your user to the `input` group and
log in again. Keys still reach the desktop.

Closing the window leaves the app in the system tray. The menu is **Open window**, **Start listening** / **Stop listening**, and **Quit**. With the language set to Japanese those items are 画面を開く, 待受の開始, 待受の停止, and 終了. **Quit** is what exits.

## Download

Get the latest files from the [Releases page](https://github.com/drkai-lab/T1-VREmoKey-Free/releases/latest).

| OS | File |
| --- | --- |
| Windows (recommended) | `vrc-expression-hotkeys-<version>-setup.exe` (installer) |
| Windows (no install) | `vrc-expression-hotkeys-gui.exe` (window) / `vrc-expression-hotkeys.exe` (console) |
| Debian / Ubuntu | `vrc-expression-hotkeys_<version>-1_amd64.deb` |
| Arch | `vrc-expression-hotkeys-<version>-1-x86_64.pkg.tar.zst` |
| Linux (tarball) | `vrc-expression-hotkeys-linux-<version>.tar.gz` (+ `.sha256`) |
| Unity | `com.drkai-lab.vrc-expression-hotkeys-<version>.tgz` |

## Install on Windows (installer)

1. Download `vrc-expression-hotkeys-<version>-setup.exe` from the [Releases page](https://github.com/drkai-lab/T1-VREmoKey-Free/releases/latest) and run it.
2. The binaries are unsigned, so SmartScreen may warn you. Click **More info**, then **Run anyway**.
3. Optional tasks during setup (both off by default):
   - **Create a desktop icon**
   - **Start at logon**: starts the app automatically when you sign in to Windows.
4. Start **T1-VREmoKey-Free** from the Start menu. You can close the window; the app stays in the tray until you choose **Quit**.
5. Uninstall from **Settings > Apps > Installed apps**, or with the Start menu uninstall shortcut.

Without installing, download `vrc-expression-hotkeys-gui.exe` and run it directly.

## Install on Linux

On Wayland (Hyprland / Omarchy) the VRChat-only filter uses `hyprctl`; on X11 it uses `xdotool` or `xprop`. Reading keyboards needs membership in the `input` group (`sudo usermod -aG input "$USER"`, then log in again). The package also ships an input-device rule.

Debian or Ubuntu:

```bash
sudo apt install ./vrc-expression-hotkeys_<version>-1_amd64.deb
vrc-expression-hotkeys-gui
vrc-expression-hotkeys --listen
```

Arch Linux, from the release package (a `PKGBUILD` is also in `rust/packaging/`):

```bash
sudo pacman -U vrc-expression-hotkeys-<version>-1-x86_64.pkg.tar.zst
```

From the tarball:

```bash
sha256sum -c vrc-expression-hotkeys-linux-<version>.tar.gz.sha256
tar xzf vrc-expression-hotkeys-linux-<version>.tar.gz
cd vrc-expression-hotkeys-linux-<version>
./install.sh                  # into ~/.local
./install.sh --autostart      # headless listener at login
./install.sh --autostart-gui  # window at login
```

`--autostart` and `--autostart-gui` cannot be used together. The headless option is a systemd user service. The window option is an XDG autostart entry. Uninstall with `./uninstall.sh`.

A `.deb` install can enable the same headless service with `systemctl --user enable --now vrc-expression-hotkeys.service`.

## Run from a checkout

```bash
cd rust
cargo run --release --bin vrc-expression-hotkeys-gui
cargo run --release --bin vrc-expression-hotkeys -- --listen
```

The Python 1.0 script is still in the repository:

```bash
python3 vrc-expression-hotkeys.py
python3 vrc-expression-hotkeys.py --listen
```

Settings live in the app config directory (`~/.config/vrc-expression-hotkeys`
on Linux, `%APPDATA%\vrc-expression-hotkeys` on Windows). Profiles are
`avatars/<id>/profile.json` and `thumbs/*.png` under that directory.

## Manual checklist

- [ ] Export a profile from Unity and import it. Names and thumbnails show on F1..F4.
- [ ] Import a profile that also has expression 5 or higher. Those rows are skipped, and a notice says so. Shift+F5 does nothing.
- [ ] Assign F3, focus VRChat, hold Shift+F3. The expression turns on. Release sends false.
- [ ] The same chord does nothing while another app is focused.
- [ ] Send `/avatar/change` with the avatar id to the listen port and confirm the dropdown switches.
- [ ] With the listen port already taken, confirm the busy message and that the window stays open.
- [ ] An expression with no PNG shows Expression N.

## Unity package

The editor tools live in `unity/` (F1 through F4 only). Two ways to add them in Unity. See also [unity/README.md](unity/README.md).

### A. From the release tarball

1. Download `com.drkai-lab.vrc-expression-hotkeys-<ver>.tgz` from the [Releases page](https://github.com/drkai-lab/T1-VREmoKey-Free/releases/latest).
2. In Unity, open **Window > Package Manager**.
3. Click **+**.
4. Choose **Add package from tarball...**.
5. Pick `com.drkai-lab.vrc-expression-hotkeys-<ver>.tgz`.

### B. From Git

Git has to be installed.

1. Open **Window > Package Manager**.
2. Click **+**.
3. Choose **Add package from git URL...**.
4. Enter `https://github.com/drkai-lab/T1-VREmoKey-Free.git?path=unity`.
5. Click **Add**.

To pin a version, append `#v2.0.0`:

`https://github.com/drkai-lab/T1-VREmoKey-Free.git?path=unity#v2.0.0`

## License

MIT. See [LICENSE](LICENSE).

The window draws Japanese with embedded [Noto Sans JP](https://fonts.google.com/noto/specimen/Noto+Sans+JP) Regular (SIL Open Font License 1.1). The notice is [rust/assets/OFL](rust/assets/OFL), and installers copy it next to the program. The tray menu and the file dialog are native controls: on Windows their text is UTF-16, and they use the system UI font.
