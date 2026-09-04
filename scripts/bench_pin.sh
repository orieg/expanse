# shellcheck shell=sh
# scripts/bench_pin.sh — confine a wall-clock benchmark to the performance cores (#639).
#
# SOURCED, never executed:
#
#     . "$REPO_ROOT/scripts/bench_pin.sh"
#
# sets the *calling shell's* CPU affinity, so every `cargo bench`, `cargo run`
# and helper process it later spawns inherits the pin. That is the whole point:
# the pin belongs to the runner, not to each harness, so a benchmark added
# later cannot forget it and no call site has to be edited when one is.
#
# Why. The bare-metal reference host is a hybrid part — performance cores at
# 5.0-5.1 GHz and efficiency cores at 3.8 GHz — and the scheduler is free to
# place a benchmark thread on either. `scripts/perf_counters.py` already
# refuses to collect unpinned counters here and prefixes its workload with
# `taskset -c <p-cores>`; this file is the same rule for the wall-clock lane,
# which had no pin at all. Measured on the reference host, the same criterion
# arm run on the E-cores takes 1.576x the time it takes on the P-cores, with
# separated intervals, so an unnoticed migration is a phantom regression of
# that size. A BCa interval cannot see it: the interval is tight and clean when
# no migration happens, and tight and wrong when one does.
#
# Fail-loud (AGENTS.md section 8.1): on a hybrid host with no `taskset`, or
# when the affinity call does not take, this refuses to run rather than let a
# suite produce numbers whose core class is unknown.
#
# Environment:
#   EXPANSE_BENCH_PIN unset  auto — pin to the kernel's `cpu_core` CPU list on
#                            a hybrid host; no pin on a uniform one, where
#                            there is nothing to pin away from.
#   EXPANSE_BENCH_PIN=<list> pin to exactly this `taskset -c` list, on any
#                            host. This is the SMT-off / single-sibling knob
#                            (e.g. `0,2,4,6,8,10,12,14` on the reference host)
#                            and the way to reproduce a figure on a machine
#                            whose PMU names this file does not know.
#   EXPANSE_BENCH_PIN=off    do not pin, and say so loudly. For deliberate
#                            unpinned comparisons (`scripts/pin_exposure.py`).
#
# Publishes `EXPANSE_BENCH_PIN_APPLIED` for provenance: the CPU list, or
# `none` (uniform host), or `off`.

_expanse_pin_expand() {
    # "0-3,8" -> one CPU id per line, sorted.
    # The trailing newline matters: `read` drops a final line that has none,
    # so `printf '%s'` here would silently swallow the last range in the list.
    printf '%s\n' "$1" | tr ',' '\n' | while IFS= read -r _part; do
        case "$_part" in
            "") ;;
            *-*)
                _lo=${_part%%-*}
                _hi=${_part##*-}
                while [ "$_lo" -le "$_hi" ]; do
                    printf '%s\n' "$_lo"
                    _lo=$((_lo + 1))
                done
                ;;
            *) printf '%s\n' "$_part" ;;
        esac
    done | sort -n | uniq
}

_expanse_pin_read() {
    # The kernel's CPU list for a PMU, e.g. `0-15` for `cpu_core`. Same source
    # of truth `scripts/perf_counters.py` reads, so the two lanes cannot drift
    # onto different masks.
    # EXPANSE_PIN_SYSFS_ROOT exists so the refusal paths below can be exercised
    # against a synthetic topology (`scripts/check_bench_pin.py --self-test`).
    # It is never set on a real run.
    _root=${EXPANSE_PIN_SYSFS_ROOT:-/sys/devices}
    if [ -r "$_root/$1/cpus" ]; then
        tr -d '\n' < "$_root/$1/cpus"
    fi
}

