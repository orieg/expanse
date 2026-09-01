#!/bin/bash
set -euo pipefail

# Builds the three Debian packages (libexpanse1 / libexpanse-dev / libjudy-compat)
# for one architecture from a staged dist/ tree.
#
# Usage: package_deb.sh <version> <deb_arch> <dist_dir>
#   deb_arch: amd64 | arm64 | riscv64
#   dist_dir: directory containing lib/ (and optionally lib/glibc-hwcaps/) + the
#             C headers are read from include/.

VERSION=${1:-"0.4.0"}
DEB_ARCH=${2:-"amd64"}
DIST_DIR=${3:-"dist"}
DEB_DIR="debian_build"

case "${DEB_ARCH}" in
    amd64)   TRIPLE="x86_64-linux-gnu" ;;
    arm64)   TRIPLE="aarch64-linux-gnu" ;;
    riscv64) TRIPLE="riscv64-linux-gnu" ;;
    *) echo "::error::package_deb.sh: unsupported deb arch '${DEB_ARCH}'" >&2; exit 1 ;;
esac

if [ ! -d "${DIST_DIR}/lib" ]; then
    echo "::error::package_deb.sh: '${DIST_DIR}/lib' does not exist" >&2
    exit 1
fi

echo "Packaging libexpanse ${VERSION} for ${DEB_ARCH} (${TRIPLE}) from ${DIST_DIR}..."

rm -rf "${DEB_DIR}"

# Normalize the runtime soname chain into a private staging dir so packaging is
# correct whether dist/lib holds a full chain (from build_hwcaps.sh) or only the
# unversioned libexpanse.so that `cargo build -p expanse-capi` emits.
normalize_soname() {
    # $1 = source lib dir, $2 = destination staging dir
    local src="$1" dst="$2"
    mkdir -p "$dst"
    if [ -f "${src}/libexpanse.so.1.0.0" ]; then
        cp "${src}/libexpanse.so.1.0.0" "${dst}/libexpanse.so.1.0.0"
    elif [ -f "${src}/libexpanse.so" ] && [ ! -L "${src}/libexpanse.so" ]; then
        cp "${src}/libexpanse.so" "${dst}/libexpanse.so.1.0.0"
    else
        return 1
    fi
    ln -sf libexpanse.so.1.0.0 "${dst}/libexpanse.so.1"
    ln -sf libexpanse.so.1 "${dst}/libexpanse.so"
    return 0
}

STAGE="${DEB_DIR}/_stage"
if ! normalize_soname "${DIST_DIR}/lib" "${STAGE}/lib"; then
    echo "::error::package_deb.sh: no libexpanse.so runtime library found in ${DIST_DIR}/lib" >&2
    exit 1
fi

# Optional glibc-hwcaps variants (x86_64 only).
HWCAPS=()
if [ -d "${DIST_DIR}/lib/glibc-hwcaps" ]; then
    for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
        if [ -d "${DIST_DIR}/lib/glibc-hwcaps/${HWCAP}" ]; then
            normalize_soname "${DIST_DIR}/lib/glibc-hwcaps/${HWCAP}" "${STAGE}/lib/glibc-hwcaps/${HWCAP}" \
                && HWCAPS+=("${HWCAP}")
        fi
    done
fi

# Setup package directories
mkdir -p "${DEB_DIR}/libexpanse1/DEBIAN"
mkdir -p "${DEB_DIR}/libexpanse-dev/DEBIAN"
mkdir -p "${DEB_DIR}/libjudy-compat/DEBIAN"

LIBDIR="usr/lib/${TRIPLE}"

# 1. libexpanse1 package: versioned runtime library only (.so.1.0.0 + .so.1 soname link)
mkdir -p "${DEB_DIR}/libexpanse1/${LIBDIR}"
cp -d "${STAGE}/lib/libexpanse.so.1.0.0" "${STAGE}/lib/libexpanse.so.1" \
    "${DEB_DIR}/libexpanse1/${LIBDIR}/"

for HWCAP in "${HWCAPS[@]}"; do
    mkdir -p "${DEB_DIR}/libexpanse1/${LIBDIR}/glibc-hwcaps/${HWCAP}"
    cp -d "${STAGE}/lib/glibc-hwcaps/${HWCAP}/libexpanse.so.1.0.0" \
          "${STAGE}/lib/glibc-hwcaps/${HWCAP}/libexpanse.so.1" \
          "${DEB_DIR}/libexpanse1/${LIBDIR}/glibc-hwcaps/${HWCAP}/"
done

cat <<EOF > "${DEB_DIR}/libexpanse1/DEBIAN/control"
Package: libexpanse1
Version: ${VERSION}
Architecture: ${DEB_ARCH}
Maintainer: Nicolas Brousse <nicolas@brousse.info>
Description: Expanse trie engine shared library with glibc-hwcaps support.
EOF

