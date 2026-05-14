#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="wasm32-unknown-unknown"
CRATE_NAME="rustycropbot"

WASM_SOURCE="${ROOT_DIR}/target/${TARGET_TRIPLE}/release/${CRATE_NAME}.wasm"
WASM_DEST="${ROOT_DIR}/web/${CRATE_NAME}.wasm"
ASSETS_SOURCE="${ROOT_DIR}/src/assets"
ASSETS_DEST="${ROOT_DIR}/web/assets"

CARGO_BIN="${CARGO_BIN:-$(command -v cargo 2>/dev/null || true)}"
if [ -z "${CARGO_BIN}" ]; then
  CARGO_BIN="${HOME}/.cargo/bin/cargo"
fi
if [ ! -x "${CARGO_BIN}" ]; then
  echo "cargo not found; install Rust or set CARGO_BIN to your cargo executable." >&2
  exit 1
fi

"${CARGO_BIN}" build \
  --release \
  --target "${TARGET_TRIPLE}" \
  --manifest-path "${ROOT_DIR}/Cargo.toml"

mkdir -p "${ROOT_DIR}/web"
cp "${WASM_SOURCE}" "${WASM_DEST}"

mkdir -p "${ASSETS_DEST}"
cp -a "${ASSETS_SOURCE}/." "${ASSETS_DEST}/"

"${ROOT_DIR}/scripts/generate-wasm-indexes.sh"

for dir in entity particle sound structure items; do
  src_dir="${ROOT_DIR}/src/${dir}"
  dest_dir="${ASSETS_DEST}/${dir}"
  mkdir -p "${dest_dir}"
  cp -a "${src_dir}/." "${dest_dir}/"
done

printf 'Built wasm and copied assets to web output.\n'
