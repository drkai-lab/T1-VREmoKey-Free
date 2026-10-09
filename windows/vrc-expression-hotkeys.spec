# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec: console build (Windows)."""

import os

from PyInstaller.config import CONF

CONF['distpath'] = os.path.normpath(os.path.join(SPECPATH, os.pardir, 'dist-win'))
os.makedirs(CONF['distpath'], exist_ok=True)

block_cipher = None

a = Analysis(
    [os.path.join(SPECPATH, os.pardir, 'vrc-expression-hotkeys.py')],
    pathex=[],
    binaries=[],
    datas=[],
    hiddenimports=[],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=['fcntl', 'termios', 'pwd', 'grp', 'resource', 'posix', 'posixpath'],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='vrc-expression-hotkeys',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    version=os.path.join(SPECPATH, 'version.txt'),
)
