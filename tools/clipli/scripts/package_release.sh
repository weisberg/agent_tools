#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${CRATE_DIR}/../.." && pwd)"
OUT_DIR="${OUT_DIR:-${CRATE_DIR}/target/dist}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(rustc -vV | awk '/^host:/ {print $2}')}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${CRATE_DIR}/Cargo.toml" | head -n 1)"

if [[ -z "${VERSION}" ]]; then
  echo "Could not read clipli version from Cargo.toml" >&2
  exit 1
fi

PACKAGE_NAME="clipli-${VERSION}-${TARGET_TRIPLE}"
STAGE_DIR="${OUT_DIR}/${PACKAGE_NAME}"

if [[ "${TARGET_TRIPLE}" == "$(rustc -vV | awk '/^host:/ {print $2}')" ]]; then
  cargo build --manifest-path "${CRATE_DIR}/Cargo.toml" --release
  BIN_PATH="${CRATE_DIR}/target/release/clipli"
else
  cargo build --manifest-path "${CRATE_DIR}/Cargo.toml" --release --target "${TARGET_TRIPLE}"
  BIN_PATH="${CRATE_DIR}/target/${TARGET_TRIPLE}/release/clipli"
fi

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/completions"

cp "${BIN_PATH}" "${STAGE_DIR}/clipli"
cp "${REPO_ROOT}/LICENSE" "${STAGE_DIR}/LICENSE"
cp "${CRATE_DIR}/README.md" "${STAGE_DIR}/README.md"
cp "${CRATE_DIR}/RELEASE.md" "${STAGE_DIR}/RELEASE.md"

"${STAGE_DIR}/clipli" completions bash > "${STAGE_DIR}/completions/clipli.bash"
"${STAGE_DIR}/clipli" completions zsh > "${STAGE_DIR}/completions/_clipli"
"${STAGE_DIR}/clipli" completions fish > "${STAGE_DIR}/completions/clipli.fish"

(
  cd "${OUT_DIR}"
  tar -czf "${PACKAGE_NAME}.tar.gz" "${PACKAGE_NAME}"
  shasum -a 256 "${PACKAGE_NAME}.tar.gz" > "${PACKAGE_NAME}.tar.gz.sha256"
  shasum -a 256 "${PACKAGE_NAME}.tar.gz" > SHA256SUMS
)

cat <<EOF
Created:
${OUT_DIR}/${PACKAGE_NAME}.tar.gz
${OUT_DIR}/${PACKAGE_NAME}.tar.gz.sha256
${OUT_DIR}/SHA256SUMS
EOF
