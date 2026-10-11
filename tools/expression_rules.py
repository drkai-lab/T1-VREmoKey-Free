"""Face-expression rules for the free edition.

The Unity editor applies the same decisions. This module parses Unity YAML
so the decisions, the FX embed shape, and the profile document can be
checked without the editor.

A clip is a candidate when it moves a face-mesh blend shape. Clips that are
only a blink, a viseme, a mouth reset, a hand pose, empty, or a default
face are excluded and keep a reason, so a person can put them back.
"""

from __future__ import annotations

import re
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

SLOT_COUNT = 4
SYNC_LIMIT = 256
LAYER_TAG = "VREmoKey"

REASONS = {
    "empty": "空のクリップ",
    "hand": "手のポーズだけ",
    "mouth_reset": "口リセット",
    "blink": "まばたき",
    "viseme": "Viseme",
    "default_face": "DefaultFace 相当",
    "not_face": "顔の blendShape を動かさない",
    "expression": "",
}

_ANIMATOR_TYPE = {1: "float", 3: "int", 4: "bool", 9: "trigger"}
_EXPR_TYPE = {0: "int", 1: "float", 2: "bool"}
_VISEME_TOKENS = {
    "sil",
    "pp",
    "ff",
    "th",
    "dd",
    "kk",
    "ch",
    "ss",
    "nn",
    "rr",
    "aa",
    "e",
    "ih",
    "oh",
    "ou",
}
_EYE_HINTS = (
    "blink",
    "pupil",
    "iris",
    "highlight",
    "high_light",
    "eyeclose",
    "eye_close",
    "eyes_close",
    "まばたき",
    "瞳",
    "ハイライト",
)
_BLINK_HINTS = (
    "blink",
    "まばたき",
    "eyeclose",
    "eye_close",
    "eyes_close",
    "目閉じ",
    "目を閉じ",
)
_MOUTH_HINTS = ("mouth", "lip", "viseme", "vrc.v_", "口", "jaw")
_FACIAL_HINTS = _EYE_HINTS + _MOUTH_HINTS + ("brow", "眉", "笑", "頬", "cheek")


@dataclass
class Curve:
    attribute: str
    path: str
    class_id: int
    value: float


@dataclass
class Clip:
    name: str
    path: str
    guid: str
    curves: list[Curve]
    other_motion: bool


@dataclass
class Verdict:
    name: str
    path: str
    guid: str
    kind: str
    include: bool
    reason: str
    source: str


@dataclass
class Scan:
    fx_name: str
    clips: list[Verdict]
    write_defaults: bool
    write_on: int
    write_off: int
    synced_bits: int | None
    hand_layers: list[str]
    layers: list[str]
    unresolved_fx_clips: int
    controller: dict = field(default_factory=dict)
    expression_parameters: list | None = None
    epsilon: float = 8.0


def parameter_name(index: int) -> str:
    return f"Expression{index}"


def osc_address(parameter: str) -> str:
    return "/avatar/parameters/" + parameter


def quote(value: str | None) -> str:
    if value is None:
        value = ""
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def sanitize(value: str | None) -> str:
    if not value:
        return ""
    chars = [
        ch
        for ch in value.strip()
        if ch.isalnum() or ch in "._-"
    ]
    text = "".join(chars)
    return text[:128]


def build_profile(avatar_id: str, display_name: str, blueprint_id: str, rows: list[dict]) -> str:
    """Same document the Unity exporter writes. Indexes outside 1..4 are dropped."""
    lines = []
    for row in rows or []:
        if row is None:
            continue
        index = int(row.get("index", 0))
        name = row.get("name") or ""
        if index < 1 or index > SLOT_COUNT or not name:
            continue
        lines.append(
            "      {"
            + '"index": '
            + str(index)
            + ', "name": '
            + quote(name)
            + ', "parameter": '
            + quote(row.get("parameter") or "")
            + ', "osc": '
            + quote(row.get("osc") or "")
            + ', "type": "bool", "thumb": '
            + quote(row.get("thumb") or "")
            + "}"
        )
    body = ",\n".join(lines)
    return (
        "{\n"
        + '  "id": '
        + quote(avatar_id)
        + ",\n"
        + '  "name": '
        + quote(display_name)
        + ",\n"
        + '  "blueprint_id": '
        + quote(blueprint_id)
        + ",\n"
        + '  "expressions": [\n'
        + body
        + "\n  ]\n}\n"
    )


