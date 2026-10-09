#!/usr/bin/env python3
"""Chords, profiles, focus filter, and a scan that the tree stays free of device code."""

import contextlib
import importlib.machinery
import importlib.util
import io
import json
import os
import socket
import struct
import tempfile
import time
import unittest
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
SCRIPT = os.path.join(ROOT, "vrc-expression-hotkeys.py")

loader = importlib.machinery.SourceFileLoader("veh", SCRIPT)
spec = importlib.util.spec_from_loader("veh", loader)
veh = importlib.util.module_from_spec(spec)
loader.exec_module(veh)


def _chars(*codes):
    return "".join(chr(code) for code in codes)


# Built from character codes so this file does not contain the banned text.
BANNED = (
    _chars(84, 49),  # product token
    _chars(116, 49),
    _chars(72, 73, 68),
    _chars(117, 115, 98, 102, 115),
    _chars(73, 79, 72, 73, 68),
    _chars(49, 49, 56, 57),
    _chars(56, 56, 57, 48),
    _chars(117, 100, 101, 118),
    _chars(116, 49, 45, 107, 101, 121, 98, 111, 97, 114, 100),
)


def _png(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    # Minimal 1x1 PNG. The app only checks that the file exists.
    raw = (
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01"
        b"\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx"
        b"\x9cc\xf8\xcf\xc0\x00\x00\x00\x03\x00\x01\x00\x05\xfe\xd4\xef\x00\x00\x00\x00IEND\xaeB`\x82"
    )
    with open(path, "wb") as handle:
        handle.write(raw)


class ChordTest(unittest.TestCase):
    def setUp(self):
        self.tracker = veh.ChordTracker()
        self.f1 = veh.EVDEV_TO_FKEY and 59
        self.shift = 42

    def test_codes(self):
        self.assertEqual(veh.SLOT_COUNT, 4)
        self.assertEqual(veh.FKEY_NAMES, ["F1", "F2", "F3", "F4"])
        self.assertEqual(veh.EVDEV_TO_FKEY[59], "F1")
        self.assertEqual(veh.EVDEV_TO_FKEY[62], "F4")
        self.assertNotIn(63, veh.EVDEV_TO_FKEY)
        self.assertNotIn(88, veh.EVDEV_TO_FKEY)
        self.assertIn(63, veh.IGNORED_EVDEV)
        self.assertIn(88, veh.IGNORED_EVDEV)
        self.assertEqual(veh.VK_TO_FKEY[0x70], "F1")
        self.assertEqual(veh.VK_TO_FKEY[0x73], "F4")
        self.assertNotIn(0x74, veh.VK_TO_FKEY)
        self.assertIn(0x7B, veh.IGNORED_VK)

    def test_f5_through_f12_are_ignored(self):
        self.tracker.note_evdev("kbd", self.shift, 1)
        self.assertEqual(self.tracker.note_evdev("kbd", 63, 1), [])
        self.assertEqual(self.tracker.note_evdev("kbd", 88, 1), [])
        self.assertEqual(self.tracker.note_evdev("kbd", 63, 0), [])
        self.tracker.note_vk("win", 0xA0, False)
        self.assertEqual(self.tracker.note_vk("win", 0x74, False), [])
        self.assertEqual(self.tracker.note_vk("win", 0x7B, False), [])
        self.assertEqual(self.tracker.note_vk("win", 0x73, False), [("F4", True)])

    def test_press_and_release(self):
        self.assertEqual(self.tracker.note_evdev("kbd", self.shift, 1), [])
        self.assertEqual(self.tracker.note_evdev("kbd", self.f1, 1), [("F1", True)])
        self.assertEqual(self.tracker.note_evdev("kbd", self.f1, 0), [("F1", False)])

    def test_repeat_is_ignored(self):
        self.tracker.note_evdev("kbd", self.shift, 1)
        self.tracker.note_evdev("kbd", self.f1, 1)
        self.assertEqual(self.tracker.note_evdev("kbd", self.f1, 2), [])

    def test_f_without_shift_is_ignored(self):
        self.assertEqual(self.tracker.note_evdev("kbd", self.f1, 1), [])
        self.assertEqual(self.tracker.note_evdev("kbd", self.f1, 0), [])

    def test_shift_across_keyboards(self):
        self.tracker.note_evdev("board-a", self.shift, 1)
        self.assertEqual(self.tracker.note_evdev("board-b", self.f1, 1), [("F1", True)])

    def test_releasing_shift_sends_false(self):
        self.tracker.note_evdev("board-a", self.shift, 1)
        self.tracker.note_evdev("board-b", self.f1, 1)
        self.assertEqual(self.tracker.note_evdev("board-a", self.shift, 0), [("F1", False)])
        self.assertEqual(self.tracker.note_evdev("board-b", self.f1, 0), [])

    def test_windows_vk(self):
        self.tracker.note_vk("win", 0xA0, False)
        self.assertEqual(self.tracker.note_vk("win", 0x70, False), [("F1", True)])
        self.assertEqual(self.tracker.note_vk("win", 0x70, False), [])
        self.assertEqual(self.tracker.note_vk("win", 0x70, True), [("F1", False)])

    def test_unplug_releases(self):
        self.tracker.note_evdev("kbd", self.shift, 1)
        self.tracker.note_evdev("kbd", self.f1, 1)
        self.assertEqual(self.tracker.drop_device("kbd"), [("F1", False)])

    def test_keyboard_bitmap(self):
        empty = bytearray(96)
        self.assertFalse(veh.looks_like_keyboard(empty))
        bits = bytearray(96)
        bits[veh.KEY_F1 // 8] |= 1 << (veh.KEY_F1 % 8)
        self.assertTrue(veh.looks_like_keyboard(bits))


class FocusTest(unittest.TestCase):
    def setUp(self):
        self.tracker = veh.ChordTracker()
        self.f1 = 59
        self.shift = 42
        self.focused = True
        self.gate = veh.FocusGate(veh.resolve_target({}), lambda: self.focused)

    def _down(self, gate=None):
        return veh.observe_evdev(self.tracker, gate or self.gate, "kbd", self.f1, 1)

    def test_unfocused_press_stays_off(self):
        self.focused = False
        self.tracker.note_evdev("kbd", self.shift, 1)
        self.assertEqual(self._down(), [])
        self.focused = True
        self.assertEqual(self._down(), [])
        self.assertEqual(veh.observe_evdev(self.tracker, self.gate, "kbd", self.f1, 0), [])

    def test_release_after_focus_leaves_sends_false(self):
        self.tracker.note_evdev("kbd", self.shift, 1)
        self.assertEqual(self._down(), [("F1", True)])
        self.focused = False
        self.assertEqual(
            veh.observe_evdev(self.tracker, self.gate, "kbd", self.f1, 0),
            [("F1", False)])

    def test_windows_repeat_outside_target_does_not_arm(self):
        self.focused = False
        self.tracker.note_vk("win", 0xA0, False)
        self.assertEqual(veh.observe_vk(self.tracker, self.gate, "win", 0x70, False), [])
        self.focused = True
        self.assertEqual(veh.observe_vk(self.tracker, self.gate, "win", 0x70, False), [])
        self.assertEqual(veh.observe_vk(self.tracker, self.gate, "win", 0x70, True), [])

    def test_disabled_filter_sends(self):
        gate = veh.FocusGate(veh.resolve_target({"enabled": False}), lambda: False)
        self.tracker.note_evdev("kbd", self.shift, 1)
        self.assertEqual(self._down(gate), [("F1", True)])

    def test_unknown_deny_warns_once(self):
        gate = veh.FocusGate(veh.resolve_target({"when_unknown": "deny"}), lambda: None)
        self.tracker.note_evdev("kbd", self.shift, 1)
        with contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertEqual(self._down(gate), [])
            self.assertEqual(self._down(gate), [])
        self.assertEqual(err.getvalue().count("WARNING: cannot tell"), 1)

    def test_unknown_allow_sends(self):
        gate = veh.FocusGate(veh.resolve_target({"when_unknown": "allow"}), lambda: None)
        self.tracker.note_evdev("kbd", self.shift, 1)
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(self._down(gate), [("F1", True)])

    def test_process_and_window_match(self):
        image = r"C:\Games\VRChat\VRChat.exe"
        self.assertTrue(veh.process_matches(image, ["vrchat.exe"]))
        self.assertFalse(veh.process_matches("notepad.exe", ["VRChat.exe"]))
        fields = veh.parse_hypr_active(json.dumps({
            "class": "steam_app_438100",
            "title": "VRChat",
        }))
        self.assertTrue(veh.window_matches(fields, veh.DEFAULT_TARGET["linux_match"]))
        self.assertFalse(veh.window_matches(fields, ["firefox"]))

    def test_detector_order(self):
        calls = []

        def run(argv):
            calls.append(argv[0])
            return json.dumps({"class": "steam_app_438100", "title": "VRChat"})

        fields = veh.linux_active_fields(
            run=run, which=lambda name: "/usr/bin/hyprctl" if name == "hyprctl" else None)
        self.assertEqual(calls, ["hyprctl"])
        self.assertTrue(veh.window_matches(fields, ["steam_app_438100"]))

    def test_xprop_fallback(self):
        calls = []

        def which(name):
            return name if name in {"xdotool", "xprop"} else None

        def run(argv):
            calls.append(argv[0])
            if argv[0] == "xdotool":
                return None
            if argv[:2] == ["xprop", "-root"]:
                return "_NET_ACTIVE_WINDOW(WINDOW): window id # 0x1234"
            return 'WM_CLASS(STRING) = "steam_app_438100"\n'

        fields = veh.linux_active_fields(run=run, which=which)
        self.assertEqual(calls, ["xdotool", "xprop", "xprop"])
        self.assertTrue(veh.window_matches(fields, ["steam_app_438100"]))

    def test_default_target(self):
        target = veh.resolve_target(None)
        self.assertTrue(target["enabled"])
        self.assertEqual(target["windows_process"], ["VRChat.exe"])
        self.assertEqual(target["linux_match"], ["steam_app_438100", "VRChat"])
        self.assertEqual(target["when_unknown"], "deny")


class OscTest(unittest.TestCase):
    def test_bool_packet(self):
        sender = veh.OscSender()
        down = sender.packet("/avatar/parameters/Expression1", True)
        up = sender.packet("/avatar/parameters/Expression1", False)
        self.assertIn(b",T", down)
        self.assertIn(b",F", up)
        self.assertTrue(down.endswith(b"\x01"))
        self.assertTrue(up.endswith(b"\x00"))
        address, value, kind = veh.parse_osc_packet(down)
        self.assertEqual(address, "/avatar/parameters/Expression1")
        self.assertTrue(value)
        self.assertEqual(kind, "bool")

    def test_string_packet(self):
        packet = veh.encode_osc_string("/avatar/change", "smile")
        address, value, kind = veh.parse_osc_packet(packet)
        self.assertEqual((address, value, kind), ("/avatar/change", "smile", "string"))

    def test_event_struct(self):
        blob = struct.pack("llHHi", 1, 2, veh.KERNEL_EV_KEY, veh.KEY_F1, 1)
        self.assertEqual(len(blob), veh.INPUT_EVENT.size)
        unpacked = veh.INPUT_EVENT.unpack(blob)
        self.assertEqual(unpacked[2:], (veh.KERNEL_EV_KEY, veh.KEY_F1, 1))


class ProfileTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="veh-")
        self.root = self.tmp.name

    def tearDown(self):
        self.tmp.cleanup()

    def test_default_assignment_is_identity(self):
        settings = veh.default_settings()
        hotkeys = veh.hotkeys_for(settings, self.root)
        self.assertEqual(hotkeys["F1"], "/avatar/parameters/Expression1")
        self.assertEqual(hotkeys["F4"], "/avatar/parameters/Expression4")
        self.assertNotIn("F5", hotkeys)
        self.assertNotIn("F12", hotkeys)
        self.assertEqual(len(hotkeys), 4)

    def test_reassign_and_missing_thumb(self):
        profile = veh.default_profile()
        profile["id"] = "smile"
        profile["name"] = "Smile"
        profile["expressions"][2]["name"] = "Grin"
        profile["expressions"][2]["osc"] = "/avatar/parameters/Grin"
        profile["expressions"][2]["thumb"] = "thumbs/3.png"
        veh.save_profile(profile, self.root)
        settings = veh.default_settings()
        settings["active_avatar"] = "smile"
        settings["fkey_assign"]["F3"] = 3
        slot = veh.expression_slot(3, veh.active_profile(settings, self.root), self.root)
        self.assertEqual(slot["label"], "Expression3")
        self.assertEqual(slot["thumb"], "")
        self.assertEqual(slot["osc"], "/avatar/parameters/Grin")
        _png(os.path.join(self.root, "avatars", "smile", "thumbs", "3.png"))
        slot = veh.expression_slot(3, veh.load_profile("smile", self.root), self.root)
        self.assertEqual(slot["label"], "Grin")
        self.assertEqual(
            os.path.normpath(slot["thumb"]),
            os.path.join(self.root, "avatars", "smile", "thumbs", "3.png"))

    def test_backslash_thumb_path(self):
        profile = veh.default_profile()
        profile["id"] = "smile"
        profile["expressions"][0]["thumb"] = "thumbs\\1.png"
        path = veh.profile_thumb_path(profile, profile["expressions"][0], self.root)
        self.assertEqual(
            path, os.path.normpath(os.path.join(self.root, "avatars", "smile", "thumbs", "1.png")))

    def test_import_zip_and_reject_parent_path(self):
        src = os.path.join(self.root, "pack")
        os.makedirs(src)
        profile = veh.default_profile()
        profile["id"] = "smile"
        profile["blueprint_id"] = "avtr_smile"
        profile["expressions"][0]["thumb"] = "thumbs/1.png"
        with open(os.path.join(src, "profile.json"), "w", encoding="utf-8") as handle:
            json.dump(profile, handle)
        _png(os.path.join(src, "thumbs", "1.png"))
        pack = os.path.join(self.root, "smile.zip")
        with zipfile.ZipFile(pack, "w") as archive:
            archive.write(os.path.join(src, "profile.json"), "smile/profile.json")
            archive.write(os.path.join(src, "thumbs", "1.png"), "smile/thumbs/1.png")
        loaded = veh.import_profile_pack(pack, self.root)
        self.assertEqual(loaded["id"], "smile")
        self.assertTrue(os.path.isfile(
            os.path.join(self.root, "avatars", "smile", "thumbs", "1.png")))

    def test_import_ignores_indexes_above_four(self):
        src = os.path.join(self.root, "wide")
        os.makedirs(src)
        profile = veh.default_profile()
        profile["id"] = "wide"
        profile["expressions"].append({
            "index": 12,
            "name": "Extra",
            "parameter": "Extra",
            "osc": "/avatar/parameters/Extra",
            "type": "bool",
            "thumb": "",
        })
        with open(os.path.join(src, "profile.json"), "w", encoding="utf-8") as handle:
            json.dump(profile, handle)
        loaded = veh.import_profile_pack(src, self.root)
        indexes = [item["index"] for item in loaded["expressions"]]
        self.assertEqual(indexes, [1, 2, 3, 4])
        self.assertEqual(loaded["import_skipped"], [12])
        self.assertIn("Ignored indexes: 12", loaded["import_notice"])
        self.assertIn("12", veh.import_notice([12], "ja"))
        with open(
            os.path.join(self.root, "avatars", "wide", "profile.json"),
            encoding="utf-8",
        ) as handle:
            stored = json.load(handle)
        self.assertNotIn("import_notice", stored)
        self.assertEqual([item["index"] for item in stored["expressions"]], [1, 2, 3, 4])
        self.assertIsNone(veh.expression_slot(12, loaded, self.root))
        bad = os.path.join(self.root, "bad.zip")
        with zipfile.ZipFile(bad, "w") as archive:
            archive.writestr("../outside.json", "{}")
        with self.assertRaises(ValueError):
            veh.import_profile_pack(bad, self.root)

    def test_avatar_change_switches_profile(self):
        saved = (veh.CONFIG_DIR, veh.CONFIG_FILE, veh.STATE_DIR, veh.LOG_FILE)
        veh.CONFIG_DIR = self.root
        veh.CONFIG_FILE = os.path.join(self.root, "settings.json")
        veh.STATE_DIR = self.root
        veh.LOG_FILE = os.path.join(self.root, "listener.log")
        try:
            profile = veh.default_profile()
            profile["id"] = "smile"
            profile["blueprint_id"] = "avtr_smile"
            veh.save_profile(profile, self.root)
            veh.save_settings(veh.default_settings())
            switched = veh.note_avatar_change("avtr_smile", self.root)
            self.assertEqual(switched["id"], "smile")
            self.assertEqual(veh.load_settings()["active_avatar"], "smile")
            self.assertIsNone(veh.note_avatar_change("missing", self.root))
        finally:
            veh.CONFIG_DIR, veh.CONFIG_FILE, veh.STATE_DIR, veh.LOG_FILE = saved

    def test_busy_port_is_not_fatal(self):
        held = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        held.bind(("127.0.0.1", 0))
        port = held.getsockname()[1]
        try:
            listener = veh.AvatarOscListener(port, lambda _avatar: None)
            self.assertFalse(listener.start())
            self.assertFalse(listener.ok)
            self.assertTrue(listener.error)
        finally:
            held.close()

    def test_change_message_invokes_callback(self):
        seen = []
        probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
        probe.close()
        listener = veh.AvatarOscListener(port, seen.append)
        self.assertTrue(listener.start())
        try:
            packet = veh.encode_osc_string("/avatar/change", "avtr_smile")
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
                sock.sendto(packet, ("127.0.0.1", port))
            deadline = time.time() + 2
            while time.time() < deadline and not seen:
                time.sleep(0.02)
            self.assertEqual(seen, ["avtr_smile"])
        finally:
            listener.stop()


class ListenerTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="veh-listen-")
        self._saved = (
            veh.CONFIG_DIR, veh.CONFIG_FILE, veh.STATE_DIR, veh.LOG_FILE,
            veh.open_key_source)
        veh.CONFIG_DIR = self.tmp.name
        veh.CONFIG_FILE = os.path.join(self.tmp.name, "settings.json")
        veh.STATE_DIR = self.tmp.name
        veh.LOG_FILE = os.path.join(self.tmp.name, "listener.log")
        self.closed = []

        class Fake:
            sent = False

            def poll(self, timeout):
                if not self.sent:
                    self.sent = True
                    return [
                        ("evdev", "kbd", 42, 1),
                        ("evdev", "kbd", 59, 1),
                        ("evdev", "kbd", 59, 0),
                    ]
                time.sleep(0.02)
                return []

            def close(self):
                outer.closed.append(True)

        outer = self
        self.source = Fake()
        veh.open_key_source = lambda: self.source
        veh._listener = None

    def tearDown(self):
        veh.stop_expression_listener()
        veh._listener = None
        (
            veh.CONFIG_DIR, veh.CONFIG_FILE, veh.STATE_DIR, veh.LOG_FILE,
            veh.open_key_source,
        ) = self._saved
        self.tmp.cleanup()

    def test_listener_sends_press_and_release(self):
        srv = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        srv.bind(("127.0.0.1", 0))
        srv.settimeout(0.2)
        port = srv.getsockname()[1]
        try:
            settings = veh.default_settings()
            settings["osc"] = {"host": "127.0.0.1", "port": port}
            settings["target"] = {"enabled": False}
            veh.save_settings(settings)
            veh.start_expression_listener()
            blob = b""
            deadline = time.time() + 3
            while time.time() < deadline and b",F" not in blob:
                with contextlib.suppress(TimeoutError):
                    blob += srv.recv(4096)
            veh.stop_expression_listener()
            self.assertTrue(self.closed)
            self.assertIn(b"/avatar/parameters/Expression1", blob)
            self.assertIn(b",T", blob)
            self.assertIn(b",F", blob)
        finally:
            srv.close()


