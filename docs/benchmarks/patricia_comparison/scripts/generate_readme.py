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

import json
import sys
from pathlib import Path

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results"
REPO = BASE.parent.parent.parent
sys.path.insert(0, str(REPO / "scripts"))
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
        *memory_table(mem),
    ]
    return "\n".join(md).rstrip() + "\n"


def main() -> int:
    text = render()
    target = BASE / "README.md"
    if "--check" in sys.argv:
        if target.read_text() != text:
            print("README.md is stale; run generate_readme.py", file=sys.stderr)
            return 1
        return 0
    target.write_text(text)
    print(f"wrote {target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
