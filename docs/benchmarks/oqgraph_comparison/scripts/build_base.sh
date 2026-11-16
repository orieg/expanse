#!/bin/bash
# Runs inside oqeval:24.04 with /w = ~/oqeval. Builds libexpanse (release) and MariaDB variant A.
set -euxo pipefail
J=${JOBS:-16}
# --- libexpanse at 82eb56af ---
cd /w/expanse
CARGO_TARGET_DIR=/w/expanse-target cargo build --release -p expanse-capi -j "$J"
P=/w/prefix-expanse; rm -rf $P; mkdir -p $P/lib $P/include
cp /w/expanse-target/release/libexpanse.so $P/lib/
cp include/Judy.h include/expanse.h include/expanse.hpp $P/include/
ln -s libexpanse.so $P/lib/libJudy.so.1
ln -s libexpanse.so $P/lib/libJudy.so
readelf -d $P/lib/libexpanse.so | grep -E 'SONAME|NEEDED' || true
# --- MariaDB, variant A (distro libjudy) ---
COMMON=(-G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo -DUPDATE_SUBMODULES=OFF -DWITH_WSREP=OFF
  -DWITH_UNIT_TESTS=OFF -DWITH_EMBEDDED_SERVER=OFF -DWITH_MARIABACKUP=OFF
  -DWITH_SSL=system -DWITH_PCRE=system -DWITH_LIBFMT=system
  -DPLUGIN_OQGRAPH=DYNAMIC -DPLUGIN_ROCKSDB=NO -DPLUGIN_COLUMNSTORE=NO -DPLUGIN_MROONGA=NO
  -DPLUGIN_SPIDER=NO -DPLUGIN_CONNECT=NO -DPLUGIN_S3=NO -DPLUGIN_SPHINX=NO -DPLUGIN_DUCKDB=NO)
printf '%s\n' "${COMMON[@]}" > /w/cmake-common-flags.txt
cmake -S /w/mariadb -B /w/build-A "${COMMON[@]}" 2>&1 | tee /w/cmake-A.log | grep -i -E 'judy|oqgraph|boost' || true
nice cmake --build /w/build-A -j "$J"
ldd /w/build-A/storage/oqgraph/ha_oqgraph.so | grep -i judy
dpkg -s libjudy-dev libjudy1 2>/dev/null | grep -E '^(Package|Version)' 
