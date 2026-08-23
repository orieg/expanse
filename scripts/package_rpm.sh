#!/bin/bash
set -euo pipefail

VERSION=${1:-"0.3.0"}
ARCH=${2:-"x86_64"}
DIST_DIR=${3:-"dist"}
RPM_TOPDIR="rpm_build"

echo "Building RPM packages for Expanse version ${VERSION} (${ARCH})..."

case "${ARCH}" in
    amd64|x86_64) RPM_ARCH="x86_64" ;;
    arm64|aarch64) RPM_ARCH="aarch64" ;;
    riscv64) RPM_ARCH="riscv64" ;;
    *) RPM_ARCH="${ARCH}" ;;
esac

rm -rf "${RPM_TOPDIR}"
mkdir -p "${RPM_TOPDIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS,BUILDROOT}

SPEC_FILE="${RPM_TOPDIR}/SPECS/expanse.spec"

cat <<'SPECEOF' > "${SPEC_FILE}"
Name:           libexpanse
Version:        VERSION_PLACEHOLDER
Release:        1%{?dist}
Summary:        Modern Judy arrays and high-performance digital tree engine
License:        MIT OR Apache-2.0
URL:            https://github.com/orieg/expanse
BuildArch:      ARCH_PLACEHOLDER

%description
Expanse is a clean-room, pure-Rust implementation of Judy arrays modernized
for 64-bit microarchitectures with zero-allocation immediates, SWAR/SIMD vectorization,
and lock-free optimistic concurrency control (OCC).

%package devel
Summary:        Development files for libexpanse
Requires:       %{name}%{?_isa} = %{version}-%{release}

%description devel
Development headers, static library, and pkg-config manifests for Expanse.

%package -n libjudy-compat
Summary:        Drop-in libjudy compatibility symlinks
Requires:       %{name}%{?_isa} = %{version}-%{release}
Provides:       libjudy = 1.0.5
Conflicts:      libjudy

%description -n libjudy-compat
Drop-in compatibility library providing libJudy.so.1 symlinks directed to libexpanse.

%prep

%build

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}/usr/lib64
mkdir -p %{buildroot}/usr/include
mkdir -p %{buildroot}/usr/lib64/pkgconfig

# Runtime libraries
if [ -d "DIST_DIR_PLACEHOLDER/lib" ]; then
    cp -P DIST_DIR_PLACEHOLDER/lib/libexpanse.so* %{buildroot}/usr/lib64/ 2>/dev/null || true
    if [ -f "DIST_DIR_PLACEHOLDER/lib/libexpanse.a" ]; then
        cp DIST_DIR_PLACEHOLDER/lib/libexpanse.a %{buildroot}/usr/lib64/
    fi
elif [ -d "package/lib" ]; then
    cp -P package/lib/libexpanse.so* %{buildroot}/usr/lib64/ 2>/dev/null || true
    if [ -f "package/lib/libexpanse.a" ]; then
        cp package/lib/libexpanse.a %{buildroot}/usr/lib64/
    fi
fi

# glibc-hwcaps variants
for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
    if [ -d "DIST_DIR_PLACEHOLDER/lib/glibc-hwcaps/${HWCAP}" ]; then
        mkdir -p "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}"
        cp -P DIST_DIR_PLACEHOLDER/lib/glibc-hwcaps/${HWCAP}/libexpanse.so* "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}/" 2>/dev/null || true
    fi
done

# Headers
if [ -d "crates/expanse-capi/include" ]; then
    cp crates/expanse-capi/include/*.h* %{buildroot}/usr/include/
elif [ -d "package/include" ]; then
    cp package/include/*.h* %{buildroot}/usr/include/
fi

# Compat symlink
ln -sf libexpanse.so.1 %{buildroot}/usr/lib64/libJudy.so.1
for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
    if [ -d "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}" ]; then
        ln -sf libexpanse.so.1 "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}/libJudy.so.1"
    fi
done

# Pkg-config files
if [ -f "extra/pkgconfig/expanse.pc.in" ]; then
    sed -e "s|@PREFIX@|/usr|g" \
        -e "s|@LIBDIR@|/usr/lib64|g" \
        -e "s|@INCLUDEDIR@|/usr/include|g" \
        -e "s|@VERSION@|VERSION_PLACEHOLDER|g" \
        extra/pkgconfig/expanse.pc.in > %{buildroot}/usr/lib64/pkgconfig/expanse.pc
fi
if [ -f "extra/pkgconfig/judy.pc.in" ]; then
    sed -e "s|@PREFIX@|/usr|g" \
        -e "s|@LIBDIR@|/usr/lib64|g" \
        -e "s|@INCLUDEDIR@|/usr/include|g" \
        -e "s|@VERSION@|VERSION_PLACEHOLDER|g" \
        extra/pkgconfig/judy.pc.in > %{buildroot}/usr/lib64/pkgconfig/judy.pc
fi

%files
/usr/lib64/libexpanse.so*

%files devel
/usr/include/*.h
/usr/lib64/libexpanse.a
/usr/lib64/pkgconfig/*.pc

%files -n libjudy-compat
/usr/lib64/libJudy.so.1

SPECEOF

sed -i.bak \
    -e "s|VERSION_PLACEHOLDER|${VERSION}|g" \
    -e "s|ARCH_PLACEHOLDER|${RPM_ARCH}|g" \
    -e "s|DIST_DIR_PLACEHOLDER|${DIST_DIR}|g" \
    "${SPEC_FILE}"
rm -f "${SPEC_FILE}.bak"

if command -v rpmbuild &>/dev/null; then
    rpmbuild --define "_topdir $(pwd)/${RPM_TOPDIR}" --target "${RPM_ARCH}" -bb "${SPEC_FILE}"
    find "${RPM_TOPDIR}/RPMS" -name "*.rpm" -exec cp {} . \;
    echo "RPM packaging completed successfully!"
else
    echo "rpmbuild command not found, generated spec file at ${SPEC_FILE}"
fi
