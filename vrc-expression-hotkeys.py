#!/usr/bin/env python3
"""Shift+F1..F4 expression hotkeys for VRChat.

Free edition: four expressions, per-avatar names and thumbnails, a small
assignment window, and a chord listener. It does not open a device or send
a device report. Shift+F5..F12 are ignored.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import errno
import io
import json
import os
import queue
import select
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
import zipfile

try:
    import fcntl
except ImportError:
    fcntl = None

for _stream in ("stdout", "stderr"):
    if getattr(sys, _stream, None) is None:
        setattr(sys, _stream, io.StringIO())
del _stream

VERSION = "1.0.0"
APP_NAME = "vrc-expression-hotkeys"
IS_WIN = sys.platform == "win32"
IS_MAC = sys.platform == "darwin"
IS_LINUX = sys.platform.startswith("linux")

if IS_WIN:
    _root = os.environ.get("APPDATA") or os.path.expanduser("~")
    CONFIG_DIR = os.path.join(_root, APP_NAME)
    STATE_DIR = os.path.join(_root, APP_NAME, "state")
elif IS_MAC:
    _root = os.path.expanduser("~/Library/Application Support")
    CONFIG_DIR = os.path.join(_root, APP_NAME)
    STATE_DIR = os.path.join(_root, APP_NAME, "state")
else:
    CONFIG_DIR = os.path.expanduser(f"~/.config/{APP_NAME}")
    STATE_DIR = os.path.expanduser(f"~/.local/state/{APP_NAME}")

CONFIG_FILE = os.path.join(CONFIG_DIR, "settings.json")
LOG_FILE = os.path.join(STATE_DIR, "listener.log")
DEFAULT_AVATAR_ID = "default"
AVATAR_LISTEN_PORT = 9001
OSC_PORT = 9000

INPUT_EVENT = struct.Struct("llHHi")
KERNEL_EV_KEY = 0x01
_EVDEV_F = {
    1: 59, 2: 60, 3: 61, 4: 62, 5: 63, 6: 64,
    7: 65, 8: 66, 9: 67, 10: 68, 11: 87, 12: 88,
}
SLOT_COUNT = 4
EVDEV_TO_FKEY = {
    code: f"F{number}" for number, code in _EVDEV_F.items() if number <= SLOT_COUNT}
IGNORED_EVDEV = frozenset(
    code for number, code in _EVDEV_F.items() if number > SLOT_COUNT)
SHIFT_EVDEV = frozenset({42, 54})
KEY_F1 = 59
VK_TO_FKEY = {0x6F + number: f"F{number}" for number in range(1, SLOT_COUNT + 1)}
IGNORED_VK = frozenset(0x6F + number for number in range(SLOT_COUNT + 1, 13))
SHIFT_VK = frozenset({0x10, 0xA0, 0xA1})
FKEY_NAMES = [f"F{number}" for number in range(1, SLOT_COUNT + 1)]
WH_KEYBOARD_LL = 13
WM_KEYUP = 0x0101
WM_SYSKEYUP = 0x0105
LLKHF_UP = 0x80

DEFAULT_TARGET = {
    "enabled": True,
    "windows_process": ["VRChat.exe"],
    "linux_match": ["steam_app_438100", "VRChat"],
    "when_unknown": "deny",
}


def log(message):
    try:
        os.makedirs(STATE_DIR, exist_ok=True)
        stamp = time.strftime("%Y-%m-%d %H:%M:%S")
        with open(LOG_FILE, "a", encoding="utf-8") as handle:
            handle.write(f"{stamp} {message}\n")
    except OSError:
        pass


def default_settings():
    return {
        "osc": {"host": "127.0.0.1", "port": OSC_PORT},
        "avatar_osc": {"port": AVATAR_LISTEN_PORT},
        "enabled": True,
        "target": dict(DEFAULT_TARGET),
        "active_avatar": DEFAULT_AVATAR_ID,
        "fkey_assign": {name: number for number, name in enumerate(FKEY_NAMES, start=1)},
    }


def load_settings():
    try:
        with open(CONFIG_FILE, encoding="utf-8") as handle:
            data = json.load(handle)
    except FileNotFoundError:
        data = {}
    except (OSError, json.JSONDecodeError):
        return default_settings()
    if not isinstance(data, dict):
        return default_settings()
    return data


def save_settings(settings):
    os.makedirs(CONFIG_DIR, exist_ok=True)
    tmp = CONFIG_FILE + ".tmp"
    with open(tmp, "w", encoding="utf-8") as handle:
        json.dump(settings, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    os.replace(tmp, CONFIG_FILE)


def _pad(raw):
    # OSC 1.0: NUL-terminate, then fill to a 4-byte boundary. A length that
    # is already a multiple of 4 still needs four NUL bytes.
    return raw + b"\x00" * (4 - (len(raw) % 4))


class OscSender:
    def __init__(self, host="127.0.0.1", port=OSC_PORT):
        self.host = host
        self.port = int(port)

    def packet(self, address, value):
        # Bool packets keep a trailing byte after the type tag. VRChat
        # accepts this form for expression parameters.
        flag = b",T" if value else b",F"
        extra = b"\x01" if value else b"\x00"
        return _pad(address.encode()) + _pad(flag) + extra

    def send_bool(self, address, value):
        return self._send(self.packet(address, value))

    def _send(self, packet):
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
                sock.sendto(packet, (self.host, self.port))
            return True
        except OSError:
            return False


def osc_from_settings(settings):
    osc = settings.get("osc") if isinstance(settings, dict) else {}
    if not isinstance(osc, dict):
        osc = {}
    host = osc.get("host") or "127.0.0.1"
    try:
        port = int(osc.get("port", OSC_PORT))
    except (TypeError, ValueError):
        port = OSC_PORT
    return OscSender(host, port)


def parse_osc_packet(data):
    if not data:
        return None
    end = data.find(b"\x00")
    if end <= 0:
        return None
    try:
        address = data[:end].decode("utf-8")
    except UnicodeDecodeError:
        return None
    index = (end + 4) & ~3
    if index >= len(data) or data[index:index + 1] != b",":
        return None
    tag_end = data.find(b"\x00", index)
    if tag_end < 0:
        return None
    tag = data[index + 1:tag_end].decode("ascii", "replace")
    cursor = (tag_end + 4) & ~3
    if tag.startswith("T"):
        return address, True, "bool"
    if tag.startswith("F"):
        return address, False, "bool"
    if tag.startswith("i") and cursor + 4 <= len(data):
        return address, struct.unpack(">i", data[cursor:cursor + 4])[0], "int"
    if tag.startswith("s") and cursor < len(data):
        text_end = data.find(b"\x00", cursor)
        if text_end < 0:
            return None
        return address, data[cursor:text_end].decode("utf-8", "replace"), "string"
    return None


def encode_osc_string(address, text):
    return _pad(address.encode()) + _pad(b",s") + _pad(text.encode())


def safe_avatar_id(value):
    text = str(value or "").strip()
    cleaned = "".join(ch for ch in text if ch.isalnum() or ch in "._-")
    if cleaned in {"", ".", ".."}:
        return ""
    return cleaned[:128]


def profile_root(root=None):
    return os.path.join(root or CONFIG_DIR, "avatars")


def default_profile():
    expressions = []
    for number in range(1, SLOT_COUNT + 1):
        parameter = f"Expression{number}"
        expressions.append({
            "index": number,
            "name": parameter,
            "parameter": parameter,
            "osc": f"/avatar/parameters/{parameter}",
            "type": "bool",
            "thumb": "",
        })
    return {
        "id": DEFAULT_AVATAR_ID,
        "name": "Default",
        "blueprint_id": "",
        "expressions": expressions,
    }


def normalize_profile(raw):
    if not isinstance(raw, dict):
        return None
    avatar_id = safe_avatar_id(raw.get("id") or raw.get("blueprint_id") or "")
    if not avatar_id:
        return None
    expressions = []
    for item in raw.get("expressions") or []:
        if not isinstance(item, dict):
            continue
        try:
            index = int(item.get("index"))
        except (TypeError, ValueError):
            continue
        if index < 1 or index > SLOT_COUNT:
            continue
        name = str(item.get("name") or f"Expression{index}").strip() or f"Expression{index}"
        parameter = str(item.get("parameter") or name).strip() or name
        osc = str(item.get("osc") or f"/avatar/parameters/{parameter}").strip()
        expressions.append({
            "index": index,
            "name": name,
            "parameter": parameter,
            "osc": osc,
            "type": "bool",
            "thumb": str(item.get("thumb") or "").strip(),
        })
    expressions.sort(key=lambda item: item["index"])
    return {
        "id": avatar_id,
        "name": str(raw.get("name") or avatar_id).strip() or avatar_id,
        "blueprint_id": str(raw.get("blueprint_id") or "").strip(),
        "expressions": expressions,
    }


def load_profile(avatar_id, root=None):
    safe = safe_avatar_id(avatar_id)
    if not safe:
        return default_profile() if avatar_id in {None, "", DEFAULT_AVATAR_ID} else None
    path = os.path.join(profile_root(root), safe, "profile.json")
    try:
        with open(path, encoding="utf-8") as handle:
            data = json.load(handle)
    except (OSError, json.JSONDecodeError):
        return default_profile() if safe == DEFAULT_AVATAR_ID else None
    profile = normalize_profile(data)
    if profile is None:
        return default_profile() if safe == DEFAULT_AVATAR_ID else None
    return profile


def list_profiles(root=None):
    found = {}
    base = profile_root(root)
    if os.path.isdir(base):
        for name in sorted(os.listdir(base)):
            profile = load_profile(name, root)
            if profile:
                found[profile["id"]] = profile
    found.setdefault(DEFAULT_AVATAR_ID, default_profile())
    ordered = [found.pop(DEFAULT_AVATAR_ID)]
    ordered.extend(found[key] for key in sorted(found))
    return ordered


def profile_thumb_path(profile, expr, root=None):
    thumb = str(expr.get("thumb") or "") if isinstance(expr, dict) else ""
    if not thumb or not profile:
        return ""
    if os.path.isabs(thumb):
        return os.path.normpath(thumb)
    parts = [part for part in thumb.replace("\\", "/").split("/") if part not in {"", "."}]
    if not parts or ".." in parts:
        return ""
    return os.path.normpath(os.path.join(
        profile_root(root), str(profile.get("id") or ""), *parts))


def save_profile(profile, root=None):
    normalized = normalize_profile(profile)
    if normalized is None:
        raise ValueError("invalid profile")
    dest = os.path.join(profile_root(root), normalized["id"])
    os.makedirs(dest, exist_ok=True)
    path = os.path.join(dest, "profile.json")
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as handle:
        json.dump(normalized, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    os.replace(tmp, path)
    return normalized


def _copy_thumbs(src_dir, profile, root):
    dest = os.path.join(profile_root(root), profile["id"])
    for expr in profile["expressions"]:
        thumb = str(expr.get("thumb") or "").replace("\\", "/")
        parts = [part for part in thumb.split("/") if part not in {"", "."}]
        if not thumb or os.path.isabs(thumb) or ".." in parts:
            expr["thumb"] = ""
            continue
        source = os.path.join(src_dir, *parts)
        if not os.path.isfile(source):
            expr["thumb"] = ""
            continue
        target_rel = f"thumbs/{expr['index']}.png"
        target = os.path.join(dest, "thumbs", f"{expr['index']}.png")
        os.makedirs(os.path.dirname(target), exist_ok=True)
        shutil.copyfile(source, target)
        expr["thumb"] = target_rel
    return profile


def skipped_expression_indexes(raw):
    """Indexes the free edition will not keep. Only values above the slot cap."""
    skipped = []
    if not isinstance(raw, dict):
        return skipped
    for item in raw.get("expressions") or []:
        if not isinstance(item, dict):
            continue
        try:
            index = int(item.get("index"))
        except (TypeError, ValueError):
            continue
        if index > SLOT_COUNT:
            skipped.append(index)
    return skipped


def import_notice(skipped, lang="en"):
    if not skipped:
        return ""
    indexes = ", ".join(str(index) for index in skipped)
    if lang == "ja":
        return f"無料版は表情 {SLOT_COUNT} つまでです。{indexes} は読み飛ばしました。"
    return f"Free edition keeps {SLOT_COUNT} expressions. Ignored indexes: {indexes}."


def _import_profile_dir(src, root):
    with open(os.path.join(src, "profile.json"), encoding="utf-8") as handle:
        raw = json.load(handle)
    skipped = skipped_expression_indexes(raw)
    profile = normalize_profile(raw)
    if profile is None:
        raise ValueError("invalid profile.json")
    saved = save_profile(_copy_thumbs(src, profile, root), root)
    if skipped:
        notice = import_notice(skipped)
        log(notice)
        saved["import_skipped"] = skipped
        saved["import_notice"] = notice
    return saved


def import_profile_pack(path, root=None):
    path = os.path.abspath(path)
    if os.path.isdir(path):
        return _import_profile_dir(path, root)
    if path.lower().endswith(".json"):
        return _import_profile_dir(os.path.dirname(path), root)
    if not zipfile.is_zipfile(path):
        raise ValueError("profile pack must be a .zip, a folder, or profile.json")
    import tempfile
    with tempfile.TemporaryDirectory(prefix="veh-profile-") as tmp:
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                name = info.filename.replace("\\", "/")
                parts = [part for part in name.split("/") if part not in {"", "."}]
                if name.startswith("/") or ".." in parts:
                    raise ValueError("unsafe path in profile pack")
            archive.extractall(tmp)
        found = None
        for dirpath, _dirs, files in os.walk(tmp):
            if "profile.json" in files:
                found = dirpath
                break
        if not found:
            raise ValueError("profile.json not found")
        return _import_profile_dir(found, root)


def find_profile_id(token, root=None):
    text = str(token or "").strip()
    if not text:
        return None
    safe = safe_avatar_id(text)
    if safe and os.path.isfile(os.path.join(profile_root(root), safe, "profile.json")):
        return safe
    if safe == DEFAULT_AVATAR_ID:
        return DEFAULT_AVATAR_ID
    for profile in list_profiles(root):
        if text in {profile["id"], profile.get("blueprint_id"), profile.get("name")}:
            return profile["id"]
    return None


def active_profile(settings, root=None):
    avatar_id = ""
    if isinstance(settings, dict):
        avatar_id = settings.get("active_avatar") or ""
    if avatar_id:
        profile = load_profile(avatar_id, root)
        if profile and (
            avatar_id == DEFAULT_AVATAR_ID
            or os.path.isfile(os.path.join(profile_root(root), profile["id"], "profile.json"))
        ):
            return profile
    return default_profile()


def activate_avatar(settings, avatar_id, root=None):
    profile = load_profile(avatar_id, root)
    if profile is None:
        return None
    settings["active_avatar"] = profile["id"]
    return profile


def normalize_assign(raw):
    out = {}
    if not isinstance(raw, dict):
        raw = {}
    for number, name in enumerate(FKEY_NAMES, start=1):
        value = raw.get(name, number)
        try:
            index = int(value)
        except (TypeError, ValueError):
            index = 0
        out[name] = index if 1 <= index <= SLOT_COUNT else 0
    return out


def expression_slot(index, profile, root=None):
    """Name, thumbnail, and OSC address for one expression index."""
    try:
        index = int(index)
    except (TypeError, ValueError):
        return None
    if index < 1 or index > SLOT_COUNT or not isinstance(profile, dict):
        return None
    expr = None
    for item in profile.get("expressions") or []:
        if isinstance(item, dict) and item.get("index") == index:
            expr = item
            break
    label = f"Expression{index}"
    if expr is None:
        return {"index": index, "label": label, "thumb": "", "osc": ""}
    path = profile_thumb_path(profile, expr, root)
    thumb = path if path and os.path.isfile(path) else ""
    osc = str(expr.get("osc") or "")
    label = str(expr.get("name") or "").strip() or label
    return {"index": index, "label": label, "thumb": thumb, "osc": osc}


def hotkeys_for(settings, root=None):
    profile = active_profile(settings, root)
    assign = normalize_assign(settings.get("fkey_assign") if isinstance(settings, dict) else None)
    hotkeys = {}
    for name, index in assign.items():
        if not index:
            continue
        slot = expression_slot(index, profile, root)
        if slot and slot["osc"]:
            hotkeys[name] = slot["osc"]
    return hotkeys


def _as_bool(value, default=True):
    if isinstance(value, str):
        return value.strip().casefold() not in {"0", "false", "no", "off"}
    if value is None:
        return default
    return bool(value)


def _string_list(value, default):
    if value is None:
        return list(default)
    if isinstance(value, str):
        text = value.strip()
        return [text] if text else list(default)
    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    return list(default)


def resolve_target(raw):
    if not isinstance(raw, dict):
        raw = {}
    unknown = str(raw.get("when_unknown") or "deny").strip().lower()
    if unknown not in {"allow", "deny"}:
        unknown = "deny"
    return {
        "enabled": _as_bool(raw.get("enabled", True), True),
        "windows_process": _string_list(
            raw.get("windows_process"), DEFAULT_TARGET["windows_process"]),
        "linux_match": _string_list(raw.get("linux_match"), DEFAULT_TARGET["linux_match"]),
        "when_unknown": unknown,
    }


def resolve_config(settings, root=None):
    block = settings if isinstance(settings, dict) else {}
    return {
        "enabled": _as_bool(block.get("enabled", True), True),
        "target": resolve_target(block.get("target")),
        "hotkeys": hotkeys_for(block, root),
    }


def avatar_listen_port(settings):
    block = settings.get("avatar_osc") if isinstance(settings, dict) else {}
    if not isinstance(block, dict):
        block = {}
    try:
        port = int(block.get("port", AVATAR_LISTEN_PORT))
    except (TypeError, ValueError):
        return AVATAR_LISTEN_PORT
    return port if 1 <= port <= 65535 else AVATAR_LISTEN_PORT


class ChordTracker:
    """Shift+F chord. TRUE once on press, FALSE on release, no repeat."""

    def __init__(self):
        self._shift = {}
        self._latched = {}
        self._blocked = {}

    def shift_held(self):
        return any(self._shift.values())

    def would_press(self, device, code, value, fkeys):
        if code in IGNORED_EVDEV or code in IGNORED_VK:
            return False
        if value != 1:
            return False
        name = fkeys.get(code)
        if not name or not self.shift_held():
            return False
        blocked = self._blocked.get(device)
        if blocked and name in blocked:
            return False
        latched = self._latched.get(device)
        return not latched or name not in latched

    def note_evdev(self, device, code, value, accept_press=True):
        if code in IGNORED_EVDEV:
            return []
        return self._note(device, code, value, SHIFT_EVDEV, EVDEV_TO_FKEY, accept_press)

    def note_vk(self, device, vk, is_up, accept_press=True):
        if vk in IGNORED_VK:
            return []
        value = 0 if is_up else 1
        return self._note(device, vk, value, SHIFT_VK, VK_TO_FKEY, accept_press)

    def drop_device(self, device):
        self._shift.pop(device, None)
        self._blocked.pop(device, None)
        released = self._take(device)
        if not self.shift_held():
            released.extend(self._take_all())
        return released

    def _note(self, device, code, value, shift_codes, fkeys, accept_press):
        if code in shift_codes:
            return self._set_shift(device, code, value != 0)
        name = fkeys.get(code)
        if name is None:
            return []
        if value == 2:
            return []
        if value == 1:
            blocked = self._blocked.get(device)
            if blocked and name in blocked:
                return []
            if not self.shift_held():
                return []
            latched = self._latched.get(device)
            if latched and name in latched:
                return []
            if not accept_press:
                self._blocked.setdefault(device, set()).add(name)
                return []
            self._latched.setdefault(device, set()).add(name)
            return [(name, True)]
        if value == 0:
            blocked = self._blocked.get(device)
            if blocked and name in blocked:
                blocked.discard(name)
                if not blocked:
                    self._blocked.pop(device, None)
                return []
            latched = self._latched.get(device)
            if not latched or name not in latched:
                return []
            latched.discard(name)
            if not latched:
                self._latched.pop(device, None)
            return [(name, False)]
        return []

    def _set_shift(self, device, code, down):
        held = self._shift.setdefault(device, set())
        if down:
            held.add(code)
            return []
        held.discard(code)
        if not held:
            self._shift.pop(device, None)
        if not self.shift_held():
            return self._take_all()
        return []

    def _take(self, device):
        names = self._latched.pop(device, ())
        return [(name, False) for name in sorted(names, key=lambda item: int(item[1:]))]

    def _take_all(self):
        events = []
        for device in list(self._latched):
            events.extend(self._take(device))
        return events


class FocusGate:
    def __init__(self, target, detect):
        self.target = target
        self.detect = detect
        self.last_state = None
        self._warned = False

    def allows_press(self):
        if not self.target["enabled"]:
            self.last_state = True
            return True
        try:
            state = self.detect()
        except Exception as exc:
            log(f"focus check failed: {exc}")
            state = None
        self.last_state = state
        if state is None:
            self._warn()
            return self.target["when_unknown"] == "allow"
        return bool(state)

    def _warn(self):
        if self._warned:
            return
        self._warned = True
        if self.target["when_unknown"] == "allow":
            action = "Presses are sent anyway."
        else:
            action = "Presses are ignored until the focused app can be read."
        message = "WARNING: cannot tell which window is focused. " + action
        log(message)
        print(message, file=sys.stderr)


def observe_evdev(tracker, gate, device, code, value):
    would = tracker.would_press(device, code, value, EVDEV_TO_FKEY)
    accept = gate.allows_press() if would else True
    return tracker.note_evdev(device, code, value, accept_press=accept)


def observe_vk(tracker, gate, device, vk, is_up):
    value = 0 if is_up else 1
    would = tracker.would_press(device, vk, value, VK_TO_FKEY)
    accept = gate.allows_press() if would else True
    return tracker.note_vk(device, vk, is_up, accept_press=accept)


def process_matches(image, names):
    if not image or not names:
        return False
    base = os.path.basename(str(image).replace("\\", "/")).casefold()
    wanted = {os.path.basename(str(name).replace("\\", "/")).casefold() for name in names}
    return base in wanted


def window_matches(fields, patterns):
    if not fields or not patterns:
        return False
    haystack = "\n".join(str(field) for field in fields if field).casefold()
    return any(str(pattern).casefold() in haystack for pattern in patterns if pattern)


def parse_hypr_active(raw):
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return None
    if not isinstance(data, dict):
        return None
    fields = []
    for key in ("class", "initialClass", "title", "initialTitle"):
        value = data.get(key)
        if isinstance(value, str) and value:
            fields.append(value)
    return fields


def parse_xprop_window(raw):
    fields = []
    for line in raw.splitlines():
        if "=" not in line:
            continue
        rest = line.split("=", 1)[1]
        for part in rest.split(","):
            text = part.strip().strip('"')
            if text and text != "0x0":
                fields.append(text)
    return fields or None


def _capture(argv, timeout=0.4):
    try:
        completed = subprocess.run(
            argv, check=False, capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if completed.returncode != 0:
        return None
    return completed.stdout


def linux_active_fields(run=None, which=None):
    run = run or _capture
    which = which or shutil.which
    if which("hyprctl"):
        raw = run(["hyprctl", "activewindow", "-j"])
        if raw:
            fields = parse_hypr_active(raw)
            if fields is not None:
                return fields
    if which("xdotool"):
        raw = run(["xdotool", "getactivewindow", "getwindowname", "getwindowclassname"])
        if raw:
            lines = [line.strip() for line in raw.splitlines() if line.strip()]
            if lines:
                return lines
    if which("xprop"):
        root = run(["xprop", "-root", "_NET_ACTIVE_WINDOW"])
        if root:
            window_id = ""
            for token in root.replace(",", " ").split():
                if token.startswith("0x") and token != "0x0":
                    window_id = token
            if window_id:
                detail = run(["xprop", "-id", window_id, "WM_CLASS", "_NET_WM_NAME"])
                if detail:
                    return parse_xprop_window(detail)
    return None


def linux_focused(target, run=None, which=None):
    fields = linux_active_fields(run=run, which=which)
    if fields is None:
        return None
    return window_matches(fields, target["linux_match"])


def query_foreground_image(user32, kernel32):
    hwnd = user32.GetForegroundWindow()
    if not hwnd:
        return None
    pid = ctypes.c_uint32()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    if not pid.value:
        return None
    handle = kernel32.OpenProcess(0x1000, False, pid.value)
    if not handle:
        return None
    try:
        size = ctypes.c_uint32(32768)
        buf = ctypes.create_unicode_buffer(size.value)
        if not kernel32.QueryFullProcessImageNameW(handle, 0, buf, ctypes.byref(size)):
            return None
        return buf.value
    finally:
        kernel32.CloseHandle(handle)


def windows_focused(user32, kernel32, target):
    image = query_foreground_image(user32, kernel32)
    if not image:
        return None
    return process_matches(image, target["windows_process"])


def probe_focus(target):
    if IS_WIN:
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        user32.GetForegroundWindow.restype = ctypes.c_void_p
        user32.GetWindowThreadProcessId.argtypes = [
            ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
        kernel32.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
        kernel32.OpenProcess.restype = ctypes.c_void_p
        kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
        kernel32.QueryFullProcessImageNameW.argtypes = [
            ctypes.c_void_p, ctypes.c_uint32, ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_uint32)]
        kernel32.QueryFullProcessImageNameW.restype = ctypes.c_int
        return windows_focused(user32, kernel32, target)
    if IS_LINUX:
        return linux_focused(target)
    return None


def _key_bits(fd):
    if fcntl is None:
        return b""
    length = (0x300 + 7) // 8
    buf = bytearray(length)
    try:
        fcntl.ioctl(fd, (2 << 30) | (length << 16) | (ord("E") << 8) | 0x21, buf)
    except OSError:
        return b""
    return bytes(buf)


def _bits_include(buf, code):
    index = code // 8
    if index >= len(buf):
        return False
    return bool(buf[index] & (1 << (code % 8)))


def looks_like_keyboard(buf):
    if _bits_include(buf, KEY_F1):
        return True
    return any(_bits_include(buf, code) for code in SHIFT_EVDEV)


def _try_open_keyboard(path):
    try:
        flags = os.O_RDONLY | os.O_NONBLOCK | getattr(os, "O_CLOEXEC", 0)
        fd = os.open(path, flags)
    except PermissionError:
        return "denied"
    except OSError:
        return None
    if not looks_like_keyboard(_key_bits(fd)):
        os.close(fd)
        return None
    return fd


def list_event_nodes():
    try:
        names = os.listdir("/dev/input")
    except OSError:
        return []
    return sorted(
        os.path.join("/dev/input", name) for name in names if name.startswith("event"))


def _valid_host(host):
    host = (host or "").strip()
    if not host or len(host) > 253 or host.startswith(".") or host.endswith("."):
        return False
    return all(ch.isascii() and (ch.isalnum() or ch in ".-:[]") for ch in host)


def _valid_port(text):
    text = (text or "").strip()
    if not text or (len(text) > 1 and text.startswith("0")):
        return None
    try:
        port = int(text)
    except ValueError:
        return None
    if 1 <= port <= 65535:
        return port
    return None


def classify_keyboards(open_count, denied_count):
    if open_count:
        return "ok"
    if denied_count:
        return "permission"
    return "none"


class LinuxKeySource:
    """Readable keyboards via evdev. The device is never grabbed."""

    def __init__(self):
        self.fds = {}
        self.pending = {}
        self._next_scan = 0.0
        self._perm_warned = False
        self.problem = "ok"
        self.rescan()

    def rescan(self):
        wanted = set(list_event_nodes())
        denied = []
        for path in sorted(wanted):
            if path in self.fds.values():
                continue
            opened = _try_open_keyboard(path)
            if opened == "denied":
                denied.append(path)
            elif isinstance(opened, int):
                self.fds[opened] = path
                self.pending[opened] = bytearray()
        dropped = []
        for fd, path in list(self.fds.items()):
            if path not in wanted:
                self._close_fd(fd)
                dropped.append(path)
        self.problem = classify_keyboards(len(self.fds), len(denied))
        if self.problem == "permission" and not self._perm_warned:
            self._perm_warned = True
            log(
                "no keyboard is readable "
                f"({', '.join(denied)}). Add your user to the input group and sign in again: "
                'sudo usermod -aG input "$USER"')
        self._next_scan = time.time() + 1.0
        return dropped

    def _close_fd(self, fd):
        self.fds.pop(fd, None)
        self.pending.pop(fd, None)
        with contextlib.suppress(OSError):
            os.close(fd)

    def poll(self, timeout):
        events = []
        if time.time() >= self._next_scan or not self.fds:
            for path in self.rescan():
                events.append(("drop", path))
        if not self.fds:
            if timeout:
                time.sleep(min(timeout, 0.25))
            return events
        ready, _, _ = select.select(list(self.fds), [], [], timeout)
        for fd in ready:
            try:
                blob = os.read(fd, INPUT_EVENT.size * 64)
            except OSError as exc:
                if exc.errno in (errno.EAGAIN, errno.EWOULDBLOCK):
                    continue
                path = self.fds.get(fd)
                self._close_fd(fd)
                if path:
                    events.append(("drop", path))
                continue
            if not blob:
                path = self.fds.get(fd)
                self._close_fd(fd)
                if path:
                    events.append(("drop", path))
                continue
            pending = self.pending.setdefault(fd, bytearray())
            pending.extend(blob)
            size = INPUT_EVENT.size
            while len(pending) >= size:
                _sec, _usec, ev_type, code, value = INPUT_EVENT.unpack_from(pending)
                del pending[:size]
                if ev_type != KERNEL_EV_KEY:
                    continue
                path = self.fds.get(fd)
                if path is not None:
                    events.append(("evdev", path, code, value))
        return events

    def close(self):
        for fd in list(self.fds):
            self._close_fd(fd)


class WinKeyHook:
    """Low-level keyboard hook. CallNextHookEx always, so keys are not swallowed."""

    def __init__(self):
        self.queue = queue.Queue()
        self.thread_id = None
        self._thread = threading.Thread(target=self._run, name="expr-hook", daemon=True)
        self._thread.start()

    def poll(self, timeout):
        items = []
        deadline = time.time() + max(0.0, timeout)
        while True:
            remaining = 0 if timeout == 0 else deadline - time.time()
            if remaining < 0:
                break
            try:
                items.append(self.queue.get(timeout=remaining))
            except queue.Empty:
                break
            if timeout == 0:
                while True:
                    try:
                        items.append(self.queue.get_nowait())
                    except queue.Empty:
                        return items
        return items

    def close(self):
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        for _ in range(25):
            if self.thread_id:
                user32.PostThreadMessageW(self.thread_id, 0x0012, 0, 0)
                break
            time.sleep(0.02)
        self._thread.join(timeout=1.5)

    def _run(self):
        self.thread_id = ctypes.windll.kernel32.GetCurrentThreadId()
        try:
            self._loop()
        except Exception as exc:
            log(f"keyboard hook failed: {exc}")

    def _loop(self):
        import ctypes.wintypes as wt

        user32 = ctypes.WinDLL("user32", use_last_error=True)
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

        class HookStruct(ctypes.Structure):
            _fields_ = [
                ("vkCode", wt.DWORD), ("scanCode", wt.DWORD),
                ("flags", wt.DWORD), ("time", wt.DWORD),
                ("dwExtraInfo", ctypes.c_size_t),
            ]

        class MSG(ctypes.Structure):
            _fields_ = [
                ("hwnd", wt.HWND), ("message", wt.UINT),
                ("wParam", wt.WPARAM), ("lParam", wt.LPARAM),
                ("time", wt.DWORD), ("pt_x", wt.LONG), ("pt_y", wt.LONG),
            ]

        hookproc = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, ctypes.c_int, wt.WPARAM, wt.LPARAM)
        user32.SetWindowsHookExW.argtypes = [ctypes.c_int, hookproc, wt.HMODULE, wt.DWORD]
        user32.SetWindowsHookExW.restype = ctypes.c_void_p
        user32.CallNextHookEx.argtypes = [ctypes.c_void_p, ctypes.c_int, wt.WPARAM, wt.LPARAM]
        user32.CallNextHookEx.restype = ctypes.c_ssize_t
        user32.UnhookWindowsHookEx.argtypes = [ctypes.c_void_p]
        user32.GetMessageW.argtypes = [ctypes.POINTER(MSG), wt.HWND, wt.UINT, wt.UINT]
        user32.GetMessageW.restype = ctypes.c_int
        kernel32.GetModuleHandleW.argtypes = [wt.LPCWSTR]
        kernel32.GetModuleHandleW.restype = wt.HMODULE
        state = {"hook": None}

        @hookproc
        def low_level(n_code, w_param, l_param):
            try:
                if n_code >= 0 and l_param:
                    info = ctypes.cast(l_param, ctypes.POINTER(HookStruct)).contents
                    vk = int(info.vkCode)
                    if vk in SHIFT_VK or vk in VK_TO_FKEY:
                        is_up = bool(info.flags & LLKHF_UP) or w_param in (WM_KEYUP, WM_SYSKEYUP)
                        self.queue.put(("vk", vk, is_up))
            except Exception as exc:
                log(f"hook error: {exc}")
            return user32.CallNextHookEx(state["hook"], n_code, w_param, l_param)

        state["proc"] = low_level
        module = kernel32.GetModuleHandleW(None)
        hook = user32.SetWindowsHookExW(WH_KEYBOARD_LL, low_level, module, 0)
        if not hook:
            log(f"SetWindowsHookExW failed ({ctypes.get_last_error()})")
            return
        state["hook"] = hook
        msg = MSG()
        try:
            while True:
                result = user32.GetMessageW(ctypes.byref(msg), None, 0, 0)
                if result in (0, -1):
                    break
                user32.TranslateMessage(ctypes.byref(msg))
                user32.DispatchMessageW(ctypes.byref(msg))
        finally:
            user32.UnhookWindowsHookEx(hook)


def open_key_source():
    try:
        if IS_WIN:
            return WinKeyHook()
        if IS_LINUX:
            return LinuxKeySource()
    except Exception as exc:
        log(f"key source unavailable: {exc}")
    return None


def _settings_mtime():
    try:
        return os.path.getmtime(CONFIG_FILE)
    except OSError:
        return None


class ExpressionListener:
    def __init__(self):
        self.tracker = ChordTracker()
        self.last = ""
        self.focused = None
        self._stop = threading.Event()
        self._thread = None
        self._lock = threading.Lock()
        self.key_problem = "ok"

    def is_running(self):
        return bool(self._thread and self._thread.is_alive())

    def start(self):
        with self._lock:
            if self.is_running():
                return
            self._stop.clear()
            self._thread = threading.Thread(target=self._loop, name="expressions", daemon=True)
            self._thread.start()

    def stop(self):
        self._stop.set()
        thread = self._thread
        if thread is not None:
            thread.join(timeout=2.0)
        with self._lock:
            if self._thread is thread:
                self._thread = None

    def apply_evdev(self, cfg, osc, gate, device, code, value):
        self._emit(cfg, osc, gate, observe_evdev(self.tracker, gate, device, code, value))

    def apply_vk(self, cfg, osc, gate, vk, is_up):
        self._emit(cfg, osc, gate, observe_vk(self.tracker, gate, "win", vk, is_up))

    def _emit(self, cfg, osc, gate, events):
        for name, down in events:
            address = cfg["hotkeys"].get(name)
            if not address:
                continue
            osc.send_bool(address, down)
            state = "DOWN" if down else "UP"
            self.last = f"Shift+{name} {state}"
            self.focused = gate.last_state
            log(f"{self.last} -> {address}")

    def _dispatch(self, cfg, osc, gate, item):
        kind = item[0]
        if kind == "drop":
            self._emit(cfg, osc, gate, self.tracker.drop_device(item[1]))
        elif kind == "evdev":
            self.apply_evdev(cfg, osc, gate, item[1], item[2], item[3])
        elif kind == "vk":
            self.apply_vk(cfg, osc, gate, item[1], item[2])

    def _loop(self):
        source = None
        mtime = None
        cfg = None
        osc = None
        gate = None
        try:
            while not self._stop.is_set():
                current = _settings_mtime()
                if cfg is None or current != mtime:
                    mtime = current
                    settings = load_settings()
                    cfg = resolve_config(settings)
                    osc = osc_from_settings(settings)
                    target = cfg["target"]
                    gate = FocusGate(target, lambda target=target: probe_focus(target))
                if not cfg["enabled"]:
                    if source is not None:
                        source.close()
                        source = None
                    self.key_problem = "ok"
                    self._stop.wait(0.4)
                    continue
                if source is None:
                    source = open_key_source()
                if source is None:
                    self.key_problem = "unavailable"
                    self._stop.wait(1.0)
                    continue
                try:
                    batch = source.poll(0.25)
                    self.key_problem = getattr(source, "problem", "ok")
                except Exception as exc:
                    log(f"key read failed: {exc}")
                    with contextlib.suppress(Exception):
                        source.close()
                    source = None
                    continue
                for item in batch:
                    self._dispatch(cfg, osc, gate, item)
        finally:
            if source is not None:
                with contextlib.suppress(Exception):
                    source.close()


_listener = None


def expression_listener():
    global _listener
    if _listener is None:
        _listener = ExpressionListener()
    return _listener


def start_expression_listener():
    expression_listener().start()


def stop_expression_listener():
    if _listener is not None:
        _listener.stop()


class AvatarOscListener:
    """Listen for /avatar/change. A busy port is reported and is not fatal."""

    def __init__(self, port, on_change, host="127.0.0.1"):
        self.port = int(port)
        self.host = host
        self.on_change = on_change
        self.error = ""
        self.ok = False
        self._sock = None
        self._stop = threading.Event()
        self._thread = None

    def start(self):
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            sock.bind((self.host, self.port))
        except OSError as exc:
            self.error = str(exc)
            self.ok = False
            sock.close()
            log(f"avatar OSC port {self.port} is busy: {self.error}")
            return False
        sock.settimeout(0.3)
        self._sock = sock
        self.error = ""
        self.ok = True
        self._thread = threading.Thread(target=self._loop, name="avatar-osc", daemon=True)
        self._thread.start()
        return True

    def stop(self):
        self._stop.set()
        sock = self._sock
        self._sock = None
        if sock is not None:
            with contextlib.suppress(OSError):
                sock.close()
        thread = self._thread
        self._thread = None
        if thread is not None and thread is not threading.current_thread():
            thread.join(timeout=1)

    @property
    def running(self):
        return self._thread is not None and self._thread.is_alive()

    def _loop(self):
        while not self._stop.is_set():
            sock = self._sock
            if sock is None:
                break
            try:
                data, _addr = sock.recvfrom(4096)
            except socket.timeout:
                continue
            except OSError:
                break
            parsed = parse_osc_packet(data)
            if not parsed:
                continue
            address, value, kind = parsed
            if not str(address).endswith("/avatar/change"):
                continue
            if kind != "string" or not str(value).strip():
                continue
            try:
                self.on_change(str(value).strip())
            except Exception as exc:
                log(f"avatar change failed: {exc}")


def note_avatar_change(avatar_id, root=None):
    settings = load_settings()
    found = find_profile_id(avatar_id, root)
    if not found:
        log(f"avatar {avatar_id} has no profile")
        return None
    profile = activate_avatar(settings, found, root)
    save_settings(settings)
    log(f"avatar -> {found}")
    return profile


def run_listen():
    log(f"listener starting (pid {os.getpid()})")
    start_expression_listener()
    port = avatar_listen_port(load_settings())
    avatar = AvatarOscListener(port, lambda avatar_id: note_avatar_change(avatar_id))
    avatar.start()
    try:
        while expression_listener().is_running():
            time.sleep(0.2)
    except KeyboardInterrupt:
        pass
    finally:
        avatar.stop()
        stop_expression_listener()


def run_gui():
    try:
        import tkinter as tk
        from tkinter import filedialog, ttk
    except Exception as exc:
        print(f"Tkinter is required for the window: {exc}", file=sys.stderr)
        return 1

    class App:
        def __init__(self, root):
            self.root = root
            self.settings = load_settings()
            self.lang = self.settings.get("lang", "en")
            self._photos = {}
            self._updating = False
            self._avatar_ids = []
            self._avatar_listener = None
            root.title(self.tr("subtitle"))
            root.geometry("720x640")
            root.minsize(560, 480)
            self._build()
            self._reload_rows()
            self._sync_listener()
            self._sync_avatar()
            self._poll()
            shot = os.environ.get("VEH_GUI_SHOT")
            if shot:
                root.after(500, lambda: self._shot(shot))

        def tr(self, key):
            table = {
                "en": {
                    "enable": "Listen for Shift+F1..F4",
                    "filter": "Only while VRChat is in front",
                    "avatar": "Avatar",
                    "import": "Import avatar profile",
                    "listen": "Avatar OSC port",
                    "unassigned": "(unassigned)",
                    "busy": "Avatar OSC port is busy. Forward /avatar/change with an OSC router.",
                    "ok": "Listening for /avatar/change",
                    "running": "Listening",
                    "stopped": "Stopped",
                    "last": "Last",
                    "subtitle": "VRChat expression hotkeys (F1-F4)",
                    "host": "Host",
                    "port": "Port",
                    "language": "Language",
                    "port_invalid": "Enter a number from 1 to 65535.",
                    "host_invalid": "Enter a host name or an IP address.",
                    "key_permission": (
                        "The keyboard cannot be read. Add your user to the input group, "
                        'then sign in again. Example: sudo usermod -aG input "$USER"'
                    ),
                    "key_none": (
                        "No keyboard was found. Check the connection, "
                        "then turn listening off and on."
                    ),
                    "key_unsupported": (
                        "This OS cannot listen for keys. Assignments can still be edited."
                    ),
                    "none": "(none)",
                    "import_bad_pack": "Import a .zip, a folder, or profile.json.",
                    "import_unsafe": "The profile pack contains an unsafe path.",
                    "import_missing": "profile.json was not found.",
                },
                "ja": {
                    "enable": "Shift+F1..F4 を待つ",
                    "filter": "VRChat が前面のときだけ",
                    "avatar": "アバター",
                    "import": "アバタープロファイルを読み込む",
                    "listen": "アバター OSC ポート",
                    "unassigned": "(未割り当て)",
                    "busy": "アバター OSC ポートが使用中です。OSC ルーターで転送してください。",
                    "ok": "/avatar/change を待受中",
                    "running": "待受中",
                    "stopped": "停止",
                    "last": "最後",
                    "subtitle": "VRChat の表情ホットキー (F1-F4)",
                    "host": "ホスト",
                    "port": "ポート",
                    "language": "言語",
                    "port_invalid": "1〜65535 の数字を入れてください。",
                    "host_invalid": "ホスト名か IP アドレスを入れてください。",
                    "key_permission": (
                        "キーボードを読めません。ユーザーを input グループに入れて、"
                        'ログインし直してください。端末での例: sudo usermod -aG input "$USER"'
                    ),
                    "key_none": (
                        "キーボードが見つかりません。接続を確認し、待受を一度止めてから入れ直してください。"
                    ),
                    "key_unsupported": (
                        "この OS ではキーの待受に対応していません。割り当ては編集できます。"
                    ),
                    "none": "(なし)",
                    "import_bad_pack": "読み込めるのは zip、フォルダ、profile.json です。",
                    "import_unsafe": "プロファイルの中に安全でないパスがあります。",
                    "import_missing": "profile.json が見つかりません。",
                },
            }
            lang = table.get(self.lang, table["en"])
            return lang.get(key, table["en"].get(key, key))

        def _build(self):
            top = ttk.Frame(self.root)
            top.pack(fill="x", padx=8, pady=8)
            self.enabled = tk.BooleanVar(value=_as_bool(self.settings.get("enabled", True), True))
            self.enable_btn = ttk.Checkbutton(top, text=self.tr("enable"), variable=self.enabled)
            self.enable_btn.pack(anchor="w")
            target = resolve_target(self.settings.get("target"))
            self.vrchat_only = tk.BooleanVar(value=target["enabled"])
            self.filter_btn = ttk.Checkbutton(
                top, text=self.tr("filter"), variable=self.vrchat_only)
            self.filter_btn.pack(anchor="w")

            host_row = ttk.Frame(self.root)
            host_row.pack(fill="x", padx=8, pady=2)
            osc = self.settings.get("osc") if isinstance(self.settings.get("osc"), dict) else {}
            self.host_lbl = ttk.Label(host_row, text=self.tr("host"))
            self.host_lbl.pack(side="left")
            self.osc_host = tk.StringVar(value=str(osc.get("host") or "127.0.0.1"))
            host_entry = ttk.Entry(host_row, textvariable=self.osc_host, width=16)
            host_entry.pack(side="left", padx=4)
            host_entry.bind("<FocusOut>", lambda _event: self._commit_osc())
            self.port_lbl = ttk.Label(host_row, text=self.tr("port"))
            self.port_lbl.pack(side="left")
            self.osc_port = tk.StringVar(value=str(osc.get("port") or OSC_PORT))
            port_entry = ttk.Entry(host_row, textvariable=self.osc_port, width=6)
            port_entry.pack(side="left", padx=4)
            port_entry.bind("<FocusOut>", lambda _event: self._commit_osc())
            self.listen_lbl = ttk.Label(host_row, text=self.tr("listen"))
            self.listen_lbl.pack(side="left", padx=(12, 4))
            self.listen_var = tk.StringVar(value=str(avatar_listen_port(self.settings)))
            listen_entry = ttk.Entry(host_row, textvariable=self.listen_var, width=6)
            listen_entry.pack(side="left")
            listen_entry.bind("<FocusOut>", lambda _event: self._commit_listen())

            avatar_row = ttk.Frame(self.root)
            avatar_row.pack(fill="x", padx=8, pady=4)
            self.avatar_lbl = ttk.Label(avatar_row, text=self.tr("avatar"))
            self.avatar_lbl.pack(side="left")
            self.avatar_var = tk.StringVar()
            self.avatar_box = ttk.Combobox(
                avatar_row, textvariable=self.avatar_var, state="readonly", width=36)
            self.avatar_box.pack(side="left", padx=4)
            self.avatar_box.bind("<<ComboboxSelected>>", self.on_avatar)
            self.import_btn = ttk.Button(
                avatar_row, text=self.tr("import"), command=self.on_import)
            self.import_btn.pack(side="left")
            self.lang_lbl = ttk.Label(avatar_row, text=self.tr("language"))
            self.lang_lbl.pack(side="left", padx=(12, 4))
            self.lang_var = tk.StringVar(value="日本語" if self.lang == "ja" else "English")
            self.lang_box = ttk.Combobox(
                avatar_row, textvariable=self.lang_var, state="readonly", width=12,
                values=["日本語", "English"])
            self.lang_box.pack(side="left")
            self.lang_box.bind("<<ComboboxSelected>>", self.on_lang)

            self.field_error = tk.Label(
                self.root, anchor="w", justify="left", wraplength=680,
                fg="#7A1430", bg="#FFE8F0")
            self.warn = tk.Label(
                self.root, anchor="w", justify="left", wraplength=680,
                fg="#7A1430", bg="#FFE8F0")
            self.status = tk.Label(self.root, anchor="w", justify="left")
            self.status.pack(fill="x", padx=8)
            body = ttk.Frame(self.root)
            body.pack(fill="both", expand=True, padx=8, pady=8)
            self.vars = {}
            self.boxes = {}
            self.thumbs = {}
            for row, name in enumerate(FKEY_NAMES):
                ttk.Label(body, text=name, width=4).grid(row=row, column=0, sticky="w")
                thumb = tk.Label(body)
                thumb.grid(row=row, column=1, padx=4)
                var = tk.StringVar()
                box = ttk.Combobox(body, textvariable=var, state="readonly", width=36)
                box.grid(row=row, column=2, sticky="we", pady=1)
                box.bind("<<ComboboxSelected>>", lambda _e, n=name: self.on_pick(n))
                self.vars[name] = var
                self.boxes[name] = box
                self.thumbs[name] = thumb
            body.columnconfigure(2, weight=1)
            self.enabled.trace_add("write", lambda *_n: self._save_flags())
            self.vrchat_only.trace_add("write", lambda *_n: self._save_flags())
            self.osc_host.trace_add("write", lambda *_n: self._show_field_errors())
            self.osc_port.trace_add("write", lambda *_n: self._show_field_errors())
            self.listen_var.trace_add("write", lambda *_n: self._show_field_errors())

        def _profile(self):
            return active_profile(self.settings)

        def _labels(self, profile):
            labels = [self.tr("unassigned")]
            indexes = [0]
            for expr in profile["expressions"]:
                slot = expression_slot(expr["index"], profile)
                label = slot["label"] if slot else f"Expression{expr['index']}"
                labels.append(f"{expr['index']}  {label}")
                indexes.append(expr["index"])
            return labels, indexes

        def _photo(self, hit, target=28):
            path = hit.get("thumb") if hit else ""
            if path and os.path.isfile(path):
                key = (path, target)
                cached = self._photos.get(key)
                if cached is not None:
                    return cached
                try:
                    image = tk.PhotoImage(file=path)
                    factor = max(1, image.width() // target)
                    if factor > 1:
                        image = image.subsample(factor, factor)
                except tk.TclError:
                    image = None
                if image is not None:
                    self._photos[key] = image
                    return image
            key = ("generic", target)
            cached = self._photos.get(key)
            if cached is not None:
                return cached
            image = tk.PhotoImage(width=target, height=target)
            image.put("#3d4c5c", to=(0, 0, target, target))
            margin = max(2, target // 8)
            image.put("#d7c4a3", to=(margin, margin, target - margin, target - margin))
            self._photos[key] = image
            return image

        def _reload_rows(self):
            profiles = list_profiles()
            self._avatar_ids = [item["id"] for item in profiles]
            labels = []
            for item in profiles:
                if item["id"] == DEFAULT_AVATAR_ID:
                    labels.append(item["name"])
                else:
                    labels.append(f"{item['name']} ({item['id']})")
            current = self.settings.get("active_avatar") or DEFAULT_AVATAR_ID
            if current not in self._avatar_ids:
                current = DEFAULT_AVATAR_ID
            self._updating = True
            try:
                self.avatar_box.configure(values=labels)
                self.avatar_var.set(labels[self._avatar_ids.index(current)])
                profile = self._profile()
                choices, indexes = self._labels(profile)
                self._indexes = indexes
                assign = normalize_assign(self.settings.get("fkey_assign"))
                for name, var in self.vars.items():
                    self.boxes[name].configure(values=choices)
                    chosen = assign.get(name, 0)
                    var.set(choices[indexes.index(chosen)] if chosen in indexes else choices[0])
                    slot = expression_slot(chosen, profile) if chosen else None
                    photo = self._photo(slot) if slot else None
                    self.thumbs[name].configure(image=photo or "")
                    self.thumbs[name].image = photo
            finally:
                self._updating = False

        def _save_flags(self):
            if self._updating:
                return
            self.settings["enabled"] = bool(self.enabled.get())
            target = self.settings.setdefault("target", {})
            if not isinstance(target, dict):
                target = {}
                self.settings["target"] = target
            target["enabled"] = bool(self.vrchat_only.get())
            save_settings(self.settings)
            self._sync_listener()

        def _commit_osc(self):
            if self._updating:
                return
            host = self.osc_host.get().strip()
            port = _valid_port(self.osc_port.get())
            osc = self.settings.setdefault("osc", {})
            if not isinstance(osc, dict):
                osc = {}
                self.settings["osc"] = osc
            changed = False
            if _valid_host(host) and osc.get("host") != host:
                osc["host"] = host
                self.osc_host.set(host)
                changed = True
            if port is not None and osc.get("port") != port:
                osc["port"] = port
                self.osc_port.set(str(port))
                changed = True
            if changed:
                save_settings(self.settings)
            self._show_field_errors()

        def _commit_listen(self):
            if self._updating:
                return
            port = _valid_port(self.listen_var.get())
            if port is None:
                self._show_field_errors()
                return
            self.listen_var.set(str(port))
            if avatar_listen_port(self.settings) == port:
                self._show_field_errors()
                return
            block = self.settings.setdefault("avatar_osc", {})
            if not isinstance(block, dict):
                block = {}
                self.settings["avatar_osc"] = block
            block["port"] = port
            save_settings(self.settings)
            self._sync_avatar()
            self._show_field_errors()

        def _show_field_errors(self):
            notes = []
            if not _valid_host(self.osc_host.get()):
                notes.append(self.tr("host_invalid"))
            port_bad = _valid_port(self.osc_port.get()) is None
            listen_bad = _valid_port(self.listen_var.get()) is None
            if port_bad or listen_bad:
                notes.append(self.tr("port_invalid"))
            self._show_note(self.field_error, "\n".join(notes))

        def _sync_listener(self):
            if _as_bool(self.settings.get("enabled", True), True):
                start_expression_listener()
            else:
                stop_expression_listener()

        def _sync_avatar(self):
            port = avatar_listen_port(self.settings)
            current = self._avatar_listener
            if current is not None and current.port == port and current.running:
                return
            if current is not None:
                current.stop()
            listener = AvatarOscListener(port, self._on_avatar_message)
            self._avatar_listener = listener
            listener.start()

        def _on_avatar_message(self, avatar_id):
            profile = note_avatar_change(avatar_id)

            def apply():
                self.settings = load_settings()
                self._reload_rows()
                if profile is not None:
                    self.status.configure(text=profile["name"])

            with contextlib.suppress(tk.TclError):
                self.root.after(0, apply)

        def on_avatar(self, _event=None):
            if self._updating:
                return
            index = self.avatar_box.current()
            if index < 0 or index >= len(self._avatar_ids):
                return
            activate_avatar(self.settings, self._avatar_ids[index])
            save_settings(self.settings)
            self._reload_rows()

        def on_import(self):
            path = filedialog.askopenfilename(
                parent=self.root, filetypes=[("Profile", "*.zip *.json"), ("All", "*")])
            if not path:
                return
            try:
                profile = import_profile_pack(path)
            except (OSError, ValueError, json.JSONDecodeError) as exc:
                self.status.configure(text=self._present_error(str(exc)))
                return
            activate_avatar(self.settings, profile["id"])
            save_settings(self.settings)
            self._reload_rows()
            skipped = profile.get("import_skipped") or []
            notice = import_notice(skipped, self.lang) if skipped else profile["name"]
            self.status.configure(text=notice)

        def on_pick(self, name):
            if self._updating:
                return
            label = self.vars[name].get()
            choices, indexes = self._labels(self._profile())
            index = indexes[choices.index(label)] if label in choices else 0
            store = self.settings.setdefault("fkey_assign", {})
            if index:
                store[name] = index
            else:
                store[name] = 0
            save_settings(self.settings)
            self._reload_rows()

        def on_lang(self, _event=None):
            picked = self.lang_var.get()
            self.lang = "ja" if picked == "日本語" else "en"
            self.lang_var.set("日本語" if self.lang == "ja" else "English")
            self.settings["lang"] = self.lang
            save_settings(self.settings)
            self.root.title(self.tr("subtitle"))
            self.enable_btn.configure(text=self.tr("enable"))
            self.filter_btn.configure(text=self.tr("filter"))
            self.avatar_lbl.configure(text=self.tr("avatar"))
            self.import_btn.configure(text=self.tr("import"))
            self.listen_lbl.configure(text=self.tr("listen"))
            self.host_lbl.configure(text=self.tr("host"))
            self.port_lbl.configure(text=self.tr("port"))
            self.lang_lbl.configure(text=self.tr("language"))
            self._show_field_errors()
            self._reload_rows()

        def _show_note(self, label, text):
            label.configure(text=text)
            mapped = bool(label.winfo_ismapped())
            if text and not mapped:
                label.pack(fill="x", padx=8, pady=(2, 0), before=self.status)
            elif not text and mapped:
                label.pack_forget()

        def _present_error(self, err):
            key = {
                "profile pack must be a .zip, a folder, or profile.json": "import_bad_pack",
                "unsafe path in profile pack": "import_unsafe",
                "profile.json not found": "import_missing",
            }.get(err)
            return self.tr(key) if key else err

        def _poll(self):
            listener = expression_listener()
            state = self.tr("running") if listener.is_running() else self.tr("stopped")
            last = listener.last or self.tr("none")
            avatar = self._avatar_listener
            if avatar is not None and avatar.ok:
                port_state = self.tr("ok")
            elif avatar is not None and avatar.error:
                port_state = self.tr("busy")
            else:
                port_state = ""
            self.status.configure(text=f"{state} | {self.tr('last')}: {last} {port_state}")
            problem = listener.key_problem if self.enabled.get() else "ok"
            warning = {
                "permission": "key_permission",
                "none": "key_none",
                "unavailable": "key_unsupported",
            }.get(problem, "")
            self._show_note(self.warn, self.tr(warning) if warning else "")
            self.root.after(400, self._poll)

        def _shot(self, path):
            self.root.update()
            try:
                from PIL import ImageGrab
                image = ImageGrab.grab(bbox=(
                    self.root.winfo_rootx(), self.root.winfo_rooty(),
                    self.root.winfo_rootx() + self.root.winfo_width(),
                    self.root.winfo_rooty() + self.root.winfo_height()))
                os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
                image.save(path)
            except Exception:
                pass
            self.root.after(200, self.on_close)

        def on_close(self):
            stop_expression_listener()
            if self._avatar_listener is not None:
                self._avatar_listener.stop()
            self.root.destroy()

    root = tk.Tk()
    app = App(root)
    root.app = app
    root.protocol("WM_DELETE_WINDOW", app.on_close)
    root.mainloop()
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        prog=APP_NAME,
        description="Free VRChat expression hotkeys (Shift+F1..F4)")
    parser.add_argument("--version", action="version", version=f"{APP_NAME} {VERSION}")
    parser.add_argument(
        "--listen", action="store_true",
        help="run the chord listener without the window")
    args = parser.parse_args(argv)
    if args.listen:
        try:
            run_listen()
        except KeyboardInterrupt:
            return 0
        return 0
    return run_gui()


if __name__ == "__main__":
    sys.exit(main())
