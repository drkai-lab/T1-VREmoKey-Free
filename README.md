# VRChat Expression Hotkeys

Free edition. Hold Shift and press F1 through F4 to toggle a VRChat
expression. This edition has four expressions. Each avatar can have its
own names and preview images. This repository does not talk to keyboard
hardware and does not write firmware.

A separate full edition exists for the T1 mini keyboard. That edition
supports 12 expressions, the keyboard's own buttons, and the dial. It is
not part of this project.

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

## Download

Windows: the installer or the two executables (console and windowed) from
the release assets, once a release is published.

Linux:

```bash
tar xzf vrc-expression-hotkeys-linux-1.0.0.tar.gz
cd vrc-expression-hotkeys-linux-1.0.0
python3 vrc-expression-hotkeys.py
```

From a checkout:

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

See [unity/README.md](unity/README.md). Copy `unity/` into the project's
`Packages` folder, or add the folder as a local package.

## License

MIT. See [LICENSE](LICENSE).
