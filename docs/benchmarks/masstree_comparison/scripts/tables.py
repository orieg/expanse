#!/usr/bin/env python3
"""Markdown tables for the Masstree arm (#661), derived from results/.

Every cell, ratio, verdict, flag and count below is computed from the committed
``results/baseline_*.json`` (AGENTS.md §8.2: nothing in a generated table is
typed by hand). The README's result sections are pasted from this script's
output, so a re-measurement regenerates the tables rather than editing numbers
in place.

    python3 docs/benchmarks/masstree_comparison/scripts/tables.py [--quick] [--all]

The pre-registered directions (METHODOLOGY §6) are encoded once, below, so the
CONFIRMED / REFUTED / UNPREDICTED LOSS labels are derived too. Ratios follow §5:
latency is Masstree ÷ Expanse and throughput is Expanse ÷ Masstree, so above
1.000 always means Expanse is faster.
"""

import json
import sys
from collections import Counter
from pathlib import Path

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results" / ("quick" if "--quick" in sys.argv else "")

# ---------------------------------------------------------------------------
# METHODOLOGY §6, encoded. Returns 'masstree', 'expanse', 'boundary_or_masstree'
# or '' (not pre-registered) for a latency / throughput cell.
# ---------------------------------------------------------------------------

def registered_int(cell: dict) -> str:
    p, d, k = cell["pillar"], cell["dist"], cell.get("scan_k", 0)
    if p == "scan":
        return "masstree" if k in (10, 100) else ""
    if p == "lookup_hit":
        return {"sequential": "expanse", "sparse": "expanse", "random": "expanse"}.get(d, "")
    if p == "lookup_miss":
        # §6.4: the 50% pillar is predicted only where the hit prediction is high.
        return {"sequential": "expanse", "sparse": "expanse"}.get(d, "")
    if p == "insert":
        return "expanse"
    return ""


def registered_str(cell: dict) -> str:
    p, d, n = cell["pillar"], cell["dist"], cell["population"]
    if p == "scan":
        return "masstree"
    if d == "prefixed" and p in ("lookup_hit", "lookup_miss", "insert"):
        return "masstree"
    if d == "counter" and p in ("lookup_hit", "insert") and n >= 990_000:
        return "expanse"
    if d == "short" and p == "lookup_hit":
        return "expanse"
    return ""


def registered_conc(cell: dict, role: str) -> str:
    w, r = cell["writers"], cell["readers"]
    if role == "writer":
        if w >= 16:
            return ""          # §6.4: SMT
        if w >= 2:
            return "masstree"
        return "boundary_or_masstree"
    # reader
    if w == 0:
        return "expanse"
    return "masstree" if w < 16 else ""


def label(v: str, reg: str) -> str:
    if v == "NOT_REPRESENTABLE_MASSTREE":
        return "no Masstree cell (§3.4)"
    if v == "BOUNDARY_RESULT":
        if reg == "boundary_or_masstree":
            return "`BOUNDARY_RESULT` — `CONFIRMED`"
        return "`BOUNDARY_RESULT`"
    who = "Expanse" if v == "expanse" else "Masstree"
    if not reg:
        return f"{who} — `not pre-registered`"
    if reg == "boundary_or_masstree":
        return f"{who} — `CONFIRMED`" if v == "masstree" else f"{who} — **`REFUTED`** (in Expanse's favour)"
    if v == reg:
        return f"{who} — `CONFIRMED`"
    if reg == "expanse" and v == "masstree":
        return f"{who} — **`UNPREDICTED LOSS`**"
    return f"{who} — **`REFUTED`**"


def load(name: str) -> dict:
    p = RESULTS / name
    return json.loads(p.read_text()) if p.exists() else None


def fmt_ratio(c: dict, key: str = "masstree_over_expanse") -> str:
    if c.get(key) is None:
        return "—"
    return f"{c[key]:.3f} [{c['ci_lower']:.3f}, {c['ci_upper']:.3f}]"


def fmt_mt(v, not_rep: int) -> str:
    return f"withheld ({not_rep:,} keys > 255 B)" if v is None else f"{v:.2f}"


# ---------------------------------------------------------------------------

