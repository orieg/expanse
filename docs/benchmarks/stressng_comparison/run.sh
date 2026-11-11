#!/usr/bin/env bash
# ==============================================================================
# One independent run of the stress-ng x Judy comparison (METHODOLOGY.md).
#
#   docs/benchmarks/stressng_comparison/run.sh <run_number> <host_label> [lib_ab|counters|phase|phase_delete]
#
# With `lib_ab`, the run is the old-vs-new library diagnostic of README.md
# (scripts/diag_lib_ab.py) instead of the pre-registered comparison; it needs
# $SNG_D/expcompat-82eb56af/lib as well, and writes diag_lib_ab_* raw rows.
# With `counters`, it is the front-end counter diagnostic on the same three
# arms (scripts/diag_find_counters.py); it needs `perf` and writes
# diag_find_counters_* raw rows. With `phase`, it is the phase-counter
# diagnostic (scripts/diag_phase_counters.py), which compiles
# scripts/phase_probe.c and needs `perf`, `cc` and the N64 library of the
# layout diagnostic; it writes diag_phase_counters_* raw rows.
# With `phase_delete`, it is the same driver on the #1679 delete phase
# (SNG_PHASE_SET=delete), against #1708's library; it writes
# diag_delete_phase_* raw rows.
#
# <host_label> is `ref` (reference host) or `zen5` (second host); it selects the
# core pin of METHODOLOGY.md §3. SNG_D names the host's stress-ng tree, built by
# scripts/build.sh: b-stockbuild/stress-ng, stock/usr/lib/x86_64-linux-gnu and
# expcompat/lib. Raw rows and provenance are written under $SNG_D/results.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
RUN="${1:?run number}"
HOST_LABEL="${2:?host label: ref or zen5}"
MODE="${3:-comparison}"
SNG_D="${SNG_D:-/var/tmp/stressng-eval}"

# Host-wide benchmark lock (docs/BENCHMARKING.md rule 8), held for the whole run.
if [ -z "${EXPANSE_BENCH_LOCK_HELD:-}" ]; then
  exec python3 "${REPO_ROOT}/scripts/bench_lock.py" --suite "$(basename "${SCRIPT_DIR}")" --wait 7200 -- \
    bash "${SCRIPT_DIR}/$(basename "${BASH_SOURCE[0]}")" "$@"
fi

case "${HOST_LABEL}" in
  ref)  export EXPANSE_BENCH_PIN=0,2,4,6,8,10,12,14 ;;
  zen5) export EXPANSE_BENCH_PIN=0-7 ;;
  *) echo "REFUSE: unknown host label ${HOST_LABEL}" >&2; exit 2 ;;
esac

# Refusals before anything is timed (METHODOLOGY.md §7).
if [ -n "$(docker ps -q 2>/dev/null)" ]; then
  echo "REFUSE: containers running:" >&2; docker ps --format '{{.Names}}' >&2; exit 3
fi
if pgrep -fa '[z]map|[m]ysqld|[m]ariadbd|[k]vbench|[a]ct_runner|[s]moke_[0-9]' >&2; then
  echo "REFUSE: sibling or foreign workload running" >&2; exit 3
fi
avail=$(df -BG --output=avail "${SNG_D}" | tail -1 | tr -dc 0-9)
[ "${avail}" -ge 20 ] || { echo "REFUSE: ${avail} GB free under ${SNG_D}" >&2; exit 3; }
l1=$(cut -d' ' -f1 /proc/loadavg)
for _ in $(seq 1 60); do
  awk -v l="${l1}" 'BEGIN{exit !(l<=1.5)}' && break
  sleep 15; l1=$(cut -d' ' -f1 /proc/loadavg)
done
awk -v l="${l1}" 'BEGIN{exit !(l<=1.5)}' || { echo "REFUSE: load1 ${l1} after 15 min" >&2; exit 3; }

# The pin helper checks the workspace toolchain first (#728).
export PATH="${HOME}/.cargo/bin:${PATH}"
# Core pin (docs/BENCHMARKING.md rule 2): every stress-ng process inherits it.
# shellcheck source-path=SCRIPTDIR/../../..
. "${REPO_ROOT}/scripts/bench_pin.sh"

mkdir -p "${SNG_D}/results"
export SNG_D REPO_ROOT
case "${MODE}" in
  comparison) python3 "${SCRIPT_DIR}/scripts/driver.py" "${RUN}" "${HOST_LABEL}" \
                "${SNG_D}/results/${HOST_LABEL}_run${RUN}" ;;
  lib_ab)     python3 "${SCRIPT_DIR}/scripts/diag_lib_ab.py" "${RUN}" "${HOST_LABEL}" \
                "${SNG_D}/results/diag_lib_ab_${HOST_LABEL}_run${RUN}" ;;
  counters)   python3 "${SCRIPT_DIR}/scripts/diag_find_counters.py" "${RUN}" "${HOST_LABEL}" \
                "${SNG_D}/results/diag_find_counters_${HOST_LABEL}_run${RUN}" ;;
  phase)      python3 "${SCRIPT_DIR}/scripts/diag_phase_counters.py" "${RUN}" "${HOST_LABEL}" \
                "${SNG_D}/results/diag_phase_counters_${HOST_LABEL}_run${RUN}" ;;
  phase_delete) SNG_PHASE_SET=delete python3 "${SCRIPT_DIR}/scripts/diag_phase_counters.py" "${RUN}" "${HOST_LABEL}" \
                "${SNG_D}/results/diag_delete_phase_${HOST_LABEL}_run${RUN}" ;;
  *) echo "REFUSE: unknown mode ${MODE}" >&2; exit 2 ;;
esac
