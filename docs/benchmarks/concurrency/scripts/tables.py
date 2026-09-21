#!/usr/bin/env python3
"""Every table in `docs/benchmarks/concurrency/README.md`, derived from the
committed artifacts (AGENTS.md section 8.2) — nothing here is typed.

Sections emitted, in README order:

- `2. Line-transfer matrix` from `results/line_transfer.json`
- `3. Attribution — D1` from the two FFI suites' `results/step0/baseline_concurrent*.json`
  (throughput + health cells, the `a1982ff2` pair Step 0 was measured on) and
  their `counters_<cell>.json`
- `4. Attribution — D2` from the same
- `5. The counter's own spread (P0.4)` from the H cells of both runs
- `6. Ablations — padding the writer lock` from `results/ablations.json`
- `6b. The string wrapper's reader mode` from `results/ablations_str.json`
- `8. Fine-grained write brackets gate` via `fine_grained_brackets_gate.py`
- `9. Multi-writer OLC gate` via `multi_writer_olc_gate.py`
- `10. Writer scaling` from `results/baseline_writer_scaling.json`
- `11.8` arm (a), `lock-padded` and their combination, one process per cell,
  from `results/{combined_alloc_padded,ablation_alloc,padded}_writer_scaling_bad1bd3d{,_run2}.json`,
  beside the `726b01fc` multi-cell artifacts of §11.7 for the position and
  `str` W = 1 comparisons
- `11.9` the `perf c2c` contention ranking, the frequency droop at W = 8 and
  the host load of the two `writer_scaling_diagnostic` runs at `ac8f1c6d`,
  from `results/diagnostic_writer_scaling_ac8f1c6d{,_run2}.json` via
  `scripts/c2c_ranking.py`
- `12. Mixed read/write concurrency` from `results/baseline_concurrent_mixed.json`
  and its second run, `results/baseline_concurrent_mixed_run2.json`
- `14. Wrapper mutation profiles` from the `callgrind_annotate` listings under
  `results/callgrind_wrapper_mutations/` via `scripts/callgrind_wrapper_ranking.py`
- `15` the string-wrapper baselines at `170a4bc3`: the #929 writer arms from
  `results/baseline_writer_scaling_170a4bc3_{pin0-15,percore}{,_run2}.json` and
  the #730 readers-only sweep from
  `results/baseline_readers_only_writer_scaling_170a4bc3_{pin0-15,percore}{,_run2}.json`,
  with the per-reader cost and round checks from `scripts/reader_scaling_bounds.py`.
  (`13` is `scripts/reader_scaling_bounds.py --table`, not this file.)
- `16` the per-wrapper `perf c2c` contention ranking at `0c6b7832`, one
  subsection per writer arm, from
  `results/c2c_{str,bytes,blob}_writer_scaling_0c6b7832{,_run2}.json` via
  `scripts/c2c_ranking.py`
- `22` the #730 readers-only sweep re-measured at `7cd5140e`, from
  `results/baseline_readers_only_writer_scaling_7cd5140e_{pin0-15,percore}{,_run2}.json`
  beside the `170a4bc3` set §15 reads. A baseline, not an evaluation: those
  artifacts carry `readers_only.preregistration` `null`, so under
  `METHODOLOGY.md` §16.5 no cell has a verdict, and nothing in the section
  prints one. METHODOLOGY §16.3's floors are read as constants (AGENTS.md
  section 8.19).

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
    """Per-probe ns of one reader from the cell's aggregate reader rate — the
    runner's own estimator, the median over rounds of the harness's M ops/s."""
    m = cell.get("expanse_reader_mops_median")
    if not m:
        return None
    return readers / m * 1e3


def counters(results: Path, cell_name: str) -> dict | None:
    return load(results / f"counters_{cell_name}.json")


def role_event(art: dict | None, role: str, ev: str) -> dict | None:
    if not art:
        return None
    return art.get("roles", {}).get(role, {}).get("events", {}).get(ev)


# ---- 3. D1 -----------------------------------------------------------------
def d1() -> list[str]:
    out = ["## 3. Attribution — D1, readers under one writer (METHODOLOGY §4, §5)", "",
           "| cell | run | spin time (P0.1) | restarts | fallback | writer RFO / insert (P0.2) | writer HITM / insert | reader HITM / probe (vs alone) | reader cycles / probe (vs alone) | unattributed | verdict |",
           "|---|--:|---|---|---|---|---|---|---|---|---|"]
    any_rows = False
    for label, results, arm, ccell in D1:
        for run, fname in ((1, "step0/baseline_concurrent.json"), (2, "step0/baseline_concurrent_run2.json")):
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
            hitm_w = role_event(c2, "writer", "mem_load_l3_hit_retired.xsnp_hitm")
            hitm_w_txt = fmt_iv(hitm_w, 2) if pm(hitm_w) is not None else "`NOT_INSTRUMENTED`"
            c0 = counters(results, ccell.replace("_w1_r8", "_w0_r8"))
            def pair(ev: str, digits: int) -> str:
                a, b = role_event(c2, "reader", ev), role_event(c0, "reader", ev)
                if pm(a) is None:
                    return "`NOT_INSTRUMENTED`"
                if pm(b) is None:
                    return f"{pm(a):.{digits}f} (control not measured)"
                return f"{pm(a):.{digits}f} vs {pm(b):.{digits}f} alone"
            out.append(f"| `{label}` | {run} | {spin_txt} | {rs_txt} | 0 by construction | {rfo_txt} | {hitm_w_txt} | {pair('mem_load_l3_hit_retired.xsnp_hitm', 2)} | {pair('cycles', 0)} | {un_txt} | {verdict} |")
    if not any_rows:
        for i, (label, *_rest) in enumerate(D1):
            out.append(f"| `{label}` | — | {PENDING if i == 0 else 'pending'} | pending | 0 by construction | pending | pending | pending | pending | pending | pending |")
    return out


# ---- 4. D2 -----------------------------------------------------------------
def d2() -> list[str]:
    out = ["## 4. Attribution — D2, writers under load (METHODOLOGY §4, §5)", "",
           "| cell | context switches / insert (P0.3) | writer off-CPU share | cycles / insert | RFO / insert | HITM / insert | futex / insert | verdict |",
           "|---|---|---|---|---|---|---|---|"]
    any_rows = False
    for label, results, arm, w, ccell, note in D2:
        c = counters(results, ccell)
        art = load(results / "step0" / "baseline_concurrent.json")
        h = health_cell(art, arm, w, 8) if art else None  # H cells are R=8; handoffs at W>=2 there
        cs = role_event(c, "writer", "context-switches")
        rfo = role_event(c, "writer", "l2_rqsts.rfo_miss")
        futex = role_event(c, "writer", "syscalls:sys_enter_futex")
        if c is None and h is None:
            continue
        any_rows = True
        cs_txt = fmt_iv(cs, 3) if pm(cs) is not None else "`NOT_INSTRUMENTED`"
        # Off-CPU share: 1 - on-CPU ms per insert / wall ms per insert per writer
        # (task-clock is per thread; the wall time is the round's writer phase
        # divided by that writer's share of the fresh keys).
        off_txt = "`NOT_INSTRUMENTED`"
        tc = role_event(c, "writer", "task-clock")
        if c and pm(tc) is not None:
            shares = []
            for rr in c.get("rounds_raw", []):
                wo, ws = rr.get("write_ops"), rr.get("writer_elapsed_s")
                tcm = rr.get("per_op", {}).get("writer", {}).get("task-clock")
                if wo and ws and tcm is not None:
                    wall_ms = ws * 1e3 / (wo / w)
                    shares.append(1.0 - tcm / wall_ms)
            if shares:
                shares.sort()
                off_txt = f"{shares[len(shares)//2]:.0%} (median of {len(shares)} rounds)"
        cyc = role_event(c, "writer", "cycles")
        cyc_txt = fmt_iv(cyc, 0) if pm(cyc) is not None else "`NOT_INSTRUMENTED`"
        hitm = role_event(c, "writer", "mem_load_l3_hit_retired.xsnp_hitm")
        hitm_txt = fmt_iv(hitm, 2) if pm(hitm) is not None else "`NOT_INSTRUMENTED`"
        rfo_txt = fmt_iv(rfo, 2) if pm(rfo) is not None else "`NOT_INSTRUMENTED`"
        fut_txt = fmt_iv(futex, 3) if pm(futex) is not None else "`NOT_INSTRUMENTED`"
        _ = h
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
        out.append(f"| `{label}` | {cs_txt} | {off_txt} | {cyc_txt} | {rfo_txt} | {hitm_txt} | {fut_txt} | {verdict} |")
    if not any_rows:
        for i, (label, _r, _a, _w, _c, note) in enumerate(D2):
            v = note or "pending"
            out.append(f"| `{label}` | {PENDING if i == 0 else 'pending'} | pending | pending | pending | pending | pending or `NOT_INSTRUMENTED` | {v} |")
    return out


# ---- 5. counter spread -----------------------------------------------------
def spread() -> list[str]:
    out = ["## 5. The counter's own spread (P0.4)", "",
           "| suite | arm | W | spins ÷ read_ops run 1 | run 2 | ratio | restart run 1 | run 2 | ratio | verdict |",
           "|---|---|--:|--:|--:|--:|--:|--:|--:|---|"]
    any_rows = False
    for name, results in (("hot_comparison", HOT), ("masstree_comparison", MT)):
        a, b = load(results / "step0" / "baseline_concurrent.json"), load(results / "step0" / "baseline_concurrent_run2.json")
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


# ---- 6. ablations ----------------------------------------------------------
def ablations() -> list[str]:
    out = ["## 6. Ablations — the #789 features on C1 W=1 and C2 W=1 R=8 (`results/ablations.json`)", "",
           "| variant | W | R | Expanse inserts M/s [BCa 95%] | Expanse lookups M/s [BCa 95%] | vs `default` inserts | vs `default` lookups |",
           "|---|--:|--:|---|---|---|---|"]
    art = load(SUITE / "results" / "ablations.json")
    if art is None:
        out.append(f"| `default` | 1 | 0 | {PENDING} | no readers | — | — |")
        return out
    base = {(c["writers"], c["readers"]): c for c in art["cells"] if c["variant"] == "default"}

    def rel(iv: dict | None, ref: dict | None) -> str:
        if not iv or not ref or not pm(ref):
            return "—"
        r = pm(iv) / pm(ref)
        overlap = not (iv["ci_upper"] < ref["ci_lower"] or iv["ci_lower"] > ref["ci_upper"])
        return f"{r:.2f}×" + (" (intervals overlap)" if overlap else "")

    for c in art["cells"]:
        b = base.get((c["writers"], c["readers"]))
        wi, ri = c["expanse_writer_mops"], c["expanse_reader_mops"]
        ws = fmt_iv(wi, 2)
        rs = fmt_iv(ri, 2) if ri else "no readers"
        out.append(f"| `{c['variant']}` | {c['writers']} | {c['readers']} | {ws} | {rs} | "
                   f"{rel(wi, b and b['expanse_writer_mops'])} | {rel(ri, b and b['expanse_reader_mops']) if ri else '—'} |")
    return out


# ---- 6b. the string wrapper's reader mode ---------------------------------
HIGH_MODE_MOPS = 3.0


def ablations_str() -> list[str]:
    """`results/ablations_str.json`: the base engine's string C2 cell under
    `default` and `lock-padded`. The cell is bimodal per process (one round
    per process in the two-commit runs; here the harness's own round loop),
    so beside the BCa mean each row counts the rounds above
    `HIGH_MODE_MOPS` — a descriptive split at the gap between the two
    observed modes, not a gate."""
    out = ["### 6b. The string wrapper's reader mode — `lock-padded` on the base engine "
           "(`results/ablations_str.json`)", "",
           "| variant | W | R | Expanse inserts M/s [BCa 95%] | Expanse lookups M/s [BCa 95%] | "
           f"lookup rounds ≥ {HIGH_MODE_MOPS:.0f} M/s | lookups min–max |",
           "|---|--:|--:|---|---|--:|--:|"]
    art = load(SUITE / "results" / "ablations_str.json")
    if art is None:
        out.append(f"| `default` | 1 | 8 | {PENDING} | {PENDING} | — | — |")
        return out
    for c in art["cells"]:
        wi, ri = c["expanse_writer_mops"], c["expanse_reader_mops"]
        ws = fmt_iv(wi, 2)
        if ri:
            rs = fmt_iv(ri, 2)
            vals = [r["expanse_reader_mops"] for r in c["rounds_raw"] if r.get("expanse_reader_mops") is not None]
            high = f"{sum(v >= HIGH_MODE_MOPS for v in vals)} / {len(vals)}"
            span = f"{min(vals):.2f}–{max(vals):.2f}"
        else:
            rs, high, span = "no readers", "—", "—"
        out.append(f"| `{c['variant']}` | {c['writers']} | {c['readers']} | {ws} | {rs} | {high} | {span} |")
    return out


# ---- 10. native writer-scaling sweep (Phase 1.5D) -------------------------
# Verdict per cell, fixed before the first sweep ran: a W >= 2 cell scales iff
# the paired-BCa lower bound of C(W) clears 1.0 (AGENTS.md section 8.4); an
# interval straddling 1.0 is a BOUNDARY_RESULT; an interval wholly below 1.0 is
# RETROGRADE. `str` holds the writer mutex for the whole operation, so it is
# the alpha = 1 reference curve and never a gate cell.
def writer_scaling() -> list[str]:
    out = ["## 10. Writer scaling — the native Phase 1.5D sweep (`results/baseline_writer_scaling.json`)", "",
           "| arm | W | Expanse inserts M/s [BCa 95%] | C(W) [paired BCa 95%] | verdict | fallbacks / insert | contention / insert | largest causes (share of fallbacks) |",
           "|---|--:|---|---|---|--:|--:|---|"]
    art = load(SUITE / "results" / "baseline_writer_scaling.json")
    if art is None:
        out.append(f"| — | — | {PENDING} | pending | pending | pending | pending | pending |")
        return out
    where = "baseline_writer_scaling.json"
    for arm in ("map", "set", "str"):
        cells = sorted((c for c in need(art, "throughput", where) if c["arm"] == arm),
                       key=lambda c: c["writers"])
        for c in cells:
            w = c["writers"]
            at = f"{where} {arm} W={w}"
            mean = need(c, "expanse_writer_mops_mean", at)
            lo, hi = need(c, "writer_ci_lower", at), need(c, "writer_ci_upper", at)
            cn = need(c, "scaling_factor_c_n", at)
            cn_lo, cn_hi = need(c, "scaling_factor_c_n_ci_lower", at), need(c, "scaling_factor_c_n_ci_upper", at)
            if w == 1:
                verdict = "baseline"
            elif arm == "str":
                verdict = "reference, not gated"
            elif cn_lo >= 1.0:
                verdict = "`SCALES`"
            elif cn_hi >= 1.0:
                verdict = "`BOUNDARY_RESULT`"
            else:
                verdict = "`RETROGRADE`"
            shares = need(c, "fallback_cause_share", at)
            per_ins = need(c, "fallback_causes_per_insert", at)
            top = sorted(((k, v) for k, v in shares.items() if v > 0), key=lambda kv: -kv[1])
            listed = ", ".join(f"{k} {v * 100:.1f}%" for k, v in top[:3])
            causes = listed or ("none — no OLC path" if arm == "str" else "none")
            c_str = "1.00 (by definition)" if w == 1 else f"{cn:.2f} [{cn_lo:.2f}, {cn_hi:.2f}]"
            out.append(f"| `{arm}` | {w} | {mean:.2f} [{lo:.2f}, {hi:.2f}] | {c_str} | {verdict} | "
                       f"{need(c, 'fallback_rate', at) * 100:.2f}% | {per_ins['contention'] * 100:.2f}% | {causes} |")

    # Step 4.0 counters (#837): how contention splits, what arriving writers pay
    # at a closed gate, and which branch mutation the branch_split bucket holds.
    # The cycle columns stay in cycles: the artifact carries no cycle rate, and
    # the counters build is untimed, so no share of wall time can be derived.
    out += ["", "**Step 4.0 counters** — from the separate `occ-stats` build, summed over the eight rounds:", "",
            "| arm | W | contention / insert | gate-closed share of contention | retry-exhausted | restarts / insert "
            "| gate-blocked entries / insert | gate-wait cycles / insert | drain cycles / fallback "
            "| branch_split: subarray · linear · prefix · remove · upgrade |",
            "|---|--:|--:|--:|--:|--:|--:|--:|--:|---|"]
    for arm in ("map", "set"):
        cells = sorted((c for c in need(art, "throughput", where) if c["arm"] == arm),
                       key=lambda c: c["writers"])
        for c in cells:
            w = c["writers"]
            at = f"{where} {arm} W={w}"
            cont = need(c, "contention_subsets_total", at)
            total = cont["gate_closed"] + cont["retry_exhausted"]
            share = f"{cont['gate_closed'] / total * 100:.1f}%" if total else "—"
            bs = need(c, "branch_split_subsets_total", at)
            bs_total = sum(bs.values())
            split = " · ".join(
                f"{bs[k] / bs_total * 100:.1f}%" if bs_total else "—"
                for k in ("subarray", "linear", "prefix", "remove", "upgrade"))
            out.append(
                f"| `{arm}` | {w} | {need(c, 'fallback_causes_per_insert', at)['contention'] * 100:.2f}% | {share} "
                f"| {cont['retry_exhausted']:,} | {need(c, 'lock_restarts_per_insert', at):.4f} "
                f"| {need(c, 'gate_blocked_entries_per_insert', at) * 100:.1f}% "
                f"| {need(c, 'gate_wait_cycles_per_insert', at):,.0f} "
                f"| {need(c, 'quiesce_drain_cycles_per_fallback', at):,.0f} | {split} |")
    return out


# ---- 12. mixed read/write concurrency --------------------------------------
# Report-only: no gate is pre-registered on this instrument, so the section
# prints intervals and no verdict. The second artifact is a second dispatch of
# the same commit, and a level or a C(N) is read from both runs (rule 18).
MIXED_RUNS = ("baseline_concurrent_mixed.json", "baseline_concurrent_mixed_run2.json")
SYNC32 = "sync32"


def _mops_digits(mean: float) -> int:
    """Decimals for a rate in M ops/s: three significant figures, down to 0.001."""
    return 0 if mean >= 100 else 1 if mean >= 10 else 2 if mean >= 1 else 3


def _mops_iv(cell: dict, prefix: str, at: str) -> str:
    mean, lo, hi = (need(cell, prefix + k, at) / 1e6 for k in ("mean", "ci_lower", "ci_upper"))
    d = _mops_digits(mean)
    return f"{mean:.{d}f} [{lo:.{d}f}, {hi:.{d}f}]"


def _c_n_iv(cell: dict, at: str) -> str:
    if need(cell, "threads", at) == 1:
        return "—"
    mean, lo, hi = (need(cell, "scaling_c_n_" + k, at) for k in ("mean", "ci_lower", "ci_upper"))
    return f"{mean:.2f} [{lo:.2f}, {hi:.2f}]"


def _pct(p: float) -> str:
    return f"{p:.3g}%" if p >= 0.001 else f"{p:.4f}%"


def mixed_concurrency() -> list[str]:
    out = ["## 12. Mixed read/write concurrency — `benches/concurrency.rs` "
           "(`results/baseline_concurrent_mixed.json`)", ""]
    arts = [load(SUITE / "results" / name) for name in MIXED_RUNS]
    cells = [None if art is None else
             {(c["engine_key"], c["workload"], c["threads"]): c for c in need(art, "throughput", name)}
             for art, name in zip(arts, MIXED_RUNS)]
    if cells[0] is not None and cells[1] is not None and set(cells[0]) != set(cells[1]):
        raise SystemExit(f"{MIXED_RUNS[0]} and {MIXED_RUNS[1]} cover different cells: "
                         f"{sorted(set(cells[0]) ^ set(cells[1]))}")

    facts = []
    for label, art, name in zip(("run 1", "run 2"), arts, MIXED_RUNS):
        if art is None:
            facts.append(f"{label} {PENDING}")
            continue
        prov = need(art, "provenance", name)
        foreign = max(need(need(c, "load", name), "foreign_busy_cpus", name)
                      for c in need(art, "throughput", name))
        facts.append(f"{label} `{name}` at `{str(need(prov, 'commit', name))[:8]}`, pin "
                     f"`{need(prov, 'core_pin', name)}`, {need(prov, 'rounds', name)} rounds, "
                     f"largest foreign busy CPUs over a group {foreign:.2f}")
    out += ["Artifacts: " + "; ".join(facts) + ".", ""]

    out += ["| arm | workload | N | total M ops/s, run 1 [BCa 95%] | run 2 | C(N), run 1 [paired BCa 95%] | run 2 |",
            "|---|---|--:|---|---|---|---|"]
    if cells[0] is None:
        out.append(f"| — | — | — | {PENDING} | pending | pending | pending |")
    else:
        for key, c in cells[0].items():
            if key[0] == SYNC32:
                continue
            at, at2 = f"{MIXED_RUNS[0]} {key}", f"{MIXED_RUNS[1]} {key}"
            other = None if cells[1] is None else cells[1][key]
            out.append(
                f"| `{c['engine']}` | {need(c, 'read_pct', at)}% read | {key[2]} "
                f"| {_mops_iv(c, 'total_ops_s_', at)} "
                f"| {'pending' if other is None else _mops_iv(other, 'total_ops_s_', at2)} "
                f"| {_c_n_iv(c, at)} | {'pending' if other is None else _c_n_iv(other, at2)} |")

    out += ["", "| run | writer duty | readers | validated reads M/s [BCa 95%] | writes M/s [BCa 95%] "
                "| Busy rate | refused writes |",
            "|---|---|--:|---|---|--:|--:|"]
    for label, ix, name in zip(("run 1", "run 2"), cells, MIXED_RUNS):
        rows = [] if ix is None else [(k, c) for k, c in ix.items() if k[0] == SYNC32]
        if not rows:
            out.append(f"| {label} | — | — | {PENDING if ix is None else 'not measured'} "
                       f"| pending | pending | pending |")
            continue
        for key, c in rows:
            at = f"{name} {key}"
            duty = key[1].split(" / ")[0].removeprefix("writer ").removesuffix(" duty")
            out.append(
                f"| {label} | {duty} | {key[2]} | {_mops_iv(c, 'read_ops_s_', at)} "
                f"| {_mops_iv(c, 'write_ops_s_', at)} | {_pct(need(c, 'busy_pct', at))} "
                f"| {need(c, 'refused_writes', at):,} |")
    return out


# ---- 11.8 arm (a), lock-padded and the combination, one process per cell ----
# The #930 re-run at `bad1bd3d` (METHODOLOGY.md section 15): three two-build
# comparisons, two runs each, every timed cell in a harness process of its own.
# The per-run verdict is the artifact's own (METHODOLOGY section 11,
# `writer_scaling.py::compute_paired_scaling_ratios`); the two-run label is
# rule 18's — the same verdict on the same cell in both runs, else
# INCONCLUSIVE. Every other column is derived from `rounds_raw` and the load
# snapshots, and the README labels it so.
PERCELL_COMMIT = "bad1bd3d"
MULTICELL_COMMIT = "726b01fc"
# (subsection, artifact stem, variant build, how the README names it)
PERCELL_BUILDS = (
    ("11.8.1", "combined_alloc_padded_writer_scaling", "ablation-sharded-alloc,lock-padded",
     "both changes together, not pre-registered"),
    ("11.8.2", "ablation_alloc_writer_scaling", "ablation-sharded-alloc", "arm a"),
    ("11.8.3", "padded_writer_scaling", "lock-padded", "not pre-registered"),
)
# The §11.7 artifacts: multi-cell processes, no `cell_isolation` field.
MULTICELL_BUILDS = (
    ("ablation_alloc_writer_scaling", "ablation-sharded-alloc"),
    ("padded_writer_scaling", "lock-padded"),
)
PERCELL_ARMS = ("map", "set", "str")


def _pc_runs(stem: str, commit: str, variant: str, isolation: str | None) -> list[dict] | None:
    """Both runs of one comparison, or None if either is absent. A present
    artifact at another commit, isolation or variant is an error (section 8.1)."""
    arts = []
    for suffix in ("", "_run2"):
        path = SUITE / "results" / f"{stem}_{commit}{suffix}.json"
        art = load(path)
        if art is None:
            return None
        prov = need(art, "provenance", path.name)
        if prov.get("commit") != commit or prov.get("cell_isolation") != isolation:
            raise SystemExit(f"{path.name}: commit {prov.get('commit')!r}, cell_isolation "
                             f"{prov.get('cell_isolation')!r}; expected {commit!r}, {isolation!r}")
        names = {c["variant_name"] for c in need(art, "comparison", path.name)}
        if names != {variant}:
            raise SystemExit(f"{path.name}: compares {sorted(names)}, expected {variant!r}")
        arts.append(art)
    return arts


def _pc_cell(art: dict, key: str, arm: str, w: int) -> dict:
    hits = [c for c in need(art, key, key) if c["arm"] == arm and c["writers"] == w]
    if len(hits) != 1:
        raise SystemExit(f"{key}: {len(hits)} cells for {arm} W={w}, expected 1")
    return hits[0]


def _pc_writers(art: dict, arm: str) -> list[int]:
    return sorted(c["writers"] for c in art["throughput"] if c["arm"] == arm)


def _pc_cmp(art: dict, arm: str, w: int) -> dict:
    hits = [c for c in art["comparison"] if c["arm"] == arm]
    if len(hits) != 1 or hits[0].get("ratio_direction") != "c_variant_over_c_default":
        raise SystemExit(f"comparison for {arm}: expected one C_variant / C_default entry")
    return hits[0]["per_writer"][str(w)]


def _pc_iv(m: float, lo: float, hi: float, digits: int, method: str = "bca") -> str:
    tail = "" if method == "bca" else f" ({method})"
    return f"{m:.{digits}f} [{lo:.{digits}f}, {hi:.{digits}f}]{tail}"


def _pc_two_run(v1: str, v2: str) -> str:
    if v1 != v2:
        return "`INCONCLUSIVE` (verdicts differ)"
    if v1 == "INCONCLUSIVE":
        return "`INCONCLUSIVE`"
    return f"`{v1}` in both runs"


def _pc_rounds(art: dict, key: str, arm: str, w: int) -> dict[int, dict]:
    rows = _pc_cell(art, key, arm, w)["rounds_raw"]
    out = {int(r["round"]): r for r in rows}
    if len(out) != len(rows):
        raise SystemExit(f"{key} {arm} W={w}: duplicate round in rounds_raw")
    return out


def _pc_throughput_ratio(art: dict, arm: str, w: int) -> str:
    """Per-round T_variant / T_default paired by round index, BCa 95%."""
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    from bca_bootstrap import bca_bootstrap_ci_with_method
    d = _pc_rounds(art, "throughput", arm, w)
    v = _pc_rounds(art, "throughput_variant", arm, w)
    if d.keys() != v.keys():
        raise SystemExit(f"{arm} W={w}: default and variant rounds differ")
    ratios = [v[k]["writer_mops"] / d[k]["writer_mops"] for k in sorted(d)]
    m, lo, hi, method = bca_bootstrap_ci_with_method(ratios, confidence=0.95)
    return _pc_iv(m, lo, hi, 3, method)


def writer_scaling_percell() -> list[str]:
    from statistics import median

    out: list[str] = []
    runs = {stem: _pc_runs(stem, PERCELL_COMMIT, variant, "process")
            for _, stem, variant, _ in PERCELL_BUILDS}
    if any(a is None for a in runs.values()):
        return ["#### 11.8.1 Verdicts", "", f"| build | {PENDING} |", "|---|---|"]

    # Verdicts, per build.
    for sub, stem, variant, label in PERCELL_BUILDS:
        a1, a2 = runs[stem]
        out += [f"#### {sub} `{variant}` ({label}) — verdicts", "",
                "| arm | W | run 1 | run 1 verdict | run 2 | run 2 verdict | intervals overlap | two-run verdict |",
                "|---|--:|---|---|---|---|---|---|"]
        for arm in PERCELL_ARMS:
            for w in _pc_writers(a1, arm):
                if w == 1:
                    continue
                c1, c2 = _pc_cmp(a1, arm, w), _pc_cmp(a2, arm, w)
                ivs = []
                for c in (c1, c2):
                    ivs.append(_pc_iv(c["ratio_c_variant_over_c_default_mean"], c["ratio_ci_lower"],
                                      c["ratio_ci_upper"], 4, c.get("ratio_ci_method", "unlabelled")))
                overlap = (c1["ratio_ci_lower"] <= c2["ratio_ci_upper"]
                           and c2["ratio_ci_lower"] <= c1["ratio_ci_upper"])
                out.append(f"| `{arm}` | {w} | {ivs[0]} | `{c1['verdict']}` | {ivs[1]} | `{c2['verdict']}` "
                           f"| {'yes' if overlap else '**no**'} | {_pc_two_run(c1['verdict'], c2['verdict'])} |")
        out.append("")

    # Absolute writer throughput at W = 1 and W = 8, and the paired throughput ratio.
    out += ["#### 11.8.4 Throughput per build, one process per cell", "",
            "| build | arm | W | run 1 default | run 1 variant | run 2 default | run 2 variant |",
            "|---|---|--:|---|---|---|---|"]
    for _, stem, variant, _ in PERCELL_BUILDS:
        for arm in PERCELL_ARMS:
            for w in (1, 8):
                cols = []
                for art in runs[stem]:
                    for key in ("throughput", "throughput_variant"):
                        c = _pc_cell(art, key, arm, w)
                        cols.append(_pc_iv(c["expanse_writer_mops_mean"], c["writer_ci_lower"],
                                           c["writer_ci_upper"], 2, c.get("writer_ci_method", "unlabelled")))
                out.append(f"| `{variant}` | `{arm}` | {w} | " + " | ".join(cols) + " |")
    out += ["", "| build | arm | W | T_variant ÷ T_default, run 1 | T_variant ÷ T_default, run 2 |",
            "|---|---|--:|---|---|"]
    for _, stem, variant, _ in PERCELL_BUILDS:
        for arm in PERCELL_ARMS:
            for w in (1, 8):
                out.append(f"| `{variant}` | `{arm}` | {w} | "
                           + " | ".join(_pc_throughput_ratio(a, arm, w) for a in runs[stem]) + " |")
    out.append("")

    # Schedule position: the combined build's W = 8 `map` variant cell here, and
    # the two single builds' in the §11.7 multi-cell artifacts.
    comb = runs[PERCELL_BUILDS[0][1]]
    out += ["#### 11.8.5 Schedule position, W = 8 `map` variant cell", "",
            "| position in the round's cell order | run 1 rounds | run 1 median M ops/s | run 2 rounds | run 2 median M ops/s |",
            "|--:|---|--:|---|--:|"]
    by = []
    for art in comb:
        pos: dict[int, list[dict]] = {}
        for r in _pc_rounds(art, "throughput_variant", "map", 8).values():
            pos.setdefault(int(r["position"]), []).append(r)
        by.append(pos)
    for p in sorted(set(by[0]) | set(by[1])):
        cells = []
        for pos in by:
            rs = sorted(pos.get(p, []), key=lambda r: r["round"])
            cells.append(", ".join(str(r["round"]) for r in rs) or "—")
            cells.append(f"{median(r['writer_mops'] for r in rs):.2f}" if rs else "—")
        out.append(f"| {p} | " + " | ".join(cells) + " |")
    out += ["", f"| build (`{MULTICELL_COMMIT}`, one process per build per round) | position in the process | "
            "run 1 rounds | run 1 median M ops/s | run 2 rounds | run 2 median M ops/s |",
            "|---|--:|---|--:|---|--:|"]
    for stem, variant in MULTICELL_BUILDS:
        arts = _pc_runs(stem, MULTICELL_COMMIT, variant, None)
        if arts is None:
            out.append(f"| `{variant}` | — | {PENDING} | — | — | — |")
            continue
        by = []
        for art in arts:
            pos = {}
            for r in _pc_rounds(art, "throughput_variant", "map", 8).values():
                pos.setdefault(int(r["position"]), []).append(r)
            by.append(pos)
        for p in sorted(set(by[0]) | set(by[1])):
            cells = []
            for pos in by:
                rs = sorted(pos.get(p, []), key=lambda r: r["round"])
                cells.append(", ".join(str(r["round"]) for r in rs) or "—")
                cells.append(f"{median(r['writer_mops'] for r in rs):.2f}" if rs else "—")
            out.append(f"| `{variant}` | {p} | " + " | ".join(cells) + " |")
    out.append("")

    # The `set` W = 1 control under arm (a), per round.
    sharded = runs[PERCELL_BUILDS[1][1]]
    out += ["#### 11.8.6 `set` W = 1 under `ablation-sharded-alloc`, per round", "",
            "| run | round | variant position | variant M ops/s | default position | default M ops/s | variant ÷ default |",
            "|--:|--:|--:|--:|--:|--:|--:|"]
    for i, art in enumerate(sharded, start=1):
        d = _pc_rounds(art, "throughput", "set", 1)
        v = _pc_rounds(art, "throughput_variant", "set", 1)
        for k in sorted(d):
            out.append(f"| {i} | {k} | {v[k]['position']} | {v[k]['writer_mops']:.2f} | {d[k]['position']} "
                       f"| {d[k]['writer_mops']:.2f} | {v[k]['writer_mops'] / d[k]['writer_mops']:.3f} |")
    out.append("")

    # The combination against the product of the single-change C(W) ratio means.
    out += ["#### 11.8.7 The combination against the two single changes", "",
            "| arm | W | run | combined C(W) ratio | `ablation-sharded-alloc` | `lock-padded` | product of the two (derived) |",
            "|---|--:|--:|--:|--:|--:|--:|"]
    for arm in ("map", "set"):
        for i in (0, 1):
            vals = [_pc_cmp(runs[stem][i], arm, 8)["ratio_c_variant_over_c_default_mean"]
                    for _, stem, _, _ in PERCELL_BUILDS]
            out.append(f"| `{arm}` | 8 | {i + 1} | {vals[0]:.4f} | {vals[1]:.4f} | {vals[2]:.4f} "
                       f"| {vals[1] * vals[2]:.4f} |")
    out.append("")

    # The default build's `str` W = 1 rate, before and after one process per cell.
    out += ["#### 11.8.8 The default build's `str` W = 1 rate", "",
            "| commit | cells per process | artifact | default `str` W = 1 M ops/s [BCa 95%] |",
            "|---|---|---|---|"]
    rows = [(MULTICELL_COMMIT, "several", stem, variant, None) for stem, variant in MULTICELL_BUILDS]
    rows += [(PERCELL_COMMIT, "one", stem, variant, "process") for _, stem, variant, _ in PERCELL_BUILDS]
    for commit, per, stem, variant, iso in rows:
        arts = _pc_runs(stem, commit, variant, iso)
        if arts is None:
            continue
        for suffix, art in zip(("", "_run2"), arts):
            c = _pc_cell(art, "throughput", "str", 1)
            out.append(f"| `{commit}` | {per} | `{stem}_{commit}{suffix}.json` | "
                       f"{_pc_iv(c['expanse_writer_mops_mean'], c['writer_ci_lower'], c['writer_ci_upper'], 2, c.get('writer_ci_method', 'unlabelled'))} |")
    out.append("")

    # Host load (section 8.17): the cell windows, and the phase snapshots taken
    # back to back, whose `*_since_prev` fields cover no measurable wall time.
    out += ["#### 11.8.9 Host load", "",
            "| artifact | cell `foreign_busy_cpus`, min – max | peak `load1` | largest `load1` shift between consecutive snapshots "
            "| back-to-back phase snapshots: `own` / `foreign` since the previous one |",
            "|---|---|--:|--:|---|"]

    def num(x) -> str:
        return "null" if x is None else f"{x:.2f}"

    for _, stem, _, _ in PERCELL_BUILDS:
        for suffix, art in zip(("", "_run2"), runs[stem]):
            name = f"{stem}_{PERCELL_COMMIT}{suffix}.json"
            fb = [need(c["load"], "foreign_busy_cpus", name)
                  for key in ("throughput", "throughput_variant") for c in art[key]]
            loads = art["provenance"]["loads"]
            peak = max(l["load1"] for l in loads)
            shift = max(abs(b["load1"] - a["load1"]) for a, b in zip(loads, loads[1:]))
            back = []
            for a, b in zip(loads, loads[1:]):
                if b["monotonic_s"] == a["monotonic_s"]:
                    if b["child_cpu_s"] != a["child_cpu_s"]:
                        raise SystemExit(f"{name}: {b['label']} shares monotonic_s but not child_cpu_s")
                    back.append(f"{num(b['own_busy_cpus_since_prev'])} / {num(b['foreign_busy_cpus_since_prev'])}")
            out.append(f"| `{name}` | {min(fb):.2f} – {max(fb):.2f} | {peak:.2f} | {shift:.2f} | "
                       + "; ".join(back) + " |")
    return out


# ---- 11.9 contention ranking at ac8f1c6d (perf c2c, one thread per P-core) ----
# Two `writer_scaling_diagnostic` CI dispatches (#930). The run URLs are not in
# the artifacts, so they are named here beside the file each one produced.
DIAGNOSTIC_COMMIT = "ac8f1c6d"
DIAGNOSTIC_RUNS = (
    ("diagnostic_writer_scaling_ac8f1c6d.json", "https://github.com/orieg/expanse/actions/runs/35015212785"),
    ("diagnostic_writer_scaling_ac8f1c6d_run2.json", "https://github.com/orieg/expanse/actions/runs/35015232548"),
)
ISSUE_930 = "[#930](https://github.com/orieg/expanse/issues/930)"


def contention_ranking() -> list[str]:
    """README section 11.9, rendered by `scripts/c2c_ranking.py` from both runs."""
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import c2c_ranking

    runs = []
    for name, url in DIAGNOSTIC_RUNS:
        art = load(SUITE / "results" / name)
        if art is None:
            return ["#### 11.9.1 The recordings", "", "| | run 1 | run 2 |", "|---|---|---|",
                    f"| artifact | pending ({ISSUE_930}) | pending ({ISSUE_930}) |"]
        commit = need(need(art, "provenance", name), "commit", name)
        if commit != DIAGNOSTIC_COMMIT:
            raise SystemExit(f"{name}: measured at {commit}, section 11.9 reads {DIAGNOSTIC_COMMIT}")
        runs.append((name, url, art))
    return c2c_ranking.render(runs, section="11.9")


def wrapper_profiles() -> list[str]:
    """README section 14, rendered by `scripts/callgrind_wrapper_ranking.py` (#929)."""
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import callgrind_wrapper_ranking

    return callgrind_wrapper_ranking.render_committed()


# ---- 15. string-wrapper baselines at 170a4bc3 -------------------------------
# PR 10 of the concurrency plan: the #929 writer arms and the #730 readers-only
# sweep, two CI dispatches per suite per pin at one commit. A baseline: no gate
# is evaluated and no verdict is printed. Cross-run and cross-pin columns state
# a direction only where both runs agree (docs/BENCHMARKING.md rule 18), and
# they never print a ratio of two unpaired levels, which would carry no interval.
BASELINE_COMMIT = "170a4bc3"
BASELINE_ROUNDS = 8
# (pin as applied, file-name tag, how the README names it)
BASELINE_PINS = (
    ("0-15", "pin0-15", "`0-15`"),
    ("0,2,4,6,8,10,12,14", "percore", "per-core"),
)
# (artifact stem, engine commit, pin tag, run) -> CI run. The run URLs are not
# in the artifacts, so they are named here beside the file each one produced.
# The commit is part of the key because the readers-only sweep has since been
# re-measured at a second head (section 22), and one map keeps every run id in
# one place (AGENTS.md section 8.18).
BASELINE_RUNS = {
    ("baseline_writer_scaling", "170a4bc3", "pin0-15", 1): 35021552023,
    ("baseline_writer_scaling", "170a4bc3", "pin0-15", 2): 35021581528,
    ("baseline_writer_scaling", "170a4bc3", "percore", 1): 35021610717,
    ("baseline_writer_scaling", "170a4bc3", "percore", 2): 35021636713,
    ("baseline_readers_only_writer_scaling", "170a4bc3", "pin0-15", 1): 35021567680,
    ("baseline_readers_only_writer_scaling", "170a4bc3", "pin0-15", 2): 35021596408,
    ("baseline_readers_only_writer_scaling", "170a4bc3", "percore", 1): 35021624186,
    ("baseline_readers_only_writer_scaling", "170a4bc3", "percore", 2): 35021650065,
    ("baseline_readers_only_writer_scaling", "7cd5140e", "pin0-15", 1): 35548107965,
    ("baseline_readers_only_writer_scaling", "7cd5140e", "percore", 1): 35548333424,
    ("baseline_readers_only_writer_scaling", "7cd5140e", "pin0-15", 2): 35548554236,
    ("baseline_readers_only_writer_scaling", "7cd5140e", "percore", 2): 35548780869,
}
BASELINE_WRITER_ARMS = ("map", "set", "str", "bytes", "blob")
BASELINE_READER_ARMS = ("map", "set", "str")
ISSUE_730 = "[#730](https://github.com/orieg/expanse/issues/730)"
ISSUE_929 = "[#929](https://github.com/orieg/expanse/issues/929)"


def _bl_name(stem: str, tag: str, run: int, commit: str = BASELINE_COMMIT) -> str:
    return f"{stem}_{commit}_{tag}{'' if run == 1 else '_run2'}.json"


def _bl_artifacts(stem: str, commit: str = BASELINE_COMMIT) -> dict[tuple[str, int], dict] | None:
    """Every (pin tag, run) artifact of one suite at one head, or None if any is absent.

    A present artifact at another commit, pin, isolation or round count is an
    error (section 8.1), never a row.
    """
    out = {}
    for pin, tag, _ in BASELINE_PINS:
        for run in (1, 2):
            name = _bl_name(stem, tag, run, commit)
            art = load(SUITE / "results" / name)
            if art is None:
                return None
            prov = need(art, "provenance", name)
            got = (prov.get("commit"), prov.get("core_pin"), prov.get("cell_isolation"))
            if got != (commit, pin, "process"):
                raise SystemExit(f"{name}: (commit, core_pin, cell_isolation) = {got}; expected "
                                 f"({commit!r}, {pin!r}, 'process')")
            for c in need(art, "throughput", name):
                if c["rounds"] != BASELINE_ROUNDS or len(c["rounds_raw"]) != BASELINE_ROUNDS:
                    raise SystemExit(f"{name}: {c['arm']} cell has {c['rounds']} rounds, "
                                     f"expected {BASELINE_ROUNDS}")
            out[(tag, run)] = art
    return out


def _bl_cell(art: dict, arm: str, key: str, n: int) -> dict:
    hits = [c for c in art["throughput"] if c["arm"] == arm and c[key] == n]
    if len(hits) != 1:
        raise SystemExit(f"{len(hits)} cells for {arm} {key}={n}, expected 1")
    return hits[0]


def _bl_iv(c: dict, mean: str, lo: str, hi: str, method: str, digits: int) -> str:
    return _pc_iv(need(c, mean, mean), need(c, lo, lo), need(c, hi, hi), digits,
                  c.get(method) or "unlabelled")


def _bl_overlap(a: tuple[float, float], b: tuple[float, float]) -> str:
    return "yes" if a[0] <= b[1] and b[0] <= a[1] else "**no**"


def _bl_vs(ivs: list[tuple[float, float]], ref: float) -> str:
    """Where both runs' intervals sit against `ref`, stated only if they agree."""
    if all(lo > ref for lo, _ in ivs):
        return f"above {ref:g} in both runs"
    if all(hi < ref for _, hi in ivs):
        return f"below {ref:g} in both runs"
    return "not the same in both runs"


def _bl_pin_direction(pairs: list[tuple[tuple[float, float], tuple[float, float]]]) -> str:
    """Per-core against `0-15`, per run: stated only where both runs' intervals are disjoint the same way."""
    if all(pc[0] > p15[1] for pc, p15 in pairs):
        return "per-core higher in both runs"
    if all(pc[1] < p15[0] for pc, p15 in pairs):
        return "per-core lower in both runs"
    return "not the same in both runs"


def _bl_ns_per_probe(cell: dict) -> tuple[list[float], dict]:
    """Per round, each reader's own loop time over its probes, averaged over readers; with its BCa interval.

    Each reader makes one probe per prefilled key at W = 0 (checked, not assumed).
    """
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import reader_scaling_bounds

    prefill, readers = int(cell["prefill"]), int(cell["readers"])
    series = []
    for r in sorted(cell["rounds_raw"], key=lambda x: x["round"]):
        if int(r["reader_ops"]) != readers * prefill or len(r["reader_thread_elapsed_s"]) != readers:
            raise SystemExit(f"{cell['arm']} R={readers} round {r['round']}: {r['reader_ops']} probes over "
                             f"{len(r['reader_thread_elapsed_s'])} reader times, expected {readers} x {prefill}")
        times = r["reader_thread_elapsed_s"]
        series.append(sum(times) / len(times) * 1e9 / prefill)
    return series, reader_scaling_bounds.per_arm_interval(series)


def string_wrapper_baselines() -> list[str]:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import reader_scaling_bounds

    writers = _bl_artifacts("baseline_writer_scaling")
    readers = _bl_artifacts("baseline_readers_only_writer_scaling")
    if writers is None or readers is None:
        return ["#### 15.1 Artifacts, runs and host load", "", "| suite | artifact |", "|---|---|",
                f"| writer arms | pending ({ISSUE_929}) |", f"| readers-only | pending ({ISSUE_730}) |"]

    out = ["#### 15.1 Artifacts, runs and host load", "",
           "| suite | pin | run | artifact | CI run | load snapshots | `load1` at start, peak | largest `load1` shift "
           "between consecutive snapshots | cell `foreign_busy_cpus`, min – max | governor on the pinned CPUs |",
           "|---|---|--:|---|---|--:|---|--:|---|---|"]
    for stem, suite, arts in (("baseline_writer_scaling", "`writer_scaling`", writers),
                              ("baseline_readers_only_writer_scaling", "`writer_scaling_readers_only`", readers)):
        for _, tag, label in BASELINE_PINS:
            for run in (1, 2):
                art = arts[(tag, run)]
                name = _bl_name(stem, tag, run)
                prov = art["provenance"]
                loads = prov["loads"]
                peak = max(l["load1"] for l in loads)
                shift = max((abs(b["load1"] - a["load1"]) for a, b in zip(loads, loads[1:])), default=0.0)
                fb = [need(c["load"], "foreign_busy_cpus", name) for c in art["throughput"]]
                govs = sorted(set(prov["host"]["scaling_governor_by_cpu"].values()))
                rid = BASELINE_RUNS[(stem, BASELINE_COMMIT, tag, run)]
                out.append(f"| {suite} | {label} | {run} | `results/{name}` | "
                           f"[{rid}](https://github.com/orieg/expanse/actions/runs/{rid}) | {len(loads)} | "
                           f"{loads[0]['load1']:.2f}, {peak:.2f} | {shift:.2f} | {min(fb):.2f} – {max(fb):.2f} | "
                           f"{', '.join(f'`{g}`' for g in govs)} |")
    out.append("")

    # Writer arms: the levels at W = 1 and W = 8, and C(W) at every W >= 2.
    out += ["#### 15.2 Writer arms — levels and C(W)", "",
            "| arm | workload | pin | run | W = 1 M ops/s [BCa 95%] | W = 8 M ops/s [BCa 95%] "
            "| C(2) [paired BCa 95%] | C(4) [paired BCa 95%] | C(8) [paired BCa 95%] |",
            "|---|---|---|--:|---|---|---|---|---|"]
    for arm in BASELINE_WRITER_ARMS:
        for _, tag, label in BASELINE_PINS:
            for run in (1, 2):
                art = writers[(tag, run)]
                w1, w8 = _bl_cell(art, arm, "writers", 1), _bl_cell(art, arm, "writers", 8)
                cols = [_bl_iv(c, "expanse_writer_mops_mean", "writer_ci_lower", "writer_ci_upper",
                               "writer_ci_method", 2) for c in (w1, w8)]
                cols += [_bl_iv(_bl_cell(art, arm, "writers", w), "scaling_factor_c_n_mean",
                                "scaling_factor_c_n_ci_lower", "scaling_factor_c_n_ci_upper",
                                "scaling_factor_c_n_ci_method", 3) for w in (2, 4, 8)]
                out.append(f"| `{arm}` | `{w1['workload_id']}` | {label} | {run} | " + " | ".join(cols) + " |")
    out.append("")

    def w_iv(art: dict, arm: str, w: int, what: str) -> tuple[float, float]:
        c = _bl_cell(art, arm, "writers", w)
        if what == "level":
            return c["writer_ci_lower"], c["writer_ci_upper"]
        return c["scaling_factor_c_n_ci_lower"], c["scaling_factor_c_n_ci_upper"]

    out += ["#### 15.3 Writer arms — run against run, and pin against pin", "",
            "| arm | pin | W = 1 intervals overlap across runs | W = 8 intervals overlap across runs "
            "| C(8) intervals overlap across runs | C(8) against 1.0 |",
            "|---|---|---|---|---|---|"]
    for arm in BASELINE_WRITER_ARMS:
        for _, tag, label in BASELINE_PINS:
            a1, a2 = writers[(tag, 1)], writers[(tag, 2)]
            out.append(f"| `{arm}` | {label} | {_bl_overlap(w_iv(a1, arm, 1, 'level'), w_iv(a2, arm, 1, 'level'))} "
                       f"| {_bl_overlap(w_iv(a1, arm, 8, 'level'), w_iv(a2, arm, 8, 'level'))} "
                       f"| {_bl_overlap(w_iv(a1, arm, 8, 'c'), w_iv(a2, arm, 8, 'c'))} "
                       f"| {_bl_vs([w_iv(a1, arm, 8, 'c'), w_iv(a2, arm, 8, 'c')], 1.0)} |")
    out += ["", "| arm | W = 1 level, per-core against `0-15` | W = 8 level, per-core against `0-15` "
            "| C(8), per-core against `0-15` |", "|---|---|---|---|"]
    for arm in BASELINE_WRITER_ARMS:
        cols = []
        for w, what in ((1, "level"), (8, "level"), (8, "c")):
            cols.append(_bl_pin_direction([(w_iv(writers[("percore", run)], arm, w, what),
                                            w_iv(writers[("pin0-15", run)], arm, w, what)) for run in (1, 2)]))
        out.append(f"| `{arm}` | " + " | ".join(cols) + " |")
    out.append("")

    # Readers-only: the levels at R = 1 and R = 8, S(8), and the per-reader cost.
    out += ["#### 15.4 Readers-only — levels, S(8) and per-reader cost", "",
            "| arm | workload | pin | run | R = 1 reader M ops/s [BCa 95%] | R = 8 reader M ops/s [BCa 95%] "
            "| S(8) [paired BCa 95%] | ns per probe per reader, R = 1 [BCa 95%] | ns per probe per reader, R = 8 [BCa 95%] "
            "| slowest over mean reader loop, R = 8 rounds (`max_over_mean_bias`), min – max "
            "| R = 8 rounds beyond 3 MADs (`round_outliers`) |",
            "|---|---|---|--:|---|---|---|---|---|---|---|"]
    for arm in BASELINE_READER_ARMS:
        for _, tag, label in BASELINE_PINS:
            for run in (1, 2):
                art = readers[(tag, run)]
                r1, r8 = _bl_cell(art, arm, "readers", 1), _bl_cell(art, arm, "readers", 8)
                cols = [_bl_iv(c, "reader_mops_mean", "reader_ci_lower", "reader_ci_upper", "reader_ci_method", 2)
                        for c in (r1, r8)]
                cols.append(_bl_iv(r8, "scaling_s_r", "scaling_s_r_ci_lower", "scaling_s_r_ci_upper",
                                   "scaling_s_r_ci_method", 3))
                for c in (r1, r8):
                    _, iv = _bl_ns_per_probe(c)
                    cols.append(_pc_iv(iv["mean"], iv["lo"], iv["hi"], 1, iv["method"]))
                bias = [reader_scaling_bounds.max_over_mean_bias(r["reader_thread_elapsed_s"])
                        for r in sorted(r8["rounds_raw"], key=lambda x: x["round"])]
                cols.append(f"{min(bias) * 100:.2f}% – {max(bias) * 100:.2f}%")
                mops = [float(r["reader_mops"]) for r in sorted(r8["rounds_raw"], key=lambda x: x["round"])]
                cols.append(", ".join(str(i) for i in reader_scaling_bounds.round_outliers(mops)) or "none")
                out.append(f"| `{arm}` | `{r1['workload_id']}` | {label} | {run} | " + " | ".join(cols) + " |")
    out.append("")

    def r_iv(art: dict, arm: str, r: int, what: str) -> tuple[float, float]:
        c = _bl_cell(art, arm, "readers", r)
        if what == "level":
            return c["reader_ci_lower"], c["reader_ci_upper"]
        return c["scaling_s_r_ci_lower"], c["scaling_s_r_ci_upper"]

    out += ["#### 15.5 Readers-only — run against run, and pin against pin", "",
            "| arm | pin | R = 1 intervals overlap across runs | R = 8 intervals overlap across runs "
            "| S(8) intervals overlap across runs | S(8) against 8, the reader count |",
            "|---|---|---|---|---|---|"]
    for arm in BASELINE_READER_ARMS:
        for _, tag, label in BASELINE_PINS:
            a1, a2 = readers[(tag, 1)], readers[(tag, 2)]
            out.append(f"| `{arm}` | {label} | {_bl_overlap(r_iv(a1, arm, 1, 'level'), r_iv(a2, arm, 1, 'level'))} "
                       f"| {_bl_overlap(r_iv(a1, arm, 8, 'level'), r_iv(a2, arm, 8, 'level'))} "
                       f"| {_bl_overlap(r_iv(a1, arm, 8, 's'), r_iv(a2, arm, 8, 's'))} "
                       f"| {_bl_vs([r_iv(a1, arm, 8, 's'), r_iv(a2, arm, 8, 's')], 8.0)} |")
    out += ["", "| arm | R = 1 level, per-core against `0-15` | R = 8 level, per-core against `0-15` "
            "| S(8), per-core against `0-15` |", "|---|---|---|---|"]
    for arm in BASELINE_READER_ARMS:
        cols = []
        for r, what in ((1, "level"), (8, "level"), (8, "s")):
            cols.append(_bl_pin_direction([(r_iv(readers[("percore", run)], arm, r, what),
                                            r_iv(readers[("pin0-15", run)], arm, r, what)) for run in (1, 2)]))
        out.append(f"| `{arm}` | " + " | ".join(cols) + " |")
    return out


# ---- 16. per-wrapper contention ranking at 0c6b7832 -------------------------
# The #929 step-2 `perf c2c` recordings of the `str`, `bytes` and `blob` writer
# wrappers, two per arm. Not CI dispatches: the driver ran on the reference
# host directly, so the recordings table names what produced them rather than
# a run URL. One `render` call per arm, so each arm's six tables sit under its
# own subsection and are read the way §11.9 reads the map/set pair.
WRAPPER_C2C_COMMIT = "0c6b7832"
WRAPPER_C2C_ARMS = (("str", "16.1"), ("bytes", "16.2"), ("blob", "16.3"))
WRAPPER_C2C_SOURCE = "driver on the reference host (not a CI dispatch)"
ISSUE_929 = "[#929](https://github.com/orieg/expanse/issues/929)"


def wrapper_contention_ranking() -> list[str]:
    """README section 16, rendered by `scripts/c2c_ranking.py`, one call per arm (#929)."""
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import c2c_ranking

    out: list[str] = []
    for arm, section in WRAPPER_C2C_ARMS:
        names = (f"c2c_{arm}_writer_scaling_{WRAPPER_C2C_COMMIT}.json",
                 f"c2c_{arm}_writer_scaling_{WRAPPER_C2C_COMMIT}_run2.json")
        runs = []
        for name in names:
            art = load(SUITE / "results" / name)
            if art is None:
                out += [f"#### {section}.1 The recordings", "", "| | run 1 | run 2 |",
                        "|---|---|---|",
                        f"| artifact | pending ({ISSUE_929}) | pending ({ISSUE_929}) |", ""]
                break
            commit = need(need(art, "provenance", name), "commit", name)
            if commit != WRAPPER_C2C_COMMIT:
                raise SystemExit(f"{name}: measured at {commit}, section {section} reads "
                                 f"{WRAPPER_C2C_COMMIT}")
            runs.append((name, WRAPPER_C2C_SOURCE, art))
        else:
            out += c2c_ranking.render(runs, section=section, run_label="recording") + [""]
    return out


# ---- 22. the #730 readers-only sweep re-measured at 7cd5140e ----------------
# Four `writer_scaling_readers_only` dispatches at a second head, two per pin,
# 8 rounds per cell, one harness process per timed cell. `readers_only`
# `preregistration` reads null in all four artifacts, so under METHODOLOGY §16.5
# these runs are a BASELINE and carry no verdict: nothing here prints a verdict
# label. METHODOLOGY §16.3's floors and R = 1 references are read as constants
# and never recomputed against this head (AGENTS.md §8.19). Cross-head columns
# state a direction only where both runs of a pin agree with intervals disjoint
# from both runs at the other head (docs/BENCHMARKING.md rule 18), and the
# point-estimate difference is labelled unpaired in the header it sits under,
# because these are separate dispatches and not the interleaved two-commit form
# rule 18 asks of a before/after claim on a concurrent cell.
REMEASURE_COMMIT = "7cd5140e"
REMEASURE_READERS = (1, 2, 4, 8)
REMEASURE_STEPS = ((1, 2), (2, 4), (4, 8))
# METHODOLOGY §16.3, per pin tag: the R = 8 floor the #730 gate registers, and
# the R = 1 reference its side condition names. Constants, not derivations.
PREREG_16_3 = {
    "pin0-15": {"floor_r8": 259.198, "ref_r1": 218.1226},
    "percore": {"floor_r8": 261.062, "ref_r1": 217.4392},
}


def _ro_iv(art: dict, arm: str, readers: int) -> dict:
    """The per-reader ns-per-probe interval of one readers-only cell (METHODOLOGY §16.2's series)."""
    _, iv = _bl_ns_per_probe(_bl_cell(art, arm, "readers", readers))
    return iv


def _ro_cell_iv(iv: dict) -> str:
    return _pc_iv(iv["mean"], iv["lo"], iv["hi"], 3, iv["method"])


def _ro_direction(new: list[dict], old: list[dict]) -> str:
    """Where this head's two runs sit against both runs at the other head, stated only if they agree.

    Each run must be disjoint from *both* of the other head's intervals in the
    same direction; a within-run interval does not bound between-run spread, so
    agreeing with one run of a pair is not enough (rule 18).
    """
    old_lo, old_hi = min(o["lo"] for o in old), max(o["hi"] for o in old)
    if all(n["hi"] < old_lo for n in new):
        return f"lower than both `{BASELINE_COMMIT}` runs, in both runs"
    if all(n["lo"] > old_hi for n in new):
        return f"higher than both `{BASELINE_COMMIT}` runs, in both runs"
    return "not the same in both runs"


def readers_only_remeasure() -> list[str]:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import reader_scaling_bounds

    old = _bl_artifacts("baseline_readers_only_writer_scaling")
    new = _bl_artifacts("baseline_readers_only_writer_scaling", REMEASURE_COMMIT)
    if old is None or new is None:
        return ["#### 22.1 Artifacts, runs and host load", "", "| head | artifact |", "|---|---|",
                f"| `{REMEASURE_COMMIT}` | pending ({ISSUE_730}) |"]

    out = ["#### 22.1 Artifacts, runs and host load", "",
           "| pin | run | artifact | CI run | `provenance.commit` | `readers_only.preregistration` "
           "| `readers_only.void` | load snapshots | `load1` at start, peak | busy-CPU window per cell "
           "| cell `foreign_busy_cpus`, min – max | governor on the pinned CPUs |",
           "|---|--:|---|---|---|---|---|--:|---|--:|---|---|"]
    for _, tag, label in BASELINE_PINS:
        for run in (1, 2):
            art = new[(tag, run)]
            name = _bl_name("baseline_readers_only_writer_scaling", tag, run, REMEASURE_COMMIT)
            prov, ro = art["provenance"], need(art, "readers_only", name)
            loads = prov["loads"]
            peak = max(l["load1"] for l in loads)
            fb = [need(c["load"], "foreign_busy_cpus", name) for c in art["throughput"]]
            wall = sorted({float(need(c["load"], "wall_s", name)) for c in art["throughput"]})
            govs = sorted(set(prov["host"]["scaling_governor_by_cpu"].values()))
            rid = BASELINE_RUNS[("baseline_readers_only_writer_scaling", REMEASURE_COMMIT, tag, run)]
            prereg = "`null`" if ro.get("preregistration") is None else f"`{ro['preregistration']}`"
            void = "empty" if not need(ro, "void", name) and ro["void"] == [] else f"`{ro['void']}`"
            out.append(f"| {label} | {run} | `results/{name}` | "
                       f"[{rid}](https://github.com/orieg/expanse/actions/runs/{rid}) | "
                       f"`{prov['commit']}` | {prereg} | {void} | {len(loads)} | "
                       f"{loads[0]['load1']:.2f}, {peak:.2f} | "
                       f"{wall[0]:.1f} – {wall[-1]:.1f} s | {min(fb):.2f} – {max(fb):.2f} | "
                       f"{', '.join(f'`{g}`' for g in govs)} |")
    out.append("")

    out += [f"#### 22.2 The `str` per-reader cost at `{REMEASURE_COMMIT}`, beside `{BASELINE_COMMIT}`", "",
            f"| pin | run | R | `{BASELINE_COMMIT}` ns per probe per reader [BCa 95%] "
            f"| `{REMEASURE_COMMIT}` ns per probe per reader [BCa 95%] "
            "| unpaired difference in the point estimates (rule 18: not a paired claim) "
            f"| intervals overlap across the heads | `{REMEASURE_COMMIT}` rounds beyond 3 MADs (`round_outliers`) "
            f"| `{REMEASURE_COMMIT}` ns per probe to last join |",
            "|---|--:|--:|---|---|--:|---|---|--:|"]
    for _, tag, label in BASELINE_PINS:
        for run in (1, 2):
            for r in REMEASURE_READERS:
                o, n = _ro_iv(old[(tag, run)], "str", r), _ro_iv(new[(tag, run)], "str", r)
                cell = _bl_cell(new[(tag, run)], "str", "readers", r)
                series, _ = _bl_ns_per_probe(cell)
                flagged = reader_scaling_bounds.round_outliers(series)
                out.append(f"| {label} | {run} | {r} | {_ro_cell_iv(o)} | {_ro_cell_iv(n)} | "
                           f"{n['mean'] - o['mean']:+.3f} ns, {(n['mean'] / o['mean'] - 1) * 100:+.2f} % | "
                           f"{_bl_overlap((o['lo'], o['hi']), (n['lo'], n['hi']))} | "
                           f"{', '.join(str(i) for i in flagged) or 'none'} | "
                           f"{cell['reader_ns_per_probe_to_last_join']:.3f} |")
    out.append("")

    out += ["#### 22.3 Which `str` cells moved in both runs (rule 18)", "",
            f"| pin | R | `{REMEASURE_COMMIT}` against `{BASELINE_COMMIT}` "
            f"| spread between the two `{BASELINE_COMMIT}` runs | spread between the two `{REMEASURE_COMMIT}` runs |",
            "|---|--:|---|--:|--:|"]
    for _, tag, label in BASELINE_PINS:
        for r in REMEASURE_READERS:
            o = [_ro_iv(old[(tag, run)], "str", r) for run in (1, 2)]
            n = [_ro_iv(new[(tag, run)], "str", r) for run in (1, 2)]
            def spread(ivs: list[dict]) -> str:
                a, b = ivs[0]["mean"], ivs[1]["mean"]
                return f"{abs(a - b) / min(a, b) * 100:.2f} %"
            out.append(f"| {label} | {r} | {_ro_direction(n, o)} | {spread(o)} | {spread(n)} |")
    out.append("")

    out += ["#### 22.4 Where the cells sit against METHODOLOGY §16.3's registered constants", "",
            "| pin | run | R = 8 ns per probe per reader [BCa 95%] | §16.3 R = 8 floor | point estimate over "
            "the floor | interval upper bound over the floor | R = 1 ns per probe per reader [BCa 95%] "
            "| §16.3 R = 1 reference | interval lower bound against the reference |",
            "|---|--:|---|--:|--:|--:|---|--:|--:|"]
    for _, tag, label in BASELINE_PINS:
        for run in (1, 2):
            pre = PREREG_16_3[tag]
            r8, r1 = _ro_iv(new[(tag, run)], "str", 8), _ro_iv(new[(tag, run)], "str", 1)
            over, pt = r8["hi"] - pre["floor_r8"], r8["mean"] - pre["floor_r8"]
            out.append(f"| {label} | {run} | {_ro_cell_iv(r8)} | {pre['floor_r8']:.3f} ns | "
                       f"{pt:+.3f} ns, {pt / pre['floor_r8'] * 100:+.2f} % | "
                       f"{over:+.3f} ns, {over / pre['floor_r8'] * 100:+.2f} % | {_ro_cell_iv(r1)} | "
                       f"{pre['ref_r1']:.4f} ns | {r1['lo'] - pre['ref_r1']:+.3f} ns |")
    out.append("")

    out += [f"#### 22.5 How the `str` per-reader cost rises with R at `{REMEASURE_COMMIT}`", "",
            "| pin | run | R = 1 ns per probe per reader [BCa 95%] | R = 8 ns per probe per reader [BCa 95%] "
            "| R = 8 over R = 1 | R = 1 as a share of R = 8 "
            "| ns added per additional reader, R 1 → 2 | R 2 → 4 | R 4 → 8 |",
            "|---|--:|---|---|--:|--:|--:|--:|--:|"]
    for _, tag, label in BASELINE_PINS:
        for run in (1, 2):
            ivs = {r: _ro_iv(new[(tag, run)], "str", r) for r in REMEASURE_READERS}
            steps = [f"{(ivs[b]['mean'] - ivs[a]['mean']) / (b - a):.2f} ns" for a, b in REMEASURE_STEPS]
            out.append(f"| {label} | {run} | {_ro_cell_iv(ivs[1])} | {_ro_cell_iv(ivs[8])} | "
                       f"{ivs[8]['mean'] / ivs[1]['mean']:.4f} | "
                       f"{ivs[1]['mean'] / ivs[8]['mean'] * 100:.2f} % | " + " | ".join(steps) + " |")
    out.append("")

    out += [f"#### 22.6 The `map` and `set` arms at `{REMEASURE_COMMIT}`, beside `{BASELINE_COMMIT}`", "",
            f"| arm | pin | run | R | `{BASELINE_COMMIT}` ns per probe per reader [BCa 95%] "
            f"| `{REMEASURE_COMMIT}` ns per probe per reader [BCa 95%] | intervals overlap across the heads "
            f"| `{REMEASURE_COMMIT}` against `{BASELINE_COMMIT}`, both runs (rule 18) |",
            "|---|---|--:|--:|---|---|---|---|"]
    for arm in ("map", "set"):
        for _, tag, label in BASELINE_PINS:
            for run in (1, 2):
                for r in (1, 8):
                    o, n = _ro_iv(old[(tag, run)], arm, r), _ro_iv(new[(tag, run)], arm, r)
                    both = _ro_direction([_ro_iv(new[(tag, k)], arm, r) for k in (1, 2)],
                                         [_ro_iv(old[(tag, k)], arm, r) for k in (1, 2)])
                    out.append(f"| `{arm}` | {label} | {run} | {r} | {_ro_cell_iv(o)} | {_ro_cell_iv(n)} | "
                               f"{_bl_overlap((o['lo'], o['hi']), (n['lo'], n['hi']))} | {both} |")
    return out


def main() -> int:
    import fine_grained_brackets_gate  # the §8 fine-grained write brackets verdicts, beside this file
    import multi_writer_olc_gate  # the §9 multi-writer OLC verdicts, beside this file

    blocks = [
        line_transfer(),
        d1(),
        d2(),
        spread(),
        ablations(),
        ablations_str(),
        fine_grained_brackets_gate.render(),
        multi_writer_olc_gate.render(),
        writer_scaling(),
        writer_scaling_percell(),
        contention_ranking(),
        mixed_concurrency(),
        wrapper_profiles(),
        string_wrapper_baselines(),
        wrapper_contention_ranking(),
        readers_only_remeasure(),
    ]
    print("\n\n".join("\n".join(b) for b in blocks))
    return 0


if __name__ == "__main__":
    sys.exit(main())