# 2. libexpanse-dev package: headers, static library, pkg-config, man pages, and the unversioned .so dev symlink
mkdir -p "${DEB_DIR}/libexpanse-dev/usr/include"
mkdir -p "${DEB_DIR}/libexpanse-dev/${LIBDIR}/pkgconfig"
mkdir -p "${DEB_DIR}/libexpanse-dev/usr/share/man/man3"

cp include/*.h* "${DEB_DIR}/libexpanse-dev/usr/include/"
if [ -f "${DIST_DIR}/lib/libexpanse.a" ]; then
    cp "${DIST_DIR}/lib/libexpanse.a" "${DEB_DIR}/libexpanse-dev/${LIBDIR}/"
fi
ln -sf libexpanse.so.1 "${DEB_DIR}/libexpanse-dev/${LIBDIR}/libexpanse.so"

if [ -f "extra/pkgconfig/expanse.pc.in" ]; then
    sed -e "s|@prefix@|/usr|g" \
        -e "s|@VERSION@|${VERSION}|g" \
        -e "s|/lib|/${LIBDIR#usr/}|g" \
        extra/pkgconfig/expanse.pc.in > "${DEB_DIR}/libexpanse-dev/${LIBDIR}/pkgconfig/expanse.pc"
fi

# AGENTS.md §8.1: a missing source tree must fail loudly, not ship a package
# that silently lacks the man pages the control block advertises.
if [ ! -d "man/man3" ]; then
    echo "error: man/man3 not found — cannot build libexpanse-dev with man pages" >&2
    exit 1
fi
for f in man/man3/expanse*.3; do
    [ -f "$f" ] || { echo "error: no expanse*.3 man pages in man/man3" >&2; exit 1; }
    gzip -9nc "$f" > "${DEB_DIR}/libexpanse-dev/usr/share/man/man3/$(basename "$f").gz"
done

cat <<EOF > "${DEB_DIR}/libexpanse-dev/DEBIAN/control"
Package: libexpanse-dev
Version: ${VERSION}
Architecture: ${DEB_ARCH}
Maintainer: Nicolas Brousse <nicolas@brousse.info>
Depends: libexpanse1 (= ${VERSION})
Description: Expanse trie engine development files (headers, static library, man pages).
EOF

# 3. libjudy-compat package: drop-in libJudy.so.1 soname pointing at libexpanse.so.1 + pkg-config + Judy man pages
mkdir -p "${DEB_DIR}/libjudy-compat/${LIBDIR}"
mkdir -p "${DEB_DIR}/libjudy-compat/usr/share/man/man3"

ln -sf libexpanse.so.1 "${DEB_DIR}/libjudy-compat/${LIBDIR}/libJudy.so.1"

for HWCAP in "${HWCAPS[@]}"; do
    mkdir -p "${DEB_DIR}/libjudy-compat/${LIBDIR}/glibc-hwcaps/${HWCAP}"
    ln -sf libexpanse.so.1 "${DEB_DIR}/libjudy-compat/${LIBDIR}/glibc-hwcaps/${HWCAP}/libJudy.so.1"
done

# judy.pc emits `-lexpanse` and `-I${includedir}`, which need the unversioned
# .so dev symlink and Judy.h — both shipped by libexpanse-dev, not here. Placing
# it in libjudy-compat (a runtime package depending only on libexpanse1) makes
# `pkg-config --cflags --libs judy` resolve to files the user has not installed.
if [ -f "extra/pkgconfig/judy.pc.in" ]; then
    sed -e "s|@prefix@|/usr|g" \
        -e "s|@VERSION@|${VERSION}|g" \
        -e "s|/lib|/${LIBDIR#usr/}|g" \
        extra/pkgconfig/judy.pc.in > "${DEB_DIR}/libexpanse-dev/${LIBDIR}/pkgconfig/judy.pc"
fi

for f in man/man3/Judy*.3; do
    [ -f "$f" ] || { echo "error: no Judy*.3 man pages in man/man3" >&2; exit 1; }
    gzip -9nc "$f" > "${DEB_DIR}/libjudy-compat/usr/share/man/man3/$(basename "$f").gz"
done

cat <<EOF > "${DEB_DIR}/libjudy-compat/DEBIAN/control"
Package: libjudy-compat
Version: ${VERSION}
Architecture: ${DEB_ARCH}
Maintainer: Nicolas Brousse <nicolas@brousse.info>
Depends: libexpanse1 (= ${VERSION})
Conflicts: libjudy
Provides: libjudy
Description: Drop-in compatibility for libjudy applications (symlinks, man pages).
EOF

# Build packages
dpkg-deb --build "${DEB_DIR}/libexpanse1" "libexpanse1_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --build "${DEB_DIR}/libexpanse-dev" "libexpanse-dev_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --build "${DEB_DIR}/libjudy-compat" "libjudy-compat_${VERSION}_${DEB_ARCH}.deb"

echo "Debian packaging completed successfully (${DEB_ARCH})!"
