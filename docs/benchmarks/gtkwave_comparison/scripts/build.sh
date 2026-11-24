#!/usr/bin/env bash
# Build the three gtkwave arms of the drop-in evaluation inside an
# Ubuntu 24.04 container, with /w as the work directory and the expanse
# source tree at /w/expanse.
#
#   A   stock build: Ubuntu's gtkwave 3.3.116-1build2 source, configured as
#       debian/rules does (--enable-gtk3 --enable-judy), against the distro
#       libjudy-dev 1.0.5-5.1build1 (Judy.h and -lJudy).
#   B   the same source rebuilt against expanse's include/Judy.h, linked
#       to libexpanse through a libJudy.so development link.
#   B1  A's binaries, run with LD_LIBRARY_PATH=/w/compat, where
#       libJudy.so.1 -> libexpanse.so.1 (the libjudy-compat link that
#       .github/workflows/release.yml ships). Nothing to build.
#
# Prerequisites (apt): build-essential libjudy-dev valgrind time python3
# dpkg-dev, the gtkwave Build-Depends (libgtk-3-dev zlib1g-dev libbz2-dev
# flex gperf tcl-dev tk-dev liblzma-dev), deb-src enabled for
# `apt-get source gtkwave`, and a Rust toolchain at or above the MSRV.
set -euo pipefail
W=/w
SRC=$W/gtkwave-3.3.116

cd "$W"
[ -d "$SRC" ] || apt-get source gtkwave

# libexpanse
# EXPANSE_LINK_SONAME=1 sets DT_SONAME libexpanse.so.1, as the release build does.
( cd "$W/expanse" && EXPANSE_LINK_SONAME=1 cargo build --release -p expanse-capi )
mkdir -p "$W/lib" "$W/compat"
cp "$W/expanse/target/release/libexpanse.so" "$W/lib/libexpanse.so.1"
ln -sf libexpanse.so.1 "$W/lib/libexpanse.so"
# development link so gtkwave's configure (AC_CHECK_LIB Judy JudySLIns) and
# its -lJudy resolve to libexpanse; DT_NEEDED records the SONAME.
ln -sf libexpanse.so.1 "$W/lib/libJudy.so"
ln -sf "$W/lib/libexpanse.so.1" "$W/compat/libJudy.so.1"

# Arm A (SKIP_STOCK=1 keeps an existing stock build)
if [ -z "${SKIP_STOCK:-}" ]; then
  rm -rf "$W/build-stock"; cp -a "$SRC" "$W/build-stock"
  ( cd "$W/build-stock" \
    && ./configure --enable-gtk3 --enable-judy --disable-mime-update > "$W/configure-stock.log" 2>&1 \
    && make -j"$(nproc)" > "$W/make-stock.log" 2>&1 )
fi

# Arm B: expanse's Judy.h ahead of /usr/include, libexpanse ahead of libJudy
rm -rf "$W/build-expanse"; cp -a "$SRC" "$W/build-expanse"
( cd "$W/build-expanse" \
  && CPPFLAGS="-I$W/expanse/include" LDFLAGS="-L$W/lib -Wl,-rpath,$W/lib" \
     ./configure --enable-gtk3 --enable-judy --disable-mime-update > "$W/configure-expanse.log" 2>&1 \
  && make -j"$(nproc)" > "$W/make-expanse.log" 2>&1 )

# Evidence that each arm compiled and linked against what it claims. A probe
# that finds nothing prints nothing here rather than aborting the script.
set +e +o pipefail
{
  echo "== header resolved by arm B's include path"
  echo '#include <Judy.h>' | gcc -I"$W/expanse/include" -H -fsyntax-only -x c - 2>&1 | grep -m1 'Judy.h'
  echo "== header resolved by arm A's include path"
  echo '#include <Judy.h>' | gcc -H -fsyntax-only -x c - 2>&1 | grep -m1 'Judy.h'
  for b in src/helpers/vcd2fst src/helpers/fst2vcd src/gtkwave; do
    echo "== A $b"; ldd "$W/build-stock/$b" | grep -Ei 'judy|expanse' | sed 's/ (0x[0-9a-f]*)//'
    echo "== B $b"; ldd "$W/build-expanse/$b" | grep -Ei 'judy|expanse' | sed 's/ (0x[0-9a-f]*)//'
    echo "== B1 $b"; LD_LIBRARY_PATH=$W/compat ldd "$W/build-stock/$b" | grep -Ei 'judy|expanse' | sed 's/ (0x[0-9a-f]*)//'
  done
  echo "== -D_WAVE_HAVE_JUDY in both builds"
  grep -E '^LIBJUDY_(CFLAGS|LDADD)' "$W/build-stock/Makefile" "$W/build-expanse/Makefile"
  echo "== warnings mentioning Judy in each build log"
  echo "A: $(grep -ciE 'warning:.*judy|judy.*warning:' "$W/make-stock.log" || true)  B: $(grep -ciE 'warning:.*judy|judy.*warning:' "$W/make-expanse.log" || true)"
  echo "== total compiler warnings in each build log"
  echo "A: $(grep -c 'warning:' "$W/make-stock.log" || true)  B: $(grep -c 'warning:' "$W/make-expanse.log" || true)"
} > "$W/build_evidence.txt" 2>&1
cat "$W/build_evidence.txt"
