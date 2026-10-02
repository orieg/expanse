#!/usr/bin/env bash
# Installs a pinned, checksum-verified cargo-cyclonedx release binary.
#
# Usage:
#   scripts/install_cyclonedx.sh [dest_dir]
#
# Defaults to $HOME/.local/bin if dest_dir is not specified.
# Appends dest_dir to $GITHUB_PATH if running inside GitHub Actions.
set -euo pipefail

DEST_DIR="${1:-"${HOME}/.local/bin"}"
VERSION="0.5.9"

# Detect OS
OS_RAW="$(uname -s)"
case "${OS_RAW}" in
  Linux)  OS="linux" ;;
  Darwin) OS="darwin" ;;
  *)
    echo "::error::unsupported operating system: ${OS_RAW}" >&2
    exit 1
    ;;
esac

# Detect architecture
ARCH_RAW="$(uname -m)"
case "${ARCH_RAW}" in
  x86_64|amd64)   ARCH="amd64" ;;
  aarch64|arm64)  ARCH="arm64" ;;
  *)
    echo "::error::unsupported architecture: ${ARCH_RAW}" >&2
    exit 1
    ;;
esac

# Pinned SHA-256 checksums from CycloneDX/cyclonedx-rust-cargo release v0.5.9 sha256.sum
case "${OS}_${ARCH}" in
  linux_amd64)
    ARCHIVE="cargo-cyclonedx-x86_64-unknown-linux-musl.tar.xz"
    SHA256="9bd3e599314f50810c9d98b8b68a617ff9d3cc20873968d90b29d121f6b226ff"
    MEMBER="cargo-cyclonedx-x86_64-unknown-linux-musl/cargo-cyclonedx"
    ;;
  linux_arm64)
    ARCHIVE="cargo-cyclonedx-aarch64-unknown-linux-gnu.tar.xz"
    SHA256="7bf131ca5389b07a4f10c182bcf8a5ad339d64408b6f0d8f6834a0bd6120a06a"
    MEMBER="cargo-cyclonedx-aarch64-unknown-linux-gnu/cargo-cyclonedx"
    ;;
  darwin_amd64)
    ARCHIVE="cargo-cyclonedx-x86_64-apple-darwin.tar.xz"
    SHA256="59d2a583fa632f8759456c1b531340331255b277386d23c598a3dbbc916fde63"
    MEMBER="cargo-cyclonedx-x86_64-apple-darwin/cargo-cyclonedx"
    ;;
  darwin_arm64)
    ARCHIVE="cargo-cyclonedx-aarch64-apple-darwin.tar.xz"
    SHA256="4c53dfa21e70b65bf7f8d2592aadde3bcb02c1a40b6ec63b877e5ca65a29e180"
    MEMBER="cargo-cyclonedx-aarch64-apple-darwin/cargo-cyclonedx"
    ;;
  *)
    echo "::error::unsupported OS/architecture combination: ${OS}_${ARCH}" >&2
    exit 1
    ;;
esac

URL="https://github.com/CycloneDX/cyclonedx-rust-cargo/releases/download/cargo-cyclonedx-${VERSION}/${ARCHIVE}"

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

echo "Downloading cargo-cyclonedx v${VERSION} (${OS}_${ARCH})..."
curl --fail --silent --show-error --location --retry 3 --output "${tmp}/${ARCHIVE}" "${URL}"

echo "Verifying SHA-256 checksum..."
if command -v sha256sum >/dev/null 2>&1; then
  echo "${SHA256}  ${tmp}/${ARCHIVE}" | sha256sum --check --quiet
elif command -v shasum >/dev/null 2>&1; then
  echo "${SHA256}  ${tmp}/${ARCHIVE}" | shasum -a 256 --check --status
else
  echo "::error::no sha256 verification tool found (expected sha256sum or shasum)" >&2
  exit 1
fi

mkdir -p "${DEST_DIR}"
tar -xJf "${tmp}/${ARCHIVE}" -C "${tmp}"
cp "${tmp}/${MEMBER}" "${DEST_DIR}/cargo-cyclonedx"
chmod +x "${DEST_DIR}/cargo-cyclonedx"

echo "Verifying cargo-cyclonedx installation..."
"${DEST_DIR}/cargo-cyclonedx" cyclonedx --version

if [ -n "${GITHUB_PATH:-}" ]; then
  echo "${DEST_DIR}" >> "${GITHUB_PATH}"
fi

echo "Successfully installed cargo-cyclonedx to ${DEST_DIR}/cargo-cyclonedx"