def profile_rows(names: list[str]) -> list[dict]:
    rows = []
    for offset, name in enumerate(names[:SLOT_COUNT], start=1):
        parameter = parameter_name(offset)
        rows.append(
            {
                "index": offset,
                "name": name,
                "parameter": parameter,
                "osc": osc_address(parameter),
                "thumb": f"thumbs/{offset}.png",
            }
        )
    return rows


def weight_epsilon(values: list[float]) -> float:
    """Blend shapes are either 0..100 or 0..1. Ignore rest poses under 8 percent."""
    if any(abs(value) > 1.5 for value in values):
        return 8.0
    return 0.08


def parse_clip_text(text: str) -> tuple[str, list[Curve], bool]:
    name_match = re.search(r"^  m_Name: (.*)$", text, re.M)
    name = _unescape(name_match.group(1).strip()) if name_match else ""
    other = any(
        _section_has_items(text, key)
        for key in ("m_EulerCurves", "m_PositionCurves", "m_RotationCurves", "m_ScaleCurves")
    )
    return name, _float_curves(_float_block(text)), other


def classify_clip(
    clip: Clip,
    face_paths: set[str],
    epsilon: float,
    baseline_guid: str,
    baseline: dict[tuple[str, str], float],
) -> str:
    shapes = _face_weights(clip, face_paths)
    hand = _has_hand(clip)
    if not clip.curves and not clip.other_motion:
        return "empty"
    if _is_default_name(clip.name) or (baseline_guid and clip.guid == baseline_guid):
        return "default_face"
    if shapes and _all_quiet(shapes, epsilon):
        if _mouthish(clip.name, shapes):
            return "mouth_reset"
        if hand and not clip.other_motion:
            return "hand"
        # A face clip that never leaves zero is a rest pose, not an empty file.
        return "default_face"
    diff = _deltas(shapes, baseline, epsilon)
    if not diff:
        if shapes:
            return "default_face"
        if hand:
            return "hand"
        if clip.other_motion or clip.curves:
            return "not_face"
        return "empty"
    names = [key[1] for key, _value in diff]
    blink_name = _is_blink_name(clip.name)
    if all(_is_eye(name) for name in names) and (
        any(_is_blink_shape(name) for name in names) or blink_name
    ):
        return "blink"
    if all(_is_viseme_shape(name) for name in names):
        return "viseme"
    return "expression"


