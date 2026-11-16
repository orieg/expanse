#!/bin/bash
# Inside oqeval:24.04. Builds the bitset differential harness per variant and diffs traces.
set -euo pipefail
H=/w/harness; P=/w/prefix-expanse; O=/w/mariadb/storage/oqgraph; mkdir -p $H/out
CXX="g++ -O2 -g -std=c++17 -I$O"
$CXX -o $H/hA $H/bitset_diff.cc $O/oqgraph_judy.cc -lJudy
$CXX -o $H/hB -I$P/include $H/bitset_diff.cc $O/oqgraph_judy.cc -L$P/lib -lexpanse -Wl,-rpath,$P/lib
$CXX -o $H/hC -I$P/include $H/bitset_diff.cc /w/native/oqgraph_judy.cc -L$P/lib -lexpanse -Wl,-rpath,$P/lib
echo "hA NEEDED: $(readelf -d $H/hA | grep -o 'libJudy[^]]*')"
fail=0
for seed in 1 2 3 0x9e3779b97f4a7c15 12345678901 7 99 2026; do
  $H/hA $seed 300000 > $H/out/A.$seed
  $H/hB $seed 300000 > $H/out/B.$seed
  LD_LIBRARY_PATH=$P/lib $H/hA $seed 300000 > $H/out/B1.$seed 2> $H/out/B1.$seed.err || echo "seed=$seed B1 exit=$? stderr: $(head -c 200 $H/out/B1.$seed.err)"
  $H/hC $seed 300000 > $H/out/C.$seed
  for v in B B1 C; do
    if cmp -s $H/out/A.$seed $H/out/$v.$seed; then r=same; else r=DIFF; fail=1; fi
    echo "seed=$seed $v vs A: $r ($(sha256sum < $H/out/$v.$seed | cut -c1-12) vs $(sha256sum < $H/out/A.$seed | cut -c1-12))"
  done
done
# Which Judy library did the B1 run resolve?
LD_LIBRARY_PATH=$P/lib LD_DEBUG=libs $H/hA 1 1 2>&1 >/dev/null | grep -m2 -E 'libJudy|calling init: .*(expanse|Judy)' || true
echo "harness fail=$fail"
gcc -O1 -o $H/probe_stock $H/count_errno_probe.c -lJudy
gcc -O1 -o $H/probe_expanse -I$P/include $H/count_errno_probe.c -L$P/lib -lexpanse -Wl,-rpath,$P/lib
echo "--- stock libJudy"; $H/probe_stock
echo "--- libexpanse (Judy.h drop-in)"; $H/probe_expanse
