#!/usr/bin/env bash
# Installs a pinned, checksum-verified OpenSSF Scorecard CLI release binary.
#
# Usage:
#   scripts/install_scorecard.sh [dest_dir]
#
# Defaults to $HOME/.local/bin if dest_dir is not specified.
# Appends dest_dir to $GITHUB_PATH if running inside GitHub Actions.
set -euo pipefail

DEST_DIR="${1:-"${HOME}/.local/bin"}"
VERSION="5.5.0"

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

# Pinned SHA-256 checksums from scorecard_checksums.txt for release v5.5.0
case "${OS}_${ARCH}" in
  linux_amd64)
    SHA256="83b90a05c1540ef1390db1cd5711e5fd04be9c1d8537fb84d39d02092d6a8dff"
    ;;
  linux_arm64)
    SHA256="3ce59d20c1d53e540c4a14e0da1e0d96b3b294e8ddc96a3c5a7b8a637b32991e"
    ;;
  darwin_amd64)
    SHA256="979487ca20e726f6a4d2bd63a0a4c544184f589724b3d12d2ba8d0ea80889063"
    ;;
  darwin_arm64)
    SHA256="bac6371a4f810d6bdd0b65d63c3311906bdfe3ba0d76a5ea743ce24ced170fcf"
    ;;
  *)
    echo "::error::unsupported OS/architecture combination: ${OS}_${ARCH}" >&2
    exit 1
    ;;
esac

URL="https://github.com/ossf/scorecard/releases/download/v${VERSION}/scorecard_${VERSION}_${OS}_${ARCH}.tar.gz"

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

echo "Downloading OpenSSF Scorecard v${VERSION} (${OS}_${ARCH})..."
curl --fail --silent --show-error --location --retry 3 --output "${tmp}/scorecard.tar.gz" "${URL}"

echo "Verifying SHA-256 checksum..."
if command -v sha256sum >/dev/null 2>&1; then
  echo "${SHA256}  ${tmp}/scorecard.tar.gz" | sha256sum --check --quiet
elif command -v shasum >/dev/null 2>&1; then
  echo "${SHA256}  ${tmp}/scorecard.tar.gz" | shasum -a 256 --check --status
else
  echo "::error::no sha256 verification tool found (expected sha256sum or shasum)" >&2
  exit 1
fi

mkdir -p "${DEST_DIR}"
tar -xzf "${tmp}/scorecard.tar.gz" -C "${DEST_DIR}" scorecard
chmod +x "${DEST_DIR}/scorecard"

echo "Verifying scorecard installation..."
"${DEST_DIR}/scorecard" version

if [ -n "${GITHUB_PATH:-}" ]; then
  echo "${DEST_DIR}" >> "${GITHUB_PATH}"
fi

echo "Successfully installed OpenSSF Scorecard to ${DEST_DIR}/scorecard"
