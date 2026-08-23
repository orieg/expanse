#!/bin/bash
set -euo pipefail

VERSION=${1:-"1.0.0"}
ARCH="amd64"
DIST_DIR="dist"
DEB_DIR="debian_build"

echo "Packaging version ${VERSION} for ${ARCH}..."

# Setup package directories
mkdir -p "${DEB_DIR}/libexpanse1/DEBIAN"
mkdir -p "${DEB_DIR}/libexpanse-dev/DEBIAN"
mkdir -p "${DEB_DIR}/libjudy-compat/DEBIAN"

# 1. libexpanse1 package
mkdir -p "${DEB_DIR}/libexpanse1/usr/lib/x86_64-linux-gnu"
cp -d "${DIST_DIR}/lib/libexpanse.so"* "${DEB_DIR}/libexpanse1/usr/lib/x86_64-linux-gnu/"
rm -f "${DEB_DIR}/libexpanse1/usr/lib/x86_64-linux-gnu/libJudy.so.1" # Remove judy symlink for this package

for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
    mkdir -p "${DEB_DIR}/libexpanse1/usr/lib/x86_64-linux-gnu/glibc-hwcaps/${HWCAP}"
    cp -d "${DIST_DIR}/lib/glibc-hwcaps/${HWCAP}/libexpanse.so"* "${DEB_DIR}/libexpanse1/usr/lib/x86_64-linux-gnu/glibc-hwcaps/${HWCAP}/"
    rm -f "${DEB_DIR}/libexpanse1/usr/lib/x86_64-linux-gnu/glibc-hwcaps/${HWCAP}/libJudy.so.1"
done

cat <<EOF > "${DEB_DIR}/libexpanse1/DEBIAN/control"
Package: libexpanse1
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: Expanse Maintainers <maintainers@expanse.local>
Description: Expanse trie engine shared library with glibc-hwcaps support.
EOF

# 2. libexpanse-dev package
mkdir -p "${DEB_DIR}/libexpanse-dev/usr/include"
mkdir -p "${DEB_DIR}/libexpanse-dev/usr/lib/x86_64-linux-gnu"
cp crates/expanse-capi/include/*.h* "${DEB_DIR}/libexpanse-dev/usr/include/" || echo "No headers found, skipping"
cp "${DIST_DIR}/lib/libexpanse.a" "${DEB_DIR}/libexpanse-dev/usr/lib/x86_64-linux-gnu/"

cat <<EOF > "${DEB_DIR}/libexpanse-dev/DEBIAN/control"
Package: libexpanse-dev
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: Expanse Maintainers <maintainers@expanse.local>
Depends: libexpanse1 (= ${VERSION})
Description: Expanse trie engine development files (headers, static library).
EOF

# 3. libjudy-compat package
mkdir -p "${DEB_DIR}/libjudy-compat/usr/lib/x86_64-linux-gnu"
ln -sf libexpanse.so.1 "${DEB_DIR}/libjudy-compat/usr/lib/x86_64-linux-gnu/libJudy.so.1"

for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
    mkdir -p "${DEB_DIR}/libjudy-compat/usr/lib/x86_64-linux-gnu/glibc-hwcaps/${HWCAP}"
    ln -sf libexpanse.so.1 "${DEB_DIR}/libjudy-compat/usr/lib/x86_64-linux-gnu/glibc-hwcaps/${HWCAP}/libJudy.so.1"
done

cat <<EOF > "${DEB_DIR}/libjudy-compat/DEBIAN/control"
Package: libjudy-compat
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: Expanse Maintainers <maintainers@expanse.local>
Depends: libexpanse1 (= ${VERSION})
Conflicts: libjudy
Provides: libjudy
Description: Drop-in compatibility for libjudy applications.
EOF

# Build packages
dpkg-deb --build "${DEB_DIR}/libexpanse1" "libexpanse1_${VERSION}_${ARCH}.deb"
dpkg-deb --build "${DEB_DIR}/libexpanse-dev" "libexpanse-dev_${VERSION}_${ARCH}.deb"
dpkg-deb --build "${DEB_DIR}/libjudy-compat" "libjudy-compat_${VERSION}_${ARCH}.deb"

echo "Debian packaging completed successfully!"
