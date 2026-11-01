#!/usr/bin/env bash
# Build the release binary and pack it with the sample programs into a zip
# artifact: dist/ssem-simulator-v<version>-<platform>.zip
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)
if [ -z "$VERSION" ]; then
    echo "error: could not read version from Cargo.toml" >&2
    exit 1
fi

HOST_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
case "$HOST_TRIPLE" in
    x86_64-unknown-linux-gnu)                     PLATFORM="linux-x86_64" ;;
    aarch64-unknown-linux-gnu)                    PLATFORM="linux-aarch64" ;;
    x86_64-pc-windows-msvc|x86_64-pc-windows-gnu) PLATFORM="windows-x86_64" ;;
    x86_64-apple-darwin)                          PLATFORM="macos-x86_64" ;;
    aarch64-apple-darwin)                         PLATFORM="macos-aarch64" ;;
    *)                                            PLATFORM="$HOST_TRIPLE" ;;
esac

ARTIFACT="ssem-simulator-v${VERSION}-${PLATFORM}"

echo "Building release binary (${HOST_TRIPLE})..."
cargo build --release

BIN="target/release/ssem-simulator"
if [ -f "${BIN}.exe" ]; then
    BIN="${BIN}.exe"
fi

STAGE="dist/${ARTIFACT}"
rm -rf dist
mkdir -p "${STAGE}"
cp "$BIN" "${STAGE}/"
cp -r samples "${STAGE}/"
cp README.md LICENSE "${STAGE}/"

cd dist
if command -v zip >/dev/null 2>&1; then
    zip -r -q "${ARTIFACT}.zip" "${ARTIFACT}"
else
    # Windows runners may lack zip(1); fall back to Python's zipfile module,
    # which preserves unix permission bits via ZipInfo.from_file.
    PY=$(command -v python3 || command -v python)
    "$PY" - "${ARTIFACT}" "${ARTIFACT}.zip" <<'EOF'
import os, sys, zipfile
root, out = sys.argv[1], sys.argv[2]
parent = os.path.dirname(root)
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
    zf.write(root, os.path.basename(root))
    for dirpath, dirnames, filenames in os.walk(root):
        for name in sorted(dirnames) + sorted(filenames):
            p = os.path.join(dirpath, name)
            zf.write(p, os.path.relpath(p, parent))
EOF
fi
cd ..

echo "Created dist/${ARTIFACT}.zip"
