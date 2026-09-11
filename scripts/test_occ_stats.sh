#!/bin/bash
set -euo pipefail

# Run every expanse-trie test that exists only with the `occ-stats` feature.
#
# Usage: scripts/test_occ_stats.sh
#
# The `occ_stats` counters are compiled out of the default build (AGENTS.md
# §2.1 invariant 5), so a test that reads them is absent from, or empty in,
# `cargo test --workspace`. The CI `test` job and `scripts/gate.sh` both call
# this script, so the two run the same set.
#
# **Integration targets are discovered, not listed.** A target joins when its
# crate-level `#![cfg(...)]` names the feature. A hand-kept list ran one target
# while two more sat behind the feature and ran nowhere.
#
# **A binary that runs zero tests fails the script.** A target whose other cfg
# terms exclude the host would otherwise pass as an empty run, which is how the
# two unlisted targets looked from the outside.
#
# The lib pass is single-threaded: the counters are process-global, and lib
# tests reset them or assert exact deltas while other tests move them.

cd "$(dirname "$0")/.."

fail() { echo "test_occ_stats.sh: $*" >&2; exit 1; }

targets=()
for f in crates/expanse/tests/*.rs; do
  if grep -qE '^#!\[cfg\(.*feature = "occ-stats"' "$f"; then
    targets+=(--test "$(basename "$f" .rs)")
  fi
done
[ "${#targets[@]}" -gt 0 ] ||
  fail "no integration target is gated on occ-stats; the discovery pattern no longer matches"

log=$(mktemp)
trap 'rm -f "$log"' EXIT

run() {
  echo "+ $*"
  "$@" 2>&1 | tee -a "$log"
}

run cargo test -p expanse-trie --features occ-stats "${targets[@]}"
run cargo test -p expanse-trie --features occ-stats --lib -- --test-threads=1

if grep -q "running 0 tests" "$log"; then
  fail "a test binary ran 0 tests with occ-stats on (see its output above)"
fi
integration=$(( ${#targets[@]} / 2 ))
expected=$(( integration + 1 ))
seen=$(grep -cE '^running [0-9]+ tests?' "$log" || true)
[ "$seen" -eq "$expected" ] ||
  fail "expected $expected test binaries ($integration integration + lib), saw $seen"
echo "test_occ_stats.sh: $expected binaries, each ran at least one test"
