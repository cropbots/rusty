#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="wasm32-unknown-unknown"
CRATE_NAME="rustycropbot"

WASM_SOURCE="${ROOT_DIR}/target/${TARGET_TRIPLE}/release/${CRATE_NAME}.wasm"
WASM_DEST="${ROOT_DIR}/web/${CRATE_NAME}.wasm"
ASSETS_SOURCE="${ROOT_DIR}/src/assets"
ASSETS_DEST="${ROOT_DIR}/web/assets"

cargo build \
  --release \
  --target "${TARGET_TRIPLE}" \
  --manifest-path "${ROOT_DIR}/Cargo.toml"

mkdir -p "${ROOT_DIR}/web"
cp "${WASM_SOURCE}" "${WASM_DEST}"

mkdir -p "${ASSETS_DEST}"
cp -a "${ASSETS_SOURCE}/." "${ASSETS_DEST}/"

"${ROOT_DIR}/scripts/generate-wasm-indexes.sh"

for dir in entity particle sound structure; do
  src_dir="${ROOT_DIR}/src/${dir}"
  dest_dir="${ASSETS_DEST}/${dir}"
  mkdir -p "${dest_dir}"
  cp -a "${src_dir}/." "${dest_dir}/"
done

printf 'Built wasm and copied assets to web output.\n'
