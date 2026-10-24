#!/usr/bin/env python3
"""Bounds behind #1586's stall measurement (concurrency METHODOLOGY §44.2, AGENTS.md §8.8 commit 1).

`SyncExpanseStrMap::mem_used` (`crates/expanse/src/sync.rs`) reads through
`Shared::read_locked`: it takes the fallback mutex, closes the writer gate
(`quiesce_writers`), takes the writer mutex, and walks every node shell and
suffix leaf (`StrNode::shell_bytes`, `crates/expanse/src/strmap.rs`). An insert
arriving while the gate is closed spins in `enter_writer_blocking` until the
walk ends. The three functions below are the arithmetic §44 reads:

1. `stalled_fraction(walk_s, period_s)` — the share of wall time during which
   no insert completes, for a poller that reads once per `period_s`;
2. `spin_core_seconds(walk_s, writers)` — CPU time the writers can burn
   spinning per reading;
3. `walk_projection(ir_per_key, keys, ir_per_s)` — the walk's duration
   projected from its instruction count. §44 uses it only to size the run.
   It is a projection, never a prediction: the walk misses cache once the
   tree outgrows the last-level cache, and an instruction count cannot see
   that.

`arm_ir_per_key` turns the #1587 Callgrind arm's total into instructions per
key, and the self-test reads the arm's read count and population from
`crates/expanse/benches/instructions.rs`, so a change there fails it.

Design B (METHODOLOGY §47) adds the arithmetic its gates read:

4. `g_stall_outcome(mem_used_lo, mem_used_hi, len_hi, k)` — G-stall's rule;
5. `poll_band(exact_before, exact_after, tol)` and `poll_step_bound(...)` —
   G-accuracy's envelope for a reading taken while writers run;
6. `shell_term(...)` and `instr_bound(...)` — G-instr's per-arm two-sided
   bound: the measured suffix-half delta, plus a derived shell-half term from
   the arm's measured shell allocation and free counts, minus the measured
   walk the change deletes, widened by the review threshold;
7. `mem_used_fold_bound(...)` — what `sync_strmap_mem_used` collapses to;
8. `density_ceiling(...)` — G-density's per-shape ceiling;
9. `run_seconds_bound(**DISPATCH_47)` — the G-stall dispatch's length.

§47.11 (one increment on design B: shells and suffix leaves allocated without
zeroing, and no second zeroing of a deferred freelist hit) adds:

10. `zeroing_term(calls, libc, page_memset_ir)` and `zeroing_bound(base, term)` — the per-arm
    two-sided Callgrind bound against design B's head `a37a044e`, on each of
    two C libraries, from the arm's measured call records in
    `results/strmap_zeroing_baseline_callgrind_a37a044e.json`
    (`--design-b-zeroing` prints the table).

Every input of 6 to 8 is a measured figure in
`results/strmap_design_b_baseline_callgrind_f65060d8.json` or
`results/allocator_overhead_f65060d8.txt`, or a constant below whose
derivation §47.3 states.

Sources
-------
The arm figures are the PR-vs-main pass of CI run 39394551086 (PR #1587, head
5b7ae9bf): `sync_strmap_mem_used/routes` 15,539,041 instructions and
`sync_strmap_mem_used/short` 16,688,555, each over `MEM_USED_READS` (4) reads
of a 50,000-key map (`POP`). The rest is arithmetic, stated per function.

Usage:
    python3 scripts/strmap_stall_bounds.py             # the figures, then the self-test
    python3 scripts/strmap_stall_bounds.py --self-test # the self-test only
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
INSTRUCTIONS_RS = REPO_ROOT / "crates" / "expanse" / "benches" / "instructions.rs"

# The #1587 arm (CI run 39394551086, PR-vs-main pass): total instructions per
# variant, the reads it makes and the keys each read walks.
ARM_TOTAL_IR = {"routes": 15_539_041, "short": 16_688_555}
ARM_READS = 4
ARM_KEYS = 50_000
ARM_RUN = "https://github.com/orieg/expanse/actions/runs/39394551086"

# §44's populations, the ones #1586 names.
POPULATIONS = (2_000_000, 20_000_000)


def _finite(name: str, x: float) -> float:
    if isinstance(x, bool) or not isinstance(x, (int, float)) or not math.isfinite(x):
        raise ValueError(f"{name} must be a finite number, got {x!r}")
    return float(x)


def stalled_fraction(walk_s: float, period_s: float) -> float:
    """The share of wall time with the writer gate closed, for one poller.

    One poller makes one reading per `period_s`, and every reading holds the
    gate closed for `walk_s`. While it is closed no writer enters, so no insert
    completes, whatever the writer count: the fraction is per wall second and
    W does not divide it. For `walk_s <= period_s` it is `walk_s / period_s`
    (METHODOLOGY §44.2). The poller is one thread and its readings do not
    overlap, so a walk longer than the period leaves the next reading due on
    the walk's return: the readings run back to back and the gate is closed
    for the whole window. The fraction is therefore `walk_s / max(walk_s,
    period_s)`, which is 1 there rather than a ratio above 1.
    """
    walk = _finite("walk_s", walk_s)
    period = _finite("period_s", period_s)
    if walk < 0:
        raise ValueError(f"walk_s must be >= 0, got {walk_s!r}")
    if period <= 0:
        raise ValueError(f"period_s must be > 0, got {period_s!r}")
    return walk / max(walk, period)


def spin_core_seconds(walk_s: float, writers: int) -> float:
    """CPU seconds the writers can spend spinning during one reading: `walk_s × writers`.

    An arriving writer spins in `enter_writer_blocking` (`core::hint::spin_loop`,
    no yield) until the gate reopens, so each of W writers can burn at most the
    walk's duration per reading. It is an upper bound: a writer that is not
    trying to enter during the walk burns nothing.
    """
    walk = _finite("walk_s", walk_s)
    if walk < 0:
        raise ValueError(f"walk_s must be >= 0, got {walk_s!r}")
    if isinstance(writers, bool) or not isinstance(writers, int) or writers < 0:
        raise ValueError(f"writers must be a non-negative integer, got {writers!r}")
    return walk * writers


def arm_ir_per_key(total_ir: int, reads: int, keys: int) -> float:
    """Instructions per key of one walk: `total_ir / (reads × keys)`."""
    for name, v in (("total_ir", total_ir), ("reads", reads), ("keys", keys)):
        if isinstance(v, bool) or not isinstance(v, int) or v <= 0:
            raise ValueError(f"{name} must be a positive integer, got {v!r}")
    return total_ir / (reads * keys)


def walk_projection(ir_per_key: float, keys: int, ir_per_s: float) -> float:
    """Seconds one walk would take at `ir_per_s`: `ir_per_key × keys / ir_per_s`.

    Labelled a projection: `ir_per_key` is a Callgrind count at 50,000 keys,
    where the tree fits in cache, and `ir_per_s` is an assumed instruction
    rate, not a measurement. Past the last-level cache the walk's dependent
    loads miss and the rate falls, which an instruction count does not show.
    METHODOLOGY §44 uses it to size the run and never compares it with a
    result.
    """
    ipk = _finite("ir_per_key", ir_per_key)
    rate = _finite("ir_per_s", ir_per_s)
    if ipk <= 0:
        raise ValueError(f"ir_per_key must be > 0, got {ir_per_key!r}")
    if isinstance(keys, bool) or not isinstance(keys, int) or keys <= 0:
        raise ValueError(f"keys must be a positive integer, got {keys!r}")
    if rate <= 0:
        raise ValueError(f"ir_per_s must be > 0, got {ir_per_s!r}")
    return ipk * keys / rate


def ir_per_key(shape: str) -> float:
    """The #1587 arm's instructions per key for `shape` (`routes` or `short`)."""
    if shape not in ARM_TOTAL_IR:
        raise ValueError(f"unknown shape {shape!r}; expected one of {sorted(ARM_TOTAL_IR)}")
    return arm_ir_per_key(ARM_TOTAL_IR[shape], ARM_READS, ARM_KEYS)