_expanse_pin_apply() {
    _mask=$1
    if ! command -v taskset >/dev/null 2>&1; then
        echo "refusing to start: CPUs $_mask were requested but \`taskset\` is not on" >&2
        echo "PATH (install util-linux). An unpinned wall-clock arm on a hybrid host can" >&2
        echo "land on an efficiency core and report a regression that is core placement," >&2
        echo "not code." >&2
        echo "Set EXPANSE_BENCH_PIN=off to measure unpinned deliberately." >&2
        exit 1
    fi
    if ! taskset -c -p "$_mask" $$ >/dev/null 2>&1; then
        echo "refusing to start: \`taskset -c -p $_mask $$\` failed; the benchmark shell" >&2
        echo "could not be confined to CPUs $_mask. A cgroup or affinity policy may be" >&2
        echo "overriding it. No benchmark was run and no numbers were produced." >&2
        exit 1
    fi
    # Read the affinity back rather than trusting the call: a mask the kernel
    # silently narrowed, or an E-core still in the set, means the pin did not
    # hold and every number after it is unattributable.
    _actual=$(taskset -c -p $$ 2>/dev/null | sed 's/.*: *//')
    if [ "$(_expanse_pin_expand "$_actual")" != "$(_expanse_pin_expand "$_mask")" ]; then
        echo "refusing to start: asked for CPUs $_mask but the shell reports $_actual." >&2
        echo "The pin did not hold, so core placement is unknown for this run." >&2
        exit 1
    fi
    if [ -n "$_expanse_e_cpus" ]; then
        _clash=$( { _expanse_pin_expand "$_actual"; echo "--"; _expanse_pin_expand "$_expanse_e_cpus"; } \
            | awk '/^--$/ { seen = 1; next } !seen { p[$0] = 1; next } ($0 in p) { printf "%s ", $0 }' \
            | sed 's/ *$//')
        if [ -n "$_clash" ]; then
            echo "refusing to start: the requested CPU list $_mask includes efficiency" >&2
            echo "core(s) $_clash. Wall-clock arms measured across two core classes are" >&2
            echo "not comparable run to run. Set EXPANSE_BENCH_PIN=off to do it anyway." >&2
            exit 1
        fi
    fi
    EXPANSE_BENCH_PIN_APPLIED=$_mask
    export EXPANSE_BENCH_PIN_APPLIED
    echo "core pin: benchmark shell confined to CPUs $_mask (AGENTS.md section 8.4, #639)"
}

_expanse_bench_pin() {
    _expanse_p_cpus=$(_expanse_pin_read cpu_core)
    _expanse_e_cpus=$(_expanse_pin_read cpu_atom)

    case "${EXPANSE_BENCH_PIN:-}" in
        off|OFF)
            EXPANSE_BENCH_PIN_APPLIED=off
            export EXPANSE_BENCH_PIN_APPLIED
            echo "core pin: DISABLED by EXPANSE_BENCH_PIN=off — this run may migrate" >&2
            echo "between core classes, and its wall-clock figures are not publishable" >&2
            echo "on a hybrid host without saying so (#639)." >&2
            return 0
            ;;
        "")
            if [ -z "$_expanse_e_cpus" ]; then
                # Uniform host: no second core class to migrate onto, so a pin
                # would only shrink the machine. Same rule perf_counters.py
                # applies — pin where there is something to pin away from.
                EXPANSE_BENCH_PIN_APPLIED=none
                export EXPANSE_BENCH_PIN_APPLIED
                echo "core pin: not needed — this host exposes one core class"
                return 0
            fi
            if [ -z "$_expanse_p_cpus" ]; then
                echo "refusing to start: an efficiency-core PMU is present but the kernel" >&2
                echo "publishes no /sys/devices/cpu_core/cpus, so the performance cores" >&2
                echo "cannot be named. Set EXPANSE_BENCH_PIN=<cpu list> explicitly." >&2
                exit 1
            fi
            _expanse_pin_apply "$_expanse_p_cpus"
            ;;
        *)
            _expanse_pin_apply "${EXPANSE_BENCH_PIN}"
            ;;
    esac
}

_expanse_bench_pin