class SourceContractTest(unittest.TestCase):
    def test_hook_passes_keys_through(self):
        with open(SCRIPT, encoding="utf-8") as handle:
            text = handle.read()
        self.assertIn("WH_KEYBOARD_LL", text)
        self.assertIn("CallNextHookEx", text)
        self.assertIn("GetForegroundWindow", text)
        self.assertIn("QueryFullProcessImageNameW", text)
        self.assertIn("hyprctl", text)
        self.assertIn("xdotool", text)
        self.assertIn("xprop", text)
        self.assertNotIn("RegisterHotKey(", text)
        self.assertNotIn("GetAsyncKeyState(", text)
        self.assertNotIn("EVIOCGRAB", text)

    def test_code_has_no_device_tokens(self):
        scanned = []
        skip_dirs = {".git", "__pycache__", ".ruff_cache", "dist", "dist-win", "build"}
        for dirpath, dirnames, filenames in os.walk(ROOT):
            dirnames[:] = [name for name in dirnames if name not in skip_dirs]
            for filename in filenames:
                lowered = filename.lower()
                if lowered.startswith("readme"):
                    continue
                path = os.path.join(dirpath, filename)
                if not filename.endswith((
                    ".py", ".cs", ".yml", ".yaml", ".iss", ".spec", ".json", ".toml", ".sh",
                    ".asmdef", ".txt", ".md",
                )):
                    continue
                with open(path, encoding="utf-8", errors="replace") as handle:
                    text = handle.read()
                scanned.append(os.path.relpath(path, ROOT))
                for needle in BANNED:
                    self.assertNotIn(needle, text, f"{needle!r} in {path}")
        self.assertIn("vrc-expression-hotkeys.py", scanned)
        self.assertIn(os.path.join("unity", "package.json"), scanned)


if __name__ == "__main__":
    unittest.main(verbosity=2)
