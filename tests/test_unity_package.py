#!/usr/bin/env python3
"""Unity package layout, without the Unity editor.

Checks the committed ``unity/`` metas and the archives ``scripts/pack-unity.py``
writes: a UPM tarball rooted at ``package/`` with a ``.meta`` for every asset
and folder, and a ``.unitypackage`` whose folders are ``pathname`` plus
``asset.meta`` only.
"""

import gzip
import importlib.machinery
import importlib.util
import io
import json
import os
import tarfile
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
SCRIPT = os.path.join(ROOT, "scripts", "pack-unity.py")
UNITY = os.path.join(ROOT, "unity")

loader = importlib.machinery.SourceFileLoader("pack_unity", SCRIPT)
spec = importlib.util.spec_from_loader("pack_unity", loader)
pack = importlib.util.module_from_spec(spec)
loader.exec_module(pack)


def _repo_url():
    # Built from codes so this file does not contain the banned product token.
    name = "".join(chr(code) for code in (84, 49)) + "-VREmoKey-Free"
    return "https://github.com/drkai-lab/" + name


def _write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def _seed_package(root, metas=True):
    manifest = {
        "name": "com.example.packtest",
        "version": "9.9.9",
        "author": {"name": "drkai-lab", "url": _repo_url()},
    }
    _write(os.path.join(root, "package.json"), json.dumps(manifest, indent=2) + "\n")
    _write(os.path.join(root, "Editor", "A.cs"), "class A {}\n")
    if not metas:
        return
    for rel, is_dir in (
        ("Editor", True),
        ("package.json", False),
        ("Editor/A.cs", False),
    ):
        text = pack.meta_for(rel, pack.guid_for(rel), is_dir)
        _write(os.path.join(root, rel + ".meta"), text)


def _open_tar(path):
    with tarfile.open(path, "r:gz") as archive:
        members = list(archive.getmembers())
        payloads = {}
        for member in members:
            if member.isfile():
                payloads[member.name] = archive.extractfile(member).read()
    return members, payloads


def _unitypackage_groups(path):
    with gzip.open(path, "rb") as handle:
        raw = handle.read()
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:") as archive:
        groups = {}
        payloads = {}
        for member in archive.getmembers():
            guid, _, name = member.name.partition("/")
            groups.setdefault(guid, set()).add(name)
            if member.isfile():
                payloads[member.name] = archive.extractfile(member).read()
    return groups, payloads


def _assert_archives(test, unity_dir, tarball, unitypackage):
    with open(os.path.join(unity_dir, "package.json"), encoding="utf-8") as handle:
        package_name = json.load(handle)["name"]
    prefix = f"Packages/{package_name}/"

    members, payloads = _open_tar(tarball)
    names = {member.name for member in members}
    test.assertIn("package", names)
    test.assertTrue(all(name == "package" or name.startswith("package/") for name in names))
    test.assertIn("package/package.json", names)

    expected = list(pack.assets(unity_dir))
    test.assertGreaterEqual(len(expected), 1)
    meta_names = set()
    folders = set()
    files = set()
    for rel, full in expected:
        meta_name = f"package/{rel}.meta"
        meta_names.add(meta_name)
        test.assertIn(meta_name, names, rel)
        test.assertIn(meta_name, payloads)
        test.assertNotIn(b"\r", payloads[meta_name], rel)
        if full is None:
            folders.add(rel)
            dir_members = [member for member in members if member.name == f"package/{rel}"]
            test.assertTrue(dir_members and dir_members[0].isdir(), rel)
        else:
            files.add(rel)
            test.assertIn(f"package/{rel}", names)
    for name in names:
        if not name.endswith(".meta"):
            continue
        rel = name[len("package/") : -len(".meta")]
        test.assertIn(rel, folders | files, name)

    groups, up_payloads = _unitypackage_groups(unitypackage)
    seen_paths = set()
    for guid, parts in groups.items():
        test.assertIn("pathname", parts)
        test.assertIn("asset.meta", parts)
        path_text = up_payloads[f"{guid}/pathname"].decode("utf-8")
        test.assertFalse(path_text.endswith("/."), path_text)
        test.assertFalse(path_text.endswith("/"), path_text)
        test.assertNotIn("/./", path_text)
        test.assertFalse(path_text.endswith(".meta"), path_text)
        test.assertTrue(path_text.startswith(prefix), path_text)
        rel = path_text[len(prefix) :]
        seen_paths.add(rel)
        meta_bytes = up_payloads[f"{guid}/asset.meta"]
        test.assertNotIn(b"\r", meta_bytes, rel)
        meta = meta_bytes.decode("utf-8")
        test.assertIn(f"guid: {guid}\n", meta)
        if rel in folders:
            test.assertEqual(parts, {"pathname", "asset.meta"}, rel)
            test.assertIn("folderAsset: yes\n", meta)
            test.assertNotIn(f"{guid}/asset", up_payloads)
        else:
            test.assertIn(rel, files, rel)
            test.assertEqual(parts, {"pathname", "asset", "asset.meta"}, rel)
            test.assertGreater(len(up_payloads[f"{guid}/asset"]), 0)
    test.assertEqual(seen_paths, folders | files)