def plan_install(chosen: list[str], controller: dict, expression_parameters: list | None) -> dict:
    """Describe the bool parameters and FX layers. Does not touch files.

    Layers are appended so they win over hand-gesture layers. Write Defaults
    follows the majority of the states already in the controller. A second
    pass updates an existing free-edition layer instead of adding another.
    """
    refused = {
        "ok": False,
        "error": "",
        "write_defaults": False,
        "synced_bits": None,
        "added_bits": 0,
        "limit": SYNC_LIMIT,
        "slot_count": SLOT_COUNT,
        "parameters": [],
        "layers": [],
        "expression_parameters_missing": expression_parameters is None,
    }
    if len(chosen) > SLOT_COUNT:
        refused["error"] = "無料版は 4 スロットまでです"
        return refused
    write_on = int(controller.get("write_on", 0))
    write_off = int(controller.get("write_off", 0))
    write_defaults = write_on > write_off
    layer_rows = list(controller.get("layers") or [])
    layer_names = [row["name"] for row in layer_rows]
    parameters = []
    layers = []
    added_bits = 0
    for offset, clip_name in enumerate(chosen, start=1):
        pname = parameter_name(offset)
        animator_type = _named_type(controller.get("parameters") or [], pname)
        expr_type = _expr_type(expression_parameters, pname)
        if (animator_type not in (None, "bool")) or (expr_type not in (None, "bool")):
            refused["error"] = f"{pname} は bool ではありません"
            refused["write_defaults"] = write_defaults
            return refused
        existing = next((row for row in layer_rows if row["name"] == pname), None)
        if existing is not None and not existing.get("ours"):
            refused["error"] = f"{pname} というレイヤーが既にあるため追加しません"
            refused["write_defaults"] = write_defaults
            return refused
        if expression_parameters is not None and expr_type is None:
            added_bits += 1
        parameters.append(
            {
                "name": pname,
                "type": "bool",
                "synced": True,
                "already_present": animator_type == "bool",
                "expression_present": expr_type == "bool",
            }
        )
        layers.append(
            {
                "name": pname,
                "parameter": pname,
                "clip": clip_name,
                "write_defaults": write_defaults,
                "already_present": existing is not None,
                "states": ["Off", "On"],
                "on_condition": "If",
                "off_condition": "IfNot",
                "has_exit_time": False,
                "tag": LAYER_TAG,
                "placement": "end",
            }
        )
    bits = None if expression_parameters is None else synced_bits(expression_parameters)
    if bits is not None and bits + added_bits > SYNC_LIMIT:
        refused["error"] = (
            f"同期ビットが {SYNC_LIMIT} を超えます (現在 {bits}、追加 {added_bits})"
        )
        refused["synced_bits"] = bits
        refused["added_bits"] = added_bits
        refused["write_defaults"] = write_defaults
        return refused
    final_names = [name for name in layer_names if name not in {row["name"] for row in layers}]
    final_names.extend(row["name"] for row in layers)
    hand_indexes = [index for index, name in enumerate(final_names) if "hand" in name.lower()]
    for row in layers:
        index = final_names.index(row["name"])
        row["index"] = index
        row["above_hand"] = all(index > hand for hand in hand_indexes) if hand_indexes else True
    return {
        "ok": True,
        "error": "",
        "write_defaults": write_defaults,
        "synced_bits": bits,
        "added_bits": added_bits,
        "limit": SYNC_LIMIT,
        "slot_count": SLOT_COUNT,
        "parameters": parameters,
        "layers": layers,
        "expression_parameters_missing": expression_parameters is None,
    }


def synced_bits(items: list[dict]) -> int:
    total = 0
    for item in items:
        if not item.get("synced"):
            continue
        value_type = int(item.get("value_type", 2))
        total += 8 if value_type in (0, 1) else 1
    return total


def scan_tree(root: str | Path) -> Scan:
    folder = Path(root)
    parsed: list[Clip] = []
    for anim in sorted(folder.rglob("*.anim")):
        meta = Path(str(anim) + ".meta")
        guid = _read_guid(meta) if meta.is_file() else ""
        text = anim.read_text(encoding="utf-8", errors="replace")
        name, curves, other = parse_clip_text(text)
        parsed.append(
            Clip(
                name=name or anim.stem,
                path=anim.relative_to(folder).as_posix(),
                guid=guid,
                curves=curves,
                other_motion=other,
            )
        )
    clips = _dedupe(parsed)
    controller_path = _pick_fx(folder)
    controller = _empty_controller()
    fx_guids: set[str] = set()
    if controller_path is not None:
        controller = parse_controller(controller_path.read_text(encoding="utf-8", errors="replace"))
        fx_guids = controller["guids"]
    params_path = _pick_parameters(folder)
    expression_parameters = None
    if params_path is not None:
        expression_parameters = parse_expression_parameters(
            params_path.read_text(encoding="utf-8", errors="replace")
        )
    values = [
        curve.value
        for clip in clips
        for curve in clip.curves
        if curve.attribute.startswith("blendShape.")
    ]
    epsilon = weight_epsilon(values) if values else 8.0
    face_paths = discover_face_paths(clips)
    baseline = next((clip for clip in clips if _is_default_name(clip.name)), None)
    if baseline is not None:
        baseline = max(
            (clip for clip in clips if _is_default_name(clip.name)),
            key=lambda clip: len(_face_weights(clip, face_paths)),
        )
    baseline_guid = baseline.guid if baseline and baseline.guid else ""
    baseline_map = _face_weights(baseline, face_paths) if baseline else {}
    verdicts = []
    local_guids = {clip.guid for clip in clips if clip.guid}
    for clip in clips:
        kind = classify_clip(clip, face_paths, epsilon, baseline_guid, baseline_map)
        in_fx = bool(clip.guid) and clip.guid in fx_guids
        source = "fx+folder" if in_fx else "folder"
        verdicts.append(
            Verdict(
                name=clip.name,
                path=clip.path,
                guid=clip.guid,
                kind=kind,
                include=kind == "expression",
                reason=REASONS[kind],
                source=source,
            )
        )
    verdicts.sort(key=lambda item: (not item.include, item.path))
    hand_layers = [name for name in controller["layer_names"] if "hand" in name.lower()]
    return Scan(
        fx_name=controller["name"],
        clips=verdicts,
        write_defaults=controller["write_on"] > controller["write_off"],
        write_on=controller["write_on"],
        write_off=controller["write_off"],
        synced_bits=None if expression_parameters is None else synced_bits(expression_parameters),
        hand_layers=hand_layers,
        layers=list(controller["layer_names"]),
        unresolved_fx_clips=len(fx_guids - local_guids),
        controller={
            "parameters": controller["parameters"],
            "layers": controller["layers"],
            "write_on": controller["write_on"],
            "write_off": controller["write_off"],
        },
        expression_parameters=expression_parameters,
        epsilon=epsilon,
    )