def run_seconds_bound(processes_by_keys: dict[int, int], window_s: float, prefill_s_per_key: float,
                      post_s: float, build_s: float, blackout_s: float, longest_process_s: float) -> float:
    """An upper bound on one dispatch's length, to size the job timeout (not a prediction).

    Sum of: every process's window, every process's prefill at `prefill_s_per_key`
    (`processes_by_keys` maps N to the number of processes that prefill N keys),
    `post_s` per process for the closing readings and teardown, and `build_s`.
    Then the hourly guard (METHODOLOGY §44.8): each hour boundary the run spans
    costs at most the blackout plus the lead time of one process that could not
    start before it, `blackout_s + longest_process_s`, and the pauses themselves
    can push the run across one more boundary, so the hour count is taken after
    adding them, iterated to a fixed point.
    """
    vals = (window_s, prefill_s_per_key, post_s, build_s, blackout_s, longest_process_s)
    if any(_finite("bound input", v) < 0 for v in vals):
        raise ValueError(f"inputs must be finite and >= 0, got {vals}")
    if not processes_by_keys or any(
            isinstance(k, bool) or not isinstance(k, int) or k <= 0
            or isinstance(n, bool) or not isinstance(n, int) or n < 0
            for k, n in processes_by_keys.items()):
        raise ValueError(f"processes_by_keys must map positive N to process counts, got {processes_by_keys!r}")
    procs = sum(processes_by_keys.values())
    work = (procs * (window_s + post_s) + build_s
            + sum(k * n for k, n in processes_by_keys.items()) * prefill_s_per_key)
    per_hour = blackout_s + longest_process_s
    total = work
    for _ in range(100):
        hours = math.ceil(total / 3600)
        nxt = work + hours * per_hour
        if nxt == total:
            return total
        total = nxt
    raise ValueError("the hourly pauses do not converge: per-hour loss is not below one hour")


# --- Design B (METHODOLOGY §47) ----------------------------------------------

# G-stall's k: the mem_used arm's upper bound may be at most this many times the
# len arm's. §44.11's len arm, per-round largest insert latency at the M1 cell,
# spread over its 5 rounds by 241,151 / 106,239 = 2.27x (routes) and
# 166,783 / 92,799 = 1.80x (short); k sits above that spread.
G_STALL_K = 3.0

# G-accuracy: the caller's tolerance, and the step allowance per poll.
ACCURACY_TOL = 0.10
STEP_FRAC = 0.02

# G-instr's per-call constants, in instructions (derived, not measured: §47.3).
PLAIN_SHELL_ALLOC_IR = (25, 85)
PLAIN_SHELL_FREE_IR = (15, 45)
SHARED_SHELL_ALLOC_IR = (150, 340)
SHARED_SHELL_RETIRE_DELTA_IR = (-65, 45)
FOLD_IR_PER_READ = (64, 1000)
# §6's review threshold, applied as codegen and layout slack on every string arm,
# with a floor for small arms; and the non-string arms' bound.
CODEGEN_SLACK_FRAC = 0.001
CODEGEN_SLACK_ABS = 500
NON_STRING_FRAC = 0.0001

# G-density: bytes a shell adds per key beyond the packed build's count of it
# (accounted 48 against 40 counted, plus one slab page header and tail per 84
# blocks), and the census rounding (two decimals) as slack.
SHELL_HELD_BYTES = 9.0
SHELL_HEADERS_BYTES = 1.0
DENSITY_SLACK = 0.02


def _nonneg_int(name: str, v) -> int:
    if isinstance(v, bool) or not isinstance(v, int) or v < 0:
        raise ValueError(f"{name} must be a non-negative integer, got {v!r}")
    return v


