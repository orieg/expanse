#!/usr/bin/env bash
# docs/benchmarks/hot_comparison/scripts/leaf_cap_cascaded_wallclock.sh
#
# The wall-clock half of the LEAF_CAP 32-vs-48 pair in the cascaded regime
# (#715, METHODOLOGY.md §9.10.5): `crates/expanse/benches/leaf_cap_cascaded_wallclock.rs`
# (`contains` / `get`, hit and miss, 1M keys at 63 bits, λ = 30.52) built twice —
# at the shipped `LEAF_CAP` and at a `LEAF_CAP = 48` build-time patch of
# `crates/expanse/src/types.rs` — and run interleaved on the bare-metal
# reference host. Run this on that host only; it takes the host-wide bench lock
# and the P-core pin (docs/BENCHMARKING.md rules 2 and 8, AGENTS.md §8.4), and
# a figure it produces anywhere else is not publishable.
#
# Design (issue #715, scope item 3):
#
#   run 1  cap 32   -> results/leaf_cap_cascaded_contains_cap32_a.json
#   run 2  cap 32   -> results/leaf_cap_cascaded_contains_cap32_repeat.json
#   run 3  cap 48   -> results/leaf_cap_cascaded_contains_cap48_a.json
#   run 4  cap 32   -> results/leaf_cap_cascaded_contains_cap32_b.json
#   run 5  cap 48   -> results/leaf_cap_cascaded_contains_cap48_b.json
#
# Runs 1 and 2 are the same binary back to back: the same-build A/A repeat.
# The #712 pair at λ = 15.26 read a 0.944× [0.936, 0.952] drift of one cap-32
# binary between its two rounds — larger than any between-build difference it
# found — so here the drift of one build is bounded first, and a cap-48 /
# cap-32 ratio is read only against that floor. Runs 3-5 are the interleaved
# A/B/A/B rounds. Both binaries are built before any run starts, so no rebuild
# sits between arms (the #712 rounds recorded the rebuild as their load).
#
# Each run snapshots /proc/loadavg first; each is harvested into its own
# `results/baseline_*`-shaped artifact by scripts/bench_baseline.py with BCa
# 95% intervals, and the script ends by rendering the same-build and
# between-build comparisons with `--against`. What it prints is the table for
# METHODOLOGY.md §9.10.5; the JSON is what the table resolves to.
#
# Preconditions, all checked before the first build:
#   * `EXPANSE_HOST_DESC` set to the anonymised hardware description recorded
#     in every artifact (AGENTS.md §7: CPU model, cores, cache, OS — never a
#     hostname). Refused when unset.
#   * a clean `crates/expanse/src/types.rs` (the patch is applied and reverted
#     here; a dirty file would be silently measured as "cap 32").
#   * `cargo`, `python3`, `taskset` on PATH; the bench lock free.
#
# Usage, from the repository root on the reference host:
#
#     EXPANSE_HOST_DESC='<cpu model>, <cores>, <cache>, <kernel>' \
#       docs/benchmarks/hot_comparison/scripts/leaf_cap_cascaded_wallclock.sh
#
# Environment:
#   EXPANSE_BENCH_LOCK   lock directory (default ${TMPDIR:-/tmp}/expanse-bench.lock)
#   EXPANSE_BENCH_PIN    see scripts/bench_pin.sh (auto on a hybrid host)
#   LEAF_CAP_WORK        scratch directory for the two binaries and the five
#                        criterion trees (default: a mktemp dir, removed on exit)
#   LEAF_CAP_KEEP=1      keep the scratch directory
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SUITE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${SUITE_DIR}/../../.." && pwd)"
RESULTS="${SUITE_DIR}/results"
TYPES_RS="${REPO_ROOT}/crates/expanse/src/types.rs"
BENCH=leaf_cap_cascaded_wallclock
SUITE_LABEL="hot_comparison/leaf_cap_cascaded_contains"

cd "${REPO_ROOT}"

# ---- preconditions ---------------------------------------------------------

if [ -z "${EXPANSE_HOST_DESC:-}" ]; then
  echo "refusing to start: EXPANSE_HOST_DESC is unset. Every artifact records an" >&2
  echo "anonymised hardware description (AGENTS.md §7); set it, e.g." >&2
  echo "  EXPANSE_HOST_DESC='<CPU model>, <P/E cores>, <threads>, <L3>, <kernel>'" >&2
  exit 2
fi
for tool in cargo python3 taskset git; do
  command -v "${tool}" >/dev/null 2>&1 || { echo "refusing to start: ${tool} not on PATH" >&2; exit 2; }
done
if ! git diff --quiet -- "${TYPES_RS}"; then
  echo "refusing to start: ${TYPES_RS} has uncommitted changes; the cap-32 arm" >&2
  echo "must be the committed constant, and the cap-48 patch is applied here." >&2
  exit 2
fi
if ! grep -q '^pub const LEAF_CAP: usize = 32;' "${TYPES_RS}"; then
  echo "refusing to start: the shipped LEAF_CAP is not 32 in ${TYPES_RS}; this" >&2
  echo "script's file names and the §9.10.5 table assume a 32 vs 48 pair." >&2
  exit 2
fi
COMMIT="$(git rev-parse --short=8 HEAD)"

