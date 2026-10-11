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

The command name stays `vrc-expression-hotkeys`, and settings stay in the same directories, so an existing setup keeps working. Version 2.1.0 is the Rust app. Its window title and tray name are **T1-VREmoKey-Free**. Release builds strip symbols and use link-time optimization. This public build is not obfuscated.

## What it does

1. In Unity, open `Tools > VRChat Expression Hotkeys > Avatar Profile`.
   With the avatar in the scene, choose **Find expressions**. Clips that
   move a face blendShape, from the FX controller and the avatar folder,
   become candidates. Blinks, visemes, a mouth reset, hand-only poses,
   empty clips, and a DefaultFace pose stay in the list with a reason, and
   can be restored. Display names can be Japanese. Assign the ones you
   keep to Shift+F1..F4. The window renders a 256×256 face thumbnail and
   puts the avatar back afterward. **Install and export** adds the
   `ExpressionN` bool and an FX layer for each filled slot (after hand
   gestures, Write Defaults matched to the existing FX, refused when the
   256 synced-bit budget would be exceeded), keeps a backup, and writes
   `profile.json` plus `thumbs/*.png`. A second run updates the same
   layer. **Restore backup** copies the original assets back. The editor
   tools use reflection, so a project without the VRChat SDK still
   compiles. The window switches among Japanese, English, and Korean from
   the buttons at the top, without restarting the editor. The choice is
   saved. The first visit follows the OS or editor language, and uses
   Japanese when that language is unknown. Expression names are not
   translated. Zip that folder if you want a single file.
2. In this app, import the pack. Pick the avatar in the dropdown, or let
   VRChat switch it. VRChat sends `/avatar/change` to `127.0.0.1:9001` by
   default. The listen port is configurable. If that port is already taken
   the app keeps running and says so; forward `/avatar/change` with any OSC
   router onto the port you chose. Switching the avatar refreshes the
   fullscreen name and thumbnail.
3. Assign each of F1..F4 from the thumbnail list. The row shows the key,
   the thumbnail, and the name. **Preview fullscreen** shows that name and
   thumbnail. A missing PNG shows a generic icon and `Expression N` on the
   row. Shift+F5..F12 do nothing.
4. Leave the app running. Shift+Fn sends that expression's OSC bool: TRUE
   on press, FALSE on release. Repeats are ignored. By default this happens
   only while VRChat is the foreground app. While the key is held, the
   fullscreen shows the expression name and thumbnail. Releasing the key
   closes it. Closing it during the hold keeps it closed until the next
   press.

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

The Windows executable does not need the Visual C++ redistributable.

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

## Walkthrough without a keyboard or VRChat

The free edition does not talk to keyboard hardware. A T1 button only matters here as the Shift+F1..F4 chord it would type. A test build can push that chord in underneath the key hook, and a small OSC stand-in records what the app sends.

The stand-in is absent from release builds. `cargo build --release` and the packages do not pass `--features emulate`, so a published binary has no `--emulate` flag and no inject queue.

```bash
cd rust
cargo test --lib walkthrough_configure_confirm_use_and_switch_avatar
cargo run --features emulate --bin vrc-expression-hotkeys -- --emulate
```

`cargo test` runs the same walkthrough as a library test. `--emulate` runs it inside the console binary and exits 0 when every step passes.

A Windows build with the feature uses the same command under Wine:

```bash
cargo build --target x86_64-pc-windows-gnu --features emulate --bin vrc-expression-hotkeys
wine target/x86_64-pc-windows-gnu/debug/vrc-expression-hotkeys.exe --emulate
```

The walkthrough:

1. Imports two avatar profiles through the app's profile importer and writes `settings.json`.
2. Reads them back and checks that F1 resolves to the first avatar's parameter. It also checks the Japanese expression name and thumbnail file the fullscreen shows.
3. Presses and releases Shift+F1 through the platform hook (`poll` on evdev, or the low-level keyboard hook). The stand-in VRChat records bool true, then false. F1's address is `/avatar/parameters/GrinA` (24 bytes). The walkthrough checks that this packet ends the address with four NUL bytes, then the type tag `,T` padded to four bytes.
4. Repeats the chord with the focus substitute forced off. No OSC is sent.
5. Clears the substitute and uses the real foreground check. The chord is sent only when that check says VRChat is focused. A headless machine usually cannot name the focused window, and `when_unknown` stays `deny`, so the chord is dropped.
6. Sends `/avatar/change` for the second avatar, checks the saved profile and the fullscreen card (Japanese name and thumbnail), then presses Shift+F1 again and expects the second parameter.

While the walkthrough is running it listens on `127.0.0.1` for `press N`, `release N` (N is 1..4), and `focus on`, `focus off`, or `focus live`. The last chord is sent through that socket, from outside the hook thread.

Limits:

- The stand-in does not create a device node. Injected events do not travel through `/dev/input` or a physical `WH_KEYBOARD_LL` packet. Wine often does not deliver real low-level hook events; this command does not claim that it does. The report line `hook: low-level installed` or `hook: low-level unavailable` says whether `SetWindowsHookExW` succeeded.
- For the duration of the walkthrough the hook ignores the physical keyboard, so typing on that machine cannot change the result. The run does not prove that a real key reaches the hook.
- The covered chord is Shift+F1..F4, press then release, one chord at a time. F5..F12, a dial, and hardware reports are out of scope. The run does check that an F5 chord produces no OSC.
- The stand-in VRChat stores OSC bools and can send `/avatar/change`. It does not load an avatar, run an animator, or accept a non-bool parameter.
- The focus substitute exists only in this test build. Without it, the real Windows foreground-process check or the Linux `hyprctl` / `xdotool` / `xprop` check runs. Passing that step means the real check ran and the gate followed it.
- The window, tray, and Unity exporter are outside this command.
- `--emulate` uses a temporary directory. It does not read or write `~/.config/vrc-expression-hotkeys` or `%APPDATA%\vrc-expression-hotkeys`.

## Manual checklist

- [ ] Export a profile from Unity and import it. Names and thumbnails show on F1..F4.
- [ ] Preview fullscreen, or hold Shift+Fn, and confirm the expression name and thumbnail. Release closes the fullscreen.
- [ ] After `/avatar/change`, the fullscreen name and thumbnail follow the new avatar.
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

To pin a version, append `#v2.1.0`:

`https://github.com/drkai-lab/T1-VREmoKey-Free.git?path=unity#v2.1.0`

## License

MIT. See [LICENSE](LICENSE).

The window draws Japanese with embedded [Noto Sans JP](https://fonts.google.com/noto/specimen/Noto+Sans+JP) Regular (SIL Open Font License 1.1). The notice is [rust/assets/OFL](rust/assets/OFL), and installers copy it next to the program. The tray menu and the file dialog are native controls: on Windows their text is UTF-16, and they use the system UI font.
