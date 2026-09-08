#!/usr/bin/env python3
"""Markdown tables for the integer-key arms (#660), derived from results/.

The string arms have had `string_tables.py` since #693; the integer tables in
README §2, §3 and §4 were typed by hand from the artifacts, which is the class
of defect #736 exists to catch and the reason a re-measurement had to redo the
paste. Every cell, ratio, verdict and count below is computed from
``results/baseline_latency.json`` (AGENTS.md §8.2: nothing in a generated table
is typed by hand).

    python3 docs/benchmarks/hot_comparison/scripts/integer_tables.py [--quick]

Bolding follows the README's existing convention and is derived, not chosen: the
winning arm's nanoseconds are bold, and a cell whose BCa interval spans parity
carries a bold `BOUNDARY_RESULT` verdict and no bold winner — a bold number is
never a cell that claims no winner (§8.4).
"""

import json
import sys
from collections import Counter
from pathlib import Path

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results" / ("quick" if "--quick" in sys.argv else "")

DISTS = ["sequential", "clustered", "sparse", "random"]
ARMS = ["set", "map"]
HEAD_N = 1_000_000
SCAN_K = [10, 100, 1000]


def load(name: str) -> dict:
    return json.loads((RESULTS / name).read_text())


def cell_index(lat: dict) -> dict:
    return {(c["arm"], c["pillar"], c["dist"], c["population"], c["scan_k"]): c
            for c in lat["cells"]}


def verdict_label(c: dict) -> str:
    v = c["verdict"]
    if v == "BOUNDARY_RESULT":
        return "**`BOUNDARY_RESULT`**"
    return "Expanse" if v == "expanse" else "**HOT**"


def ns_cells(c: dict) -> tuple[str, str]:
    """HOT and Expanse nanoseconds, the winner's in bold; neither if no winner."""
    hot = f"{c['hot_ns_per_op_median']:.2f}"
    exp = f"{c['expanse_ns_per_op_median']:.2f}"
    if c["verdict"] == "expanse":
        return hot, f"**{exp}**"
    if c["verdict"] == "hot":
        return f"**{hot}**", exp
    return hot, exp


def ratio_cell(c: dict) -> str:
    r = f"{c['hot_over_expanse']:.3f}"
    return f"**{r}**" if c["verdict"] != "expanse" else r


def latency_table(idx: dict, pillar: str, title: str, with_verdict: bool) -> str:
    out = [f"### {title}\n"]
    if with_verdict:
        out.append("| Distribution | Arm | HOT ns | Expanse ns | Ratio | Verdict |")
        out.append("|---|---|---:|---:|---:|---|")
    else:
        out.append("| Distribution | Arm | HOT ns | Expanse ns | Ratio |")
        out.append("|---|---|---:|---:|---:|")
    for arm in ARMS:
        for dist in DISTS:
            c = idx.get((arm, pillar, dist, HEAD_N, 0))
            if c is None:
                continue
            hot, exp = ns_cells(c)
            # The README emphasises the rows that do not go to Expanse.
            d = f"**{dist}**" if c["verdict"] != "expanse" else dist
            a = f"**{arm}**" if c["verdict"] != "expanse" else arm
            row = f"| {d} | {a} | {hot} | {exp} | {ratio_cell(c)} |"
            if with_verdict:
                row += f" {verdict_label(c)} |"
            out.append(row)
    out.append("")
    return "\n".join(out)


def scan_table(idx: dict) -> str:
    out = ["| Arm | Dist | N | " + " | ".join(f"k={k}" for k in SCAN_K) + " |",
           "|---|---|---:|" + "---:|" * len(SCAN_K)]
    pops = sorted({k[3] for k in idx if k[1] == "scan"})
    for arm in ARMS:
        for n in pops:
            row = []
            for k in SCAN_K:
                c = idx.get((arm, "scan", "random", n, k))
                if c is None:
                    row.append("—")
                    continue
                r = f"{c['hot_over_expanse']:.3f}"
                row.append(f"**{r}**" if c["verdict"] == "expanse" else r)
            out.append(f"| {arm} | random | {n:,} | " + " | ".join(row) + " |")
    out.append("")
    return "\n".join(out)


def scorecard(lat: dict, mem: dict) -> str:
    counts = Counter(c["verdict"] for c in lat["cells"])
    out = [f"{len(lat['cells'])} latency cells, {len(mem['cells'])} memory cells.\n",
           "| | Count |", "|---|---:|",
           f"| Expanse wins (CI excludes parity) | {counts.get('expanse', 0)} |",
           f"| HOT wins (CI excludes parity) | {counts.get('hot', 0)} |",
           f"| `BOUNDARY_RESULT` (interval spans parity) | {counts.get('BOUNDARY_RESULT', 0)} |",
           ""]
    return "\n".join(out)


def scan_share(lat: dict) -> str:
    """How many of the HOT wins are scan cells — a §3 sentence, derived."""
    hot = [c for c in lat["cells"] if c["verdict"] == "hot"]
    scans = [c for c in hot if c["pillar"] == "scan"]
    return f"{len(scans)} of this suite's {len(hot)} HOT wins are scan cells."


# --- §7.3 protocol health ---------------------------------------------------
#
# Both concurrent runs side by side (docs/BENCHMARKING.md rule 18: a
# concurrent level is quoted only across two runs). The verdict column is the
# §11.5.3 logic as `masstree_comparison/scripts/tables.py` derives it for §6.3:
# "rise with W" is evaluated per arm and per run on the restart-share medians,
# strictly increasing across writer counts; the starvation half is the median
# fallback share against 1%, and a zero is `PASS_categorical_by_design` because
# a fallback needs 64 consecutive failed walks.

