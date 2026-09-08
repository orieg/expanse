#!/usr/bin/env python3
"""Every table in `docs/benchmarks/concurrency/README.md`, derived from the
committed artifacts (AGENTS.md section 8.2) — nothing here is typed.

Sections emitted, in README order:

- `2. Line-transfer matrix` from `results/line_transfer.json`
- `3. Attribution — D1` from the two FFI suites' `baseline_concurrent*.json`
  (throughput + health cells) and their `counters_<cell>.json`
- `4. Attribution — D2` from the same
- `5. The counter's own spread (P0.4)` from the H cells of both runs

A missing artifact renders the section's rows as `pending` citing the open
tracking issue, so the README is correct before the run exists and
`scripts/check_readme_tables.py` can enforce it either way. A present
artifact that lacks a field the estimator needs is an error (section 8.1),
never a zero.

Estimators (each stated where it is used):
- per-probe ns = readers / aggregate reader M ops/s (throughput build)
- spin share of the D1 delta = `spin_time_share` (health build, share of the
  readers' elapsed time) x per-probe ns under one writer / (per-probe ns under
  one writer - per-probe ns alone)
- restart share of the delta = (read_attempts - read_ops) / read_ops x
  per-probe ns alone / delta
- fallback share = 0 by construction at these cells (stated, not computed)
- writer counters per insert come from the per-thread counter artifacts
- "unattributed" = 1 - (spin + restarts); the writer-side columns are not
  shares of the reader delta and are not subtracted from it

Usage: `python3 docs/benchmarks/concurrency/scripts/tables.py` (stdout), or
through `scripts/check_readme_tables.py --write`.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
SUITE = REPO_ROOT / "docs" / "benchmarks" / "concurrency"
HOT = REPO_ROOT / "docs" / "benchmarks" / "hot_comparison" / "results"
MT = REPO_ROOT / "docs" / "benchmarks" / "masstree_comparison" / "results"
ISSUE = "[#568](https://github.com/orieg/expanse/issues/568)"
PENDING = f"pending ({ISSUE})"

# D1 cells: (label, suite results dir, arm, counters cell name)
D1 = [
    ("hot_conc_set_w1_r8", HOT, "set", "hot_conc_set_w1_r8"),
    ("hot_conc_map_w1_r8", HOT, "map", "hot_conc_map_w1_r8"),
    ("masstree_conc_map_w1_r8", MT, "map", "masstree_conc_map_w1_r8"),
]
# D2 cells: (label, results dir, arm, W, counters cell, rule-18 note)
D2 = [
    ("masstree_conc_str_w8_r0", MT, "str", 8, "masstree_conc_str_w8_r0", ""),
    ("masstree_conc_str_w16_r0", MT, "str", 16, "masstree_conc_str_w16_r0", ""),
    ("masstree_conc_map_w8_r0", MT, "map", 8, "masstree_conc_map_w8_r0", ""),
    ("masstree_conc_map_w16_r0", MT, "map", 16, "masstree_conc_map_w16_r0", "direction only (rule 18)"),
]


def load(path: Path) -> dict | None:
    return json.loads(path.read_text()) if path.is_file() else None


def need(d: dict, key: str, where: str):
    if key not in d or d[key] is None:
        raise SystemExit(f"{where}: field `{key}` is missing; the artifact predates the estimator "
                         f"that needs it (re-measure rather than default to 0, section 8.1)")
    return d[key]


def pm(iv: dict | None) -> float | None:
    """The point estimate of an interval dict: throughput artifacts carry `mean`,
    counter artifacts `per_op_mean` (bench_counters.py::interval_over)."""
    if not iv:
        return None
    return iv.get("mean", iv.get("per_op_mean"))


def fmt_iv(iv: dict | None, digits: int = 1, unit: str = "") -> str:
    m = pm(iv)
    if m is None:
        return "n/a"
    lo, hi = iv.get("ci_lower"), iv.get("ci_upper")
    if lo is None or hi is None:
        return f"{m:.{digits}f} (no interval){unit}"
    return f"{m:.{digits}f} [{lo:.{digits}f}, {hi:.{digits}f}]{unit}"


# ---- 2. line transfer ------------------------------------------------------
def line_transfer() -> list[str]:
    out = ["## 2. Line-transfer matrix (`results/line_transfer.json`)", "",
           "| kind | mode | cells | min ns | median ns | max ns |", "|---|---|--:|--:|--:|--:|"]
    art = load(SUITE / "results" / "line_transfer.json")
    kinds = (("pause", "pause"), ("cross-core", "spin"), ("cross-core", "park"),
             ("smt-sibling", "spin"), ("smt-sibling", "park"))
    if art is None:
        for i, (k, m) in enumerate(kinds):
            cells = PENDING if i == 0 else "pending"
            out.append(f"| {k} | {m} | {cells} | pending | pending | pending |")
        return out
    for k, m in kinds:
        sel = [c for c in art["cells"] if c["kind"] == k and c["mode"] == m]
        if not sel:
            out.append(f"| {k} | {m} | 0 | not measured | not measured | not measured |")
            continue
        means = sorted(need(c, "ns_per_transfer", c["label"])["mean"] for c in sel)
        out.append(f"| {k} | {m} | {len(sel)} | {means[0]:.1f} | {means[len(means)//2]:.1f} | {means[-1]:.1f} |")
    return out


# ---- helpers over the FFI artifacts ----------------------------------------
def throughput_cell(art: dict, arm: str, w: int, r: int) -> dict | None:
    for c in art.get("throughput", []):
        if c["arm"] == arm and c["writers"] == w and c["readers"] == r:
            return c
    return None


def health_cell(art: dict, arm: str, w: int, r: int) -> dict | None:
    for c in art.get("health", []):
        if c["arm"] == arm and c["writers"] == w and c["readers"] == r:
            return c
    return None


def reader_ns(cell: dict, readers: int) -> float | None:
    """Per-probe ns of one reader from the cell's aggregate reader rate."""
    iv = cell.get("expanse_reader_mops")
    if not iv or not iv.get("mean"):
        return None
    return readers / iv["mean"] * 1e3


