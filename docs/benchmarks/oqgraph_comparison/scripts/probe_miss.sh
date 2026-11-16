#!/bin/bash
set -euo pipefail
H=/w/harness; P=/w/prefix-expanse
gcc -O1 -o $H/miss_stock $H/miss_index_probe.c -lJudy
gcc -O1 -o $H/miss_expanse -I$P/include $H/miss_index_probe.c -L$P/lib -lexpanse -Wl,-rpath,$P/lib
$H/miss_stock > $H/out/miss_stock.txt; $H/miss_expanse > $H/out/miss_expanse.txt
diff -y -W 140 $H/out/miss_stock.txt $H/out/miss_expanse.txt || true
