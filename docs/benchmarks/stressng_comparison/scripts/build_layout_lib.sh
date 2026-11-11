#!/usr/bin/env bash
# ==============================================================================
# One libexpanse build for the layout-only diagnostic (README.md, "Diagnostic:
# layout-only builds"): build.sh's libexpanse build, with RUSTFLAGS as given
# and nothing else changed, laid out as build.sh lays out expcompat/lib.
#
#   build_layout_lib.sh <source_dir> <dest_lib_dir> [rustflags] [trie_features]
#
# The source dir is a `git archive` of the commit to build. An empty rustflags
# builds the source as build.sh does, which is the control that the toolchain
# reproduces the measured file byte for byte. `trie_features` names
# `expanse-trie` features to build in (comma-separated, e.g. the #1679
# accounting ablation's `ablation-unlocked-plain-acct`); empty builds none.
# ==============================================================================
set -euo pipefail
SRC="${1:?source dir}"
DEST="${2:?destination lib dir}"
FLAGS="${3:-}"
FEATURES="${4:-}"
FEAT_ARGS=()
if [ -n "${FEATURES}" ]; then
  FEAT_ARGS=(--features "$(echo "${FEATURES}" | sed 's/[^,]*/expanse-trie\/&/g')")
fi
export PATH="${HOME}/.cargo/bin:${PATH}"
TARGET="${DEST%/lib}-target"
(cd "${SRC}" && EXPANSE_LINK_SONAME=1 CARGO_TARGET_DIR="${TARGET}" RUSTFLAGS="${FLAGS}" \
  cargo build -q --release -p expanse-capi ${FEAT_ARGS[@]+"${FEAT_ARGS[@]}"})
mkdir -p "${DEST}"
cp "${TARGET}/release/libexpanse.so" "${DEST}/libexpanse.so.1.0.0"
ln -sf libexpanse.so.1.0.0 "${DEST}/libexpanse.so.1"
ln -sf libexpanse.so.1 "${DEST}/libexpanse.so"
ln -sf libexpanse.so.1 "${DEST}/libJudy.so.1"
# A source from before the opt-in DT_SONAME (#1667) builds without one, as its
# measured file did; one that has the opt-in must carry it.
if grep -q EXPANSE_LINK_SONAME "${SRC}/crates/expanse-capi/build.rs" 2>/dev/null; then
  readelf -d "${DEST}/libexpanse.so.1.0.0" | grep -q 'SONAME.*libexpanse.so.1' \
    || { echo "libexpanse has no DT_SONAME libexpanse.so.1" >&2; exit 1; }
fi
rustc --version
sha256sum "${DEST}/libexpanse.so.1.0.0"
