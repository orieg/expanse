#!/bin/bash
set -euo pipefail

# Builds the Debian and RPM packages from a staged dist/ tree and checks that an
# unmodified `-lJudy` consumer builds against them (docs/COMPAT.md, "Library
# names and SONAME"):
#   1. the runtime library carries DT_SONAME libexpanse.so.1;
#   2. `cc ... -lJudy` against the extracted package tree links with no
#      override, records DT_NEEDED libexpanse.so.1 (versioned), and runs;
#   3. a binary that already records DT_NEEDED libJudy.so.1 (any classic
#      libjudy consumer) still loads through libjudy-compat (AGENTS.md §4.1);
#   4. a static `-lJudy` link resolves through libJudy.a;
#   5. the RPMs ship the same layout, the runtime package holds no unversioned
#      dev link, and the main package provides libexpanse.so.1;
#   6. with --install (CI or a container: it runs `dpkg -i` as root), the same consumer builds
#      with no -I/-L and runs with no LD_LIBRARY_PATH, resolving through
#      ld.so.cache and the default library path.
#
# Usage: scripts/test_packaging.sh <dist_dir> [--install]
#   dist_dir is laid out as release.yml stages it: lib/libexpanse.so.1.0.0 plus
#   the .so.1 and .so links, and lib/libexpanse.a. Run from the repository root.

DIST_DIR="${1:?usage: test_packaging.sh <dist_dir> [--install]}"
INSTALL="${2:-}"
SONAME="libexpanse.so.1"
SMOKE="crates/expanse-capi/smoke/link_judy_smoke.c"

fail() { echo "::error::test_packaging.sh: $*" >&2; exit 1; }

for tool in cc readelf nm dpkg-deb rpmbuild rpm2cpio cpio rpm; do
    command -v "$tool" >/dev/null || fail "$tool not found"
done
[ -f "$SMOKE" ] || fail "run from the repository root ($SMOKE not found)"
DIST_DIR="$(cd "$DIST_DIR" && pwd)"

VERSION="$(grep -m1 '^version' crates/expanse-capi/Cargo.toml | cut -d'"' -f2)"
case "$(uname -m)" in
    x86_64)  DEB_ARCH=amd64; RPM_ARCH=x86_64 ;;
    aarch64) DEB_ARCH=arm64; RPM_ARCH=aarch64 ;;
    *) fail "unsupported host $(uname -m)" ;;
esac
TRIPLE="$(cc -dumpmachine)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

needed() { readelf -d "$1" | sed -n 's/.*(NEEDED).*\[\(.*\)\]$/\1/p'; }
soname() { readelf -d "$1" | sed -n 's/.*(SONAME).*\[\(.*\)\]$/\1/p'; }

# A binary's DT_NEEDED must name libexpanse.so.1 and no unversioned Judy or
# Expanse library.
expect_needed() {
    local bin="$1" want="$2" list
    list="$(needed "$bin")"
    echo "$list" | grep -qx "$want" || fail "$bin: DT_NEEDED lacks $want (has: $(echo $list))"
    if echo "$list" | grep -qx -e 'libJudy.so' -e 'libexpanse.so'; then
        fail "$bin: records an unversioned DT_NEEDED (has: $(echo $list))"
    fi
}

run_ok() {
    local out
    out="$("$@")" || fail "$* exited non-zero: $out"
    [ "$out" = "ok" ] || fail "$* printed '$out', expected 'ok'"
}

# 1. The library itself.
got="$(soname "$DIST_DIR/lib/libexpanse.so.1.0.0")"
[ "$got" = "$SONAME" ] || fail "libexpanse.so.1.0.0 has DT_SONAME '$got', expected '$SONAME' (build with EXPANSE_LINK_SONAME=1)"
echo "DT_SONAME: $got"

# A stand-in for a classic libjudy consumer: link the smoke against a stub
# library whose DT_SONAME is libJudy.so.1, so the binary records exactly what a
# binary built against stock libjudy records. The stub defines the Judy symbols
# the object references and nothing else; it is never loaded.
cc -c -I include "$SMOKE" -o "$WORK/smoke.o"
mkdir -p "$WORK/stub"
nm -u "$WORK/smoke.o" | awk '$2 ~ /^Judy/ { print "void " $2 "(void) {}" }' > "$WORK/stub/stub.c"
[ -s "$WORK/stub/stub.c" ] || fail "the smoke references no Judy symbol"
cc -shared -fPIC -Wl,-soname,libJudy.so.1 -o "$WORK/stub/libJudy.so" "$WORK/stub/stub.c"
cc "$WORK/smoke.o" -L "$WORK/stub" -lJudy -o "$WORK/classic"
expect_needed "$WORK/classic" libJudy.so.1