# Host-wide benchmark lock (docs/BENCHMARKING.md rule 8): `mkdir` is atomic
# and the lock names its owner so a refused start says who holds the host.
BENCH_LOCK="${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}"
if ! mkdir "${BENCH_LOCK}" 2>/dev/null; then
  echo "refusing to start: benchmark lock ${BENCH_LOCK} is held by:" >&2
  { cat "${BENCH_LOCK}/owner" 2>/dev/null || true; } >&2
  exit 75
fi
printf 'suite=%s pid=%s start=%s\n' "${SUITE_LABEL}" "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${BENCH_LOCK}/owner"

WORK="${LEAF_CAP_WORK:-$(mktemp -d "${TMPDIR:-/tmp}/leaf-cap-cascaded.XXXXXX")}"
mkdir -p "${WORK}"

cleanup() {
  # The patch is reverted on every exit path, including a failed build.
  git checkout -- "${TYPES_RS}" 2>/dev/null || true
  rm -rf "${BENCH_LOCK}"
  if [ "${LEAF_CAP_KEEP:-0}" != "1" ]; then rm -rf "${WORK}"; fi
}
trap cleanup EXIT

# Core pin (docs/BENCHMARKING.md rule 2, #639): confine this shell — and so
# every benchmark process it spawns — to the host's performance cores.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

# ---- build both binaries before any run ------------------------------------

bench_binary() {
  # Path of the compiled bench executable, from cargo's JSON message stream.
  cargo bench -p expanse-trie --bench "${BENCH}" --no-run --message-format=json 2>/dev/null \
    | python3 -c '
import json, sys
path = None
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["name"] == sys.argv[1]:
        path = m["executable"]
if not path:
    sys.exit("no executable for " + sys.argv[1])
print(path)
' "${BENCH}"
}

echo "== build: cap 32 (committed) at ${COMMIT}"
cp "$(bench_binary)" "${WORK}/bench_cap32"

echo "== build: cap 48 (build-time patch of crates/expanse/src/types.rs)"
sed -i.bak 's/^pub const LEAF_CAP: usize = 32;/pub const LEAF_CAP: usize = 48;/' "${TYPES_RS}"
rm -f "${TYPES_RS}.bak"
grep -q '^pub const LEAF_CAP: usize = 48;' "${TYPES_RS}" || { echo "patch did not apply" >&2; exit 1; }
cp "$(bench_binary)" "${WORK}/bench_cap48"
git checkout -- "${TYPES_RS}"
git diff --quiet -- "${TYPES_RS}" || { echo "patch revert failed; ${TYPES_RS} is dirty" >&2; exit 1; }
cmp -s "${WORK}/bench_cap32" "${WORK}/bench_cap48" && { echo "the two binaries are identical; the patch was not compiled in" >&2; exit 1; }

# ---- the five runs ---------------------------------------------------------

run_arm() {
  # $1 cap (32|48), $2 round label (a|repeat|b), $3 run number
  local cap="$1" round="$2" n="$3"
  local home="${WORK}/criterion_${cap}_${round}"
  local out="${RESULTS}/leaf_cap_cascaded_contains_cap${cap}_${round}.json"
  local load
  load="$(cat /proc/loadavg)"
  echo "== run ${n}: cap ${cap}, round ${round}; loadavg before: ${load}"
  local started
  started="$(date +%s)"
  CRITERION_HOME="${home}" "${WORK}/bench_cap${cap}" --bench
  local patch_note=""
  if [ "${cap}" = "48" ]; then patch_note="; LEAF_CAP=48 build-time patch of crates/expanse/src/types.rs, reverted"; fi
  python3 "${REPO_ROOT}/scripts/bench_baseline.py" --harvest \
    --criterion-dir "${home}" --newer-than "${started}" \
    --suite "${SUITE_LABEL}" \
    --host-desc "${EXPANSE_HOST_DESC}; benchmark shell pinned by scripts/bench_pin.sh (EXPANSE_BENCH_PIN_APPLIED=${EXPANSE_BENCH_PIN_APPLIED:-unset})" \
    --commit "${COMMIT}" \
    --run-id "local: leaf_cap_cascaded_wallclock.sh run ${n} of 5 (cap ${cap}, round ${round}); loadavg before the run ${load}${patch_note}" \
    --out "${out}"
}

run_arm 32 a      1
run_arm 32 repeat 2
run_arm 48 a      3
run_arm 32 b      4
run_arm 48 b      5

# ---- the comparisons -------------------------------------------------------
#
# `--against` renders the ratio of --input over --against with its BCa 95%
# interval; `--floor-speedup 1.0` makes the gate a plain "is the interval above
# 1.0" read-out. Nothing here fails the script: the verdict is the table.

compare() {
  echo; echo "== ${3}"
  python3 "${REPO_ROOT}/scripts/bench_baseline.py" \
    --input "${RESULTS}/leaf_cap_cascaded_contains_${1}.json" \
    --against "${RESULTS}/leaf_cap_cascaded_contains_${2}.json" \
    --floor-speedup 1.0 --format markdown || true
}

compare cap32_repeat cap32_a "same build, back to back: cap 32 run 2 over run 1 (the drift floor)"
compare cap32_b      cap32_a "same build, across a cap-48 run: cap 32 run 4 over run 1"
compare cap48_a      cap32_a "between builds, round a: cap 48 over cap 32"
compare cap48_b      cap32_b "between builds, round b: cap 48 over cap 32"

echo
echo "Artifacts: ${RESULTS}/leaf_cap_cascaded_contains_cap{32,48}_{a,repeat,b}.json"
echo "Publish per METHODOLOGY.md §9.10.5 with the same-build rows beside the between-build rows."
