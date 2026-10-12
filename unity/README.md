# T1-VREmoKey-Free (Unity)

Free edition, version 2.1.0. This package keeps 4 expressions. Find the
avatar's face clips, pick up to four with a face thumbnail, and export
`profile.json` plus `thumbs/*.png`. The desktop app imports that folder
(or a zip of it) and assigns Shift+F1..F4 from the thumbnail list. Export
stops at 4. A row whose index is above 4 is left out of `profile.json`.

The full edition, T1-VREmoKey, exists for the T1 mini keyboard. That edition
supports 12 expressions, the keyboard's own buttons, and the dial. This
package does not assign hardware buttons and does not write firmware.

The editor tools use reflection, so a project without the VRChat SDK still
compiles. Preview rendering needs the avatar open in the scene. It samples
a temporary copy and does not save the scene.

## Install

### A. From the release tarball

1. Download `com.drkai-lab.vrc-expression-hotkeys-<ver>.tgz` from the project releases.
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

You can also copy this `unity/` folder into the project's `Packages` folder.

## Workflow

1. `Tools > VRChat Expression Hotkeys > Avatar Profile`.
   - The buttons at the top switch Japanese, English, and Korean immediately.
     The choice is stored for the next time the editor opens. The first
     visit follows the OS or editor language, otherwise Japanese.
     Expression names are left as typed.
   - Put the avatar in the scene and assign it. With the SDK installed,
     the blueprint id is read by reflection from a `blueprintId` field.
     Otherwise type an id.
   - **Find expressions / 表情を探す** lists clips the FX controller uses
     and clips in the avatar folder that move a face blendShape. Blinks,
     visemes, a mouth reset, hand-only poses, empty clips, and a
     DefaultFace pose are listed with a reason. **Restore / 戻す** puts
     one back. Display names can be Japanese.
   - Assign each kept clip to Shift+F1..F4 (four slots). The window renders
     a 256×256 face thumbnail and restores the avatar afterward.
   - **Install and export / FX に入れて書き出す** asks for a folder, then asks
     before it changes the controller. It adds `Expression1`..`Expression4`
     bools and FX layers for the slots you filled. Layers are placed after
     hand-gesture layers. Write Defaults follows the majority of the existing
     FX states. The install is refused when it would pass 256 synced bits,
     when a foreign layer already uses that name, or when the parameter is
     not a bool. A second run updates the `VREmoKey` layer instead of adding
     another. The first edit is copied to `VREmoKeyBackup` next to the
     project's `Assets` folder. **Restore backup / 元に戻す** asks first, copies
     the current files to `VREmoKeyBackup/before-restore-<time>`, then puts
     the original backup back.
   - The folder contains `profile.json` and `thumbs/N.png`. Zip it if you
     want one file.
2. Import the pack in the desktop app. Each of F1..F4 picks from the
   thumbnail list. **Preview fullscreen / 全画面で見る**, or holding the
   key while VRChat is in front, shows the expression name and thumbnail.
   `/avatar/change` switches that fullscreen with the profile. Indexes
   above 4 are ignored, and the desktop app shows a notice.

## What this package does not include

No scene component, no OSC receiver inside Unity, and no button or dial
mapping. The desktop app sends the OSC bools.