# check_tree <root> <libdir>: steps 2-4 against an extracted package tree.
check_tree() {
    local root="$1" libdir="$1/$2" label="$3"
    [ -L "$libdir/libJudy.so" ] || fail "$label: no libJudy.so dev link in $2"
    [ -L "$libdir/libJudy.so.1" ] || fail "$label: no libJudy.so.1 compat link in $2"
    cc -I "$root/usr/include" "$SMOKE" -L "$libdir" -lJudy -o "$WORK/linked-$label"
    expect_needed "$WORK/linked-$label" "$SONAME"
    LD_LIBRARY_PATH="$libdir" run_ok "$WORK/linked-$label"
    LD_LIBRARY_PATH="$libdir" run_ok "$WORK/classic"
    cc -I "$root/usr/include" "$SMOKE" -L "$libdir" -Wl,-Bstatic -lJudy -Wl,-Bdynamic \
        -lpthread -ldl -lm -o "$WORK/static-$label"
    if needed "$WORK/static-$label" | grep -q -e Judy -e expanse; then
        fail "$label: static -lJudy link still records a shared Judy/Expanse library"
    fi
    run_ok "$WORK/static-$label"
    echo "$label: -lJudy links DT_NEEDED $SONAME; classic libJudy.so.1 binary loads; static -lJudy links"
}

# 2-4. Debian packages, extracted.
bash scripts/package_deb.sh "$VERSION" "$DEB_ARCH" "$DIST_DIR" >/dev/null
DEBS=()
for pkg in libexpanse1 libexpanse-dev libjudy-compat; do
    f="${pkg}_${VERSION}_${DEB_ARCH}.deb"
    [ -f "$f" ] || fail "package_deb.sh produced no $f"
    mv "$f" "$WORK/"
    DEBS+=("$WORK/$f")
    dpkg-deb -x "$WORK/$f" "$WORK/deb-root"
done
rm -rf debian_build
check_tree "$WORK/deb-root" "usr/lib/$TRIPLE" deb
dpkg-deb -c "$WORK/libexpanse1_${VERSION}_${DEB_ARCH}.deb" | grep -q 'libexpanse\.so ->' \
    && fail "libexpanse1 ships the unversioned libexpanse.so dev link"
dpkg-deb -f "$WORK/libexpanse-dev_${VERSION}_${DEB_ARCH}.deb" Conflicts | grep -qw libjudy-dev \
    || fail "libexpanse-dev ships libJudy.so and Judy.h but does not conflict with libjudy-dev"

# 5. RPM packages, extracted.
bash scripts/package_rpm.sh "$VERSION" "$RPM_ARCH" "$DIST_DIR" >/dev/null
RPM_MAIN="$(ls libexpanse-"${VERSION}"-1*."${RPM_ARCH}".rpm)"
RPM_DEVEL="$(ls libexpanse-devel-"${VERSION}"-1*."${RPM_ARCH}".rpm)"
RPM_COMPAT="$(ls libjudy-compat-"${VERSION}"-1*."${RPM_ARCH}".rpm)"
mkdir -p "$WORK/rpm-root"
for f in "$RPM_MAIN" "$RPM_DEVEL" "$RPM_COMPAT"; do
    (cd "$WORK/rpm-root" && rpm2cpio "$OLDPWD/$f" | cpio -idm --quiet)
done
rpm -qpl "$RPM_MAIN" | grep -qx '/usr/lib64/libexpanse.so' \
    && fail "the runtime RPM ships the unversioned libexpanse.so dev link"
rpm -qp --provides "$RPM_MAIN" | grep -q "^${SONAME}()" \
    || fail "the runtime RPM does not provide ${SONAME} (provides: $(rpm -qp --provides "$RPM_MAIN" | tr '\n' ' '))"
rpm -qp --conflicts "$RPM_DEVEL" | grep -qx 'Judy-devel' \
    || fail "libexpanse-devel ships libJudy.so and Judy.h but does not conflict with Judy-devel"
rm -f "$RPM_MAIN" "$RPM_DEVEL" "$RPM_COMPAT"
rm -rf rpm_build
# rpm2cpio keeps the absolute /usr/lib64; the dynamic linker never searches it
# by default on Debian, so LD_LIBRARY_PATH in check_tree is what resolves it.
check_tree "$WORK/rpm-root" usr/lib64 rpm

# 6. Installed for real: no -I, no -L, no LD_LIBRARY_PATH.
if [ "$INSTALL" = "--install" ]; then
    SUDO=sudo
    [ "$(id -u)" = 0 ] && SUDO=
    $SUDO dpkg -i "${DEBS[@]}" >/dev/null
    $SUDO ldconfig
    cc "$SMOKE" -lJudy -o "$WORK/installed"
    expect_needed "$WORK/installed" "$SONAME"
    run_ok "$WORK/installed"
    run_ok "$WORK/classic"
    resolved="$(ldd "$WORK/classic" | awk '$1 == "libJudy.so.1" { print $3 }')"
    case "$(readlink -f "$resolved")" in
        */libexpanse.so.1.0.0) ;;
        *) fail "installed: libJudy.so.1 resolved to '$resolved', not libexpanse" ;;
    esac
    echo "installed: -lJudy builds with no override, DT_NEEDED $SONAME; libJudy.so.1 resolves to $resolved"
    $SUDO dpkg -r libjudy-compat libexpanse-dev libexpanse1 >/dev/null
fi

echo "test_packaging.sh: ok"
