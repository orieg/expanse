#!/usr/bin/env python3
"""Markdown tables for the string-key arms (#693), derived from results/.

Every cell, ratio, verdict and count below is computed from
``results/baseline_string_memory.json`` and ``results/baseline_string_latency.json``
(AGENTS.md §8.2: nothing in a generated table is typed by hand). The README's
string section is pasted from this script's output, so a re-measurement
regenerates the tables rather than editing numbers in place.

    python3 docs/benchmarks/hot_comparison/scripts/string_tables.py [--quick]

The pre-registered directions (METHODOLOGY §10.7) are encoded once, below, so
the scorecard's CONFIRMED / REFUTED / UNPREDICTED LOSS labels are also derived.
"""

import json
import sys
from collections import Counter
from pathlib import Path

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results" / ("quick" if "--quick" in sys.argv else "")

ARM_NAME = {"ptr": "C · ptr", "map": "D · map", "bytes": "E · bytes"}
EXPANSE_NAME = {"ptr": "ExpanseStrMap", "map": "ExpanseStrMap", "bytes": "ExpanseBytesMap"}

# §10.7, encoded. Key: (pillar, arm, dist) -> expected winner. Scan and the
# `index` memory column are shape-independent predictions.
PREREG_LOSS = {
    ("lookup_hit", "prefixed"), ("lookup_miss", "prefixed"),
    ("insert", "prefixed"),
    ("lookup_hit", "skewed"), ("lookup_miss", "skewed"),
}
PREREG_LOSS_ARMS = {"ptr", "map"}
PREREG_WIN = {
    ("lookup_hit", "counter", "ptr"), ("lookup_hit", "counter", "map"),
    ("insert", "counter", "ptr"), ("insert", "counter", "map"),
    ("lookup_hit", "short", "ptr"),
}


def load(name: str) -> dict:
    return json.loads((RESULTS / name).read_text())


# The runner's requested populations. A cell's actual population is what the
# generator held after deduplication (`skewed` at N = 1,000,000 holds 998,150
# distinct keys), so cells are grouped by the requested N they belong to and the
# actual count is printed beside it. Deduplication only ever shrinks by well
# under 1%, so nearest-requested is unambiguous.
REQUESTED = [1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000, 125_000,
             150_000, 200_000, 500_000, 1_000_000]


def bucket(n: int) -> int:
    return min(REQUESTED, key=lambda r: abs(r - n))


def fmt_hot(v, not_rep: int) -> str:
    if v is None:
        return f"withheld ({not_rep:,} keys > 255 B)"
    return f"{v:.2f}"


def registered_direction(cell: dict) -> str:
    """'hot' / 'expanse' / '' for the pre-registered winner of a latency cell."""
    p, a, d = cell["pillar"], cell["arm"], cell["dist"]
    if p == "scan" and a in PREREG_LOSS_ARMS:
        return "hot"
    if (p, d) in PREREG_LOSS and a in PREREG_LOSS_ARMS:
        return "hot"
    if (p, d, a) in PREREG_WIN:
        return "expanse"
    return ""


def label(cell: dict) -> str:
    """§6 gate taxonomy, derived from verdict and registration."""
    v = cell["verdict"]
    if v == "NOT_REPRESENTABLE_HOT":
        return "no HOT cell (§10.4)"
    reg = registered_direction(cell)
    if v == "BOUNDARY_RESULT":
        return "`BOUNDARY_RESULT`"
    if not reg:
        return f"{'Expanse' if v == 'expanse' else 'HOT'} — `not pre-registered`"
    if v == reg:
        return f"{'Expanse' if v == 'expanse' else 'HOT'} — `CONFIRMED`"
    if reg == "expanse" and v == "hot":
        return "HOT — **`UNPREDICTED LOSS`**"
    return f"{'Expanse' if v == 'expanse' else 'HOT'} — **`REFUTED`**"


def latency_tables(lat: dict) -> str:
    cells = lat["cells"]
    pops = sorted({bucket(c["population"]) for c in cells})
    n_head = pops[-1]
    out = []
    for pillar, title in (
        ("lookup_hit", "Point lookup, 100% hit"),
        ("lookup_miss", "Point lookup, 50% hit / 50% rejection-sampled miss"),
        ("insert", "Insertion into a cold structure"),
    ):
        out.append(f"### {title} (N = {n_head:,})\n")
        out.append("| Shape | Arm | N held | mean len | HOT ns | Expanse ns | HOT ÷ Expanse [BCa 95%] | Verdict |")
        out.append("|---|---|---:|---:|---:|---:|---:|---|")
        for c in sorted((x for x in cells if x["pillar"] == pillar and bucket(x["population"]) == n_head),
                        key=lambda x: (x["arm"], x["dist"])):
            hot = fmt_hot(c["hot_ns_per_op_median"], c["hot_not_representable"])
            ratio = ("—" if c["hot_over_expanse"] is None
                     else f"{c['hot_over_expanse']:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]")
            out.append(f"| `{c['dist']}` | {ARM_NAME[c['arm']]} | {c['population']:,} | {c['mean_key_len']:.1f} | {hot} | "
                       f"{c['expanse_ns_per_op_median']:.2f} | {ratio} | {label(c)} |")
        out.append("")

    out.append("### Ordered range scan (Arms C and D; HOT ÷ Expanse per visited element)\n")
    out.append("| Arm | Shape | N | k=10 | k=100 | k=1000 |")
    out.append("|---|---|---:|---:|---:|---:|")
    scan = [c for c in cells if c["pillar"] == "scan"]
    keys = sorted({(c["arm"], c["dist"], bucket(c["population"])) for c in scan})
    for arm, dist, n in keys:
        row = []
        for k in (10, 100, 1000):
            m = [c for c in scan
                 if (c["arm"], c["dist"], bucket(c["population"]), c["scan_k"]) == (arm, dist, n, k)]
            if not m:
                row.append("—")
                continue
            c = m[0]
            if c["hot_over_expanse"] is None:
                row.append(f"Expanse {c['expanse_ns_per_op_median']:.0f} ns; HOT withheld")
            else:
                row.append(f"{c['hot_over_expanse']:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]")
        out.append(f"| {ARM_NAME[arm]} | `{dist}` | {n:,} | " + " | ".join(row) + " |")
    out.append("")
    return "\n".join(out)


