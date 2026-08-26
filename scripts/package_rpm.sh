#!/bin/bash
set -euo pipefail

VERSION=${1:-"0.4.0"}
ARCH=${2:-"x86_64"}
DIST_DIR=${3:-"dist"}
RPM_TOPDIR="$(pwd)/rpm_build"

# The RPM %install scriptlet runs with rpmbuild's own working directory, so a
# relative dist path (e.g. "package") never resolves. Anchor it to an absolute
# path up front and fail loudly if it is missing.
if [ ! -d "${DIST_DIR}" ]; then
    echo "::error::package_rpm.sh: dist directory '${DIST_DIR}' does not exist" >&2
    exit 1
fi
DIST_DIR="$(cd "${DIST_DIR}" && pwd)"

echo "Building RPM packages for Expanse version ${VERSION} (${ARCH}) from ${DIST_DIR}..."

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
%define __strip /bin/true
%define __brp_strip %{nil}
%global debug_package %{nil}

Name:           libexpanse
Version:        VERSION_PLACEHOLDER
Release:        1%{?dist}
Summary:        Modern Judy arrays and high-performance digital tree engine
License:        MIT OR Apache-2.0
URL:            https://github.com/orieg/expanse

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

# Runtime libraries (error swallowing removed: a missing runtime library must fail the build)
cp -P DIST_DIR_PLACEHOLDER/lib/libexpanse.so* %{buildroot}/usr/lib64/
if [ -f "DIST_DIR_PLACEHOLDER/lib/libexpanse.a" ]; then
    cp DIST_DIR_PLACEHOLDER/lib/libexpanse.a %{buildroot}/usr/lib64/
fi

# glibc-hwcaps variants (optional; only present for x86_64 baseline builds)
for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
    if [ -d "DIST_DIR_PLACEHOLDER/lib/glibc-hwcaps/${HWCAP}" ]; then
        mkdir -p "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}"
        cp -P DIST_DIR_PLACEHOLDER/lib/glibc-hwcaps/${HWCAP}/libexpanse.so* "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}/"
    fi
done

# Headers
if [ -d "REPO_ROOT_PLACEHOLDER/crates/expanse-capi/include" ]; then
    cp REPO_ROOT_PLACEHOLDER/crates/expanse-capi/include/*.h* %{buildroot}/usr/include/
elif [ -d "REPO_ROOT_PLACEHOLDER/package/include" ]; then
    cp REPO_ROOT_PLACEHOLDER/package/include/*.h* %{buildroot}/usr/include/
elif [ -d "DIST_DIR_PLACEHOLDER/include" ]; then
    cp DIST_DIR_PLACEHOLDER/include/*.h* %{buildroot}/usr/include/
fi

# Compat symlink
ln -sf libexpanse.so.1 %{buildroot}/usr/lib64/libJudy.so.1
for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
    if [ -d "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}" ]; then
        ln -sf libexpanse.so.1 "%{buildroot}/usr/lib64/glibc-hwcaps/${HWCAP}/libJudy.so.1"
    fi
done

# Pkg-config files
if [ -f "REPO_ROOT_PLACEHOLDER/extra/pkgconfig/expanse.pc.in" ]; then
    sed -e "s|@PREFIX@|/usr|g" \
        -e "s|@LIBDIR@|/usr/lib64|g" \
        -e "s|@INCLUDEDIR@|/usr/include|g" \
        -e "s|@VERSION@|VERSION_PLACEHOLDER|g" \
        REPO_ROOT_PLACEHOLDER/extra/pkgconfig/expanse.pc.in > %{buildroot}/usr/lib64/pkgconfig/expanse.pc
fi
if [ -f "REPO_ROOT_PLACEHOLDER/extra/pkgconfig/judy.pc.in" ]; then
    sed -e "s|@PREFIX@|/usr|g" \
        -e "s|@LIBDIR@|/usr/lib64|g" \
        -e "s|@INCLUDEDIR@|/usr/include|g" \
        -e "s|@VERSION@|VERSION_PLACEHOLDER|g" \
        REPO_ROOT_PLACEHOLDER/extra/pkgconfig/judy.pc.in > %{buildroot}/usr/lib64/pkgconfig/judy.pc
fi

%files
/usr/lib64/libexpanse.so*

%files devel
/usr/include/*
/usr/lib64/libexpanse.a
/usr/lib64/pkgconfig/*.pc

%files -n libjudy-compat
/usr/lib64/libJudy.so.1

SPECEOF

REPO_ROOT="$(pwd)"

sed -i.bak \
    -e "s|VERSION_PLACEHOLDER|${VERSION}|g" \
    -e "s|DIST_DIR_PLACEHOLDER|${DIST_DIR}|g" \
    -e "s|REPO_ROOT_PLACEHOLDER|${REPO_ROOT}|g" \
    "${SPEC_FILE}"
rm -f "${SPEC_FILE}.bak"

if ! command -v rpmbuild &>/dev/null; then
    echo "::error::package_rpm.sh: rpmbuild not found; cannot build RPM packages" >&2
    exit 1
fi

rpmbuild --define "_topdir ${RPM_TOPDIR}" --target "${RPM_ARCH}" -bb "${SPEC_FILE}"
find "${RPM_TOPDIR}/RPMS" -name "*.rpm" -exec cp {} . \;

# Assert at least one RPM landed in the working directory for upload.
rpm_count=$(find . -maxdepth 1 -name "*.rpm" | wc -l | tr -d ' ')
if [ "${rpm_count}" -eq 0 ]; then
    echo "::error::package_rpm.sh: rpmbuild produced no *.rpm artifacts" >&2
    exit 1
fi
echo "RPM packaging completed successfully (${rpm_count} package(s))."