def g_stall_outcome(mem_used_lo: float, mem_used_hi: float, len_hi: float, k: float = G_STALL_K) -> str:
    """G-stall for one cell: the per-round largest insert latency, BCa bounds over rounds.

    `PASS` iff the `mem_used` arm's upper bound is at most `k` times the `len`
    arm's upper bound; `FAIL` iff its lower bound exceeds that; `INTERMEDIATE`
    otherwise (METHODOLOGY §47.3). The same rule as the driver's
    `g_stall_outcome`, which the self-test checks against this one.
    """
    lo, hi, ln, kk = (_finite(n, x) for n, x in
                      (("mem_used_lo", mem_used_lo), ("mem_used_hi", mem_used_hi), ("len_hi", len_hi), ("k", k)))
    if lo > hi or min(lo, ln) < 0 or kk <= 0:
        raise ValueError(f"need 0 <= lo <= hi, len_hi >= 0 and k > 0, got {(lo, hi, ln, kk)}")
    bound = kk * ln
    if hi <= bound:
        return "PASS"
    if lo > bound:
        return "FAIL"
    return "INTERMEDIATE"


def poll_band(exact_before: int, exact_after: int, tol: float = ACCURACY_TOL) -> tuple[float, float]:
    """The values a reading taken between two exact quiet readings may return.

    `[(1 - tol) x min, (1 + tol) x max]` of the two (G-accuracy, the caller's
    +-10%). A reading inside this band is within `tol` of the exact value at
    one of the two instants that bracket it.
    """
    b = _nonneg_int("exact_before", exact_before)
    a = _nonneg_int("exact_after", exact_after)
    t = _finite("tol", tol)
    if not 0 <= t < 1:
        raise ValueError(f"tol must be in [0, 1), got {tol!r}")
    return (1 - t) * min(a, b), (1 + t) * max(a, b)


def poll_step_bound(exact_before: int, exact_after: int, frac: float = STEP_FRAC) -> float:
    """The largest change allowed between two consecutive readings of one phase.

    `|exact_after - exact_before| + frac x max(...)`: a phase's whole growth,
    plus `frac` of the map for the bytes of operations in flight at either
    reading. A reading that walked or cached would step by more.
    """
    b = _nonneg_int("exact_before", exact_before)
    a = _nonneg_int("exact_after", exact_after)
    f = _finite("frac", frac)
    if f < 0:
        raise ValueError(f"frac must be >= 0, got {frac!r}")
    return abs(a - b) + f * max(a, b)


def shell_term(allocs: int, malloc_ir: int, frees: int, free_ir: int, retires: int,
               shared: bool) -> tuple[float, float]:
    """The shell half's instruction change on one arm, as (low, high).

    `allocs` and `frees` are the arm's measured shell allocations and frees in
    the packed build (`malloc_ir`, `free_ir` their measured inclusive cost);
    `retires` its shell retirements on a shared map. Each allocation moves from
    `malloc` to `new_shell`, costing `PLAIN_SHELL_ALLOC_IR` or
    `SHARED_SHELL_ALLOC_IR`; each plain free from `free` to `free_shell`,
    `PLAIN_SHELL_FREE_IR`; each retirement gains `SHARED_SHELL_RETIRE_DELTA_IR`.
    """
    for n, v in (("allocs", allocs), ("malloc_ir", malloc_ir), ("frees", frees), ("free_ir", free_ir),
                 ("retires", retires)):
        _nonneg_int(n, v)
    if (allocs == 0) != (malloc_ir == 0) or (frees == 0) != (free_ir == 0):
        raise ValueError("a call count and its cost must both be zero or both positive")
    a_lo, a_hi = SHARED_SHELL_ALLOC_IR if shared else PLAIN_SHELL_ALLOC_IR
    f_lo, f_hi = PLAIN_SHELL_FREE_IR
    r_lo, r_hi = SHARED_SHELL_RETIRE_DELTA_IR
    lo = allocs * a_lo - malloc_ir + frees * f_lo - free_ir + retires * r_lo
    hi = allocs * a_hi - malloc_ir + frees * f_hi - free_ir + retires * r_hi
    return float(lo), float(hi)


def instr_bound(base: int, packed: int, shell: tuple[float, float], walk_ir: int) -> tuple[float, float]:
    """G-instr's two-sided bound on one string arm, as (low, high) instruction deltas against main.

    The measured suffix half (`packed - base`), plus the shell term, minus the
    measured `shell_bytes` walk the change deletes (`walk_ir`, the packed
    build's), widened on both sides by `CODEGEN_SLACK_FRAC` of the base (with a
    `CODEGEN_SLACK_ABS` floor).
    """
    b = _nonneg_int("base", base)
    pk = _nonneg_int("packed", packed)
    w = _nonneg_int("walk_ir", walk_ir)
    if b == 0:
        raise ValueError("base must be positive")
    lo, hi = (_finite("shell", x) for x in shell)
    if lo > hi:
        raise ValueError(f"shell term low {lo} exceeds high {hi}")
    slack = max(CODEGEN_SLACK_FRAC * b, CODEGEN_SLACK_ABS)
    centre = (pk - b) - w
    return centre + lo - slack, centre + hi + slack


def mem_used_fold_bound(base: int, call_ir: int, calls: int) -> tuple[float, float]:
    """`sync_strmap_mem_used` after the change, as (low, high) instruction deltas against main.

    Every one of the arm's `calls` readings (`call_ir` their inclusive cost on
    main) becomes one fold over the accounting shards, `FOLD_IR_PER_READ` each;
    nothing else in the arm changes.
    """
    b = _nonneg_int("base", base)
    c = _nonneg_int("call_ir", call_ir)
    n = _nonneg_int("calls", calls)
    if n == 0 or c > b:
        raise ValueError(f"need calls > 0 and call_ir <= base, got {(b, c, n)}")
    slack = max(CODEGEN_SLACK_FRAC * b, CODEGEN_SLACK_ABS)
    return -c + n * FOLD_IR_PER_READ[0] - slack, -c + n * FOLD_IR_PER_READ[1] + slack


