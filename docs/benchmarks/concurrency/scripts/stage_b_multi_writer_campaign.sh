#!/usr/bin/env bash
# The #568 PR 5 measurement campaign, reference host only: the P5.1 counter
# cells at the head build, then two two-commit runs of each FFI suite's
# concurrent arm (base and head builds interleaved per round —
# scripts/bench_ab.py, docs/BENCHMARKING.md rule 18), under one host-wide lock
# and one core pin, concurrent sweeps last (AGENTS.md §8.17). Read the
# artifacts against ../METHODOLOGY.md §10 with scripts/pr5_gate.py; nothing
# here decides a verdict.
#
#   EXPANSE_BENCH_COMMIT=<head-sha> \
#   EXPANSE_BENCH_BASE_TREE=<synced tree at the base commit> \
#   EXPANSE_BENCH_BASE_COMMIT=1edfa952 \
#     nohup docs/benchmarks/concurrency/scripts/pr5_campaign.sh > pr5.log 2>&1 &
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
cd "${REPO_ROOT}"

BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=concurrency-pr5 pid=%s start=%s\n' "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"
trap 'rm -rf "${BENCH_LOCK}"' EXIT

# shellcheck source-path=SCRIPTDIR/../../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

# Both trees on the host are rsync'd, not checkouts: every commit is passed in.
: "${EXPANSE_BENCH_COMMIT:?set EXPANSE_BENCH_COMMIT to the head commit this tree was synced from}"
: "${EXPANSE_BENCH_BASE_TREE:?set EXPANSE_BENCH_BASE_TREE to the synced tree at the base commit}"
: "${EXPANSE_BENCH_BASE_COMMIT:=1edfa952}"
export EXPANSE_BENCH_COMMIT
export EXPANSE_BENCH_BASE_COMMIT
export RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=haswell"
# Every line reaches the log as it happens; a runner that dies mid-cell says why.
export PYTHONUNBUFFERED=1

stamp() { echo "== $(date -u +%H:%M:%SZ) $*"; uptime; }
HOT="${REPO_ROOT}/docs/benchmarks/hot_comparison"
MT="${REPO_ROOT}/docs/benchmarks/masstree_comparison"
BASE_CRATE="${EXPANSE_BENCH_BASE_TREE}/crates/expanse-hot-bench/Cargo.toml"
BASE_TARGET="${EXPANSE_BENCH_BASE_TREE}/crates/expanse-hot-bench/target"

stamp "1/5 base-tree harness binaries (${EXPANSE_BENCH_BASE_COMMIT})"
for f in hot/LICENSE hot/third-party/tbb/Makefile masstree/GNUmakefile.in; do
  if [ ! -e "${EXPANSE_BENCH_BASE_TREE}/third_party/${f}" ] && [ -e "${REPO_ROOT}/third_party/${f}" ]; then
    # A synced base tree may lack the submodules: share the head tree's copies.
    d="$(dirname "${EXPANSE_BENCH_BASE_TREE}/third_party/${f}")"
    rm -rf "${EXPANSE_BENCH_BASE_TREE}/third_party/${f%%/*}"
    ln -s "${REPO_ROOT}/third_party/${f%%/*}" "${EXPANSE_BENCH_BASE_TREE}/third_party/${f%%/*}"
    echo "linked third_party/${f%%/*} into the base tree"
    unset d
  fi
done
# One harness, two engines: the base binaries are the head tree's harness
# sources (which know --rounds / --round-offset) built against the base
# tree's engine, so only the engine differs between the two builds.
rsync -a --delete "${REPO_ROOT}/crates/expanse-hot-bench/src/" "${EXPANSE_BENCH_BASE_TREE}/crates/expanse-hot-bench/src/"
# Cargo fingerprints by mtime, and rsync keeps the head files' older ones: touch,
# or the base target quietly keeps the binary it built from the old sources.
find "${EXPANSE_BENCH_BASE_TREE}/crates/expanse-hot-bench/src" -type f -exec touch {} +
CARGO_TARGET_DIR="${BASE_TARGET}" cargo build --release --manifest-path "${BASE_CRATE}" \
  --features rowex --bin hot_concurrent
CARGO_TARGET_DIR="${BASE_TARGET}" cargo build --release --manifest-path "${BASE_CRATE}" \
  --features masstree --bin masstree_concurrent
BASE_HOT="${BASE_TARGET}/release/hot_concurrent"
BASE_MT="${BASE_TARGET}/release/masstree_concurrent"

stamp "2/5 P5.1 multi-writer per-thread counters at the head build, into results/pr5/"
python3 scripts/bench_counters.py --repeats 7 --no-c2c --out-dir "${MT}/results/pr5" \
  --cell masstree_conc_map_w4_r0
python3 scripts/bench_counters.py --repeats 7 --no-c2c --out-dir "${HOT}/results/pr5" \
  --cell hot_conc_map_w4_r0 --cell hot_conc_map_w8_r0

stamp "3/5 masstree two-commit sweep, run 1 then run 2"
mkdir -p "${MT}/results/pr5"
python3 "${MT}/scripts/run_all.py" --ab-base-bin "${BASE_MT}" --ab-base-commit "${EXPANSE_BENCH_BASE_COMMIT}"
mv "${MT}/results/baseline_concurrent_ab.json" "${MT}/results/pr5/baseline_concurrent_ab.json"
python3 "${MT}/scripts/run_all.py" --ab-base-bin "${BASE_MT}" --ab-base-commit "${EXPANSE_BENCH_BASE_COMMIT}"
mv "${MT}/results/baseline_concurrent_ab.json" "${MT}/results/pr5/baseline_concurrent_ab_run2.json"
git checkout -- "${MT}/results/baseline_concurrent_ab.json"

stamp "4/5 HOT two-commit sweep, run 1 then run 2"
mkdir -p "${HOT}/results/pr5"
python3 "${HOT}/scripts/run_all.py" --ab-base-bin "${BASE_HOT}" --ab-base-commit "${EXPANSE_BENCH_BASE_COMMIT}"
mv "${HOT}/results/baseline_concurrent_ab.json" "${HOT}/results/pr5/baseline_concurrent_ab.json"
python3 "${HOT}/scripts/run_all.py" --ab-base-bin "${BASE_HOT}" --ab-base-commit "${EXPANSE_BENCH_BASE_COMMIT}"
mv "${HOT}/results/baseline_concurrent_ab.json" "${HOT}/results/pr5/baseline_concurrent_ab_run2.json"
git checkout -- "${HOT}/results/baseline_concurrent_ab.json"

stamp "5/5 verdicts (read-only render)"
python3 docs/benchmarks/concurrency/scripts/pr5_gate.py
stamp "done"