def counters(results: Path, cell_name: str) -> dict | None:
    return load(results / f"counters_{cell_name}.json")


def role_event(art: dict | None, role: str, ev: str) -> dict | None:
    if not art:
        return None
    return art.get("roles", {}).get(role, {}).get("events", {}).get(ev)


# ---- 3. D1 -----------------------------------------------------------------
def d1() -> list[str]:
    out = ["## 3. Attribution — D1, readers under one writer (METHODOLOGY §4, §5)", "",
           "| cell | run | spin time (P0.1) | restarts | fallback | writer RFO / insert (P0.2) | unattributed | verdict |",
           "|---|--:|---|---|---|---|---|---|"]
    any_rows = False
    for label, results, arm, ccell in D1:
        for run, fname in ((1, "baseline_concurrent.json"), (2, "baseline_concurrent_run2.json")):
            art = load(results / fname)
            h = health_cell(art, arm, 1, 8) if art else None
            if not art or not h or "spin_time_share" not in h:
                continue
            any_rows = True
            alone = throughput_cell(art, arm, 0, 8)
            with_w = throughput_cell(art, arm, 1, 8)
            ns_alone = reader_ns(alone, 8) if alone else None
            ns_with = reader_ns(with_w, 8) if with_w else None
            spin_share = need(h, "spin_time_share", f"{label} run {run}")["median"]
            ra, ro = need(h, "read_attempts", label)["median"], need(h, "read_ops", label)["median"]
            restart_rate = (ra - ro) / ro if ro else 0.0
            if ns_alone and ns_with and ns_with > ns_alone:
                delta = ns_with - ns_alone
                spin_of_delta = spin_share * ns_with / delta
                restart_of_delta = restart_rate * ns_alone / delta
                unattr = 1.0 - spin_of_delta - restart_of_delta
                spin_txt = f"{spin_of_delta:.0%} of Δ ({spin_share:.0%} of reader time)"
                rs_txt = f"{restart_of_delta:.1%} of Δ ({restart_rate:.1%} of walks)"
                un_txt = f"{unattr:.0%}"
                verdict = "`CONFIRMED`" if spin_share >= 0.50 else ("**`REFUTED`**" if spin_share < 0.25 else "`BOUNDARY_RESULT`")
                verdict += f" (P0.1 on {spin_share:.2f})"
            else:
                spin_txt, rs_txt, un_txt, verdict = f"{spin_share:.0%} of reader time", f"{restart_rate:.1%} of walks", "n/a", "`NOT_INSTRUMENTED` (no throughput pair)"
            c1 = counters(results, ccell.replace("_r8", "_r0"))
            c2 = counters(results, ccell)
            rfo_c1 = role_event(c1, "writer", "l2_rqsts.rfo_miss")
            rfo_c2 = role_event(c2, "writer", "l2_rqsts.rfo_miss")
            if pm(rfo_c1) is not None and pm(rfo_c2) is not None:
                rfo_txt = f"{pm(rfo_c2):.2f} vs {pm(rfo_c1):.2f} alone (Δ {pm(rfo_c2)-pm(rfo_c1):+.2f})"
            else:
                rfo_txt = "`NOT_INSTRUMENTED`"
            out.append(f"| `{label}` | {run} | {spin_txt} | {rs_txt} | 0 by construction | {rfo_txt} | {un_txt} | {verdict} |")
    if not any_rows:
        for i, (label, *_rest) in enumerate(D1):
            out.append(f"| `{label}` | — | {PENDING if i == 0 else 'pending'} | pending | 0 by construction | pending | pending | pending |")
    return out