HEALTH_RUNS = (("1", "baseline_concurrent.json"), ("2", "baseline_concurrent_run2.json"))

# Derived shares that become columns once any loaded run carries them; a run
# that lacks one reads "not recorded". `locked_reads ÷ read_ops` is always a
# column — it is the share #568's Step 0 asks for, and the two committed runs
# predate the runner recording it, which the table says rather than hides.
HEALTH_EXTRA = (
    ("unconditional_share", "unconditional lock share"),
    ("handoffs_per_write", "handoffs ÷ write"),
    ("replacements_per_write", "branch replacements ÷ write"),
    ("deep_cascade_share", "deep-cascade share"),
    ("root_rewrite_share", "root-rewrite share"),
    ("spin_time_share", "spin time ÷ reader wall"),
)


def load_optional(name: str) -> dict | None:
    p = RESULTS / name
    return json.loads(p.read_text()) if p.exists() else None


def stat_cell(c: dict, key: str, fmt: str) -> str:
    """A derived-share cell: its median, `n/a` on a zero denominator, or `not recorded`."""
    if key not in c:
        return "not recorded"
    if c[key] is None:
        return "n/a"
    return fmt.format(c[key]["median"])


def health_verdict(c: dict, rising: bool | None) -> str:
    half1 = ("rise with W: n/a" if rising is None
             else "rise with W: `CONFIRMED`" if rising else "rise with W: **`REFUTED`**")
    half2 = ("**STARVATION**" if c["starvation_flag"]
             else "fallback 0 — `PASS_categorical_by_design` (needs 64 consecutive failed walks)")
    return f"{half1}; {half2}"


def health_table(runs: list[tuple[str, dict]]) -> str:
    cells = [(run, c) for run, art in runs for c in art.get("health", [])]
    if not cells:
        return "_no health cells in results/_\n"
    extras = [(k, h) for k, h in HEALTH_EXTRA if any(k in c for _, c in cells)]
    head = ["Arm", "W", "R", "run", "restart share, median [min, max]", "fallback share",
            "`sample_spins` ÷ `read_ops` (ratio of medians)", "`locked_reads` ÷ `read_ops`",
            *[h for _, h in extras], "§11.5.3"]
    out = ["| " + " | ".join(head) + " |",
           "|---|--:|--:|--:|---|---:|---:|---:|" + "---:|" * len(extras) + "---|"]
    # "rise with W", per arm and per run, on the medians.
    rising = {}
    for run, art in runs:
        for arm in {c["arm"] for c in art.get("health", [])}:
            rows = sorted((c for c in art["health"] if c["arm"] == arm and c["read_ops"]["median"] > 0),
                          key=lambda x: x["writers"])
            shares = [c["restart_share"]["median"] for c in rows]
            rising[(run, arm)] = None if len(shares) < 2 else all(b > a for a, b in zip(shares, shares[1:]))
    order = {a: i for i, a in enumerate(ARMS)}
    for run, c in sorted(cells, key=lambda rc: (order.get(rc[1]["arm"], 9), rc[1]["writers"], rc[0])):
        rs, fs = c["restart_share"], c["fallback_share"]
        spins = c["sample_spins"]["median"] / max(c["read_ops"]["median"], 1)
        row = [c["arm"], str(c["writers"]), str(c["readers"]), run,
               f"{rs['median']:.2%} [{rs['min']:.2%}, {rs['max']:.2%}]", f"{fs['median']:.4%}",
               f"{spins:.2f}", stat_cell(c, "locked_share", "{:.2%}"),
               *[stat_cell(c, k, "{:.3f}" if k.endswith("per_write") else "{:.2%}") for k, _ in extras],
               health_verdict(c, rising.get((run, c["arm"])))]
        out.append("| " + " | ".join(row) + " |")
    out.append("")
    return "\n".join(out)


def health_section() -> str:
    runs = [(run, art) for run, name in HEALTH_RUNS if (art := load_optional(name)) is not None]
    if not runs:
        return "### 7.3 Protocol health — event ratios from the diagnostic build\n\n_no concurrent artifact in results/_\n"
    out = ["### 7.3 Protocol health — event ratios from the diagnostic build\n", health_table(runs)]
    for run, art in runs:
        prov = art["provenance"]
        loads = [s["load1"] for s in prov["loads"]]
        out.append(f"load average across the concurrent run {run}: {', '.join(f'{x:.2f}' for x in loads)}; "
                   f"commit {prov.get('commit', '?')}; core pin: {prov.get('core_pin', 'unset')}")
    return "\n".join(out)


def main() -> int:
    lat = load("baseline_latency.json")
    mem = load("baseline_memory_curve.json")
    prov = lat["provenance"]
    idx = cell_index(lat)
    print(f"<!-- generated by scripts/integer_tables.py from results/ at commit {prov['commit']} -->\n")
    print(latency_table(idx, "lookup_hit", "Point lookup, 100% hit", True))
    print(latency_table(idx, "lookup_miss",
                        "Point lookup, 50% hit / 50% rejection-sampled miss", True))
    print(latency_table(idx, "insert", "Insertion into a cold structure", False))
    print("### Ordered range scan, `random` (HOT ÷ Expanse per visited element)\n")
    print(scan_table(idx))
    print("### Scorecard\n")
    print(scorecard(lat, mem))
    print(scan_share(lat))
    loads = [s["load1"] for s in prov["loads"]]
    print(f"load average across the run: {', '.join(f'{x:.2f}' for x in loads)}; "
          f"core pin: {prov.get('core_pin', 'unset')}\n")
    print(health_section())
    return 0


if __name__ == "__main__":
    sys.exit(main())
