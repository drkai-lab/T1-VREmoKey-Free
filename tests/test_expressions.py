#!/usr/bin/env python3
"""Expression rules, FX embed shape, and profile JSON. No Unity editor."""

import importlib.machinery
import importlib.util
import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
RULES = os.path.join(ROOT, "tools", "expression_rules.py")

loader = importlib.machinery.SourceFileLoader("expression_rules", RULES)
spec = importlib.util.spec_from_loader("expression_rules", loader)
rules = importlib.util.module_from_spec(spec)
sys.modules["expression_rules"] = rules
loader.exec_module(rules)


def _write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def _curve(attribute, path, class_id, value):
    return (
        "  - curve:\n"
        "      m_Curve:\n"
        f"        value: {value}\n"
        f"    attribute: {attribute}\n"
        f"    path: {path}\n"
        f"    classID: {class_id}\n"
    )


def _anim(name, curves, transforms=False):
    euler = "  m_EulerCurves:\n" if transforms else "  m_EulerCurves: []\n"
    body = "".join(_curve(*item) for item in curves)
    floats = "  m_FloatCurves: []\n" if not curves else "  m_FloatCurves:\n" + body
    return (
        "%YAML 1.1\n--- !u!74 &7400000\nAnimationClip:\n"
        f"  m_Name: {name}\n"
        "  m_RotationCurves: []\n"
        + euler
        + "  m_PositionCurves: []\n"
        "  m_ScaleCurves: []\n"
        + floats
        + "  m_PPtrCurves: []\n"
    )


def _meta(guid):
    return f"fileFormatVersion: 2\nguid: {guid}\n"


def _controller(guids, parameters, layers, write_flags):
    param_lines = ["  m_AnimatorParameters:"]
    for name, anim_type in parameters:
        param_lines.append(f"  - m_Name: {name}")
        param_lines.append(f"    m_Type: {anim_type}")
    layer_lines = ["  m_AnimatorLayers:"]
    for name in layers:
        layer_lines.append(f"    m_Name: {name}")
    states = []
    for index, flag in enumerate(write_flags):
        states.append(
            f"--- !u!1102 &{index + 1}\n"
            "AnimatorState:\n"
            f"  m_Name: State{index}\n"
            f"  m_WriteDefaultValues: {flag}\n"
        )
    guid_lines = "\n".join(
        f"  m_Motion: {{fileID: 7400000, guid: {guid}, type: 2}}" for guid in guids
    )
    return (
        "%YAML 1.1\n--- !u!91 &9100000\nAnimatorController:\n  m_Name: Demo_FX\n"
        + "\n".join(param_lines)
        + "\n"
        + "\n".join(layer_lines)
        + "\n"
        + guid_lines
        + "\n"
        + "".join(states)
    )


def _parameters(rows):
    lines = [
        "%YAML 1.1",
        "--- !u!114 &11400000",
        "MonoBehaviour:",
        "  m_Name: DemoParams",
        "  parameters:",
    ]
    for name, value_type, synced in rows:
        lines.append(f"  - name: {name}")
        lines.append(f"    valueType: {value_type}")
        lines.append(f"    networkSynced: {1 if synced else 0}")
    return "\n".join(lines) + "\n"


def _seed(root):
    joy = ("blendShape.joy", "Body", 137, 100)
    blink = ("blendShape.blink", "Body", 137, 100)
    pupil = ("blendShape.pupil", "Body", 137, 40)
    viseme = ("blendShape.vrc.v_aa", "Body", 137, 100)
    mouth_a = ("blendShape.mouth_a", "Body", 137, 0)
    mouth_i = ("blendShape.mouth_i", "Body", 137, 0)
    tooth5 = ("blendShape.tooth", "Body", 137, 5)
    tooth0 = ("blendShape.tooth", "Body", 137, 0)
    joy0 = ("blendShape.joy", "Body", 137, 0)
    hand = ("RightHand.Index.1 Stretched", "", 95, 0.4)
    wink = ("blendShape.joy", "Body", 137, 80)
    clips = {
        "aa11": ("Smile.anim", _anim("Smile", [joy])),
        "aa22": ("Blink.anim", _anim("Blink", [blink, pupil])),
        "aa33": ("vrc.v_aa.anim", _anim("vrc.v_aa", [viseme])),
        "aa44": ("mouse0.anim", _anim("mouse0", [mouth_a, mouth_i])),
        "aa55": ("ok R.anim", _anim("ok R", [hand])),
        "aa66": ("Empty.anim", _anim("Empty", [])),
        "aa77": ("DefaultFace.anim", _anim("DefaultFace", [tooth5, joy0])),
        "aa88": ("fistanimation0.anim", _anim("fistanimation0", [tooth0, joy0])),
        "aa99": ("Hat.anim", _anim("Hat", [], transforms=True)),
        "aaaa": ("Wink.anim", _anim("Wink", [wink])),
    }
    for guid, (filename, body) in clips.items():
        _write(os.path.join(root, "facial", filename), body)
        _write(os.path.join(root, "facial", filename + ".meta"), _meta(guid * 8))
    fx_guids = [
        "aa11" * 8,
        "aa22" * 8,
        "aa33" * 8,
        "aa44" * 8,
        "aa77" * 8,
        "aa88" * 8,
        "bbbb" * 8,
    ]
    controller = _controller(
        fx_guids,
        [("GestureLeft", 3), ("FaceLock", 4)],
        ["Base", "Left Right Hand", "FaceCtrl"],
        [0, 0, 0, 1],
    )
    _write(os.path.join(root, "Demo_FX.controller"), controller)
    _write(
        os.path.join(root, "DemoParams.asset"),
        _parameters([("FaceLock", 2, True), ("Emote", 0, True)]),
    )


