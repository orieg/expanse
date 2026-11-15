#!/bin/bash
# Inside oqeval:24.04. Install server once; stage one plugin dir per variant; run the oqgraph mtr suite per variant.
set -uxo pipefail
P=/w/prefix-expanse
. /w/scripts/prep_runtime.sh
mkdir -p /w/compat-lib && ln -sf $P/lib/libexpanse.so /w/compat-lib/libJudy.so.1
if [ ! -x /w/inst/bin/mariadbd ]; then cmake --install /w/build-A --prefix /w/inst > /w/install.log 2>&1; fi
for V in A B C; do
  mkdir -p /w/plugins/$V; cp /w/inst/lib/plugin/*.so /w/plugins/$V/ 2>/dev/null
  cp /w/build-$V/storage/oqgraph/ha_oqgraph.so /w/plugins/$V/ha_oqgraph.so
done
cd /w/build-A/mysql-test
for V in A B C B1; do
  PD=$V; EXTRA=(); [ $V = B1 ] && PD=A
  # mtr finds ha_oqgraph.so in the build tree; point it at the staged copy instead.
  cp /w/plugins/$PD/ha_oqgraph.so /w/build-A/storage/oqgraph/ha_oqgraph.so
  if [ $V = B1 ]; then export LD_LIBRARY_PATH=/w/compat-lib; else unset LD_LIBRARY_PATH; fi
  perl mariadb-test-run.pl --suite=oqgraph --parallel=4 --force --max-test-fail=0 --vardir=/tmp/var-$V > /w/mtr-$V.log 2>&1
  echo "mtr $V rc=$?"; grep -E "^(Completed|Failed|The servers were restarted)|tests were successful|Failed [0-9]+/" /w/mtr-$V.log | tail -3
  echo "plugin $V resolves: $(LD_LIBRARY_PATH=${LD_LIBRARY_PATH:-} ldd /w/build-A/storage/oqgraph/ha_oqgraph.so | grep -E 'Judy|expanse')"
done
cp /w/plugins/A/ha_oqgraph.so /w/build-A/storage/oqgraph/ha_oqgraph.so
