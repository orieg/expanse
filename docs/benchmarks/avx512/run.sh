#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the AVX-512 Bitmap256 cardinality sweep.
#
# Requires a host with `avx512vpopcntdq`. It refuses to run without it rather
# than reporting a scalar-only sweep as if it were the full one (AGENTS.md
# §8.1): the vector arms are the whole point of this suite.
#
# This suite is wall-clock and gates nothing. It cannot be run under Callgrind —
# Valgrind implements no AVX-512, masks the CPUID bits, and SIGILLs on the EVEX
# prefix. `crates/expanse/examples/avx512_probe.rs` demonstrates both halves.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

# --- Capability preflight: fail closed, and by name -------------------------
MISSING=""
for f in avx512f avx512vl avx512bw avx512dq avx512_vpopcntdq; do
  grep -qw "$f" /proc/cpuinfo 2>/dev/null || MISSING="$MISSING $f"
done
if [ -n "$MISSING" ]; then
  echo "refusing to start: this host lacks:${MISSING}" >&2
  echo "The AVX-512 lane measures nothing without them. It did NOT fall back to" >&2
  echo "a scalar-only report." >&2
  exit 1
fi

# Host-wide benchmark lock (docs/BENCHMARKING.md, methodology rule 8): one
# suite at a time per machine, across every checkout. `mkdir` is atomic; the
# lock names its owner so a refused start says who holds the host.
BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=%s pid=%s start=%s\n' "$(basename "${SCRIPT_DIR}")" "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"
trap 'rm -rf "${BENCH_LOCK}"' EXIT INT TERM

cd "${REPO_ROOT}"

echo "========================================================================"
echo " AVX-512 Bitmap256 cardinality sweep"
echo "========================================================================"
echo "System load at start (docs/BENCHMARKING.md rule 2, non-gating):"
uptime
ps -A -o %cpu= -o %mem= -o comm= | sort -rn | head -n 8 || true
echo ""

# Demonstrate, rather than assert, that Callgrind cannot see this kernel.
echo "--- CPUID visibility, native vs under Callgrind ---"
cargo run --quiet --release --example avx512_probe -p expanse-trie || true
if command -v valgrind >/dev/null 2>&1; then
  valgrind --tool=callgrind --callgrind-out-file=/dev/null \
    ./target/release/examples/avx512_probe 2>&1 | grep -E '^(detect|dispatch|forced)' || true
else
  echo "valgrind absent — CPUID-masking demonstration skipped (not a failure)"
fi
echo ""

# `--features avx512` is required: the vector arms sit behind an off-by-default
# feature so the declared MSRV 1.88 floor survives (core::arch's AVX-512
# intrinsics are stable only since 1.89).
STARTED=$(date +%s)
rm -rf target/criterion
cargo bench -p expanse-trie --bench avx512_bitmap --features avx512 "$@"

HOST_DESC="$(lscpu | sed -n 's/^Model name:[[:space:]]*//p' | head -1) ($(nproc) threads, $(lscpu | sed -n 's/^L3 cache:[[:space:]]*//p' | head -1 | sed 's/ (.*)//') L3, $(uname -sr))"

python3 scripts/bench_baseline.py --harvest \
  --criterion-dir target/criterion \
  --newer-than "${STARTED}" \
  --suite avx512_bitmap \
  --host-desc "${HOST_DESC}" \
  --run-id "${EXPANSE_RUN_ID:-local}" \
  --out results/baseline_avx512_bitmap.json

python3 scripts/generate_avx512_svg.py

echo ""
echo " Results written to:"
echo "   results/baseline_avx512_bitmap.json   (BCa 95% intervals)"
echo "   docs/assets/bench_avx512.svg          (regenerated chart)"
echo ""
echo "System load at end:"
uptime