def parse_controller(text: str) -> dict:
    marker = text.find("AnimatorController:")
    header = text[marker : marker + 800] if marker >= 0 else text
    name_match = re.search(r"\n  m_Name: (.*)", header)
    name = name_match.group(1).strip() if name_match else ""
    param_block = _between(text, "\n  m_AnimatorParameters:", "\n  m_AnimatorLayers:")
    parameters = []
    for chunk in re.split(r"\n  - m_Name: ", param_block)[1:]:
        pname = chunk.splitlines()[0].strip()
        type_match = re.search(r"m_Type: (\d+)", chunk)
        anim_type = int(type_match.group(1)) if type_match else 4
        parameters.append({"name": pname, "type": _ANIMATOR_TYPE.get(anim_type, "float")})
    layer_block = _between(text, "\n  m_AnimatorLayers:", "\n--- ")
    layer_names = [item.strip() for item in re.findall(r"\n    m_Name: (.*)", layer_block)]
    write_on = 0
    write_off = 0
    for part in text.split("--- "):
        if not part.startswith("!u!1102"):
            continue
        flag = re.search(r"m_WriteDefaultValues: (\d+)", part)
        if flag is None:
            continue
        if flag.group(1) == "1":
            write_on += 1
        else:
            write_off += 1
    ours = LAYER_TAG in text
    layers = [
        {"name": layer_name, "ours": ours and layer_name.startswith("Expression")}
        for layer_name in layer_names
    ]
    return {
        "name": name,
        "parameters": parameters,
        "layers": layers,
        "layer_names": layer_names,
        "write_on": write_on,
        "write_off": write_off,
        "guids": set(re.findall(r"guid: ([0-9a-fA-F]{32})", text)),
    }


def parse_expression_parameters(text: str) -> list[dict] | None:
    if "parameters:" not in text or "valueType:" not in text:
        return None
    items = []
    for chunk in re.split(r"\n  - name: ", text)[1:]:
        name = chunk.splitlines()[0].strip()
        value_type = re.search(r"valueType: (\d+)", chunk)
        synced = re.search(r"networkSynced: (\d+)", chunk)
        if value_type is None:
            continue
        items.append(
            {
                "name": name,
                "value_type": int(value_type.group(1)),
                "synced": bool(synced and synced.group(1) == "1"),
                "type": _EXPR_TYPE.get(int(value_type.group(1)), "bool"),
            }
        )
    return items


def discover_face_paths(clips: list[Clip]) -> set[str]:
    shapes: dict[str, set[str]] = defaultdict(set)
    for clip in clips:
        for curve in clip.curves:
            if curve.attribute.startswith("blendShape."):
                shapes[curve.path].add(curve.attribute.split(".", 1)[1])
    faces = set()
    for path, names in shapes.items():
        if _path_is_face(path) or _shapes_look_facial(names):
            faces.add(path)
    return faces


