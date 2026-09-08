#!/bin/bash
set -euo pipefail

# Build the glibc-hwcaps micro-architecture variants of libexpanse
# (x86-64-v2, v3, v4) into `dist/lib/glibc-hwcaps/`, where
# `scripts/package_deb.sh` and `scripts/package_rpm.sh` already look for them.
#
# Usage:
#   scripts/build_hwcaps.sh [target-triple] [dist-dir]
#
# Defaults: the host triple, and `dist`.
#
# **The baseline is not built here.** The release job builds and stages it
# (`target/<triple>/release/libexpanse.so` -> `dist/lib/`), and an earlier
# version of this script rebuilt its own baseline over that, after a
# `cargo clean -p expanse-capi` that discarded the job's artifact. Both are why
# it was never wired into a workflow safely; this version only adds the
# variants and never touches `dist/lib/libexpanse.so*`.
#
# Each variant builds into its own `--target-dir`, so no `cargo clean` is
# needed between the differing RUSTFLAGS and nothing the caller built is
# disturbed.

TARGET="${1:-$(rustc -vV | awk '/^host:/{print $2}')}"
DIST_DIR="${2:-dist}"

case "$TARGET" in
    x86_64-*) ;;
    *)
        echo "build_hwcaps.sh: $TARGET is not x86_64; glibc-hwcaps variants are" >&2
        echo "an x86-64 micro-architecture feature and there is nothing to build." >&2
        exit 0
        ;;
esac

build_variant() {
    local level=$1
    local dest="${DIST_DIR}/lib/glibc-hwcaps/${level}"
    local tdir="target/hwcaps/${level}"

    echo "building ${level} for ${TARGET} ..."
    mkdir -p "$dest"
    RUSTFLAGS="-C target-cpu=${level}" cargo build --release \
        --manifest-path crates/expanse-capi/Cargo.toml \
        --target "$TARGET" --target-dir "$tdir"

    local so="${tdir}/${TARGET}/release/libexpanse.so"
    if [ ! -f "$so" ]; then
        echo "build_hwcaps.sh: ${so} was not produced" >&2
        exit 1
    fi
    cp "$so" "${dest}/libexpanse.so.1.0.0"
    ln -sf libexpanse.so.1.0.0 "${dest}/libexpanse.so.1"
    ln -sf libexpanse.so.1 "${dest}/libexpanse.so"
    # The libjudy drop-in soname, as the baseline directory carries it.
    ln -sf libexpanse.so.1 "${dest}/libJudy.so.1"
}

build_variant x86-64-v2
build_variant x86-64-v3
build_variant x86-64-v4

echo "glibc-hwcaps variants built under ${DIST_DIR}/lib/glibc-hwcaps/"
