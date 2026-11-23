#!/usr/bin/env bash
# Run the gtkwave drop-in evaluation after build.sh, inside the same
# container. Writes everything under $OUT (default /w/out).
#
#   1. test suite: gtkwave 3.3.116 ships no `make check` tests; the target
#      is run anyway in both builds and its output kept.
#   2. correctness: the project's own conversion tools on the project's
#      own example dumps and on two synthetic VCDs (gen_vcd.py), every
#      output compared byte for byte across arms A, B and B1.
#   3. gtkwave itself, headless under Xvfb: load each FST, list every
#      facility through the Tcl API (symbol table: JudySL), and quit.
#   4. Callgrind instruction counts and peak RSS of vcd2fst and fst2vcd on
#      the synthetic VCDs, each arm, one process per measurement.
set -euo pipefail
W=/w
OUT=${OUT:-$W/out}
HERE=$(cd "$(dirname "$0")" && pwd)
A=$W/build-stock
B=$W/build-expanse
mkdir -p "$OUT"/{tests,corr,cg,rss,gui}
loadsnap() { echo "$1 $(date -u +%FT%TZ) loadavg=$(cut -d' ' -f1-3 /proc/loadavg) nproc=$(nproc)" >> "$OUT/load.txt"; }
loadsnap start
trap 'loadsnap end' EXIT

# arm -> (binary root, environment prefix)
run() { # run ARM TOOL ARGS...
  local arm=$1 tool=$2; shift 2
  case $arm in
    A)  "$A/src/$tool" "$@" ;;
    B)  "$B/src/$tool" "$@" ;;
    B1) LD_LIBRARY_PATH=$W/compat "$A/src/$tool" "$@" ;;
  esac
}
mapped() { # mapped ARM TOOL: the Judy provider ld.so resolves for that arm
  local arm=$1 tool=$2 bin=$A
  [ "$arm" = B ] && bin=$B
  if [ "$arm" = B1 ]; then LD_LIBRARY_PATH=$W/compat ldd "$bin/src/$tool"; else ldd "$bin/src/$tool"; fi \
    | grep -Ei 'judy|expanse' | awk '{print $1, "=>", $3}' | xargs
}

# ---- 1. the project's test suite
for arm in A B; do
  d=$A; [ $arm = B ] && d=$B
  rc=0; ( cd "$d" && make check ) > "$OUT/tests/make_check_$arm.log" 2>&1 || rc=$?
  echo "make check $arm rc=$rc" >> "$OUT/tests/summary.txt"
  find "$d" -name '*.trs' -o -name 'test-suite.log' | sed "s|$d/||" > "$OUT/tests/trs_files_$arm.txt"
done