def non_string_bound(base: int) -> float:
    """The largest |delta| allowed on an arm that builds no string map."""
    return NON_STRING_FRAC * _nonneg_int("base", base)


def density_ceiling(packed_bpk: float, shells_per_key: float, per_shell: float) -> float:
    """G-density's ceiling on one census column, in bytes per key.

    The packed build's figure (the suffix half, measured), plus the shells per
    key (measured) times what the shell half adds per shell (`SHELL_HELD_BYTES`
    or `SHELL_HEADERS_BYTES`, derived), plus `DENSITY_SLACK`.
    """
    v = _finite("packed_bpk", packed_bpk)
    s = _finite("shells_per_key", shells_per_key)
    ps = _finite("per_shell", per_shell)
    if min(v, s, ps) < 0:
        raise ValueError(f"inputs must be >= 0, got {(packed_bpk, shells_per_key, per_shell)}")
    return v + s * ps + DENSITY_SLACK


DESIGN_B_BASELINE = REPO_ROOT / "results" / "strmap_design_b_baseline_callgrind_f65060d8.json"


def arm_bound(row: dict) -> dict:
    """G-instr's bound for one arm of the baseline artifact: kind, (low, high) Ir deltas and percentages."""
    base = row["base"]
    if not row.get("string"):
        b = non_string_bound(base)
        lo, hi, kind = -b, b, "non-string"
    elif row["arm"] == "sync_strmap_mem_used":
        lo, hi = mem_used_fold_bound(base, row["mem_used_call_ir_base"], row["mem_used_calls"])
        kind = "mem_used fold"
    else:
        shared = bool(row.get("shared"))
        sh = shell_term(row["shell_allocs"], row["shell_malloc_ir"], row["shell_frees"], row["shell_free_ir"],
                        row["shell_retires"], shared)
        lo, hi = instr_bound(base, row["packed"], sh, row["walk_ir_packed"])
        kind = "shared string" if shared else "plain string"
    return {"kind": kind, "lo": lo, "hi": hi, "lo_pct": 100 * lo / base, "hi_pct": 100 * hi / base}


def design_b_bounds(path: Path = DESIGN_B_BASELINE) -> list[dict]:
    """Every arm of the baseline artifact with its G-instr bound (METHODOLOGY §47.3)."""
    art = json.loads(Path(path).read_text())
    out = []
    for row in art["arms"]:
        out.append({**{k: row[k] for k in ("package", "bench", "arm", "id", "base", "packed")}, **arm_bound(row)})
    return out


# §47.11: one increment on design B's head. Every constant is named in §47.11's silent constants.
ZB_BASELINE = REPO_ROOT / "results" / "strmap_zeroing_baseline_callgrind_a37a044e.json"
ZB_LIBCS = ("glibc-2.41", "glibc-2.39")
# Per collected memset call the change removes (pop_freelist's tail call, new_suffix's zeroing):
# the call's own setup, beside the memset's inclusive cost, which is removed exactly. 4 at a37a044e
# for pop_freelist's tail call (`objdump`), so [0, 6].
ZB_MEMSET_SETUP_IR = (0, 6)
# The smallest inclusive cost of one memset call: bounds how many collected calls stand behind
# an arc's measured cost (the smallest per-call figure among the base's arcs is 10.0).
ZB_MEMSET_MIN_IR = 8
# Per shared shell served from a collector freelist: `alloc_raw`'s 40-byte inline zeroing in
# `new_shell::<true>`, 4 instructions at a37a044e (`objdump`), so [0, 6].
ZB_SHELL_HIT_ZERO_IR = (0, 6)
# Per plain shell: at a37a044e the zeroing and the write of `StrNode::new()` merge into 4
# stores; the write alone needs at least 3, so [0, 2].
ZB_PLAIN_SHELL_ZERO_IR = (0, 2)
# An arc to calloc whose cost per call is at least this is read as fully collected; below it the
# count includes calls outside the measured region (the cheapest calloc measured is 71.0 Ir).
ZB_CALLOC_EXACT_MIN_IR = 60
# calloc(40) minus malloc(40) per call, the smallest difference over the three measured heap
# states (allocpc, the artifact): the least a collected calloc->malloc swap saves.
ZB_CALLOC_MINUS_MALLOC_MIN_IR = {"glibc-2.41": 24.0, "glibc-2.39": 31.0}
# malloc(40) per call with tcache holding the size, the cheapest measured malloc: the most a swap
# can save is the calloc's measured cost less this.
ZB_MALLOC_FLOOR_IR = {"glibc-2.41": 47.0, "glibc-2.39": 53.0}
# An arc to posix_memalign (one slab page) whose cost per call is at least this is read as fully
# collected; the base's fully collected arcs cost 420 to 685 per call.
ZB_POSIX_EXACT_MIN_IR = 300


