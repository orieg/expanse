#!/usr/bin/env bash
# ==============================================================================
# The #1680 trim census (README.md, "Attribution: where main's heap trims come
# from"): sng_judy_4m at W 1, 4, 8, for each arm given, two repeats each, every
# process under scripts/trim_census.c and, with `strace` as the last argument,
# under `strace -ff` as well. Untimed; runs inside the census container.
#
#   trim_census_run.sh <out_dir> <stress-ng> <trim_census.so> <arm>=<lib dir>... [strace]
#
# e.g. trim_census_run.sh out stress-ng trim_census.so N=/n/lib S=/s/lib strace
#
# WS (default "1 4 8") picks the worker counts. SNG_T (default 300, the
# METHODOLOGY.md safety bound) is stress-ng's -t: a worker still running at it
# finishes its bogo op and stops, so the census of that cell is short. W = 8
# hit it on this census's platforms, so W >= 8 runs at SNG_T_W8 (default 900),
# a value of the same length on the command line.
#
# Each (arm, W, repeat) writes <out_dir>/<arm>/W<w>/<rep>/: trim_<pid>.json per
# worker, st.<pid> per process under strace, the stress-ng log, and lib.txt,
# the libJudy.so.1 the dynamic loader resolves for that arm.
# ==============================================================================
set -euo pipefail
OUT="${1:?out dir}"; SNG="${2:?stress-ng}"; PRE="${3:?trim_census.so}"; shift 3
MODE=""; ARMS=()
for a in "$@"; do
  if [ "${a}" = strace ]; then MODE=strace; else ARMS+=("${a}"); fi
done
[ "${#ARMS[@]}" -gt 0 ] || { echo "no <arm>=<lib dir> given" >&2; exit 2; }
for spec in "${ARMS[@]}"; do
  arm="${spec%%=*}"; lib="${spec#*=}"
  for w in ${WS:-1 4 8}; do
    for rep in a b; do
      d="${OUT}/${arm}/W${w}/${rep}"
      rm -rf "${d}"; mkdir -p "${d}"
      LD_LIBRARY_PATH="${lib}" ldd "${SNG}" | grep 'libJudy.so.1' > "${d}/lib.txt"
      t="${SNG_T:-300}"; [ "${w}" -ge 8 ] && t="${SNG_T_W8:-900}"
      args=(--judy "${w}" --judy-size 4194304 --judy-ops "$((3 * w))" --seed 12345 -t "${t}" --metrics)
      run=(env TRIM_CENSUS_DIR="${d}" LD_LIBRARY_PATH="${lib}" LD_PRELOAD="${PRE}" "${SNG}" "${args[@]}")
      if [ "${MODE}" = strace ]; then
        strace -ff -e trace=brk,mmap,munmap,madvise,mremap -o "${d}/st" "${run[@]}" > "${d}/stress-ng.log" 2>&1
      else
        "${run[@]}" > "${d}/stress-ng.log" 2>&1
      fi
      n=$(find "${d}" -name 'trim_*.json' | wc -l)
      [ "${n}" -eq "${w}" ] || { echo "STOP: ${arm} W${w} ${rep}: ${n} worker reports, expected ${w}" >&2; exit 1; }
      echo "${arm} W${w} ${rep}: ${n} workers"
    done
  done
done
