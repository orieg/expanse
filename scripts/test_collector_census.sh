#!/bin/bash
set -euo pipefail

# Run every expanse-trie test that exists only with the `collector-census`
# feature (#1310).
#
# Usage: scripts/test_collector_census.sh
#
# The collector's cumulative counters are compiled out of the default build
# (AGENTS.md §2.1 invariant 5), so a test that reads them is absent from, or
# empty in, `cargo test --workspace`. The CI `test` job and `scripts/gate.sh`
# both call this script, so the two run the same set.
#
# **Integration targets are discovered, not listed**, as in
# `scripts/test_occ_stats.sh`: a target joins when a crate-level `#![cfg(...)]`
# names the feature.
#
# **A binary that runs zero tests fails the script.** The lib pass selects the
# `collector_census` unit tests by name; a rename that left the filter
# matching nothing would otherwise pass as an empty run (AGENTS.md §8.11.8).
# The lib pass runs the unconditional census tests too, so it also checks
# them with the feature's fields compiled in.

cd "$(dirname "$0")/.."

fail() { echo "test_collector_census.sh: $*" >&2; exit 1; }

targets=()
for f in crates/expanse/tests/*.rs; do
  if grep -qE '^#!\[cfg\(.*feature = "collector-census"' "$f"; then
    targets+=(--test "$(basename "$f" .rs)")
  fi
done
[ "${#targets[@]}" -gt 0 ] ||
  fail "no integration target is gated on collector-census; the discovery pattern no longer matches"

log=$(mktemp)
trap 'rm -f "$log"' EXIT

run() {
  echo "+ $*"
  "$@" 2>&1 | tee -a "$log"
}

run cargo test -p expanse-trie --features collector-census "${targets[@]}"
run cargo test -p expanse-trie --features collector-census --lib -- collector_census

if grep -q "running 0 tests" "$log"; then
  fail "a test binary ran 0 tests with collector-census on (see its output above)"
fi
if grep -qE 'test result: ok\. 0 passed' "$log"; then
  fail "a test binary passed with 0 tests run (see its output above)"
fi
integration=$(( ${#targets[@]} / 2 ))
expected=$(( integration + 1 ))
seen=$(grep -cE '^running [0-9]+ tests?' "$log" || true)
[ "$seen" -eq "$expected" ] ||
  fail "expected $expected test binaries ($integration integration + lib), saw $seen"
echo "test_collector_census.sh: $expected binaries, each ran at least one test"