def zeroing_term(calls: dict, libc: str, page_memset_ir: int) -> tuple[float, float]:
    """The increment's instruction change on one arm, as (low, high), on C library `libc`.

    `calls` holds the arm's call records at the base (artifact field names; a missing field is 0).
    The memset calls the change removes (`pop_memset_ir`, `suffix_memset_ir`) go exactly, plus
    `ZB_MEMSET_SETUP_IR` per collected call, at most their cost over `ZB_MEMSET_MIN_IR`; the one
    exception is the memset of each slab page `new_suffix` carves (`suffix_pages`, at
    `page_memset_ir` each, measured), which stays. A shared
    shell served from a freelist loses its inline zeroing (`ZB_SHELL_HIT_ZERO_IR`), a plain shell
    its merged store (`ZB_PLAIN_SHELL_ZERO_IR`). Each calloc of a shell or suffix becomes a malloc:
    on a fully collected arc (cost per call at least `ZB_CALLOC_EXACT_MIN_IR`) it saves between
    `ZB_CALLOC_MINUS_MALLOC_MIN_IR` per call and its cost less `ZB_MALLOC_FLOOR_IR` per call;
    on any other arc between nothing and its whole cost.
    """
    if libc not in ZB_LIBCS:
        raise ValueError(f"unknown C library {libc!r}")
    c = {k: _nonneg_int(k, v) for k, v in calls.items()}
    g = c.get
    memset_ir = g("pop_memset_ir", 0) + g("suffix_memset_ir", 0)
    if (g("pop_hits", 0) == 0) != (g("pop_memset_ir", 0) == 0) or \
            (g("suffix_memset_calls", 0) == 0) != (g("suffix_memset_ir", 0) == 0):
        raise ValueError("a memset count and its cost must both be zero or both positive")
    p_ir = _nonneg_int("page_memset_ir", page_memset_ir)
    pages, pages_ir = g("suffix_pages", 0), g("suffix_pages_ir", 0)
    if (pages == 0) != (pages_ir == 0) or (pages and p_ir == 0):
        raise ValueError("a page count, its cost and the page memset cost must agree")
    # The pages' memsets are inside suffix_memset_ir: at most as many as that cost holds, and exactly
    # `pages` when the arc is fully collected.
    p_hi = min(pages, g("suffix_memset_ir", 0) // p_ir) if pages else 0
    p_lo = p_hi if pages and pages_ir >= ZB_POSIX_EXACT_MIN_IR * pages else 0
    save_lo = float(memset_ir - p_hi * p_ir)
    save_hi = memset_ir - p_lo * p_ir + ZB_MEMSET_SETUP_IR[1] * memset_ir / ZB_MEMSET_MIN_IR
    hits = g("shared_shell_pops", 0) - g("shared_shell_callocs", 0)
    if hits < 0:
        raise ValueError("more shell callocs than shell freelist pops")
    save_hi += ZB_SHELL_HIT_ZERO_IR[1] * hits + ZB_PLAIN_SHELL_ZERO_IR[1] * g("plain_shells", 0)
    for n_key, ir_key in (("shared_shell_callocs", "shared_shell_calloc_ir"), ("suffix_callocs", "suffix_calloc_ir")):
        n, ir = g(n_key, 0), g(ir_key, 0)
        if n == 0:
            if ir:
                raise ValueError(f"{ir_key} without {n_key}")
            continue
        if ir >= ZB_CALLOC_EXACT_MIN_IR * n:
            save_lo += ZB_CALLOC_MINUS_MALLOC_MIN_IR[libc] * n
            save_hi += ir - ZB_MALLOC_FLOOR_IR[libc] * n
        else:
            save_hi += ir
    return -save_hi, -save_lo


def zeroing_bound(base: int, term: tuple[float, float]) -> tuple[float, float]:
    """§47.11's two-sided bound on one arm, as (low, high) Ir deltas against a37a044e: the term,
    widened on both sides by `CODEGEN_SLACK_FRAC` of the base with a `CODEGEN_SLACK_ABS` floor.
    A zero term is §47.11's "No change"."""
    b = _nonneg_int("base", base)
    if b == 0:
        raise ValueError("base must be positive")
    lo, hi = (_finite("term", x) for x in term)
    if lo > hi:
        raise ValueError(f"term low {lo} exceeds high {hi}")
    slack = max(CODEGEN_SLACK_FRAC * b, CODEGEN_SLACK_ABS)
    return lo - slack, hi + slack


def design_b_zeroing_bounds(path: Path = ZB_BASELINE) -> list[dict]:
    """Every arm of the §47.11 baseline, per C library, with its bound."""
    art = json.loads(Path(path).read_text())
    out = []
    for libc in ZB_LIBCS:
        inst = art["instruments"][libc]
        for row in inst["arms"]:
            term = zeroing_term(row.get("calls", {}), libc, inst["page_memset_ir"]["per_call"])
            lo, hi = zeroing_bound(row["base"], term)
            out.append({"libc": libc, "bench": row["bench"], "arm": row["arm"], "id": row["id"],
                        "base": row["base"], "kind": "No change" if term == (0.0, 0.0)
                        else "improves", "lo": lo, "hi": hi,
                        "lo_pct": 100 * lo / row["base"], "hi_pct": 100 * hi / row["base"]})
    return out


def _self_test_zeroing() -> None:
    # The term: exact memset removal, setup over the memset floor, one exact and one inexact arc.
    t = zeroing_term({"pop_hits": 10, "pop_memset_ir": 160, "suffix_memset_calls": 2, "suffix_memset_ir": 40,
                      "shared_shell_pops": 5, "shared_shell_callocs": 3, "shared_shell_calloc_ir": 300,
                      "suffix_callocs": 100, "suffix_calloc_ir": 50, "plain_shells": 7}, "glibc-2.41", 4114)
    # lo: 200 memset + 3 x 24 (shell arc exact: 100 per call); suffix arc inexact (0.5 per call): 0.
    # hi: 200 + 6 x 200 / 8 + 6 x 2 hits + 2 x 7 plain + (300 - 3 x 47) + 50.
    assert t == (-(200 + 150 + 12 + 14 + 159 + 50), -(200 + 72.0)), t
    assert zeroing_term({}, "glibc-2.39", 4113) == (-0.0, -0.0)
    t = zeroing_term({"suffix_callocs": 10, "suffix_calloc_ir": 1530}, "glibc-2.39", 4113)
    assert t == (-(1530 - 530), -310.0), t
    # Slab pages: their memsets stay. Exact arc (450 per page): 2 pages out of 8,328.
    t = zeroing_term({"suffix_memset_calls": 12, "suffix_memset_ir": 8328, "suffix_pages": 2, "suffix_pages_ir": 900},
                     "glibc-2.41", 4114)
    assert t == (-(8328 - 8228 + 6 * 8328 / 8), -(8328 - 8228.0)), t
    # Inexact arc (10 per page): between none and as many as the memset cost holds (2) staying.
    t = zeroing_term({"suffix_memset_calls": 12, "suffix_memset_ir": 8328, "suffix_pages": 5, "suffix_pages_ir": 50},
                     "glibc-2.41", 4114)
    assert t == (-(8328 + 6 * 8328 / 8), -(8328 - 8228.0)), t
    _raises(zeroing_term, {}, "musl", 4114)
    _raises(zeroing_term, {"pop_hits": 1}, "glibc-2.41", 4114)
    _raises(zeroing_term, {"shared_shell_callocs": 2, "shared_shell_pops": 1, "shared_shell_calloc_ir": 200},
            "glibc-2.41", 4114)
    _raises(zeroing_term, {"suffix_calloc_ir": 5}, "glibc-2.41", 4114)
    _raises(zeroing_term, {"plain_shells": -1}, "glibc-2.41", 4114)
    _raises(zeroing_term, {"suffix_pages": 1}, "glibc-2.41", 4114)
    _raises(zeroing_term, {"suffix_pages": 1, "suffix_pages_ir": 400}, "glibc-2.41", 0)
    # The bound: the term widened by 0.1% of the base, with the 500 floor.
    assert zeroing_bound(1_000_000, (-100.0, -50.0)) == (-1100.0, 950.0)
    assert zeroing_bound(100_000, (0.0, 0.0)) == (-500, 500)
    _raises(zeroing_bound, 0, (0.0, 0.0))
    _raises(zeroing_bound, 1, (1.0, 0.0))
    # The committed baseline: 286 arms per C library, and one arm pinned per library by hand.
    rows = {(r["libc"], r["bench"], r["arm"], r["id"]): r for r in design_b_zeroing_bounds()}
    assert len(rows) == 2 * 286
    r = rows[("glibc-2.41", "instructions", "sync_strmap_insert_short", "short")]
    # memset 109,420 + 500; setup 6 x 109,920 / 8; shell hits 0; shell arc 233 for 1 call (exact);
    # suffix arc 4,932,910 for 49,950 calls (98.8 per call, exact); slack 103,028.14.
    save_lo = 109_920 + 24.0 + 24.0 * 49_950
    save_hi = 109_920 + 6 * 109_920 / 8 + (233 - 47.0) + (4_932_910 - 47.0 * 49_950)
    assert r["kind"] == "improves" and (r["lo"], r["hi"]) == (-save_hi - 103_028.14, -save_lo + 103_028.14), r
    r = rows[("glibc-2.39", "instructions", "sync_strmap_insert_short", "short")]
    save_lo = 118_008 + 31.0 + 31.0 * 49_950
    save_hi = 118_008 + 6 * 118_008 / 8 + (220 - 53.0) + (7_642_375 - 53.0 * 49_950)
    assert (r["lo"], r["hi"]) == (-save_hi - 106_539.09, -save_lo + 106_539.09), r
    r = rows[("glibc-2.41", "instructions", "strmap_raw_insert", "clean")]
    # 2,101,038 memset Ir of which 387 slab pages x 4,114 stay (arc 171,106 Ir, exact); slack 41,288.529.
    keep = 2_101_038 - 387 * 4114
    assert (r["lo"], r["hi"]) == (-(keep + 6 * 2_101_038 / 8 + 2 * 893) - 41_288.529, -keep + 41_288.529), r
    r = rows[("glibc-2.41", "instructions", "set_insert", "sequential")]
    assert r["kind"] == "No change" and r["hi"] == -r["lo"] == max(1e-3 * r["base"], 500), r


def _self_test_design_b() -> None:
    # G-stall: the rule, one negative control per clause.
    assert g_stall_outcome(100, 290, 100) == "PASS"
    assert g_stall_outcome(100, 300, 100) == "PASS"          # at the bound
    assert g_stall_outcome(100, 301, 100) == "INTERMEDIATE"  # upper above, lower below
    assert g_stall_outcome(301, 400, 100) == "FAIL"
    assert g_stall_outcome(300, 400, 100) == "INTERMEDIATE"  # lower at the bound is not above it
    assert g_stall_outcome(10, 20, 10, k=1.0) == "INTERMEDIATE"
    for bad in ((2, 1, 1), (-1, 1, 1), (1, 2, -1), (1, 2, 1, 0), (float("nan"), 1, 1)):
        _raises(g_stall_outcome, *bad)
    # §44.11's M1 cell on main fails it by more than two orders of magnitude:
    # mem_used routes [517,839,257, 533,305,753] ns against 3 x 210,354.
    assert g_stall_outcome(517_839_257, 533_305_753, 210_354) == "FAIL"
    assert G_STALL_K > 241_151 / 106_239 > 166_783 / 92_799

    # G-accuracy.
    assert poll_band(100, 110) == (90.0, 121.00000000000001)
    assert poll_band(110, 100) == poll_band(100, 110)
    assert poll_band(0, 0) == (0.0, 0.0)
    assert poll_step_bound(1000, 1100) == 100 + 0.02 * 1100
    assert poll_step_bound(1100, 1000, 0.0) == 100
    for bad in ((-1, 1), (1, 1.5), (True, 1)):
        _raises(poll_band, *bad)
    _raises(poll_band, 1, 1, 1.0)
    _raises(poll_step_bound, 1, 1, -0.1)

    # shell_term: plain and shared constants, and a retire-only arm.
    lo, hi = shell_term(6253, 1_187_213, 0, 0, 0, shared=False)
    assert (lo, hi) == (6253 * 25 - 1_187_213, 6253 * 85 - 1_187_213)
    lo, hi = shell_term(50_000, 2_050_000, 50_000, 3_250_000, 0, shared=False)
    assert (lo, hi) == (50_000 * 40 - 5_300_000, 50_000 * 130 - 5_300_000)
    lo, hi = shell_term(6253, 1_349_369, 0, 0, 0, shared=True)
    assert (lo, hi) == (6253 * 150 - 1_349_369, 6253 * 340 - 1_349_369)
    assert shell_term(0, 0, 0, 0, 6253, shared=True) == (6253 * -65, 6253 * 45)
    assert shell_term(0, 0, 0, 0, 0, shared=False) == (0.0, 0.0)
    _raises(shell_term, 1, 0, 0, 0, 0, False)
    _raises(shell_term, 0, 0, 1, 0, 0, False)
    _raises(shell_term, -1, 0, 0, 0, 0, False)

    # instr_bound: suffix delta, walk, shell term and slack; the slack's floor.
    assert instr_bound(1_000_000, 1_010_000, (-100.0, 200.0), 0) == (10_000 - 100 - 1000, 10_000 + 200 + 1000)
    assert instr_bound(100_000, 100_000, (0.0, 0.0), 0) == (-500, 500)
    assert instr_bound(55_876_779, 55_849_306, (0.0, 0.0), 2_410_866) == (
        -27_473 - 2_410_866 - 55_876.779, -27_473 - 2_410_866 + 55_876.779)
    _raises(instr_bound, 0, 1, (0.0, 0.0), 0)
    _raises(instr_bound, 1, 1, (1.0, 0.0), 0)

    # mem_used_fold_bound: §46's arm, 4 readings at 9,813,786 on main.
    lo, hi = mem_used_fold_bound(15_663_913, 9_813_786, 4)
    assert (lo, hi) == (-9_813_786 + 256 - 15_663.913, -9_813_786 + 4000 + 15_663.913)
    _raises(mem_used_fold_bound, 10, 20, 1)
    _raises(mem_used_fold_bound, 10, 5, 0)
    assert non_string_bound(706_947) == 70.6947

    # density_ceiling.
    assert density_ceiling(48.72, 0.0518, SHELL_HELD_BYTES) == 48.72 + 0.0518 * 9.0 + 0.02
    _raises(density_ceiling, -1.0, 0.0, 1.0)

    # The G-stall dispatch fits the default job timeout with 1.2x headroom.
    assert run_seconds_bound(**DISPATCH_47) * 1.2 <= 180 * 60

    # The committed baseline's bounds, pinned on four arms (one per kind) by hand arithmetic.
    rows = {(r["bench"], r["arm"], r["id"]): r for r in design_b_bounds()}
    assert len(rows) == 372
    r = rows[("instructions", "strmap_insert", "routes")]
    # suffix -12,585; shells 6,253 x [25, 85] - 1,187,213; slack 42,608.73.
    assert r["kind"] == "plain string" and (r["lo"], r["hi"]) == (
        -12_585 + 6253 * 25 - 1_187_213 - 42_608.73, -12_585 + 6253 * 85 - 1_187_213 + 42_608.73), r
    r = rows[("instructions", "sync_strmap_insert_short", "short")]
    # suffix +6,169,078; one shell at [150, 340] - 41; slack 96,858.964.
    assert r["kind"] == "shared string" and (r["lo"], r["hi"]) == (
        6_169_078 + 150 - 41 - 96_858.964, 6_169_078 + 340 - 41 + 96_858.964), r
    r = rows[("instructions", "sync_strmap_mem_used", "short")]
    assert r["kind"] == "mem_used fold" and r["hi_pct"] < -60, r
    r = rows[("instructions", "set_insert", "sequential")]
    assert r["kind"] == "non-string" and r["hi"] == -r["lo"] == 1e-4 * r["base"], r


def _read_const(name: str) -> int:
    m = re.search(rf"^const {name}: usize = ([0-9_]+);", INSTRUCTIONS_RS.read_text(), re.M)
    if not m:
        raise RuntimeError(f"`const {name}` not found in {INSTRUCTIONS_RS}")
    return int(m.group(1).replace("_", ""))


def _raises(fn, *args) -> None:
    try:
        fn(*args)
    except ValueError:
        return
    raise AssertionError(f"{fn.__name__}{args} did not raise ValueError")


def self_test() -> int:
    # The arm's read count and population are the harness's own constants.
    assert _read_const("MEM_USED_READS") == ARM_READS
    assert _read_const("POP") == ARM_KEYS
    # Per-key counts: 15,539,041 / 200,000 and 16,688,555 / 200,000.
    assert abs(ir_per_key("routes") - 77.695205) < 1e-9, ir_per_key("routes")
    assert abs(ir_per_key("short") - 83.442775) < 1e-9, ir_per_key("short")
    # Per read, as the perf report prints it: 3,884,760.25 and 4,172,138.75.
    assert ARM_TOTAL_IR["routes"] / ARM_READS == 3_884_760.25
    assert ARM_TOTAL_IR["short"] / ARM_READS == 4_172_138.75
    _raises(ir_per_key, "uuid")

    # stalled_fraction: walk / period below the period, 1 at and above it.
    assert stalled_fraction(0.015, 1.0) == 0.015
    assert stalled_fraction(0.015, 0.1) == 0.15
    assert stalled_fraction(0.0, 1.0) == 0.0
    assert stalled_fraction(1.0, 1.0) == 1.0
    assert stalled_fraction(0.25, 0.1) == 1.0
    # §44.4's M1 threshold, 1 % of wall time, is a 10 ms walk at T = 1 s.
    assert stalled_fraction(0.010, 1.0) == 0.01
    for bad in ((-0.1, 1.0), (0.1, 0.0), (0.1, -1.0), (float("nan"), 1.0),
                (0.1, float("inf")), ("0.1", 1.0), (True, 1.0)):
        _raises(stalled_fraction, *bad)

    # spin_core_seconds: walk × writers.
    assert spin_core_seconds(0.015, 4) == 0.06
    assert spin_core_seconds(0.2, 8) == 1.6
    assert spin_core_seconds(0.2, 0) == 0.0
    for bad in ((0.1, -1), (0.1, 1.5), (0.1, True), (-0.1, 4), (float("nan"), 4)):
        _raises(spin_core_seconds, *bad)

    # walk_projection: 77.695205 Ir/key × 2e7 keys = 1,553,904,100 Ir.
    assert abs(walk_projection(ir_per_key("routes"), 20_000_000, 1e9) - 1.5539041) < 1e-9
    assert abs(walk_projection(ir_per_key("short"), 2_000_000, 1e9) - 0.16688555) < 1e-9
    assert abs(walk_projection(80.0, 50_000, 4e9) - 0.001) < 1e-15
    for bad in ((0.0, 10, 1e9), (-1.0, 10, 1e9), (80.0, 0, 1e9), (80.0, 10.0, 1e9),
                (80.0, 10, 0.0), (80.0, 10, float("nan")), (80.0, True, 1e9)):
        _raises(walk_projection, *bad)
    for bad in ((0, 4, 50_000), (100, 0, 50_000), (100, 4, -1), (100.0, 4, 50_000)):
        _raises(arm_ir_per_key, *bad)
    # run_seconds_bound: one process of 10 keys, no hour crossed.
    assert run_seconds_bound({10: 1}, 30, 1.0, 10, 100, 300, 60) == 30 + 10 + 100 + 10 + 360
    # Iteration: 3,500 s of work crosses one boundary; the 400 s of pauses push it across a second.
    assert run_seconds_bound({1: 1}, 3499, 0, 0, 0, 300, 100) == 3499 + 2 * 400
    for bad in (({}, 30, 1, 0, 0, 0, 0), ({0: 1}, 30, 1, 0, 0, 0, 0), ({10: -1}, 30, 1, 0, 0, 0, 0),
                ({10: 1}, -1, 1, 0, 0, 0, 0), ({10: 1}, 30, float("nan"), 0, 0, 0, 0)):
        _raises(run_seconds_bound, *bad)
    _raises(run_seconds_bound, {1: 1}, 10, 0, 0, 0, 3000, 600)
    _self_test_design_b()
    _self_test_zeroing()
    print("strmap_stall_bounds.py self-test PASSED")
    return 0


def report() -> None:
    print(f"#1587 arm ({ARM_RUN}, PR-vs-main pass):")
    for shape in ("routes", "short"):
        print(f"  {shape}: {ARM_TOTAL_IR[shape]:,} Ir / ({ARM_READS} reads x {ARM_KEYS:,} keys)"
              f" = {ir_per_key(shape):.4f} Ir/key")
    print("Walk projections (assumed instruction rates; sizing only, never a prediction):")
    for shape in ("routes", "short"):
        for keys in POPULATIONS:
            lo = walk_projection(ir_per_key(shape), keys, 4e9)
            hi = walk_projection(ir_per_key(shape), keys, 1e9)
            print(f"  {shape} N={keys:,}: {lo:.3f} s at 4e9 Ir/s .. {hi:.3f} s at 1e9 Ir/s")
    b = run_seconds_bound(**DISPATCH)
    print(f"Dispatch bound (sizing the job timeout): {b:,.0f} s from {DISPATCH}")
    b47 = run_seconds_bound(**DISPATCH_47)
    print(f"§47 G-stall dispatch bound: {b47:,.0f} s ({b47 * 1.2 / 60:,.0f} min at 1.2x) from {DISPATCH_47}")


# §44's dispatch: 24 cells x 3 arms x 5 rounds timed, plus 24 counter processes, half at
# each N; laptop prefill (loaded, sizing only) was at most 1.34e-06 s/key, so 2.7e-06 (2x)
# per key; 10 s per process after the window; 600 s per build, two builds; the longest
# process the driver's guard estimates is 2.7e-06 x 2e7 x 1.5 + 30 + 30 = 141 s.
DISPATCH = {"processes_by_keys": {2_000_000: 192, 20_000_000: 192}, "window_s": 30,
            "prefill_s_per_key": 2.7e-06, "post_s": 10, "build_s": 1200, "blackout_s": 300,
            "longest_process_s": 141}


# §47's G-stall dispatch: 2 shapes x 2 workloads x 3 arms x 5 rounds = 60 processes, all at
# N = 2e7; §44's sizing constants otherwise (§44.11's run prefilled 2e7 short keys in at most
# 9.99 s, 5.0e-07 s/key, below the 2.7e-06 used here); one build, no counters pass.
DISPATCH_47 = {"processes_by_keys": {20_000_000: 60}, "window_s": 30,
               "prefill_s_per_key": 2.7e-06, "post_s": 10, "build_s": 600, "blackout_s": 300,
               "longest_process_s": 141}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--design-b", action="store_true",
                    help="print METHODOLOGY §47.3's G-instr table for every string arm, then the self-test")
    ap.add_argument("--design-b-zeroing", action="store_true",
                    help="print METHODOLOGY §47.11's bound for every arm the increment is expected to move, "
                         "per C library, then the self-test")
    args = ap.parse_args()
    if args.design_b_zeroing:
        print("| C library | harness | arm | a37a044e | bound low | bound high |")
        print("|---|---|---|---:|---:|---:|")
        for r in design_b_zeroing_bounds():
            if r["kind"] == "No change":
                continue
            print(f"| {r['libc']} | `{r['bench']}` | `{r['arm']}/{r['id']}` | {r['base']:,} | "
                  f"{r['lo_pct']:+.4f}% | {r['hi_pct']:+.4f}% |")
    elif args.design_b:
        print("| harness | arm | main | kind | bound low | bound high |")
        print("|---|---|---:|---|---:|---:|")
        for r in design_b_bounds():
            if r["kind"] == "non-string":
                continue
            h = "capi smoke_instructions" if r["package"] == "expanse-capi" else r["bench"]
            print(f"| `{h}` | `{r['arm']}/{r['id']}` | {r['base']:,} | {r['kind']} | "
                  f"{r['lo_pct']:+.4f}% | {r['hi_pct']:+.4f}% |")
    elif not args.self_test:
        report()
    return self_test()


if __name__ == "__main__":
    sys.exit(main())
