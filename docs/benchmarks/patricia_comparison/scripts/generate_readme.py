#!/usr/bin/env python3
"""
Generates docs/benchmarks/patricia_comparison/README.md from the committed
results/baseline_*.json artifacts.

Every number, verdict and label is derived from the artifacts of one run
(AGENTS.md §8.2): the verdicts apply the rules pre-registered in METHODOLOGY.md
§3, and P7 recomputes the census with scripts/patricia_envelope.py. Nothing in
the prose below is a stamped constant about the results.

    python3 docs/benchmarks/patricia_comparison/scripts/generate_readme.py [--check]

`--check` exits non-zero if README.md differs from what the artifacts produce.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results"
REPO = BASE.parent.parent.parent
sys.path.insert(0, str(REPO / "scripts"))
import patricia_d2_bounds as pd2  # noqa: E402
import patricia_envelope as pe  # noqa: E402

TWINS = ("patricia_tree", "fast_radix_trie", "qp_trie")
CHART_FILES = ("chart_lookup.svg", "chart_insert.svg", "chart_scan.svg", "chart_string.svg",
               "chart_memory_u64.svg", "chart_memory_paths.svg")
EVAL_N = (100_000, 1_000_000)
U64_DISTS = ("sequential", "clustered", "uniform_random", "sparse_stride", "zipfian")
FULL_BYTE = ("sequential", "clustered", "uniform_random", "sparse_stride")


def load(name: str) -> dict:
    return json.loads((RESULTS / f"baseline_{name}.json").read_text())


def side(ci) -> str:
    lo, hi = ci
    return "below" if hi < 1 else ("above" if lo > 1 else "spans")


def timing_verdict(ci) -> str:
    return {"below": "PASS", "above": "REFUTED", "spans": "BOUNDARY_RESULT"}[side(ci)]


def unpredicted(ci) -> str:
    return {"below": "win", "above": "UNPREDICTED_LOSS", "spans": "BOUNDARY_RESULT"}[side(ci)]


def draws(r: dict) -> int:
    """The n a row was generated at (distinct population can be smaller)."""
    if "raw_draws" in r:
        return r["raw_draws"] // 2 if r.get("hit_rate_pct") == 50 and r.get("distribution") != "prefixed_path" else r["raw_draws"]
    return r["population"]


def fmt_ratio(r: dict, key: str) -> str:
    if key not in r:
        return "—"
    lo, hi = r[f"{key}_ci"]
    return f"{r[key]:.4f} [{lo:.4f}, {hi:.4f}]"


def status(r: dict, twin: str) -> str:
    return r.get(f"{twin}_status", "valid")


def provenance_block(p: dict) -> list[str]:
    loads = p["loads"]
    foreign = [l.get("foreign_busy_cpus_since_prev") for l in loads]
    foreign = [f for f in foreign if f is not None]
    covered = loads[-1]["label"] == "end"
    return [
        f"- Host: {p['host_description']}; core pin `{p['core_pin']}`; governor "
        f"`{p['host']['scaling_governor']}`.",
        f"- Commit `{p['commit']}`; run {p['run_id']}.",
        f"- Load: {len(loads)} snapshots, one before every harness population and one at the end "
        f"({'present' if covered else 'MISSING'}); largest busy-CPU delta from processes outside "
        f"the run {max(foreign):.2f} core-equivalents (§8.17 contamination threshold: about 1).",
    ]


def verdict_rows() -> tuple[list[str], list[str]]:
    hit, miss, ins, scan, mem, sl = (load(n) for n in
                                     ("lookup_hit", "lookup_miss", "insert", "scan", "memory", "string_lookup"))
    out, losses = [], []

    def tally(label, rows, keys):
        verdicts = []
        for r in rows:
            for k in keys:
                if k in r:
                    verdicts.append(timing_verdict(r[f"{k}_ci"]))
        counts = {v: verdicts.count(v) for v in sorted(set(verdicts))}
        overall = "PASS" if set(verdicts) == {"PASS"} else ("REFUTED" if "REFUTED" in verdicts else "BOUNDARY_RESULT")
        out.append(f"| {label} | {len(verdicts)} | {overall} | "
                   + ", ".join(f"{v} {c}" for v, c in counts.items()) + " |")

    ev = lambda rows: [r for r in rows if draws(r) in EVAL_N]
    tally("P1 `patricia_lookup_hit` vs `patricia_tree`", ev(hit["results"]), ["ratio_expanse_over_patricia_tree"])
    tally("P2 `patricia_lookup_miss` vs `patricia_tree`", ev(miss["results"]), ["ratio_expanse_over_patricia_tree"])
    tally("P3 `patricia_insert` vs `patricia_tree`", ev(ins["results"]),
          ["ratio_expanse_over_patricia_tree_generator", "ratio_expanse_over_patricia_tree_shuffled"])
    tally("P4 `patricia_scan` full traversal vs `patricia_tree`",
          [r for r in ev(scan["results"]) if r["operation"] == "full_traversal"], ["ratio_expanse_over_patricia_tree"])

    # P5 / P6: validity.
    p5 = [(f, r) for f, d in (("lookup_hit", hit), ("insert", ins), ("memory", mem), ("scan", scan))
          for r in d["results"] if r.get("distribution") in FULL_BYTE and draws(r) in EVAL_N]
    p5_ok = all(any(k.startswith("fast_radix_trie") and k.endswith("_status") and v == "invalid"
                    for k, v in r.items()) for _, r in p5)
    out.append(f"| P5 `fast_radix_trie` invalid on full-byte-range `u64` sets | {len(p5)} | "
               f"{'PASS' if p5_ok else 'REFUTED'} | exact |")
    p6 = [r for d in (sl, mem, scan) for r in d["results"] if r.get("distribution") == "prefixed_path"]
    p6_ok = all(not (k.startswith("fast_radix_trie") and k.endswith("_status") and v == "invalid")
                for r in p6 for k, v in r.items())
    out.append(f"| P6 `fast_radix_trie` valid on every path cell | {len(p6)} | "
               f"{'PASS' if p6_ok else 'REFUTED'} | exact |")

    # P7: census vs hook.
    n7, ok7 = 0, True
    for r in mem["results"]:
        if r["key_type"] == "u64":
            if r["population"] not in EVAL_N or r["distribution"] == "zipfian":
                continue
            keys = [pe.be64(k) for k in pe.u64_dist(r["distribution"], r["raw_draws"])]
            orders = ("generator", "shuffled")
        else:
            if r["population"] != 100_000:
                continue
            keys = pe.gen_paths(r["population"], r["prefix_len"])
            orders = ("generator", "sorted")
        c = pe.census(keys)
        for o in orders:
            n7 += 1
            ok7 &= (r[f"patricia_tree_{o}_requested_bytes"], r[f"patricia_tree_{o}_allocs"]) == (c["bytes"], c["nodes"])
    out.append(f"| P7 `patricia_tree` requested bytes equal the census | {n7} | {'PASS' if ok7 else 'REFUTED'} | exact |")

    n8, ok8 = 0, True
    for r in mem["results"]:
        if r.get("distribution") in ("sequential", "clustered") and r["population"] in EVAL_N:
            for o in ("generator", "shuffled"):
                n8 += 1
                ok8 &= r[f"expanse_{o}_requested_bytes"] < r[f"patricia_tree_{o}_requested_bytes"]
    out.append(f"| P8 Expanse fewer requested bytes than `patricia_tree` (sequential, clustered) | {n8} | "
               f"{'PASS' if ok8 else 'REFUTED'} | exact |")

    # T1: per valid twin and N, generator order.
    t1 = []
    for n in EVAL_N:
        for t in TWINS:
            cells = sorted((r for r in sl["results"] if r["population"] == n and r["order"] == "generator"
                            and f"ratio_expanse_over_{t}" in r), key=lambda r: r["prefix_len"])
            if len(cells) < 2:
                continue
            k = f"ratio_expanse_over_{t}"
            sep = cells[-1][f"{k}_ci"][0] > cells[0][f"{k}_ci"][1]
            mono = all(cells[i][k] < cells[i + 1][k] for i in range(len(cells) - 1))
            t1.append("PASS" if sep else ("INTERMEDIATE" if mono else "REFUTED"))
            if sep and not mono:
                losses.append(f"T1 at n = {n:,} against `{t}` passes on its 8-vs-240 rule, but the four "
                              f"point estimates are not monotone in prefix length.")
    overall = "PASS" if set(t1) == {"PASS"} else ("REFUTED" if "REFUTED" in t1 else "INTERMEDIATE")
    out.append(f"| T1 string-lookup ratio rises with prefix length | {len(t1)} | {overall} | "
               + ", ".join(f"{v} {t1.count(v)}" for v in sorted(set(t1))) + " |")

    # Unpredicted losses anywhere.
    def scan_losses(name, d, key_of):
        for r in d["results"]:
            for t in TWINS:
                for k in key_of(t):
                    if k in r and side(r[f"{k}_ci"]) == "above":
                        where = " ".join(str(r[x]) for x in ("operation", "distribution", "prefix_len", "order")
                                         if x in r)
                        pre = "NOT_PREREGISTERED " if draws(r) not in EVAL_N else ""
                        losses.append(f"{pre}UNPREDICTED_LOSS — `{name}` {where}, n = {r['population']:,}, "
                                      f"against `{t}`: {fmt_ratio(r, k)}")
    scan_losses("patricia_lookup_hit", hit, lambda t: [f"ratio_expanse_over_{t}"])
    scan_losses("patricia_lookup_miss", miss, lambda t: [f"ratio_expanse_over_{t}"])
    scan_losses("patricia_insert", ins, lambda t: [f"ratio_expanse_over_{t}_generator", f"ratio_expanse_over_{t}_shuffled"])
    scan_losses("patricia_scan", scan, lambda t: [f"ratio_expanse_over_{t}"])
    scan_losses("patricia_string_lookup", sl, lambda t: [f"ratio_expanse_over_{t}"])
    return out, losses


def ratio_table(d: dict, title: str, wid: str, row_key, keys_for) -> list[str]:
    lines = [f"### {title} (workload: {wid})", "",
             "Ratio = Expanse ns ÷ twin ns, geometric mean of per-round ratios with its BCa 95% interval; "
             "below 1 means Expanse is faster. `invalid` = the twin failed validation and was not timed.", "",
             "| n | cell | " + " | ".join(f"vs `{t}`" for t in TWINS) + " |",
             "|---|---|" + "---|" * len(TWINS)]
    for r in d["results"]:
        if r["population"] < 10_000 and draws(r) not in EVAL_N:
            continue
        cells = []
        for t in TWINS:
            ks = [k for k in keys_for(t) if k in r]
            if ks:
                cells.append("<br>".join(fmt_ratio(r, k) for k in ks))
            else:
                cells.append("invalid" if status(r, t) == "invalid" or any(
                    k.startswith(t) and k.endswith("_status") and v == "invalid" for k, v in r.items()) else "—")
        lines.append(f"| {r['population']:,} | {row_key(r)} | " + " | ".join(cells) + " |")
    return lines + [""]


def surface_table(scan: dict) -> list[str]:
    """The prefix cells' in-harness comparison of the two Expanse surfaces.

    Rendered only from rows that carry the `expanse_unbounded` arm, so an
    artifact from before that arm renders nothing here.
    """
    rows = [r for r in scan["results"] if r["operation"] == "prefix_scan"
            and "ratio_expanse_over_expanse_unbounded" in r]
    if not rows:
        return []
    lines = ["### Prefix scan: `cursor_prefix` vs the unbounded walk (workload: patricia_scan)", "",
             "Ratio = `cursor_prefix` ns ÷ `cursor_at_or_after` + per-key `starts_with` ns, both on "
             "`ExpanseStrMap`, each on its own map, timed in the same rounds as the twins; geometric mean "
             "of per-round ratios with its BCa 95% interval. Below 1 means `cursor_prefix` is faster.", "",
             "| n | order | `cursor_prefix` ns/prefix | unbounded ns/prefix | ratio |",
             "|---|---|---|---|---|"]
    for r in rows:
        lines.append(f"| {r['population']:,} | {r['order']} | {r['expanse_ns_op']:,.0f} | "
                     f"{r['expanse_unbounded_ns_op']:,.0f} | "
                     f"{fmt_ratio(r, 'ratio_expanse_over_expanse_unbounded')} |")
    return lines + [""]


def d1_section() -> list[str]:
    """Diagnostic D1 (METHODOLOGY.md Amendment A3), derived from its counter artifacts.

    Rendered only when the pre-registered artifact exists. Every verdict below is
    computed here from the per-entry counts and their intervals; none is stamped.
    """
    pre_path = RESULTS / "counters_prefix_scan_d1.json"
    if not pre_path.exists():
        return []
    sup_path = RESULTS / "counters_prefix_scan_d1_stalls.json"

    def per_entry(doc: dict) -> dict:
        out = {}
        for c in doc["cells"]:
            n = c["distinct_probes"] * c["passes"]
            out[c["arm"]] = {e: {"point": st["point"] / n,
                                 "lo": None if st["ci_lower"] is None else st["ci_lower"] / n,
                                 "hi": None if st["ci_upper"] is None else st["ci_upper"] / n,
                                 "method": st.get("ci_method")}
                             for e, st in c["counters"].items()}
        return out

    pre_doc = json.loads(pre_path.read_text())
    pre = per_entry(pre_doc)
    gen, srt = pre["strmap_prefix_scan"], pre["strmap_prefix_scan_sorted"]
    prov = pre_doc["provenance"]
    tag = f"(measured: {prov['host_description']}, {prov['commit'][:8]})"

    def cell(v: dict) -> str:
        if v["lo"] is None:
            return f"{v['point']:.4f}"
        return f"{v['point']:.4f} [{v['lo']:.4f}, {v['hi']:.4f}]"

    lines = ["### Diagnostic D1: the 1M prefix scan's build-order gap (workload: example_perf_point_lookup)", "",
             "Pre-registered in `METHODOLOGY.md` Amendment A3. Counts per yielded entry, `probe − build` over "
             f"{pre_doc['cells'][0]['counters']['cycles']['n']} paired runs on the "
             f"`{pre_doc['pmu']['selected']}` PMU, BCa 95% interval over the runs {tag}.", "",
             "| Counter | generator build | sorted build |", "|---|---|---|"]
    for e in gen:
        if e == "br_misp_retired.all_branches":
            continue
        lines.append(f"| `{e}` | {cell(gen[e])} | {cell(srt[e])} |")
    lines.append("")

    d1a = abs(gen["instructions"]["point"] - srt["instructions"]["point"]) / srt["instructions"]["point"]
    above = [e for e in ("mem_load_retired.l3_miss", "dTLB-load-misses")
             if gen[e]["lo"] is not None and srt[e]["hi"] is not None and gen[e]["lo"] > srt[e]["hi"]]
    st_g, st_s = gen["cycle_activity.stalls_l3_miss"], srt["cycle_activity.stalls_l3_miss"]
    d1c_dead = st_g["point"] == 0 and st_s["point"] == 0
    lines += [
        f"- **D1a** instructions per entry differ by {d1a * 100:.2f}% — "
        + ("**PASS** (tolerance 2%)." if d1a <= 0.02 else "**FALSIFIED** (tolerance 2%)."),
        "- **D1b** generator interval above the sorted one for "
        + (", ".join(f"`{e}`" for e in above) + " — **PASS**." if above else "neither counter — **FALSIFIED**."),
    ]
    if d1c_dead:
        lines.append("- **D1c** **NOT EVALUABLE** in the pre-registered run: `cycle_activity.stalls_l3_miss` "
                     "read 0 in both builds inside the driver's default event set, while "
                     "`mem_load_retired.l3_miss` counted in the same runs.")
    else:
        share = (st_g["point"] - st_s["point"]) / (gen["cycles"]["point"] - srt["cycles"]["point"])
        lines.append(f"- **D1c** L3-miss stalls carry {share * 100:.1f}% of the cycle gap — "
                     + ("**PASS** (at least 50%)." if share >= 0.5 else "**FALSIFIED** (at least 50%)."))
    if sup_path.exists():
        sup_doc = json.loads(sup_path.read_text())
        sup = per_entry(sup_doc)
        g, s_ = sup["strmap_prefix_scan"], sup["strmap_prefix_scan_sorted"]
        share = ((g["cycle_activity.stalls_l3_miss"]["point"] - s_["cycle_activity.stalls_l3_miss"]["point"])
                 / (g["cycles"]["point"] - s_["cycles"]["point"]))
        stag = f"(measured: {sup_doc['provenance']['host_description']}, {sup_doc['provenance']['commit'][:8]})"
        lines += [
            f"- *Post hoc, not the pre-registered verdict:* counting only `cycles`, `instructions` and "
            f"`cycle_activity.stalls_l3_miss`, same arms and parameters {stag}: L3-miss stalls "
            f"{cell(g['cycle_activity.stalls_l3_miss'])} vs {cell(s_['cycle_activity.stalls_l3_miss'])} "
            f"cycles per entry, {share * 100:.1f}% of the cycle gap. `INTERMEDIATE`: the event set was "
            f"changed after the pre-registered run read zero.",
        ]
    return lines + [""]


def d2_section(results_dir: Path = RESULTS) -> list[str]:
    """Diagnostic D2 (METHODOLOGY.md Amendment A4 and errata A5), derived from its counter artifacts.

    Every number, ratio, interval, ci_method, THP fault count, host-guard reading and
    verdict is computed here from the committed counter artifacts and patricia_d2_bounds.py.
    """
    r1_2m_path = results_dir / "counters_prefix_scan_d2_2m_run1.json"
    if not r1_2m_path.exists():
        return []

    def load_counter(name: str) -> dict:
        return json.loads((results_dir / name).read_text())

    def short_host(prov: dict) -> str:
        return re.sub(r"Linux (\d+\.\d+)\.[^)]+", r"Linux \1", prov["host_description"])

    def fmt_ci(v: dict, fmt: str = ".4f", show_non_bca: bool = False) -> str:
        lo, hi = v["ci"]
        s = f"{v['point']:{fmt}} [{lo:{fmt}}, {hi:{fmt}}]"
        if show_non_bca and v.get("ci_method") and v["ci_method"] != "bca":
            s += f" (`{v['ci_method']}`)"
        return s

    def fmt_bound(c: dict, fmt: str) -> str:
        op = "≤" if c["decided_on"] == "upper" else "≥"
        return f"{c['decided_on']} {op} {c['bound']:{fmt}}"

    r1_2m = load_counter("counters_prefix_scan_d2_2m_run1.json")
    r2_2m = load_counter("counters_prefix_scan_d2_2m_run2.json")
    r1_4k = load_counter("counters_prefix_scan_d2_4k_run1.json")
    r2_4k = load_counter("counters_prefix_scan_d2_4k_run2.json")

    a4_1 = pd2.evaluate_d2(r1_2m, registration="A4")
    a4_2 = pd2.evaluate_d2(r2_2m, registration="A4")
    diff_4k_1 = pd2.arm_differences(r1_4k)
    diff_2m_1 = pd2.arm_differences(r1_2m)
    diff_4k_2 = pd2.arm_differences(r2_4k)
    diff_2m_2 = pd2.arm_differences(r2_2m)

    runs_n = r1_2m["cells"][0]["counters"]["cycles"]["n"]
    pmu_name = r1_2m["pmu"]["selected"]
    pmu_cpus = r1_2m["pmu"]["cpus"]
    conf_pct = int(round(r1_2m["statistics"]["confidence"] * 100))

    a4_verdict = a4_1["verdict"] if a4_1["verdict"] == a4_2["verdict"] else f"{a4_1['verdict']} / {a4_2['verdict']}"
    a4_clauses_ok = all(c["met"] for c in a4_1["clauses"].values()) and all(c["met"] for c in a4_2["clauses"].values())
    a4_host_ok = a4_1["host_precondition"]["met"] and a4_2["host_precondition"]["met"]
    load1_r1 = a4_1["host_precondition"]["load1_max"]
    load1_r2 = a4_2["host_precondition"]["load1_max"]
    foreign_a4_max = max(a4_1["host_precondition"]["foreign_busy_cpus_max"],
                         a4_2["host_precondition"]["foreign_busy_cpus_max"])
    tag_r1 = f"(measured: {short_host(r1_2m['provenance'])}, {r1_2m['provenance']['commit'][:8]})"
    tag_r2 = f"(measured: {short_host(r2_2m['provenance'])}, {r2_2m['provenance']['commit'][:8]})"

    clause_meta = (
        ("D2a", "**D2a** `dTLB-load-misses`, generator build", ".4f"),
        ("D2b", "**D2b** `mem_load_retired.l3_miss` difference", ".4f"),
        ("D2c", "**D2c** `cycles` difference", ".2f"),
    )

    lines = [
        "### Diagnostic D2: the build-order gap under 2 MiB pages (workload: example_perf_point_lookup)",
        "",
        f"Pre-registered in `METHODOLOGY.md` Amendment A4. Same arms and counters as D1, with the heap backed "
        f"by 2 MiB transparent huge pages. Counts per yielded entry over {runs_n} paired runs on the "
        f"`{pmu_name}` PMU, pinned `{pmu_cpus}`, BCa {conf_pct}% interval. D2b and D2c are the generator-build "
        f"count minus the sorted-build count, taken run by run in recorded order; the two arms are separate "
        f"blocks of processes, so that order pairs nothing. Evaluated under A4 by "
        f"`scripts/patricia_d2_bounds.py --evaluate-a4` on `results/counters_prefix_scan_d2_2m_run1.json` "
        f"and `_run2.json`.",
        "",
        f"**Verdict: `{a4_verdict}` in both runs.** "
        + ("All three clauses are met in both runs" if a4_clauses_ok else "Not all clauses are met")
        + (", and A4's registered host condition is not met as written. " if not a4_host_ok else ". ")
        + f"A4 registers `loadavg <= {pd2.REGISTERED_LOAD1_MAX:.1f}` and "
        f"`foreign_busy_cpus == {pd2.REGISTERED_FOREIGN_MAX:g}`; the artifacts record a one-minute load "
        f"average of up to {load1_r1:.2f} and {load1_r2:.2f} and foreign CPU of up to {foreign_a4_max:.2f} "
        f"core-equivalents. The condition was mis-specified: the measured workload is one thread, which reads "
        f"a load average near 1 by itself, and the host guard judged both runs quiet. A PASS needs an amended "
        f"condition and fresh runs (`AGENTS.md` §8.19). This section first recorded the runs as PASS without "
        f"stating the condition.",
        "",
        f"| Clause | Bound | Run 1 {tag_r1} | Run 2 {tag_r2} |",
        "|---|---|---|---|",
    ]
    for cid, label, bfmt in clause_meta:
        c1, c2 = a4_1["clauses"][cid], a4_2["clauses"][cid]
        lines.append(f"| {label} | {fmt_bound(c1, bfmt)} | {fmt_ci(c1)} | {fmt_ci(c2)} |")

    lines += [
        "",
        "The same two dispatches ran the arms on 4 KiB pages (`counters_prefix_scan_d2_4k_run1.json`, `_run2.json`). Generator minus sorted, per entry:",
        "",
        "| Counter | 4 KiB, run 1 | 2 MiB, run 1 | 4 KiB, run 2 | 2 MiB, run 2 |",
        "|---|---|---|---|---|",
    ]
    diff_counters = ("cycles", "mem_load_retired.l3_miss", "LLC-load-misses", "dTLB-load-misses")
    for counter in diff_counters:
        lines.append(
            f"| `{counter}` | {fmt_ci(diff_4k_1[counter])} | {fmt_ci(diff_2m_1[counter])} | "
            f"{fmt_ci(diff_4k_2[counter])} | {fmt_ci(diff_2m_2[counter])} |"
        )

    c4k_1, c2m_1 = diff_4k_1["cycles"]["point"], diff_2m_1["cycles"]["point"]
    c4k_2, c2m_2 = diff_4k_2["cycles"]["point"], diff_2m_2["cycles"]["point"]
    cdrop_1, cdrop_2 = c4k_1 - c2m_1, c4k_2 - c2m_2
    cpct_1, cpct_2 = cdrop_1 / c4k_1 * 100, cdrop_2 / c4k_2 * 100
    rem_floor = math.floor(min(a4_1["clauses"]["D2c"]["ci"][0], a4_2["clauses"]["D2c"]["ci"][0]))

    l3_4k_1, l3_2m_1 = diff_4k_1["mem_load_retired.l3_miss"]["point"], diff_2m_1["mem_load_retired.l3_miss"]["point"]
    l3_4k_2, l3_2m_2 = diff_4k_2["mem_load_retired.l3_miss"]["point"], diff_2m_2["mem_load_retired.l3_miss"]["point"]
    l3_drop_pct = int(round(((l3_4k_1 - l3_2m_1) / l3_4k_1 + (l3_4k_2 - l3_2m_2) / l3_4k_2) / 2 * 100, -1))
    llc_4k_1, llc_2m_1 = diff_4k_1["LLC-load-misses"]["point"], diff_2m_1["LLC-load-misses"]["point"]
    llc_4k_2, llc_2m_2 = diff_4k_2["LLC-load-misses"]["point"], diff_2m_2["LLC-load-misses"]["point"]
    llc_pct_1 = (llc_4k_1 - llc_2m_1) / llc_4k_1 * 100
    llc_pct_2 = (llc_4k_2 - llc_2m_2) / llc_4k_2 * 100
    d2b_a4_met = a4_1["clauses"]["D2b"]["met"] and a4_2["clauses"]["D2b"]["met"]
    r2_d2a_method = a4_2["clauses"]["D2a"]["ci_method"]

    w_2m_1 = diff_2m_1["cycles"]["ci"][1] - diff_2m_1["cycles"]["ci"][0]
    w_2m_2 = diff_2m_2["cycles"]["ci"][1] - diff_2m_2["cycles"]["ci"][0]
    w_4k = max(diff_4k_1["cycles"]["ci"][1] - diff_4k_1["cycles"]["ci"][0],
               diff_4k_2["cycles"]["ci"][1] - diff_4k_2["cycles"]["ci"][0])

    lines += [
        "",
        "What the two runs show:",
        "",
        f"- **Huge pages remove the dTLB misses and part of the gap, not all of it.** The cycle gap falls "
        f"from {c4k_1:.1f} and {c4k_2:.1f} per entry on 4 KiB pages to {c2m_1:.1f} and {c2m_2:.1f} on "
        f"2 MiB pages: {cdrop_1:.1f} and {cdrop_2:.1f} cycles per entry, {cpct_1:.0f}% and {cpct_2:.0f}% "
        f"of it. At least {rem_floor} cycles per entry remain in both runs.",
        f"- **The retired-L3-miss count is not invariant under paging.** `mem_load_retired.l3_miss` falls "
        f"by about {l3_drop_pct}% ({l3_4k_1:.3f} to {l3_2m_1:.3f}, {l3_4k_2:.3f} to {l3_2m_2:.3f}) while "
        f"`LLC-load-misses` falls by about {llc_pct_1:.0f}% and {llc_pct_2:.0f}%. Clause D2b's bound is "
        + ("still met." if d2b_a4_met else "not met.")
        + " Why the two events diverge is not measured.",
        "- **Nothing here says what the remaining cycles are spent on.** D2c's bound is D1's L3-stall "
        "figure, and meeting it shows the gap is at least that large. The stall counter itself was not "
        "counted in these runs (`cycle_activity.stalls_l3_miss` reads 0 in this event set and is withheld), "
        "so no share of the remaining gap is attributed to L3 misses.",
        f"- Run 2's D2a interval, and its `dTLB-load-misses` difference, are the bias-corrected construction "
        f"(`ci_method: {r2_d2a_method}`); every other interval above is BCa.",
        "- The two runs are at different commits. Between them `strmap.rs` changed by one documentation "
        "comment, and no other file on the plain string map's scan path changed.",
        f"- Under 2 MiB pages the generator arm varies more between processes than under 4 KiB pages "
        f"(cycle-gap interval widths of {w_2m_1:.1f} and {w_2m_2:.1f} against {w_4k:.1f}). The cause is "
        f"not measured.",
        "- D2 tests no remedy: child and leaf co-allocation, prefetching and packed suffixes are each unmeasured.",
    ]

    a5_2m_1_path = results_dir / "counters_prefix_scan_d2_2m_a5_run1.json"
    if not a5_2m_1_path.exists():
        return lines + [""]

    a5_2m_1 = load_counter("counters_prefix_scan_d2_2m_a5_run1.json")
    a5_4k_1 = load_counter("counters_prefix_scan_d2_4k_a5_run1.json")
    a5_2m_2 = load_counter("counters_prefix_scan_d2_2m_a5_run2.json")
    a5_4k_2 = load_counter("counters_prefix_scan_d2_4k_a5_run2.json")

    ev_a5_1 = pd2.evaluate_d2(a5_2m_1, a5_4k_1)
    ev_a5_2 = pd2.evaluate_d2(a5_2m_2, a5_4k_2)
    pair_a5 = pd2.d2_verdict([ev_a5_1, ev_a5_2])

    dispatches = [(1, ev_a5_1, a5_2m_1, a5_4k_1), (2, ev_a5_2, a5_2m_2, a5_4k_2)]
    admissible = [(idx, ev, d2m, d4k) for idx, ev, d2m, d4k in dispatches if ev["verdict"] != "VOID"]
    adm_idx, adm_ev, adm_2m, adm_4k = admissible[-1]
    failed_clauses = [k for k, v in adm_ev["clauses"].items() if not v["met"]]
    failed_str = ", ".join(failed_clauses)
    heading_suffix = f" on {failed_str}" if failed_clauses else ""

    a5_commit = pair_a5["commit"][:9]
    a5_tag = f"(measured: {short_host(adm_2m['provenance'])}, {a5_commit})"

    lines += [
        "",
        f"#### D2 under errata A5: `{pair_a5['verdict']}`{heading_suffix} (workload: example_perf_point_lookup)",
        "",
        f"The A4-era verdict above stands as recorded and is not re-read. `METHODOLOGY.md` errata A5 replaced "
        f"A4's host condition and registered a verdict from two new dispatches at one commit. Both ran at "
        f"`{a5_commit}` {a5_tag}. Evaluated by `scripts/patricia_d2_bounds.py --evaluate` and `--verdict` on "
        f"the four `results/counters_prefix_scan_d2_{{2m,4k}}_a5_run{{1,2}}.json` artifacts.",
        "",
        f"**Pair verdict: `{pair_a5['verdict']}`, final.** "
        + (f"The one admissible dispatch ({adm_idx}) does not meet {failed_str}. " if failed_clauses else "All clauses are met. ")
        + "Under A5.5 an admissible dispatch that misses a clause is never replaced, so no further dispatch "
        "is made. The bound was not moved to the measurement (`AGENTS.md` §8.19).",
        "",
        "| Dispatch | Run | Admissible | Verdict | Host readings (`provenance`, both artifacts) |",
        "|---|---|---|---|---|",
    ]

    for idx, ev, d2m, d4k in dispatches:
        run_url = d2m["provenance"]["run_id"]
        run_num = run_url.rsplit("/", 1)[-1]
        run_col = f"[{run_num}]({run_url}) (`a5_run{idx}`)"
        if ev["verdict"] != "VOID":
            adm_col = "yes"
        elif ev["host_precondition"]["met"]:
            adm_col = "no: the host condition (H0 to H2) is met, the huge-page precondition of §3.4.2 is not"
        else:
            adm_col = "no"

        if ev["verdict"] == "VOID":
            verdict_col = f"`VOID`: `{'; '.join(ev['voids'])}`"
        elif ev["verdict"] == "FAIL":
            missed = ", ".join(k for k, v in ev["clauses"].items() if not v["met"])
            verdict_col = f"`FAIL` ({missed})"
        else:
            verdict_col = f"`{ev['verdict']}`"

        g = d2m["provenance"]["host_guard"]
        g_start = g["start"]["verdict"]
        g_run = g["run"]["verdict"]
        g_samples = g["run"]["samples"]
        g_voids = g["run"]["voids"]
        g_wf = g["run"]["worst_foreign_busy_cpus"]
        g_wp = g["run"]["worst_on_pin_foreign_busy_cpus"]
        l2m = [x["load1"] for x in d2m["provenance"]["loads"]]
        l4k = [x["load1"] for x in d4k["provenance"]["loads"]]
        f2m = [x["foreign_busy_cpus_since_prev"] for x in d2m["provenance"]["loads"][1:]]
        f4k = [x["foreign_busy_cpus_since_prev"] for x in d4k["provenance"]["loads"][1:]]
        if len(set(f2m + f4k)) == 1:
            f_str = f"foreign {f2m[0]:.2f} in all four windows"
        else:
            f_str = f"foreign {', '.join(f'{x:.2f}' for x in f2m)} and {', '.join(f'{x:.2f}' for x in f4k)}"
        host_col = (
            f"guard start {g_start}, in-run {g_run} ({g_samples} samples, {g_voids} voids, "
            f"worst foreign {g_wf:.3f}, worst on-pin {g_wp:.3f}); "
            f"`load1` {min(l2m):.2f} to {max(l2m):.2f} (2 MiB), {min(l4k):.2f} to {max(l4k):.2f} (4 KiB); {f_str}"
        )
        lines.append(f"| {idx} | {run_col} | {adm_col} | {verdict_col} | {host_col} |")

    c1_cells = {c["arm"]: c for c in a5_2m_1["cells"]}
    c2_cells = {c["arm"]: c for c in a5_2m_2["cells"]}
    c1_gen_thp = c1_cells[pd2.GENERATOR_ARM]["thp_delta"]
    c1_srt_thp = c1_cells[pd2.SORTED_ARM]["thp_delta"]
    c2_gen_thp = c2_cells[pd2.GENERATOR_ARM]["thp_delta"]
    c2_srt_thp = c2_cells[pd2.SORTED_ARM]["thp_delta"]
    c2_fb_max = max(c2_gen_thp["thp_fault_fallback"], c2_srt_thp["thp_fault_fallback"])

    adm_runs = adm_2m["cells"][0]["counters"]["cycles"]["n"]
    lines += [
        "",
        f"Dispatch 1's 2 MiB artifact records `thp_fault_fallback` {c1_gen_thp['thp_fault_fallback']} for "
        f"the generator arm (`thp_fault_alloc` {c1_gen_thp['thp_fault_alloc']:,}) and "
        f"{c1_srt_thp['thp_fault_fallback']} for the sorted arm ({c1_srt_thp['thp_fault_alloc']:,}). Under "
        f"A5.3 a void dispatch's clauses are not evaluated and its counts are not published; the artifacts "
        f"are committed so the void is reproducible. The cause of the fallback is not measured. An "
        f"operator's observation on the reference host after the run, which is in no artifact: free memory "
        f"was low with a large page cache, the kernel's `thp_fault_fallback` counter read "
        f"{c1_gen_thp['thp_fault_fallback']} (equal to the run's), and compaction had failed; the cache was "
        f"dropped and memory compacted before dispatch 2. The hypothesis it suggests, that memory was too "
        f"full and fragmented to supply 2 MiB pages, is unmeasured. Dispatch 2 recorded `thp_fault_fallback` "
        f"{c2_fb_max} in both arms (`thp_fault_alloc` {c2_gen_thp['thp_fault_alloc']:,} and "
        f"{c2_srt_thp['thp_fault_alloc']:,}).",
        "",
        f"Clauses of dispatch {adm_idx}, per yielded entry, {adm_runs} runs, generator build minus sorted "
        f"build for D2b and D2c {a5_tag}:",
        "",
        "| Clause | Registered bound | Decided on | Point [95% interval] | `ci_method` | Met |",
        "|---|---|---|---|---|---|",
    ]

    a5_clause_order = (
        ("D2b", "**D2b** `mem_load_retired.l3_miss` difference", ".4f", ".4f"),
        ("D2a", "**D2a** `dTLB-load-misses`, generator build", ".4f", ".5f"),
        ("D2c", "**D2c** `cycles` difference", ".2f", ".4f"),
    )
    for cid, label, bfmt, vfmt in a5_clause_order:
        c = adm_ev["clauses"][cid]
        if c["met"]:
            met_str = "yes"
        elif c["decided_on"] == "lower":
            gap = c["bound"] - c["ci"][0]
            rel_pt = "above" if c["point"] >= c["bound"] else "below"
            met_str = f"**no** (lower bound {gap:.4f} below the bound; point {rel_pt} it)"
        else:
            gap = c["ci"][1] - c["bound"]
            rel_pt = "below" if c["point"] <= c["bound"] else "above"
            met_str = f"**no** (upper bound {gap:.4f} above the bound; point {rel_pt} it)"
        lines.append(
            f"| {label} | {fmt_bound(c, bfmt)} | {c['decided_on']} | {fmt_ci(c, vfmt)} | "
            f"`{c['ci_method']}` | {met_str} |"
        )

    d2a_c = adm_ev["clauses"]["D2a"]
    d2b_c = adm_ev["clauses"]["D2b"]
    d1_l3 = pd2.D1_GEN_L3_POINT
    d1_l3_delta = pd2.D1_GEN_L3_POINT - pd2.D1_SORTED_L3_POINT
    reg_ret_pct = int(round(d2b_c["bound"] / d1_l3_delta * 100))
    pt_pct = d2b_c["point"] / d1_l3 * 100
    lo_pct = d2b_c["ci"][0] / d1_l3 * 100
    hi_pct = d2b_c["ci"][1] / d1_l3 * 100

    adm_diff_4k = pd2.arm_differences(adm_4k)
    adm_diff_2m = pd2.arm_differences(adm_2m)

    lines += [
        "",
        f"D2a's interval is the bias-corrected construction, not BCa (`ci_method: {d2a_c['ci_method']}`). "
        f"The D1 reference for D2b is {d1_l3:.4f} (above): the point is {pt_pct:.1f}% of it, the lower bound "
        f"{lo_pct:.1f}% and the upper bound {hi_pct:.1f}% (derived), against the registered {reg_ret_pct}%.",
        "",
        f"The 4 KiB control of dispatch {adm_idx}, same arms, generator minus sorted:",
        "",
        "| Counter | 4 KiB | 2 MiB |",
        "|---|---|---|",
    ]
    for counter in diff_counters:
        lines.append(
            f"| `{counter}` | {fmt_ci(adm_diff_4k[counter], show_non_bca=True)} | "
            f"{fmt_ci(adm_diff_2m[counter], show_non_bca=True)} |"
        )

    adm_c_4k, adm_c_2m = adm_diff_4k["cycles"]["point"], adm_diff_2m["cycles"]["point"]
    adm_c_drop = adm_c_4k - adm_c_2m
    adm_c_pct = adm_c_drop / adm_c_4k * 100
    adm_l3_4k = adm_diff_4k["mem_load_retired.l3_miss"]["point"]
    adm_l3_2m = adm_diff_2m["mem_load_retired.l3_miss"]["point"]
    adm_l3_pct = (adm_l3_4k - adm_l3_2m) / adm_l3_4k * 100
    a4_1_d2b_lo = a4_1["clauses"]["D2b"]["ci"][0]
    a4_2_d2b_lo = a4_2["clauses"]["D2b"]["ci"][0]
    d2c_met = adm_ev["clauses"]["D2c"]["met"]

    lines += [
        "",
        f"From the 4 KiB to the 2 MiB point, the cycle gap falls by {adm_c_drop:.1f} per entry "
        f"({adm_c_pct:.1f}%) and the retired-L3-miss difference by {adm_l3_pct:.1f}% (derived). Only one "
        f"admissible dispatch exists, so no statement about the direction of a change between page sizes is "
        f"made across dispatches (`docs/BENCHMARKING.md` rule 18). The two A4-era runs recorded D2b lower "
        f"bounds of {a4_1_d2b_lo:.4f} and {a4_2_d2b_lo:.4f} at other commits; why dispatch {adm_idx}'s "
        f"reading is lower is not measured, and those runs keep their label.",
        "",
        f"What `{pair_a5['verdict']}` says, in D2's registered terms: D2b's falsifier, a BCa lower bound "
        f"below {d2b_c['bound']:.4f}, "
        + ("holds" if not d2b_c["met"] else "does not hold")
        + " in the admissible dispatch, so the retired-L3-miss difference under 2 MiB pages "
        + ("could not be shown to retain" if not d2b_c["met"] else "retains")
        + f" {reg_ret_pct}% of D1's. The conclusion §3.4.4 registers for a PASS (translation misses an "
        "orthogonal overlay, with a dispersal effect that paging does not resolve) is therefore "
        + ("not supported" if pair_a5["verdict"] != "PASS" else "supported")
        + " by D2. §3.4.4's other branch, \"If D2c is falsified\", is "
        + ("not triggered, because D2c is met." if d2c_met else "triggered, because D2c is not met.")
        + " D2 registers no other consequence. D2a and D2c are reported as measured; they do not change the "
        "verdict, and the verdict does not say what the remaining cycle gap consists of. D2 tests no remedy.",
    ]
    return lines + [""]


def memory_table(mem: dict) -> list[str]:
    arms = ("expanse",) + TWINS
    lines = ["### Live heap (workload: patricia_memory)", "",
             "Requested / usable bytes per key (`" + mem["results"][0]["usable_instrument"] + "`); exact counts. "
             "Usable size excludes the allocator's per-chunk header, so neither column is the full resident "
             "footprint of small nodes. Orders: `u64` generator / shuffled, paths generator / sorted.", "",
             "| n | key set | " + " | ".join(f"`{a}`" for a in arms) + " |",
             "|---|---|" + "---|" * len(arms)]
    for r in mem["results"]:
        if r["population"] not in EVAL_N:
            continue
        orders = ("generator", "shuffled") if r["key_type"] == "u64" else ("generator", "sorted")
        name = r["distribution"] if r["key_type"] == "u64" else f"path, prefix {r['prefix_len']}"
        cells = []
        for a in arms:
            vals = []
            for o in orders:
                q = r.get(f"{a}_{o}_requested_bytes_per_key")
                u = r.get(f"{a}_{o}_usable_bytes_per_key")
                vals.append("invalid" if q is None else f"{q:.2f} / {u:.2f}")
            cells.append("<br>".join(vals))
        lines.append(f"| {r['population']:,} | {name} | " + " | ".join(cells) + " |")
    return lines + [""]


def render() -> str:
    hit, miss, ins, scan, mem, sl = (load(n) for n in
                                     ("lookup_hit", "lookup_miss", "insert", "scan", "memory", "string_lookup"))
    # Every artifact carries the run's provenance; the one with the most load
    # snapshots is the complete record (identical across files once the runner
    # re-stamps them at the end).
    prov = max((d["provenance"] for d in (hit, miss, ins, scan, mem, sl)), key=lambda p: len(p["loads"]))
    verdicts, notes = verdict_rows()
    outside = [n for n in notes if n.startswith("NOT_PREREGISTERED")]
    notes = [n for n in notes if not n.startswith("NOT_PREREGISTERED")]
    md = [
        "# Expanse vs. Patricia and Radix Tries",
        "",
        "<!-- Generated by scripts/generate_readme.py from results/baseline_*.json; do not edit by hand. -->",
        "",
        "`ExpanseMap` and `ExpanseStrMap` against three compressed radix tries that differ in how a node "
        "finds its child: [`patricia_tree`](https://github.com/sile/patricia_tree) `=0.10.2` (linked sibling "
        "list), [`fast_radix_trie`](https://github.com/bluecatengineering/fast_radix_trie) `=1.2.0` (inline "
        "child array; its `u8` child count cannot hold the `u64` key sets here, recorded `invalid`), and "
        "[`qp-trie`](https://github.com/sdleffler/qp-trie-rs) `=0.8.2` (nybble branch). Design, envelope, "
        "pre-registration and claims ceiling: [METHODOLOGY.md](METHODOLOGY.md).",
        "",
        "## Provenance",
        "",
        *provenance_block(prov),
        "",
        "## Pre-registered verdicts",
        "",
        f"Evaluated at n ∈ {{{', '.join(f'{n:,}' for n in EVAL_N)}}}, both build orders, under METHODOLOGY.md §3 "
        f"(measured: {prov['host_description']}, {prov['commit']}). No independent peer review.",
        "",
        "| Prediction | Cells | Verdict | Breakdown |",
        "|---|---|---|---|",
        *verdicts,
        "",
        "## Losses and qualifications",
        "",
        "Every evaluated cell whose interval lies wholly above 1, where the ratio is Expanse ns ÷ twin ns "
        "(so Expanse is slower); none of them is covered by a prediction, so each is an `UNPREDICTED_LOSS`.",
        "",
        *([f"- {n}" for n in notes] or ["- none"]),
        *([f"- Outside the evaluated n (`NOT_PREREGISTERED`), {len(outside)} further cells have Expanse "
           f"slower; they are in the tables below."] if outside else []),
        "",
        "## Charts",
        "",
        "Generated by `scripts/generate_charts.py` from the same artifacts. Timing charts use a log axis "
        "centred on 1, so bar length understates large ratios; the tables below carry every value and "
        f"interval (measured: {prov['host_description']}, {prov['commit']}).",
        "",
        *[f"![{c[:-4].replace('_', ' ')}](results/{c})\n" for c in CHART_FILES],
        "## Results",
        "",
        f"(measured: {prov['host_description']}, {prov['commit']})",
        "",
        *ratio_table(hit, "Point lookup, 100% hit", "patricia_lookup_hit",
                     lambda r: f"{r['distribution']} / {r['order']}", lambda t: [f"ratio_expanse_over_{t}"]),
        *ratio_table(miss, "Point lookup, 50% hit, in-range misses", "patricia_lookup_miss",
                     lambda r: f"{r['distribution']} / {r['order']}", lambda t: [f"ratio_expanse_over_{t}"]),
        *ratio_table(ins, "Cold-build insert (generator order, then shuffled)", "patricia_insert",
                     lambda r: r["distribution"],
                     lambda t: [f"ratio_expanse_over_{t}_generator", f"ratio_expanse_over_{t}_shuffled"]),
        *ratio_table(sl, "String lookup, 50% hit, by shared-prefix length", "patricia_string_lookup",
                     lambda r: f"prefix {r['prefix_len']} / {r['order']}", lambda t: [f"ratio_expanse_over_{t}"]),
        *ratio_table(scan, "Full traversal and prefix scan", "patricia_scan",
                     lambda r: f"{r['operation']} {r['distribution']} / {r['order']}",
                     lambda t: [f"ratio_expanse_over_{t}"]),
        *surface_table(scan),
        *d1_section(),
        *d2_section(),
        *memory_table(mem),
    ]
    return "\n".join(md).rstrip() + "\n"


README_PATH = BASE / "README.md"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true",
                    help="exit non-zero if README.md differs from what the artifacts produce")
    args = ap.parse_args(argv)
    text = render()
    target = README_PATH
    if args.check:
        if target.read_text() != text:
            print("README.md is stale; run generate_readme.py", file=sys.stderr)
            return 1
        return 0
    target.write_text(text)
    print(f"wrote {target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