def memory_tables(mem: dict) -> str:
    cells = mem["cells"]
    pops = sorted({bucket(c["population"]) for c in cells})
    # The README shows the head population; `--all` prints every swept N.
    show = pops if "--all" in sys.argv else ([pops[-1]] if pops else [])
    out = []
    out.append("### Memory: three columns per cell, bytes per key\n")
    out.append("`index` is what the allocator holds for the index alone, on both arms under one "
               "instrument. `external` is the harness-owned string table HOT's leaves point at, "
               "as the allocator holds it (exact `Σ(len+1)` in parentheses). `ownership` adds the "
               "external storage to HOT and nothing to Expanse, which copies its keys (§10.3).\n")
    out.append("| Shape | Arm | N | external (exact) | HOT index | Expanse index | HOT ownership | Expanse ownership | Expanse `mem_used` |")
    out.append("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    for c in sorted((x for x in cells if bucket(x["population"]) in show),
                    key=lambda x: (x["dist"], x["arm"], x["population"])):
        nr = c["hot_not_representable"]
        out.append(
            f"| `{c['dist']}` | {ARM_NAME[c['arm']]} | {c['population']:,} | "
            f"{c['external_alloc_bytes_per_key']:.2f} ({c['key_bytes_per_key']:.2f}) | "
            f"{fmt_hot(c['hot_index_bytes_per_key'], nr)} | {c['expanse_index_bytes_per_key']:.2f} | "
            f"{fmt_hot(c['hot_ownership_bytes_per_key'], nr)} | {c['expanse_ownership_bytes_per_key']:.2f} | "
            f"{c['expanse_mem_used_bytes_per_key']:.2f} |")
    out.append("")

    # Full sweep for Arm C: ownership across N, to show the shape of the curve.
    out.append("### Arm C ownership across the population sweep (B/key)\n")
    dists = sorted({c["dist"] for c in cells})
    out.append("| N | " + " | ".join(f"`{d}` HOT / Expanse" for d in dists) + " |")
    out.append("|---:|" + "---:|" * len(dists))
    for n in pops:
        row = []
        for d in dists:
            m = [c for c in cells if (c["arm"], c["dist"], bucket(c["population"])) == ("ptr", d, n)]
            if not m:
                row.append("—")
                continue
            c = m[0]
            h = c["hot_ownership_bytes_per_key"]
            row.append(f"{'—' if h is None else f'{h:.1f}'} / {c['expanse_ownership_bytes_per_key']:.1f}")
        out.append(f"| {n:,} | " + " | ".join(row) + " |")
    out.append("")
    return "\n".join(out)


def scorecard(lat: dict) -> str:
    cells = [c for c in lat["cells"] if c["verdict"] != "NOT_REPRESENTABLE_HOT"]
    counts = Counter(c["verdict"] for c in cells)
    withheld = sum(1 for c in lat["cells"] if c["verdict"] == "NOT_REPRESENTABLE_HOT")
    labels = Counter(label(c) for c in cells)
    out = ["### Scorecard (latency cells with a HOT column)\n",
           "| | Count |", "|---|---:|",
           f"| Expanse wins (CI excludes parity) | {counts.get('expanse', 0)} |",
           f"| HOT wins (CI excludes parity) | {counts.get('hot', 0)} |",
           f"| `BOUNDARY_RESULT` | {counts.get('BOUNDARY_RESULT', 0)} |",
           f"| HOT column withheld (§10.4, `beyond`) | {withheld} |",
           "", "| Label | Cells |", "|---|---:|"]
    for k, v in sorted(labels.items(), key=lambda kv: -kv[1]):
        out.append(f"| {k} | {v} |")
    out.append("")
    return "\n".join(out)


def main() -> int:
    mem = load("baseline_string_memory.json")
    lat = load("baseline_string_latency.json")
    prov = lat["provenance"]
    print(f"<!-- generated by scripts/string_tables.py from results/ at commit {prov['commit']} -->\n")
    print(memory_tables(mem))
    print(latency_tables(lat))
    print(scorecard(lat))
    loads = [s["load1"] for s in prov["loads"]]
    print(f"load average across the run: {', '.join(f'{x:.2f}' for x in loads)}; "
          f"core pin: {prov.get('core_pin', 'unset')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