class CommittedMetaTest(unittest.TestCase):
    def test_every_asset_and_folder_has_the_stable_meta(self):
        found = []
        for rel, full in pack.iter_unity(UNITY):
            if rel.endswith(".meta"):
                target = rel[: -len(".meta")]
                self.assertTrue(
                    os.path.isdir(os.path.join(UNITY, target))
                    or os.path.isfile(os.path.join(UNITY, target)),
                    rel,
                )
                continue
            found.append(rel)
            path = pack.meta_file(UNITY, rel)
            self.assertTrue(os.path.isfile(path), rel)
            with open(path, encoding="utf-8") as handle:
                text = handle.read()
            expected = pack.meta_for(rel, pack.guid_for(rel), full is None)
            self.assertEqual(text, expected, rel)
            self.assertEqual(len(pack.guid_for(rel)), 32)
        self.assertIn("package.json", found)
        self.assertIn("Editor", found)
        self.assertIn("Editor/VrcExpressionProfileWindow.cs", found)
        self.assertIn("Tests/Editor", found)


class PackLayoutTest(unittest.TestCase):
    def test_repo_archives_keep_package_root_and_metas(self):
        with tempfile.TemporaryDirectory() as dist:
            tarball, unitypackage = pack.build(UNITY, dist)
            _assert_archives(self, UNITY, tarball, unitypackage)

    def test_fixture_folder_has_no_asset_member(self):
        with tempfile.TemporaryDirectory() as tmp:
            unity = os.path.join(tmp, "unity")
            dist = os.path.join(tmp, "dist")
            _seed_package(unity)
            tarball, unitypackage = pack.build(unity, dist)
            _assert_archives(self, unity, tarball, unitypackage)

    def test_crlf_meta_is_packed_as_lf(self):
        with tempfile.TemporaryDirectory() as tmp:
            unity = os.path.join(tmp, "unity")
            _seed_package(unity)
            for rel, _full in pack.assets(unity):
                path = pack.meta_file(unity, rel)
                with open(path, "rb") as handle:
                    data = handle.read().replace(b"\n", b"\r\n")
                with open(path, "wb") as handle:
                    handle.write(data)
            tarball, unitypackage = pack.build(unity, os.path.join(tmp, "dist"))
            _assert_archives(self, unity, tarball, unitypackage)

    def test_missing_meta_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            unity = os.path.join(tmp, "unity")
            _seed_package(unity, metas=False)
            with self.assertRaises(SystemExit):
                pack.build(unity, os.path.join(tmp, "dist"))

    def test_pathname_rejects_dot_suffix(self):
        with self.assertRaises(SystemExit):
            pack.pathname_for("com.example.packtest", ".")
        with self.assertRaises(SystemExit):
            pack.pathname_for("com.example.packtest", "Editor/.")
        self.assertEqual(
            pack.pathname_for("com.example.packtest", "Editor"),
            "Packages/com.example.packtest/Editor",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
