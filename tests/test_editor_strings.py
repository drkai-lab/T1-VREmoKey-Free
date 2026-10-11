#!/usr/bin/env python3
"""The editor string table has Japanese, English, and Korean for every key.

Unity is not required. The Hangul (and the other non-ASCII UI characters)
must also exist in the bundled editor font, because Unity 2022 draws IMGUI
with a Latin font.
"""

import os
import re
import unittest

from fontTools.ttLib import TTFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
SOURCE = os.path.join(ROOT, "unity", "Editor", "EditorStrings.cs")
FONT = os.path.join(ROOT, "unity", "Editor", "EditorHangul.ttf")


def _unescape(text):
    return (
        text.replace(r"\n", "\n")
        .replace(r"\t", "\t")
        .replace(r"\"", '"')
        .replace(r"\\", "\\")
    )


def _strings_of(body):
    values = []
    index = 0
    while index < len(body):
        if body[index] != '"':
            index += 1
            continue
        index += 1
        chars = []
        while index < len(body):
            if body[index] == "\\" and index + 1 < len(body):
                chars.append(body[index : index + 2])
                index += 2
                continue
            if body[index] == '"':
                index += 1
                break
            chars.append(body[index])
            index += 1
        values.append(_unescape("".join(chars)))
    return values


def _array(text, name):
    match = re.search(
        r"static readonly string\[\]\[\] " + name + r" =\s*\{(.*?)\n        \};",
        text,
        re.S,
    )
    if not match:
        raise AssertionError(f"missing {name}")
    body = match.group(1)
    rows = []
    needle = "new[] {"
    index = 0
    while True:
        start = body.find(needle, index)
        if start < 0:
            break
        index = start + len(needle)
        depth = 1
        chars = []
        while index < len(body) and depth:
            ch = body[index]
            if ch == '"':
                chars.append(ch)
                index += 1
                while index < len(body):
                    chars.append(body[index])
                    if body[index] == "\\" and index + 1 < len(body):
                        chars.append(body[index + 1])
                        index += 2
                        continue
                    if body[index] == '"':
                        index += 1
                        break
                    index += 1
                continue
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    index += 1
                    break
            chars.append(ch)
            index += 1
        rows.append(_strings_of("".join(chars)))
    return rows


def load_tables():
    with open(SOURCE, encoding="utf-8") as handle:
        text = handle.read()
    return _array(text, "Rows"), _array(text, "LanguageHints")


def infer(hints, raw_values):
    for raw in raw_values:
        code = _map(hints, raw)
        if code:
            return code
    return "ja"


def _map(hints, raw):
    if not raw:
        return None
    folded = raw.strip().lower().replace("_", "-")
    primary = folded.split("-", 1)[0]
    for row in hints:
        code = row[0]
        for hint in row[1:]:
            if primary == hint or folded == hint:
                return code
    return None


class EditorStringTableTest(unittest.TestCase):
    def test_every_key_has_three_languages(self):
        rows, hints = load_tables()
        self.assertGreaterEqual(len(rows), 40)
        keys = []
        hangul = 0
        for row in rows:
            self.assertEqual(len(row), 4, row[:1])
            key, ja, en, ko = row
            self.assertTrue(key, row)
            self.assertTrue(ja, key)
            self.assertTrue(en, key)
            self.assertTrue(ko, key)
            self.assertNotIn(key, keys, key)
            keys.append(key)
            hangul += sum(1 for ch in ko if 0xAC00 <= ord(ch) <= 0xD7A3)
        self.assertGreater(hangul, 80)
        self.assertIn("window.title", keys)
        self.assertIn("reason.blink", keys)
        self.assertIn("err.sync", keys)
        self.assertEqual({row[0] for row in hints}, {"ja", "en", "ko"})
        self.assertEqual(infer(hints, ["Korean"]), "ko")
        self.assertEqual(infer(hints, ["en-US"]), "en")
        self.assertEqual(infer(hints, ["Japanese"]), "ja")
        self.assertEqual(infer(hints, ["", "fr-FR"]), "ja")
        self.assertEqual(infer(hints, []), "ja")

    def test_bundled_font_covers_the_ui_characters(self):
        rows, _hints = load_tables()
        font = TTFont(FONT)
        cmap = font.getBestCmap()
        missing = []
        for row in rows:
            for text in row[1:]:
                for ch in text:
                    if ord(ch) < 128 or ord(ch) in cmap:
                        continue
                    missing.append((row[0], ch, hex(ord(ch))))
        self.assertEqual(missing, [])
        self.assertIn(ord("한"), cmap)
        self.assertIn(ord("あ"), cmap)


if __name__ == "__main__":
    unittest.main()
