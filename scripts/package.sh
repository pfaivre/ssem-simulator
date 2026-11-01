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
    x86_64-unknown-linux-gnu)   PLATFORM="linux-x86_64" ;;
    aarch64-unknown-linux-gnu)  PLATFORM="linux-aarch64" ;;
    *)                          PLATFORM="$HOST_TRIPLE" ;;
esac

ARTIFACT="ssem-simulator-v${VERSION}-${PLATFORM}"

echo "Building release binary (${HOST_TRIPLE})..."
cargo build --release

STAGE="dist/${ARTIFACT}"
rm -rf dist
mkdir -p "${STAGE}"
cp "target/release/ssem-simulator" "${STAGE}/"
cp -r samples "${STAGE}/"
cp README.md LICENSE "${STAGE}/"

cd dist
zip -r -q "${ARTIFACT}.zip" "${ARTIFACT}"
cd ..

echo "Created dist/${ARTIFACT}.zip"