def int_latency_tables(lat: dict) -> str:
    cells = lat["cells"]
    pops = sorted({c["population"] for c in cells})
    show = pops if "--all" in sys.argv else [pops[-1]]
    out = []
    for n in show:
        for pillar, title in (("lookup_hit", "Point lookup, 100% hit"),
                              ("lookup_miss", "Point lookup, 50% hit / 50% rejection-sampled miss"),
                              ("insert", "Insertion into a cold structure")):
            out.append(f"### {title}, integer keys (N = {n:,})\n")
            out.append("| Distribution | λ | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |")
            out.append("|---|---:|---:|---:|---:|---|")
            for c in sorted((x for x in cells if x["pillar"] == pillar and x["population"] == n), key=lambda x: x["dist"]):
                lam = f"{c['lambda']:.1f}" if c["dist"] == "random" else "—"
                out.append(f"| `{c['dist']}` | {lam} | {c['masstree_ns_per_op_median']:.2f} | {c['expanse_ns_per_op_median']:.2f} | "
                           f"{fmt_ratio(c)} | {label(c['verdict'], registered_int(c))} |")
            out.append("")
        out.append(f"### Ordered range scan, integer keys (N = {n:,}; Masstree ÷ Expanse per visited element)\n")
        out.append("| Distribution | k=10 | k=100 | k=1000 |")
        out.append("|---|---:|---:|---:|")
        for d in ("sequential", "clustered", "sparse", "random"):
            row = []
            for k in (10, 100, 1000):
                m = [c for c in cells if (c["pillar"], c["dist"], c["population"], c["scan_k"]) == ("scan", d, n, k)]
                row.append(fmt_ratio(m[0]) + (" · " + label(m[0]["verdict"], registered_int(m[0])).split(" — ")[-1]
                                             if m and m[0]["verdict"] != "BOUNDARY_RESULT" else "") if m else "—")
            out.append(f"| `{d}` | " + " | ".join(row) + " |")
        out.append("")
    return "\n".join(out)


