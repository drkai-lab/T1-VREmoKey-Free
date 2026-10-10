#!/usr/bin/env python3
"""Pack unity/ as a UPM tarball and a .unitypackage. No Unity editor required."""

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
    else:
        body = _plain_importer("DefaultImporter")
    return f"fileFormatVersion: 2\nguid: {guid}\n{body}"


def iter_unity():
    for dirpath, dirnames, filenames in os.walk(UNITY):
        dirnames.sort()
        rel_dir = os.path.relpath(dirpath, UNITY)
        if rel_dir != ".":
            yield rel_dir.replace("\\", "/"), None
        for filename in sorted(filenames):
            full = os.path.join(dirpath, filename)
            rel = os.path.relpath(full, UNITY).replace("\\", "/")
            yield rel, full


def main():
    with open(os.path.join(UNITY, "package.json"), encoding="utf-8") as handle:
        package = json.load(handle)
    check_urls(package)
    name = package["name"]
    version = package["version"]
    os.makedirs(DIST, exist_ok=True)

    tarball = os.path.join(DIST, f"{name}-{version}.tgz")
    with tarfile.open(tarball, "w:gz") as archive:
        for rel, full in iter_unity():
            arcname = f"package/{rel}" if rel != "." else "package"
            if full is None:
                info = tarfile.TarInfo(arcname)
                info.type = tarfile.DIRTYPE
                info.mode = 0o755
                archive.addfile(info)
            else:
                archive.add(full, arcname=arcname)

    unitypackage = os.path.join(DIST, f"{name}-{version}.unitypackage")
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w") as archive:
        prefix = f"Packages/{name}"
        seen = set()
        for rel, full in iter_unity():
            guid = guid_for(rel)
            if guid in seen:
                raise SystemExit(f"guid collision for {rel}")
            seen.add(guid)
            path_text = f"{prefix}/{rel}".encode()
            add_bytes(archive, f"{guid}/pathname", path_text)
            meta = meta_for(rel, guid, full is None).encode()
            add_bytes(archive, f"{guid}/asset.meta", meta)
            if full is not None:
                with open(full, "rb") as handle:
                    add_bytes(archive, f"{guid}/asset", handle.read())
    with gzip.open(unitypackage, "wb") as handle:
        handle.write(raw.getvalue())
    print(tarball)
    print(unitypackage)


def add_bytes(archive, name, data):
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = 0o644
    archive.addfile(info, io.BytesIO(data))


if __name__ == "__main__":
    main()
