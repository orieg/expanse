#!/bin/bash
# Inside oqeval:24.04. $1 = B (Judy.h drop-in) or C (native patch).
# Stock libjudy is removed and libexpanse is installed into /usr/local the way
# libexpanse-dev + libjudy-compat would install it; no CMake flags are added.
set -euxo pipefail
V=$1; J=${JOBS:-16}; P=/w/prefix-expanse
apt-get remove -y -qq libjudy-dev libjudydebian1 >/dev/null
test ! -e /usr/include/Judy.h
cp $P/include/Judy.h $P/include/expanse.h $P/include/expanse.hpp /usr/local/include/
cp $P/lib/libexpanse.so /usr/local/lib/
ln -sf libexpanse.so /usr/local/lib/libJudy.so; ln -sf libexpanse.so /usr/local/lib/libJudy.so.1; ldconfig
SRC=/w/mariadb
if [ "$V" = C ]; then
  rm -rf /w/mariadb-C; cp -a /w/mariadb /w/mariadb-C; SRC=/w/mariadb-C
  (cd $SRC && patch -p1 < /w/native/oqgraph-expanse-native.patch)
fi
mapfile -t COMMON < /w/cmake-common-flags.txt
rm -rf /w/build-$V
cmake -S $SRC -B /w/build-$V "${COMMON[@]}" > /w/cmake-$V.log 2>&1
grep -i -E 'judy|expanse|oqgraph' /w/cmake-$V.log || true
grep -E '^(Judy_|Expanse_)' /w/build-$V/CMakeCache.txt || true
nice cmake --build /w/build-$V -j "$J" --target oqgraph
ldd /w/build-$V/storage/oqgraph/ha_oqgraph.so | grep -i -E 'judy|expanse'
readelf -d /w/build-$V/storage/oqgraph/ha_oqgraph.so | grep NEEDED