class SyntheticAvatarTest(unittest.TestCase):
    def test_same_shape_as_a_face_folder(self):
        with tempfile.TemporaryDirectory() as root:
            _seed(root)
            scan = rules.scan_tree(root)
        by_name = {item.name: item for item in scan.clips}
        self.assertEqual(by_name["Smile"].kind, "expression")
        self.assertTrue(by_name["Smile"].include)
        self.assertEqual(by_name["Smile"].source, "fx+folder")
        self.assertEqual(by_name["Wink"].kind, "expression")
        self.assertEqual(by_name["Wink"].source, "folder")
        self.assertEqual(by_name["Blink"].kind, "blink")
        self.assertEqual(by_name["Blink"].reason, "まばたき")
        self.assertEqual(by_name["vrc.v_aa"].kind, "viseme")
        self.assertEqual(by_name["vrc.v_aa"].reason, "Viseme")
        self.assertEqual(by_name["mouse0"].kind, "mouth_reset")
        self.assertEqual(by_name["mouse0"].reason, "口リセット")
        self.assertEqual(by_name["ok R"].kind, "hand")
        self.assertEqual(by_name["ok R"].reason, "手のポーズだけ")
        self.assertEqual(by_name["Empty"].kind, "empty")
        self.assertEqual(by_name["Empty"].reason, "空のクリップ")
        self.assertEqual(by_name["DefaultFace"].kind, "default_face")
        self.assertEqual(by_name["fistanimation0"].kind, "default_face")
        self.assertEqual(by_name["fistanimation0"].reason, "DefaultFace 相当")
        self.assertEqual(by_name["Hat"].kind, "not_face")
        self.assertFalse(scan.write_defaults)
        self.assertEqual(scan.synced_bits, 9)
        self.assertIn("Left Right Hand", scan.hand_layers)
        self.assertGreater(scan.unresolved_fx_clips, 0)

    def test_embed_matches_existing_controller(self):
        with tempfile.TemporaryDirectory() as root:
            _seed(root)
            scan = rules.scan_tree(root)
        plan = rules.plan_install(["Smile", "Wink"], scan.controller, scan.expression_parameters)
        self.assertTrue(plan["ok"], plan["error"])
        self.assertFalse(plan["write_defaults"])
        self.assertEqual(plan["slot_count"], 4)
        self.assertEqual(plan["added_bits"], 2)
        self.assertEqual(plan["synced_bits"], 9)
        self.assertLessEqual(plan["synced_bits"] + plan["added_bits"], plan["limit"])
        self.assertEqual(
            [item["name"] for item in plan["parameters"]],
            ["Expression1", "Expression2"],
        )
        kinds = [item["type"] for item in plan["parameters"]]
        synced = [item["synced"] for item in plan["parameters"]]
        self.assertEqual(kinds, ["bool", "bool"])
        self.assertEqual(synced, [True, True])
        self.assertFalse(any(item["already_present"] for item in plan["parameters"]))
        self.assertEqual(len(plan["layers"]), 2)
        for layer in plan["layers"]:
            self.assertEqual(layer["states"], ["Off", "On"])
            self.assertEqual(layer["on_condition"], "If")
            self.assertEqual(layer["off_condition"], "IfNot")
            self.assertFalse(layer["has_exit_time"])
            self.assertFalse(layer["write_defaults"])
            self.assertEqual(layer["placement"], "end")
            self.assertTrue(layer["above_hand"])
            self.assertGreater(
                layer["index"],
                plan["layers"][0]["index"] - 1 if layer["name"] == "Expression1" else 0,
            )
        hand_index = 1
        self.assertTrue(all(layer["index"] > hand_index for layer in plan["layers"]))
        self.assertEqual(plan["layers"][0]["clip"], "Smile")
        self.assertEqual(plan["layers"][0]["parameter"], "Expression1")

    def test_write_defaults_follow_the_majority(self):
        controller = {
            "parameters": [],
            "layers": [{"name": "Left Right Hand", "ours": False}],
            "write_on": 5,
            "write_off": 1,
        }
        locked = [{"name": "Lock", "value_type": 2, "synced": True, "type": "bool"}]
        plan = rules.plan_install(["Smile"], controller, locked)
        self.assertTrue(plan["write_defaults"])
        self.assertTrue(plan["layers"][0]["write_defaults"])
        self.assertTrue(plan["layers"][0]["above_hand"])

    def test_sync_limit_refuses_the_whole_install(self):
        parameters = [
            {"name": f"Int{index}", "value_type": 0, "synced": True, "type": "int"}
            for index in range(32)
        ]
        self.assertEqual(rules.synced_bits(parameters), 256)
        empty = {"parameters": [], "layers": [], "write_on": 0, "write_off": 1}
        plan = rules.plan_install(["Smile"], empty, parameters)
        self.assertFalse(plan["ok"])
        self.assertEqual(plan["parameters"], [])
        self.assertEqual(plan["layers"], [])
        self.assertIn("256", plan["error"])

    def test_does_not_add_a_second_parameter_or_layer(self):
        controller = {
            "parameters": [{"name": "Expression1", "type": "bool"}],
            "layers": [
                {"name": "Expression1", "ours": True},
                {"name": "Left Right Hand", "ours": False},
            ],
            "write_on": 0,
            "write_off": 2,
        }
        expr = [
            {"name": "Expression1", "value_type": 2, "synced": True, "type": "bool"},
            {"name": "Lock", "value_type": 2, "synced": True, "type": "bool"},
        ]
        plan = rules.plan_install(["Smile", "Wink"], controller, expr)
        self.assertTrue(plan["ok"], plan["error"])
        self.assertTrue(plan["parameters"][0]["already_present"])
        self.assertTrue(plan["parameters"][0]["expression_present"])
        self.assertFalse(plan["parameters"][1]["already_present"])
        self.assertEqual(plan["added_bits"], 1)
        self.assertTrue(plan["layers"][0]["already_present"])
        self.assertFalse(plan["layers"][1]["already_present"])
        names = [layer["name"] for layer in plan["layers"]]
        self.assertEqual(names.count("Expression1"), 1)
        self.assertTrue(plan["layers"][0]["above_hand"])

    def test_foreign_layer_blocks_a_duplicate_name(self):
        controller = {
            "parameters": [],
            "layers": [{"name": "Expression1", "ours": False}],
            "write_on": 0,
            "write_off": 1,
        }
        plan = rules.plan_install(["Smile"], controller, [])
        self.assertFalse(plan["ok"])
        self.assertEqual(plan["layers"], [])

    def test_quoted_unity_name(self):
        text = (
            "%YAML 1.1\n--- !u!74 &1\nAnimationClip:\n"
            '  m_Name: "\\uFF1E\\uFF1C"\n'
            "  m_FloatCurves: []\n  m_PPtrCurves: []\n"
        )
        name, _curves, _other = rules.parse_clip_text(text)
        self.assertEqual(name, "\uFF1E\uFF1C")

    def test_profile_keeps_four_japanese_rows(self):
        rows = rules.profile_rows(["笑顔", "にやり", "驚き", "ウィンク"])
        rows.append(
            {
                "index": 5,
                "name": "Later",
                "parameter": "Later",
                "osc": "/avatar/parameters/Later",
                "thumb": "thumbs/5.png",
            }
        )
        text = rules.build_profile("avtr_smile", "瑞希", "avtr_smile", rows)
        payload = json.loads(text)
        self.assertEqual(payload["name"], "瑞希")
        self.assertEqual(len(payload["expressions"]), 4)
        self.assertEqual(payload["expressions"][0]["name"], "笑顔")
        self.assertEqual(payload["expressions"][0]["parameter"], "Expression1")
        self.assertEqual(payload["expressions"][0]["osc"], "/avatar/parameters/Expression1")
        self.assertEqual(payload["expressions"][0]["type"], "bool")
        self.assertEqual(payload["expressions"][0]["thumb"], "thumbs/1.png")
        self.assertNotIn("Later", text)
        quoted = rules.build_profile("id", 'say "hi"', "id", rules.profile_rows(["a\\b"]))
        self.assertIn('\\"hi\\"', quoted)
        self.assertIn("a\\\\b", quoted)


if __name__ == "__main__":
    unittest.main()