def int_memory_tables(mem: dict) -> str:
    cells = mem["cells"]
    out = ["### Memory, integer map: two instruments per cell, bytes per key\n",
           "`allocator` is what the process holds from the C allocator after a build-only "
           "population, one instrument for both arms; on Masstree it is quantized to the 2 MiB "
           "pool slab and a cell whose measured slack exceeds a quarter of its structural bytes "
           "is flagged `QUANTUM_DOMINATED` (§3.3). `structural` is Masstree's own `json_stats` "
           "node census; `mem_used` is Expanse's own accounting. The engine columns are never "
           "mixed with the allocator columns in one ratio.\n",
           "| Distribution | λ | N | Masstree allocator (unsettled) | Expanse allocator | Masstree structural | Expanse `mem_used` | Masstree slack | slabs | leaf fill | flag |",
           "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    for c in sorted(cells, key=lambda x: (x["dist"] != "random", x["lambda"] or 0, x["dist"])):
        lam = f"{c['lambda']:.0f}" if c["lambda"] is not None else "—"
        out.append(f"| `{c['dist']}` | {lam} | {c['population']:,} | {c['masstree_alloc_bytes_per_key']:.2f} "
                   f"({c['masstree_unsettled_bytes_per_key']:.2f}) | "
                   f"{c['expanse_alloc_bytes_per_key']:.2f} | {c['masstree_structural_bytes_per_key']:.2f} | "
                   f"{c['expanse_mem_used_bytes_per_key']:.2f} | {c['masstree_slack_bytes_per_key']:.2f} | "
                   f"{c['masstree_slabs']} | {c['masstree_leaf_fill']:.3f} | `{c['masstree_label']}` |")
    out.append("")
    return "\n".join(out)


def str_latency_tables(lat: dict) -> str:
    cells = lat["cells"]
    pops = sorted({c["population"] for c in cells})
    # Populations are post-dedup counts; group by the requested bucket.
    req = [1_000, 10_000, 100_000, 1_000_000]
    bucket = lambda n: min(req, key=lambda r: abs(r - n))  # noqa: E731
    heads = sorted({bucket(c["population"]) for c in cells})
    show = heads if "--all" in sys.argv else [heads[-1]]
    out = []
    for n in show:
        for pillar, title in (("lookup_hit", "Point lookup, 100% hit"),
                              ("lookup_miss", "Point lookup, 50% hit / 50% rejection-sampled miss"),
                              ("insert", "Insertion into a cold structure")):
            out.append(f"### {title}, string keys (N = {n:,})\n")
            out.append("| Shape | N held | mean len | Masstree ns | Expanse ns | Masstree ÷ Expanse [BCa 95%] | Verdict |")
            out.append("|---|---:|---:|---:|---:|---:|---|")
            for c in sorted((x for x in cells if x["pillar"] == pillar and bucket(x["population"]) == n), key=lambda x: x["dist"]):
                out.append(f"| `{c['dist']}` | {c['population']:,} | {c['mean_key_len']:.1f} | "
                           f"{fmt_mt(c['masstree_ns_per_op_median'], c['masstree_not_representable'])} | "
                           f"{c['expanse_ns_per_op_median']:.2f} | {fmt_ratio(c)} | {label(c['verdict'], registered_str(c))} |")
            out.append("")
        out.append(f"### Ordered range scan, string keys (N = {n:,}; Masstree ÷ Expanse per visited element)\n")
        out.append("| Shape | k=10 | k=100 | k=1000 |")
        out.append("|---|---:|---:|---:|")
        for d in ("short", "counter", "prefixed", "skewed", "beyond"):
            row = []
            for k in (10, 100, 1000):
                m = [c for c in cells if (c["pillar"], c["dist"], bucket(c["population"]), c["scan_k"]) == ("scan", d, n, k)]
                if not m:
                    row.append("—")
                elif m[0]["masstree_over_expanse"] is None:
                    row.append(f"Expanse {m[0]['expanse_ns_per_op_median']:.0f} ns; Masstree withheld")
                else:
                    row.append(fmt_ratio(m[0]))
            out.append(f"| `{d}` | " + " | ".join(row) + " |")
        out.append("")
    return "\n".join(out)


def str_memory_tables(mem: dict) -> str:
    cells = mem["cells"]
    pops = sorted({c["population"] for c in cells})
    req = [1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000, 125_000, 150_000, 200_000, 500_000, 1_000_000]
    bucket = lambda n: min(req, key=lambda r: abs(r - n))  # noqa: E731
    heads = sorted({bucket(c["population"]) for c in cells})
    show = heads if "--all" in sys.argv else [heads[-1]]
    out = ["### Memory, string map: two instruments per cell, bytes per key\n",
           "Both sides copy key bytes into their own nodes, so the index column is the ownership "
           "column on both (§4). Columns as the integer table.\n",
           "| Shape | N | mean len | Masstree allocator (unsettled) | Expanse allocator | Masstree structural | Expanse `mem_used` | Masstree slack | layers | flag |",
           "|---|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    for c in sorted((x for x in cells if bucket(x["population"]) in show), key=lambda x: (x["dist"], x["population"])):
        nr = c["masstree_not_representable"]
        mt = c["masstree_alloc_bytes_per_key"]
        uns = "" if c["masstree_unsettled_bytes_per_key"] is None else f" ({c['masstree_unsettled_bytes_per_key']:.2f})"
        out.append(f"| `{c['dist']}` | {c['population']:,} | {c['mean_key_len']:.1f} | {fmt_mt(mt, nr)}{uns} | "
                   f"{c['expanse_alloc_bytes_per_key']:.2f} | "
                   f"{fmt_mt(c['masstree_structural_bytes_per_key'], nr)} | {c['expanse_mem_used_bytes_per_key']:.2f} | "
                   f"{fmt_mt(c['masstree_slack_bytes_per_key'], nr)} | {c['masstree_layers'] if c['masstree_layers'] is not None else '—'} | "
                   f"`{c['masstree_label']}` |")
    out.append("")
    out.append("### String map, allocator column across the population sweep (Masstree / Expanse B/key)\n")
    dists = sorted({c["dist"] for c in cells})
    out.append("| N | " + " | ".join(f"`{d}`" for d in dists) + " |")
    out.append("|---:|" + "---:|" * len(dists))
    for n in heads:
        row = []
        for d in dists:
            m = [c for c in cells if (c["dist"], bucket(c["population"])) == (d, n)]
            if not m:
                row.append("—")
                continue
            c = m[0]
            mt = c["masstree_alloc_bytes_per_key"]
            flag = "†" if c.get("masstree_quantum_dominated") else ""
            row.append(f"{'—' if mt is None else f'{mt:.1f}{flag}'} / {c['expanse_alloc_bytes_per_key']:.1f}")
        out.append(f"| {n:,} | " + " | ".join(row) + " |")
    out.append("\n† `QUANTUM_DOMINATED`: the allocator figure is mostly the 2 MiB slab, not the index (§3.3).\n")
    return "\n".join(out)


def concurrent_tables(conc: dict) -> str:
    out = []
    for arm, title in (("map", "MC1 — `u64` keys, Masstree vs `SyncExpanseMap`"),
                       ("str", "MC2 — `short` string keys, Masstree vs `SyncExpanseStrMap`")):
        cells = [c for c in conc["throughput"] if c["arm"] == arm]
        if not cells:
            continue
        out.append(f"### {title}\n")
        out.append("**C1 — writer throughput as writer count scales** (W writers insert 2²⁰ fresh keys into a 2²⁰ prefill; fixed work; Expanse ÷ Masstree)\n")
        out.append("| W | Masstree M/s | Expanse M/s | ratio [BCa 95%] | verdict |")
        out.append("|--:|---:|---:|---|---|")
        for c in sorted((x for x in cells if x["pillar"] == "C1"), key=lambda x: x["writers"]):
            wr = f"{c['writer_expanse_over_masstree']:.3f} [{c['writer_ci_lower']:.3f}, {c['writer_ci_upper']:.3f}]"
            out.append(f"| {c['writers']} | {c['masstree_writer_mops_median']:.2f} | {c['expanse_writer_mops_median']:.2f} | "
                       f"{wr} | {label(c['writer_verdict'], registered_conc(c, 'writer'))} |")
        out.append("")
        out.append("**C2 — reader throughput alongside writers** (8 readers probe 50/50 while W writers insert; W = 0 is the reader-only reference)\n")
        out.append("| W | Masstree readers M/s | Expanse readers M/s | ratio [BCa 95%] | verdict | Masstree writers M/s | Expanse writers M/s | writer ratio |")
        out.append("|--:|---:|---:|---|---|---:|---:|---|")
        for c in sorted((x for x in cells if x["pillar"] == "C2"), key=lambda x: x["writers"]):
            wcol = ("—", "—", "—")
            if "writer_expanse_over_masstree" in c:
                wcol = (f"{c['masstree_writer_mops_median']:.2f}", f"{c['expanse_writer_mops_median']:.2f}",
                        f"{c['writer_expanse_over_masstree']:.3f} [{c['writer_ci_lower']:.3f}, {c['writer_ci_upper']:.3f}]")
            out.append(f"| {c['writers']} | {c['masstree_reader_mops_median']:.2f} | {c['expanse_reader_mops_median']:.2f} | "
                       f"{c['reader_expanse_over_masstree']:.3f} [{c['reader_ci_lower']:.3f}, {c['reader_ci_upper']:.3f}] | "
                       f"{label(c['reader_verdict'], registered_conc(c, 'reader'))} | {wcol[0]} | {wcol[1]} | {wcol[2]} |")
        out.append("")
    health = conc.get("health", [])
    if health:
        out.append("### H — protocol health, Expanse side only (occ-stats build; event ratios, never a timing)\n")
        out.append("| Arm | W | R | restart share, median [min, max] | fallback share, median | `sample_spins` ÷ `read_ops` (medians) | flag |")
        out.append("|---|--:|--:|---|---|---:|---|")
        for c in sorted(health, key=lambda x: (x["arm"], x["writers"])):
            rs, fs = c["restart_share"], c["fallback_share"]
            spins = c["sample_spins"]["median"] / max(c["read_ops"]["median"], 1)
            out.append(f"| {c['arm']} | {c['writers']} | {c['readers']} | {rs['median']:.2%} [{rs['min']:.2%}, {rs['max']:.2%}] | "
                       f"{fs['median']:.4%} | {spins:.2f} | {'**STARVATION** (§6.3)' if c['starvation_flag'] else 'below 1% — §6.3 holds'} |")
        out.append("")
    mem = conc.get("memory", [])
    if mem:
        out.append("### M — build-only single-writer census, Masstree vs `SyncExpanseMap` (B/key)\n")
        out.append("| λ | N | Masstree allocator | SyncExpanseMap allocator | Masstree structural | Expanse `mem_used` | flag |")
        out.append("|---:|---:|---:|---:|---:|---:|---|")
        for c in sorted(mem, key=lambda x: x["lambda_target"]):
            out.append(f"| {c['lambda_target']:.0f} | {c['population']:,} | {c['masstree_alloc_bytes_per_key']:.2f} | "
                       f"{c['expanse_alloc_bytes_per_key']:.2f} | {c['masstree_structural_bytes_per_key']:.2f} | "
                       f"{c['expanse_mem_used_bytes_per_key']:.2f} | `{c['masstree_label']}` |")
        out.append("")
    return "\n".join(out)


def order_tables(sens: dict) -> str:
    """§10.2: the same population in the generator's sorted order and shuffled.
    No verdict label — this table is a sensitivity disclosure, not a cell."""
    out = ["### Sensitivity (§10.2 insertion order, §10.3 table configuration) — both arms, same population\n",
           "Sorted / single is the registered configuration every cell above was built in. Shuffled is a "
           "Fisher–Yates permutation of the same keys (Masstree's leaf fill and footprint depend on the order; "
           "Expanse's footprint does not). Concurrent is Masstree's fenced, spin-locked node version, the "
           "configuration the MC cells use, driven single-threaded here to show the protocol's own cost. "
           "Ratios are Masstree ÷ Expanse; no verdict is given against §6.\n",
           "| Arm | Shape | Order | Table | N | Masstree allocator (unsettled) | Masstree structural | leaf fill | Expanse allocator | lookup_hit ratio [BCa 95%] | insert ratio [BCa 95%] |",
           "|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|"]
    lat = sens.get("latency", [])
    for m in sorted(sens.get("memory", []), key=lambda x: (x["arm"], x["dist"], x["order"] != "sorted", x.get("table") != "single")):
        def r(pillar):
            c = [x for x in lat if (x["arm"], x["dist"], x["order"], x.get("table"), x["pillar"])
                 == (m["arm"], m["dist"], m["order"], m.get("table"), pillar)]
            return fmt_ratio(c[0]) if c else "—"
        out.append(f"| {m['arm']} | `{m['dist']}` | {m['order']} | {m.get('table', 'single')} | {m['population']:,} | "
                   f"{m['masstree_alloc_bytes_per_key']:.2f} ({m['masstree_unsettled_bytes_per_key']:.2f}) | "
                   f"{m['masstree_structural_bytes_per_key']:.2f} | {m['masstree_leaf_fill']:.3f} | {m['expanse_alloc_bytes_per_key']:.2f} | "
                   f"{r('lookup_hit')} | {r('insert')} |")
    out.append("")
    return "\n".join(out)


def scorecard(lat, slat, conc) -> str:
    labels = Counter()
    counts = Counter()
    withheld = 0
    if lat:
        for c in lat["cells"]:
            counts[c["verdict"]] += 1
            labels[label(c["verdict"], registered_int(c))] += 1
    if slat:
        for c in slat["cells"]:
            if c["verdict"] == "NOT_REPRESENTABLE_MASSTREE":
                withheld += 1
                continue
            counts[c["verdict"]] += 1
            labels[label(c["verdict"], registered_str(c))] += 1
    if conc:
        for c in conc["throughput"]:
            for role in ("writer", "reader"):
                if f"{role}_verdict" in c:
                    counts[c[f"{role}_verdict"]] += 1
                    labels[label(c[f"{role}_verdict"], registered_conc(c, role))] += 1
    out = ["### Scorecard (wall-clock cells with a Masstree column)\n", "| | Count |", "|---|---:|",
           f"| Expanse wins (CI excludes parity) | {counts.get('expanse', 0)} |",
           f"| Masstree wins (CI excludes parity) | {counts.get('masstree', 0)} |",
           f"| `BOUNDARY_RESULT` | {counts.get('BOUNDARY_RESULT', 0)} |",
           f"| Masstree column withheld (§3.4, `beyond`) | {withheld} |",
           "", "| Label | Cells |", "|---|---:|"]
    for k, v in sorted(labels.items(), key=lambda kv: -kv[1]):
        out.append(f"| {k} | {v} |")
    out.append("")
    return "\n".join(out)


def main() -> int:
    lat, slat = load("baseline_latency.json"), load("baseline_string_latency.json")
    mem, smem = load("baseline_memory.json"), load("baseline_string_memory.json")
    conc = load("baseline_concurrent.json")
    sens = load("baseline_sensitivity.json")
    prov = (lat or slat or conc or {}).get("provenance", {})
    print(f"<!-- generated by scripts/tables.py from results/ at commit {prov.get('commit', '?')} -->\n")
    if mem:
        print(int_memory_tables(mem))
    if smem:
        print(str_memory_tables(smem))
    if lat:
        print(int_latency_tables(lat))
    if slat:
        print(str_latency_tables(slat))
    if sens:
        print(order_tables(sens))
    if conc:
        print(concurrent_tables(conc))
    print(scorecard(lat, slat, conc))
    for name, d in (("main", lat), ("concurrent", conc)):
        if d:
            loads = [s["load1"] for s in d["provenance"]["loads"]]
            print(f"load average across the {name} run: {', '.join(f'{x:.2f}' for x in loads)}; "
                  f"core pin: {d['provenance'].get('core_pin', 'unset')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
