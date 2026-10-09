# T1-VREmoKey-Free (Unity)

Free edition, version 1.0.0. This package keeps 4 expressions. Name each
one, render a 256×256 face preview, and export `profile.json` plus
`thumbs/*.png`. The desktop app imports that folder (or a zip of it) and
assigns Shift+F1..F4. Suggest and export stop at 4. A row whose index is
above 4 is left out of `profile.json`.

The full edition, T1-VREmoKey, exists for the T1 mini keyboard. That edition
supports 12 expressions, the keyboard's own buttons, and the dial. This
package does not assign hardware buttons and does not write firmware.

The editor tools use reflection, so a project without the VRChat SDK still
compiles. Preview rendering needs the avatar open in the scene. It samples
a temporary copy and does not save the scene.

## Workflow

1. Copy this `unity/` folder into the project's `Packages` folder, or add
   the folder as a local package.
2. `Tools > VRChat Expression Hotkeys > Avatar Profile`.
   - Assign the avatar that is open in the scene. With the SDK installed,
     the blueprint id is read by reflection from a `blueprintId` field.
     Otherwise type an id.
   - **Suggest from avatar** lists the first 4 expression parameters and Animator clip names.
   - Set the parameter and OSC address. **Generate previews** writes 256×256
     PNGs, framed on the head bone.
   - **Export profile folder**. Zip that folder if you want one file.
3. Import the pack in the desktop app and assign F1..F4. Indexes above 4
   are ignored, and the desktop app shows a notice.

## What this package does not include

No scene component, no OSC receiver inside Unity, and no button or dial
mapping. The desktop app sends the OSC bools.
