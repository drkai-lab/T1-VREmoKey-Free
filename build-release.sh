#!/usr/bin/env bash
# Build the Linux tarball into ./dist/
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION="$(sed -n 's/^VERSION = "\(.*\)"/\1/p' "${ROOT}/vrc-expression-hotkeys.py")"
NAME="vrc-expression-hotkeys-linux-${VERSION}"
DIST="${ROOT}/dist"
STAGE="${DIST}/${NAME}"

rm -rf "${STAGE}"
mkdir -p "${STAGE}/tests"

cp -a "${ROOT}/vrc-expression-hotkeys.py" \
      "${ROOT}/README.md" \
      "${ROOT}/README.ja.md" \
      "${ROOT}/LICENSE" \
      "${ROOT}/ruff.toml" \
      "${STAGE}/"
cp -a "${ROOT}/tests/." "${STAGE}/tests/"
cp -a "${ROOT}/unity" "${STAGE}/unity"

python3 -m py_compile "${STAGE}/vrc-expression-hotkeys.py"
python3 "${STAGE}/tests/test_hotkeys.py" >/dev/null
rm -rf "${STAGE}/__pycache__" "${STAGE}/tests/__pycache__"

chmod +x "${STAGE}/vrc-expression-hotkeys.py"

tar -C "${DIST}" -czf "${DIST}/${NAME}.tar.gz" "${NAME}"
# Bare filename so `sha256sum -c` works on any machine.
(
  cd "${DIST}"
  sha256sum "${NAME}.tar.gz" > "${NAME}.tar.gz.sha256"
)
rm -rf "${STAGE}"

echo "${DIST}/${NAME}.tar.gz"
cat "${DIST}/${NAME}.tar.gz.sha256"