def format_report(scan: Scan) -> str:
    included = [item for item in scan.clips if item.include]
    excluded = [item for item in scan.clips if not item.include]
    wd = "オン" if scan.write_defaults else "オフ"
    bits = "不明" if scan.synced_bits is None else str(scan.synced_bits)
    extra = ""
    if scan.synced_bits is not None:
        extra = f" (4 つ足すと {scan.synced_bits + 4}、上限 {SYNC_LIMIT})"
    lines = [
        f"FX: {scan.fx_name or '(なし)'}",
        f"Write Defaults: 既存の多数派は{wd} (オン {scan.write_on} / オフ {scan.write_off})",
        f"同期ビット: {bits}{extra}",
        "手ジェスチャーレイヤー: "
        + (", ".join(scan.hand_layers) if scan.hand_layers else "(なし)"),
        f"FX が参照するがファイルが無いクリップ: {scan.unresolved_fx_clips}",
        f"候補 {len(included)} / 除外 {len(excluded)} / 閾値 {scan.epsilon}",
        "",
        "含める:",
    ]
    if not included:
        lines.append("  (なし)")
    for item in included:
        lines.append(f"  {item.name}\t{item.source}\t{item.path}")
    lines.append("")
    lines.append("除外:")
    if not excluded:
        lines.append("  (なし)")
    for item in excluded:
        lines.append(f"  {item.name}\t{item.reason}\t{item.source}\t{item.path}")
    return "\n".join(lines) + "\n"


def _empty_controller() -> dict:
    return {
        "name": "",
        "parameters": [],
        "layers": [],
        "layer_names": [],
        "write_on": 0,
        "write_off": 0,
        "guids": set(),
    }


def _dedupe(clips: list[Clip]) -> list[Clip]:
    groups: dict[str, list[Clip]] = defaultdict(list)
    for clip in clips:
        groups[clip.guid or clip.path].append(clip)
    chosen = []
    for items in groups.values():
        match = next((item for item in items if Path(item.path).stem == item.name), None)
        chosen.append(match or sorted(items, key=lambda item: item.path)[0])
    return chosen


def _pick_fx(folder: Path) -> Path | None:
    controllers = sorted(folder.rglob("*.controller"))
    if not controllers:
        return None
    named = [path for path in controllers if "fx" in path.name.lower()]
    if len(named) == 1:
        return named[0]
    pool = named or controllers
    return max(pool, key=lambda path: path.stat().st_size)


def _pick_parameters(folder: Path) -> Path | None:
    found = []
    for path in sorted(folder.rglob("*.asset")):
        text = path.read_text(encoding="utf-8", errors="replace")
        if "valueType:" in text and "networkSynced:" in text and "parameters:" in text:
            found.append(path)
    if not found:
        return None
    return found[0]


def _read_guid(path: Path) -> str:
    text = path.read_text(encoding="utf-8", errors="replace")
    match = re.search(r"^guid: ([0-9a-fA-F]{32})", text, re.M)
    return match.group(1) if match else ""


def _float_block(text: str) -> str:
    return _between(text, "\n  m_FloatCurves:", "\n  m_PPtrCurves:")


def _between(text: str, start: str, end: str) -> str:
    begin = text.find(start)
    if begin < 0:
        return ""
    stop = text.find(end, begin + len(start))
    if stop < 0:
        return text[begin:]
    return text[begin:stop]


def _section_has_items(text: str, key: str) -> bool:
    match = re.search(rf"^  {key}:[ \t]*(.*)$", text, re.M)
    if match is None:
        return False
    return match.group(1).strip() != "[]"


def _float_curves(block: str) -> list[Curve]:
    curves = []
    for part in block.split("\n  - curve:")[1:]:
        attr = re.search(r"\n    attribute: (.+)", part)
        path = re.search(r"\n    path: (.*)", part)
        class_id = re.search(r"\n    classID: (\d+)", part)
        if attr is None:
            continue
        values = []
        for raw in re.findall(r"\n        value: ([^\n{]+)", part):
            try:
                values.append(float(raw.strip()))
            except ValueError:
                continue
        value = max(values, key=lambda item: abs(item)) if values else 0.0
        curves.append(
            Curve(
                attribute=_unescape(attr.group(1).strip()),
                path=path.group(1).strip() if path else "",
                class_id=int(class_id.group(1)) if class_id else 0,
                value=value,
            )
        )
    return curves