# ---- 4. D2 -----------------------------------------------------------------
def d2() -> list[str]:
    out = ["## 4. Attribution — D2, writers under load (METHODOLOGY §4, §5)", "",
           "| cell | context switches / insert (P0.3) | handoffs / insert | RFO / insert | futex / insert | verdict |",
           "|---|---|---|---|---|---|"]
    any_rows = False
    for label, results, arm, w, ccell, note in D2:
        c = counters(results, ccell)
        art = load(results / "baseline_concurrent.json")
        h = health_cell(art, arm, w, 8) if art else None  # H cells are R=8; handoffs at W>=2 there
        cs = role_event(c, "writer", "context-switches")
        rfo = role_event(c, "writer", "l2_rqsts.rfo_miss")
        futex = role_event(c, "writer", "syscalls:sys_enter_futex")
        if c is None and h is None:
            continue
        any_rows = True
        cs_txt = fmt_iv(cs, 3) if pm(cs) is not None else "`NOT_INSTRUMENTED`"
        hand_txt = "n/a (R=0 cell; H cells carry R=8)" if not h or "handoffs_per_write" not in h else f"{h['handoffs_per_write']['median']:.3f} (H W={w} R=8)"
        rfo_txt = fmt_iv(rfo, 2) if pm(rfo) is not None else "`NOT_INSTRUMENTED`"
        fut_txt = fmt_iv(futex, 3) if pm(futex) is not None else "`NOT_INSTRUMENTED`"
        verdict = note or "see METHODOLOGY §4 P0.3"
        if pm(cs) is not None:
            m = pm(cs)
            if arm == "str" and w == 16:
                verdict = "`CONFIRMED`" if m >= 0.5 else ("**`REFUTED`**" if m < 0.2 else "`BOUNDARY_RESULT`")
            elif arm == "str" and w == 8:
                verdict = "`CONFIRMED`" if m <= 0.1 else "`BOUNDARY_RESULT`"
            elif arm == "map" and w == 16:
                verdict = ("`CONFIRMED`" if m <= 0.1 else "`BOUNDARY_RESULT`") + "; " + note
            elif arm == "map" and w == 8:
                verdict = "control"
        out.append(f"| `{label}` | {cs_txt} | {hand_txt} | {rfo_txt} | {fut_txt} | {verdict} |")
    if not any_rows:
        for i, (label, _r, _a, _w, _c, note) in enumerate(D2):
            v = note or "pending"
            out.append(f"| `{label}` | {PENDING if i == 0 else 'pending'} | pending | pending | pending or `NOT_INSTRUMENTED` | {v} |")
    return out


# ---- 5. counter spread -----------------------------------------------------
def spread() -> list[str]:
    out = ["## 5. The counter's own spread (P0.4)", "",
           "| suite | arm | W | spins ÷ read_ops run 1 | run 2 | ratio | restart run 1 | run 2 | ratio | verdict |",
           "|---|---|--:|--:|--:|--:|--:|--:|--:|---|"]
    any_rows = False
    for name, results in (("hot_comparison", HOT), ("masstree_comparison", MT)):
        a, b = load(results / "baseline_concurrent.json"), load(results / "baseline_concurrent_run2.json")
        if not a or not b:
            continue
        for ha in a.get("health", []):
            if ha["arm"] == "str":
                continue
            hb = health_cell(b, ha["arm"], ha["writers"], ha["readers"])
            if not hb or "sample_spin_cycles" not in ha:
                continue
            any_rows = True
            sa = ha["sample_spins"]["median"] / ha["read_ops"]["median"]
            sb = hb["sample_spins"]["median"] / hb["read_ops"]["median"]
            ra, rb = ha["restart_share"]["median"], hb["restart_share"]["median"]
            r1 = max(sa, sb) / min(sa, sb) if min(sa, sb) > 0 else float("inf")
            r2 = max(ra, rb) / min(ra, rb) if min(ra, rb) > 0 else float("inf")
            verdict = "`CONFIRMED`" if r1 <= 1.25 and r2 <= 1.25 else "**`REFUTED`**"
            out.append(f"| {name} | {ha['arm']} | {ha['writers']} | {sa:.2f} | {sb:.2f} | {r1:.2f}× | {ra:.2%} | {rb:.2%} | {r2:.2f}× | {verdict} |")
    if not any_rows:
        out.append(f"| — | — | — | {PENDING} | pending | pending | pending | pending | pending | pending |")
    return out


def main() -> int:
    blocks = [line_transfer(), d1(), d2(), spread()]
    print("\n\n".join("\n".join(b) for b in blocks))
    return 0


if __name__ == "__main__":
    sys.exit(main())
