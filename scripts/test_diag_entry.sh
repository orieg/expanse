#!/bin/bash
set -euo pipefail

# Run every expanse-trie test that exists only with the `diag-entry` feature.
#
# Usage: scripts/test_diag_entry.sh
#
# The `diag-entry` entry points (`insert_serialized`, `remove_serialized`,
# `optimistic_probe`) are compiled out of the default build, the same policy
# as `occ-stats` (AGENTS.md §2.1 invariant 5), and their tests read the
# `occ_stats` counters, so they need both features. `scripts/test_occ_stats.sh`
# builds with `occ-stats` alone and would run this module as zero tests, which
# is why it is a separate script. The CI `test` job and `scripts/gate.sh` both
# call it, so the two run the same set.
#
# Single-threaded: the counters are process-global and the module asserts
# exact deltas. A run that matches zero tests fails (AGENTS.md §8.11.8).

cd "$(dirname "$0")/.."

fail() { echo "test_diag_entry.sh: $*" >&2; exit 1; }

log=$(mktemp)
trap 'rm -f "$log"' EXIT

echo "+ cargo test -p expanse-trie --features occ-stats,diag-entry --lib -- diag_entry_ --test-threads=1"
cargo test -p expanse-trie --features occ-stats,diag-entry --lib -- diag_entry_ --test-threads=1 2>&1 | tee "$log"

grep -Eq 'test result: ok\. [1-9][0-9]* passed' "$log" ||
  fail "the diag_entry_ filter ran zero tests (see the output above)"
echo "test_diag_entry.sh: the diag_entry_ tests ran and passed"