def _unescape(raw: str) -> str:
    if len(raw) >= 2 and raw[0] == '"' and raw[-1] == '"':
        raw = raw[1:-1]
        raw = re.sub(r"\\u([0-9a-fA-F]{4})", lambda match: chr(int(match.group(1), 16)), raw)
        raw = raw.replace('\\"', '"').replace("\\\\", "\\")
    return raw


def _face_weights(clip: Clip | None, face_paths: set[str]) -> dict[tuple[str, str], float]:
    weights: dict[tuple[str, str], float] = {}
    if clip is None:
        return weights
    for curve in clip.curves:
        if not curve.attribute.startswith("blendShape."):
            continue
        if curve.path not in face_paths:
            continue
        key = (curve.path, curve.attribute.split(".", 1)[1])
        previous = weights.get(key)
        if previous is None or abs(curve.value) > abs(previous):
            weights[key] = curve.value
    return weights


def _deltas(
    shapes: dict[tuple[str, str], float],
    baseline: dict[tuple[str, str], float],
    epsilon: float,
) -> list[tuple[tuple[str, str], float]]:
    found = []
    for key, value in shapes.items():
        if abs(value - baseline.get(key, 0.0)) >= epsilon:
            found.append((key, value))
    return found


def _all_quiet(shapes: dict[tuple[str, str], float], epsilon: float) -> bool:
    return all(abs(value) < epsilon for value in shapes.values())


def _has_hand(clip: Clip) -> bool:
    for curve in clip.curves:
        text = curve.attribute.lower()
        if "lefthand" in text or "righthand" in text or "finger" in text:
            return True
    return False


def _path_is_face(path: str) -> bool:
    last = path.replace("\\", "/").rsplit("/", 1)[-1].lower()
    if last in {"body", "face", "head"} or last == "顔":
        return True
    return "face" in last or "body" in last or "顔" in last


def _shapes_look_facial(names: set[str]) -> bool:
    if not names:
        return False
    hits = sum(1 for name in names if _contains_any(name.lower(), _FACIAL_HINTS))
    return hits >= 3 or (hits / len(names)) >= 0.3


def _is_default_name(name: str) -> bool:
    folded = re.sub(r"[^a-z0-9]", "", name.lower())
    return "defaultface" in folded or "defaltface" in folded


def _is_blink_name(name: str) -> bool:
    return _contains_any(name.lower(), _BLINK_HINTS)


def _is_eye(name: str) -> bool:
    return _contains_any(name.lower(), _EYE_HINTS)


def _is_blink_shape(name: str) -> bool:
    return _contains_any(name.lower(), _BLINK_HINTS)


def _is_viseme_shape(name: str) -> bool:
    folded = name.lower()
    if folded.startswith("vrc.v_") or "viseme" in folded:
        return True
    token = re.split(r"[_\-\s]", folded, maxsplit=1)[0]
    return token in _VISEME_TOKENS


def _mouthish(name: str, shapes: dict[tuple[str, str], float]) -> bool:
    folded = re.sub(r"[\s_\-]+", "", name.lower())
    if folded in {"mouse0", "mouth0", "mouthreset"} or "口リセット" in name:
        return True
    return any(_contains_any(key[1].lower(), _MOUTH_HINTS) for key in shapes)


def _contains_any(text: str, hints: tuple[str, ...]) -> bool:
    return any(hint in text for hint in hints)


def _named_type(parameters: list[dict], name: str) -> str | None:
    for item in parameters:
        if item.get("name") == name:
            return item.get("type")
    return None


def _expr_type(items: list[dict] | None, name: str) -> str | None:
    if not items:
        return None
    for item in items:
        if item.get("name") == name:
            return item.get("type") or _EXPR_TYPE.get(int(item.get("value_type", 2)), "bool")
    return None


def main() -> None:
    import sys

    if len(sys.argv) != 2:
        raise SystemExit("usage: expression_rules.py <avatar-folder>")
    scan = scan_tree(sys.argv[1])
    sys.stdout.write(format_report(scan))


if __name__ == "__main__":
    main()
