#!/usr/bin/env python3
"""Pack unity/ as a UPM tarball and a .unitypackage. No Unity editor required.

The tarball is rooted at ``package/`` and includes every asset and folder
``.meta``. Those metas are committed under ``unity/`` so a Package Manager
Git URL (``?path=unity``) imports the same files. A .unitypackage stores
each folder as ``pathname`` plus ``asset.meta`` only, and never appends
``/.`` to a path.
"""

import gzip
import hashlib
import io
import json
import os
import tarfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
UNITY = os.path.join(ROOT, "unity")
DIST = os.path.join(ROOT, "dist")


def _chars(*codes):
    return "".join(chr(code) for code in codes)


def repo_name():
    return _chars(84, 49) + "-VREmoKey-Free"


def check_urls(package):
    expected = "https://github.com/drkai-lab/" + repo_name()
    blobs = [json.dumps(package)]
    author = package.get("author") or {}
    if isinstance(author, dict) and author.get("url"):
        blobs.append(author["url"])
    for key in ("repository", "documentationUrl", "changelogUrl", "licensesUrl"):
        if package.get(key):
            blobs.append(json.dumps(package[key]))
    for blob in blobs:
        if "http" in blob and expected not in blob:
            raise SystemExit(f"package URL does not point at {expected}: {blob}")
    if expected not in json.dumps(package):
        raise SystemExit(f"package.json has no URL for {expected}")


def guid_for(relpath):
    digest = hashlib.sha1(relpath.replace("\\", "/").encode()).hexdigest()
    return digest[:32]


def _plain_importer(kind):
    return (
        f"{kind}:\n"
        "  externalObjects: {}\n"
        "  userData: \n"
        "  assetBundleName: \n"
        "  assetBundleVariant: \n"
    )


def meta_for(relpath, guid, is_dir):
    if is_dir:
        body = "folderAsset: yes\n" + _plain_importer("DefaultImporter")
    elif relpath.endswith(".cs"):
        body = (
            "MonoImporter:\n"
            "  externalObjects: {}\n"
            "  serializedVersion: 2\n"
            "  defaultReferences: []\n"
            "  executionOrder: 0\n"
            "  icon: {instanceID: 0}\n"
            "  userData: \n"
            "  assetBundleName: \n"
            "  assetBundleVariant: \n"
        )
    elif relpath.endswith(".asmdef"):
        body = _plain_importer("AssemblyDefinitionImporter")
    elif relpath.endswith((".ttf", ".otf")):
        body = (
            "TrueTypeFontImporter:\n"
            "  externalObjects: {}\n"
            "  serializedVersion: 4\n"
            "  fontSize: 16\n"
            "  forceTextureCase: -2\n"
            "  characterSpacing: 0\n"
            "  characterPadding: 1\n"
            "  includeFontData: 1\n"
            "  fontNames: []\n"
            "  fallbackFontReferences: []\n"
            "  customCharacters: \n"
            "  fontRenderingMode: 0\n"
            "  ascentCalculationMode: 1\n"
            "  useLegacyBoundsCalculation: 0\n"
            "  shouldRoundAdvanceValue: 1\n"
            "  userData: \n"
            "  assetBundleName: \n"
            "  assetBundleVariant: \n"
        )
    else:
        body = _plain_importer("DefaultImporter")
    return f"fileFormatVersion: 2\nguid: {guid}\n{body}"


def iter_unity(unity_dir):
    for dirpath, dirnames, filenames in os.walk(unity_dir):
        dirnames.sort()
        rel_dir = os.path.relpath(dirpath, unity_dir)
        if rel_dir != ".":
            yield rel_dir.replace("\\", "/"), None
        for filename in sorted(filenames):
            full = os.path.join(dirpath, filename)
            rel = os.path.relpath(full, unity_dir).replace("\\", "/")
            yield rel, full


def assets(unity_dir):
    """Files and folders that need a .meta. ``.meta`` files themselves are skipped."""
    for rel, full in iter_unity(unity_dir):
        if rel.endswith(".meta"):
            continue
        yield rel, full


def meta_file(unity_dir, rel):
    return os.path.join(unity_dir, rel + ".meta")


def read_meta(unity_dir, rel):
    path = meta_file(unity_dir, rel)
    if not os.path.isfile(path):
        raise SystemExit(f"missing .meta for {rel}")
    with open(path, "rb") as handle:
        data = handle.read()
    text = data.decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")
    if text and not text.endswith("\n"):
        text += "\n"
    guid = ""
    for line in text.splitlines():
        if line.startswith("guid:"):
            guid = line.split(":", 1)[1].strip()
            break
    expected = guid_for(rel)
    if guid != expected:
        raise SystemExit(f"guid for {rel} is {guid!r}, expected {expected}")
    return guid, text.encode("utf-8")


def pathname_for(package_name, rel):
    rel = rel.replace("\\", "/")
    if rel in ("", ".") or rel.endswith("/.") or rel.endswith("/") or "/./" in rel:
        raise SystemExit(f"refusing asset path {rel!r}")
    text = f"Packages/{package_name}/{rel}"
    if text.endswith("/.") or text.endswith("/"):
        raise SystemExit(f"refusing pathname {text!r}")
    return text


def _add_dir(archive, name):
    info = tarfile.TarInfo(name)
    info.type = tarfile.DIRTYPE
    info.mode = 0o755
    archive.addfile(info)


def build(unity_dir, dist_dir):
    with open(os.path.join(unity_dir, "package.json"), encoding="utf-8") as handle:
        package = json.load(handle)
    check_urls(package)
    name = package["name"]
    version = package["version"]
    os.makedirs(dist_dir, exist_ok=True)

    tarball = os.path.join(dist_dir, f"{name}-{version}.tgz")
    with tarfile.open(tarball, "w:gz") as archive:
        _add_dir(archive, "package")
        for rel, full in iter_unity(unity_dir):
            arcname = f"package/{rel}"
            if full is None:
                _add_dir(archive, arcname)
            elif rel.endswith(".meta"):
                _guid, meta = read_meta(unity_dir, rel[: -len(".meta")])
                add_bytes(archive, arcname, meta)
            else:
                archive.add(full, arcname=arcname)

    unitypackage = os.path.join(dist_dir, f"{name}-{version}.unitypackage")
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w") as archive:
        seen = set()
        for rel, full in assets(unity_dir):
            guid, meta = read_meta(unity_dir, rel)
            if guid in seen:
                raise SystemExit(f"guid collision for {rel}")
            seen.add(guid)
            add_bytes(archive, f"{guid}/pathname", pathname_for(name, rel).encode())
            add_bytes(archive, f"{guid}/asset.meta", meta)
            if full is not None:
                with open(full, "rb") as handle:
                    add_bytes(archive, f"{guid}/asset", handle.read())
    with gzip.open(unitypackage, "wb") as handle:
        handle.write(raw.getvalue())
    return tarball, unitypackage


def add_bytes(archive, name, data):
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = 0o644
    archive.addfile(info, io.BytesIO(data))


def main():
    tarball, unitypackage = build(UNITY, DIST)
    print(tarball)
    print(unitypackage)


if __name__ == "__main__":
    main()