# ---- 2. inputs
mkdir -p "$W/in"
cp "$A/examples/des.fst" "$A/examples/transaction.fst" "$W/in/"
python3 "$HERE/gen_vcd.py" 64 64 2000 0x9E3779B97F4A7C15 > "$W/in/syn_small.vcd"
python3 "$HERE/gen_vcd.py" 512 64 500 0xD1B54A32D192ED03 > "$W/in/syn_large.vcd"
sha256sum "$W"/in/* | sed "s|$W/in/||" > "$OUT/corr/inputs.sha256"

# ---- 2. correctness: conversions per arm
for arm in A B B1; do
  o=$OUT/corr/$arm; mkdir -p "$o"
  for ex in des transaction; do
    run $arm helpers/fst2vcd -f "$W/in/$ex.fst" -o "$o/$ex.vcd"
    run $arm helpers/vcd2fst -v "$o/$ex.vcd" -f "$o/$ex.rt.fst"
    run $arm helpers/fst2vcd -f "$o/$ex.rt.fst" -o "$o/$ex.rt.vcd"
  done
  for s in syn_small syn_large; do
    run $arm helpers/vcd2fst -v "$W/in/$s.vcd" -f "$o/$s.fst"
    run $arm helpers/fst2vcd -f "$o/$s.fst" -o "$o/$s.rt.vcd"
    # vcd2fst's compressed-hierarchy mode, a second writer path
    run $arm helpers/vcd2fst -c -v "$W/in/$s.vcd" -f "$o/$s.c.fst"
    run $arm helpers/fst2vcd -f "$o/$s.c.fst" -o "$o/$s.c.rt.vcd"
  done
  echo "$arm vcd2fst: $(mapped $arm helpers/vcd2fst)" >> "$OUT/corr/mapped.txt"
done
# Every output's sha256 per arm, and whether the three arms agree.
( cd "$OUT/corr" && for f in $(ls A); do
    a=$(sha256sum A/$f | cut -c1-64); b=$(sha256sum B/$f | cut -c1-64); c=$(sha256sum B1/$f | cut -c1-64)
    v=identical; [ "$a" = "$b" ] && [ "$a" = "$c" ] || v=DIFFER
    echo "$f $v A=$a B=$b B1=$c"
  done ) > "$OUT/corr/compare.txt"
# Do two stock conversions of the same input agree with each other?
# (FST headers carry a creation date; a stock-vs-stock difference would mean
# a cross-arm byte difference is not evidence by itself.)
run A helpers/vcd2fst -v "$W/in/syn_small.vcd" -f "$OUT/corr/stock_again.fst"
cmp -s "$OUT/corr/stock_again.fst" "$OUT/corr/A/syn_small.fst" \
  && echo "stock vs stock FST: identical" > "$OUT/corr/stock_vs_stock.txt" \
  || echo "stock vs stock FST: DIFFER" > "$OUT/corr/stock_vs_stock.txt"
rm -f "$OUT/corr/stock_again.fst"

# ---- 3. gtkwave headless: list every facility through the Tcl API
cat > "$W/in/facs.tcl" <<'EOF'
set n [gtkwave::getNumFacs]
set fh [open $::env(FACS_OUT) w]
puts $fh "nfacs $n"
for {set i 0} {$i < $n} {incr i} { puts $fh [gtkwave::getFacName $i] }
foreach s {top.cluster0.core0.unit0.sig0 top.cluster7.core7.unit511.sig63 top.no.such} {
  puts $fh "find $s [gtkwave::addSignalsFromList [list $s]]"
}
close $fh
gtkwave::/File/Quit
EOF
for arm in A B B1; do
  for f in des.fst transaction.fst syn_large.fst; do
    src=$W/in/$f; [ -f "$src" ] || src=$OUT/corr/A/$f
    rc=0; timeout 600 xvfb-run -a \
      env FACS_OUT=$OUT/gui/${f%.fst}_$arm.txt $( [ $arm = B1 ] && echo LD_LIBRARY_PATH=$W/compat ) \
      "$( [ $arm = B ] && echo $B || echo $A )/src/gtkwave" -S "$W/in/facs.tcl" "$src" \
      > "$OUT/gui/${f%.fst}_$arm.log" 2>&1 || rc=$?
    echo "$f $arm rc=$rc" >> "$OUT/gui/rc.txt"
  done
done
( cd "$OUT/gui" && for f in des transaction syn_large; do
    a=$(sha256sum ${f}_A.txt | cut -c1-64); b=$(sha256sum ${f}_B.txt | cut -c1-64); c=$(sha256sum ${f}_B1.txt | cut -c1-64)
    v=identical; [ "$a" = "$b" ] && [ "$a" = "$c" ] || v=DIFFER
    echo "$f $v lines=$(wc -l < ${f}_A.txt) A=$a B=$b B1=$c"
  done ) > "$OUT/gui/compare.txt"

# ---- 4. Callgrind and peak RSS
loadsnap before-callgrind
cg() { # cg ARM NAME TOOL ARGS...
  local arm=$1 name=$2 tool=$3; shift 3
  local bin=$A; [ $arm = B ] && bin=$B
  local env=(); [ $arm = B1 ] && env=(LD_LIBRARY_PATH=$W/compat)
  env "${env[@]}" valgrind --tool=callgrind --callgrind-out-file="$OUT/cg/$name.$arm.out" \
      "$bin/src/$tool" "$@" > /dev/null 2> "$OUT/cg/$name.$arm.log"
  env "${env[@]}" /usr/bin/time -f '%M' -o "$OUT/rss/$name.$arm.maxrss_kb" "$bin/src/$tool" "$@" > /dev/null
}
for s in syn_small syn_large; do
  for arm in A B B1; do
    cg $arm vcd2fst_$s helpers/vcd2fst -v "$W/in/$s.vcd" -f "$W/tmp_$arm.fst"
    cg $arm fst2vcd_$s helpers/fst2vcd -f "$OUT/corr/A/$s.fst" -o "$W/tmp_$arm.vcd"
  done
done
rm -f "$W"/tmp_*.fst "$W"/tmp_*.vcd
python3 "$HERE/summarize.py" "$OUT" > "$OUT/summary.txt"
cat "$OUT/summary.txt"
