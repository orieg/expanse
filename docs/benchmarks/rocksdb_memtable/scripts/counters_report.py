#!/usr/bin/env python3
"""The #802 concurrent arm's hardware counters, rendered as markdown.

Reads `results/counters/pin_<label>/<run id>/`, where each directory holds one
`rocksdb_concurrent_counters` dispatch: `bench_counters.py`'s six
`counters_rocksdb_conc_*.json` cells and its two `perf c2c` reports. Every
figure printed is read from those files or from the wall-clock artifacts beside
them; nothing is typed by hand (AGENTS.md section 8.2).

It refuses:

- **A run that did not run on its directory's pin.** Every cell's launch pin
  and every harness row's own `cpus_allowed` must equal the directory's CPU
  set. A run could once record one pin and run on another, which #915 fixed in
  the driver and which this checks again at the artifact.
- **A pin with fewer than two runs.** A within-run interval does not bound
  between-run spread (docs/BENCHMARKING.md rule 18), so no pin comparison is
  read from one run.

It prints:

1. selected per-thread counters, every cell of every run;
2. pin effects that replicate: a (cell, event) whose intervals in both runs of
   one pin are separated from both runs of the other pin, in one direction;
3. same-pin pairs whose intervals separate, which is between-run spread;
4. achieved read and writer rates, beside the wall-clock artifacts at each pin;
5. each `perf c2c` report's load HITMs and top shared cache lines, with the
   symbols the sampler attributes each line's HITMs to.

It decides nothing about a mechanism: counters and c2c are observational
(AGENTS.md section 8.20.3).

    counters_report.py [--root DIR]
    counters_report.py --self-test
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RESULTS = HERE.parent / "results"
ROOT = RESULTS / "counters"

# Directory label -> the CPU set a run under it must have run on.
PIN_SETS = {"0-15": "0-15", "one_sibling": "0,2,4,6,8,10,12,14"}
WIDE, NARROW = "0-15", "one_sibling"
CELLS = ("idle_r1", "idle_r2", "idle_r7", "paced_r1", "paced_r2", "paced_r7")
C2C_CELLS = ("idle_r7", "paced_r7")
TABLE_EVENTS = (
    "reader/instructions", "reader/cycles", "reader/mem_load_l3_hit_retired.xsnp_hitm",
    "reader/l2_rqsts.rfo_miss", "reader/context-switches",
    "writer/instructions", "writer/mem_load_l3_hit_retired.xsnp_hitm",
    "writer/context-switches",
)
KERNEL_FLOOR = 0xFFFF800000000000


def cpu_set(cpulist: str) -> set[int]:
    out: set[int] = set()
    for part in cpulist.split(","):
        part = part.strip()
        if not part:
            continue
        lo, _, hi = part.partition("-")
        out.update(range(int(lo), int(hi or lo) + 1))
    return out


def fmt(v: float | None) -> str:
    if v is None:
        return "—"
    if abs(v) >= 100:
        return f"{v:,.0f}"
    if abs(v) >= 1:
        return f"{v:.3g}"
    return f"{v:.3g}"


# --------------------------------------------------------------------------
# runs, and the pin each must have run on
# --------------------------------------------------------------------------
def load_run(run_dir: Path) -> dict[str, dict]:
    cells = {}
    for f in sorted(glob.glob(str(run_dir / "counters_rocksdb_conc_*.json"))):
        name = os.path.basename(f)[len("counters_rocksdb_conc_"):-len(".json")]
        cells[name] = json.loads(Path(f).read_text())
    return cells


def pin_problems(cells: dict[str, dict], cpulist: str) -> list[str]:
    """Every way a run's cells did not run on `cpulist`; empty when they all did."""
    want = cpu_set(cpulist)
    out = []
    for name in CELLS:
        if name not in cells:
            out.append(f"{name}: missing")
            continue
        art = cells[name]
        launch = (art.get("provenance", {}).get("pin") or [None])[-1]
        if launch is None or cpu_set(launch) != want:
            out.append(f"{name}: launched under {launch!r}, not {cpulist}")
        for r in art.get("rounds_raw", []):
            got = r.get("harness_row", {}).get("cpus_allowed")
            if not got or got == "unknown" or cpu_set(got) != want:
                out.append(f"{name} round {r.get('round')}: ran on {got!r}, not {cpulist}")
                break
    return out


def layout_problems(runs: dict[str, dict[str, dict]]) -> list[str]:
    """`runs` is {pin label: {run id: cells}}."""
    out = []
    for label in (WIDE, NARROW):
        n = len(runs.get(label, {}))
        if n < 2:
            out.append(f"pin {label} has {n} run(s); rule 18 needs two before a pin is compared")
    for label in runs:
        if label not in PIN_SETS:
            out.append(f"pin directory {label!r} names no known CPU set")
    return out


# --------------------------------------------------------------------------
# rule 18 over two runs per pin
# --------------------------------------------------------------------------
def interval(cells: dict[str, dict], cell: str, event: str):
    v = cells.get(cell, {}).get("events", {}).get(event)
    if not v or v.get("ci_lower") is None or v.get("per_op_mean") is None:
        return None
    return (v["per_op_mean"], v["ci_lower"], v["ci_upper"])


def separated(a, b) -> bool:
    return a[2] < b[1] or b[2] < a[1]


def pin_effects(wide: list[dict], narrow: list[dict], events, cells=CELLS):
    """(replicated, spread): see the module docstring, items 2 and 3."""
    replicated, spread = [], []
    for event in events:
        for cell in cells:
            w = [interval(r, cell, event) for r in wide]
            n = [interval(r, cell, event) for r in narrow]
            if any(v is None for v in w + n):
                continue
            for label, pair in ((WIDE, w), (NARROW, n)):
                if separated(pair[0], pair[1]):
                    spread.append((label, cell, event, pair[0][0], pair[1][0]))
            cross = [(a, b) for a in w for b in n]
            if all(separated(a, b) for a, b in cross) and len({b[0] > a[0] for a, b in cross}) == 1:
                replicated.append((cell, event, [v[0] for v in w], [v[0] for v in n]))
    return replicated, spread


# --------------------------------------------------------------------------
# achieved rates
# --------------------------------------------------------------------------
def counters_rates(cells: dict[str, dict]) -> dict[str, dict]:
    out = {}
    for cell in CELLS:
        rows = [r["harness_row"] for r in cells[cell]["rounds_raw"]]
        reads = [r["read_ops"] / r["elapsed_s"] / 1e6 for r in rows]
        writes = [r["write_ops"] / r["elapsed_s"] for r in rows]
        out[cell] = {"reads": (min(reads), max(reads)), "writes": (min(writes), max(writes))}
    return out


def wallclock_rates(results: Path) -> list[tuple[str, str, dict]]:
    out = []
    for f in sorted(glob.glob(str(results / "baseline_concurrent_reads*.json"))):
        art = json.loads(Path(f).read_text())
        by = {}
        for c in art.get("cells", []):
            key = f"{c['writer_mode']}_r{c['readers']}"
            if key not in ("idle_r7", "paced_r7"):
                continue
            e = by.setdefault(key, {"reads": [], "writes": []})
            e["reads"].append(c["read_ops"] / c["elapsed_s"] / 1e6)
            e["writes"].append(c["write_ops"] / c["elapsed_s"])
        rng = {k: {"reads": (min(v["reads"]), max(v["reads"])),
                   "writes": (min(v["writes"]), max(v["writes"]))} for k, v in by.items()}
        out.append((os.path.basename(f), art.get("provenance", {}).get("core_pin"), rng))
    return out


# --------------------------------------------------------------------------
# perf c2c
# --------------------------------------------------------------------------
_TABLE_ROW = re.compile(r"^\s+(\d+)\s+(0x[0-9a-f]+)\s+\d+\s+\d+\s+([\d.]+)%\s+(\d+)\s")
_LINE_HEAD = re.compile(r"^\s+(\d+)\s+\d+\s+\d+\s+\d+\s+\d+\s+\d+\s+(0x[0-9a-f]+)\s*$")
_DETAIL = re.compile(
    r"^\s+[\d.]+%\s+([\d.]+)%\s+[\d.]+%\s+[\d.]+%\s+[\d.]+%\s+0x[0-9a-f]+\s+\d+\s+\d+\s+"
    r"0x[0-9a-f]+\s+\d+\s+\d+\s+\d+\s+\d+\s+\d+\s+(.*)$")


def symbol_name(rest: str) -> str:
    """A short name for the symbol column of a c2c detail row."""
    parts = re.split(r"\s{2,}", rest.strip())
    sym = parts[0] if parts else rest.strip()
    obj = parts[1] if len(parts) > 1 else ""
    kernel = sym.startswith("[k]")
    sym = re.sub(r"^\[[.k]\]\s*", "", sym)
    if kernel and sym.startswith("0x"):
        return "kernel (unresolved)"
    if sym.startswith("0x"):
        return f"{obj or 'user'} (unresolved)"
    if "RunCell" in sym:
        return "RunCell thread body"
    sym = sym.split("(", 1)[0]
    return sym.replace("rocksdb::", "")


def parse_c2c(text: str, top: int = 3) -> dict:
    lines = text.splitlines()
    hitm = next((int(ln.split(":")[1]) for ln in lines if "Load Local HITM" in ln), None)
    table = []
    start = next((i for i, ln in enumerate(lines) if "Shared Data Cache Line Table" in ln), None)
    if start is not None:
        for ln in lines[start:]:
            m = _TABLE_ROW.match(ln)
            if m:
                table.append({"index": int(m.group(1)), "address": m.group(2),
                              "hitm_pct": float(m.group(3)), "hitm": int(m.group(4)),
                              "symbols": {}})
            elif table and "Shared Cache Line Distribution Pareto" in ln:
                break
            if len(table) >= top:
                break
    by_index = {row["index"]: row for row in table}
    cur = None
    pareto = next((i for i, ln in enumerate(lines) if "Shared Cache Line Distribution Pareto" in ln), None)
    for ln in (lines[pareto:] if pareto is not None else []):
        m = _LINE_HEAD.match(ln)
        if m:
            cur = by_index.get(int(m.group(1)))
            continue
        m = _DETAIL.match(ln)
        if cur is not None and m:
            name = symbol_name(m.group(2))
            cur["symbols"][name] = cur["symbols"].get(name, 0.0) + float(m.group(1))
    return {"load_local_hitm": hitm, "lines": table}


def address_class(address: str) -> str:
    return "kernel" if int(address, 16) >= KERNEL_FLOOR else "user"


# --------------------------------------------------------------------------
# render
# --------------------------------------------------------------------------
def render(root: Path, results: Path) -> str:
    runs: dict[str, dict[str, dict]] = {}
    for pin_dir in sorted(root.glob("pin_*")):
        label = pin_dir.name[len("pin_"):]
        for run_dir in sorted(p for p in pin_dir.iterdir() if p.is_dir()):
            runs.setdefault(label, {})[run_dir.name] = load_run(run_dir)
    problems = layout_problems(runs)
    for label, by_run in runs.items():
        for run_id, cells in by_run.items():
            problems += [f"pin_{label}/{run_id}: {p}" for p in pin_problems(cells, PIN_SETS.get(label, ""))]
    if problems:
        raise SystemExit("counters_report.py refuses to render:\n  " + "\n  ".join(problems))

    order = [(label, run_id) for label in (WIDE, NARROW) for run_id in sorted(runs[label])]
    commits = sorted({cells[CELLS[0]]["provenance"]["commit"][:8]
                      for label, run_id in order for cells in [runs[label][run_id]]})
    out = [f"<!-- generated by docs/benchmarks/rocksdb_memtable/scripts/counters_report.py from "
           f"results/counters/ (runs {', '.join(r for _, r in order)}; commit {', '.join(commits)}) -->", ""]

    out += ["Per-thread counters per operation: mean [BCa 95%] over 5 rounds. `reader/*` divides the "
            "reader threads' rows by the round's reads and `writer/*` the writer's rows by its inserts.", ""]
    head = "| event | cell | " + " | ".join(f"`{label}` {run_id}" for label, run_id in order) + " |"
    out += [head, "|---|---|" + "---|" * len(order)]
    for event in TABLE_EVENTS:
        for cell in CELLS:
            vals = [interval(runs[label][run_id], cell, event) for label, run_id in order]
            if all(v is None for v in vals):
                continue
            cols = ["—" if v is None else f"{fmt(v[0])} [{fmt(v[1])}, {fmt(v[2])}]" for v in vals]
            out.append(f"| `{event}` | {cell} | " + " | ".join(cols) + " |")
    out.append("")

    events = sorted({e for label, run_id in order for e in runs[label][run_id][CELLS[0]]["events"]}
                    | {e for label, run_id in order for e in runs[label][run_id]["paced_r1"]["events"]})
    wide = [runs[WIDE][r] for r in sorted(runs[WIDE])[:2]]
    narrow = [runs[NARROW][r] for r in sorted(runs[NARROW])[:2]]
    replicated, spread = pin_effects(wide, narrow, events)
    total = sum(1 for e in events for c in CELLS
                if all(interval(r, c, e) is not None for r in wide + narrow))
    out += [f"**Pin effects that replicate** ({len(replicated)} of {total} (cell, event) pairs): both "
            f"`{NARROW}` runs' intervals are separated from both `{WIDE}` runs', in one direction.", ""]
    if replicated:
        out += [f"| cell | event | `{WIDE}` runs | `{NARROW}` runs |", "|---|---|---|---|"]
        for cell, event, w, n in sorted(replicated, key=lambda x: (CELLS.index(x[0]), x[1])):
            out.append(f"| {cell} | `{event}` | {', '.join(fmt(x) for x in w)} | {', '.join(fmt(x) for x in n)} |")
    else:
        out.append("None.")
    out.append("")
    out += [f"**Between-run spread** ({len(spread)} same-pin pairs whose intervals separate):", "",
            "| pin | cell | event | run A | run B |", "|---|---|---|---|---|"]
    for label, cell, event, a, b in sorted(spread, key=lambda s: -abs((s[4] - s[3]) / s[3]) if s[3] else 0):
        out.append(f"| `{label}` | {cell} | `{event}` | {fmt(a)} | {fmt(b)} |")
    out.append("")

    out += ["**Achieved rates**, min–max over rounds: aggregate reads (Mops/s) and paced writer "
            "inserts/s, in the counters harness and in the wall-clock artifacts.", "",
            "| instrument | pin | idle R=7 reads | paced R=7 reads | paced R=7 writer |", "|---|---|---|---|---|"]
    for label, run_id in order:
        r = counters_rates(runs[label][run_id])
        out.append(f"| counters {run_id} | `{PIN_SETS[label]}` | {r['idle_r7']['reads'][0]:.3f}–{r['idle_r7']['reads'][1]:.3f} "
                   f"| {r['paced_r7']['reads'][0]:.3f}–{r['paced_r7']['reads'][1]:.3f} "
                   f"| {r['paced_r7']['writes'][0]:,.0f}–{r['paced_r7']['writes'][1]:,.0f} |")
    for name, pin, rng in wallclock_rates(results):
        if "idle_r7" not in rng or "paced_r7" not in rng:
            continue
        out.append(f"| wall-clock `{name}` | `{pin}` | {rng['idle_r7']['reads'][0]:.3f}–{rng['idle_r7']['reads'][1]:.3f} "
                   f"| {rng['paced_r7']['reads'][0]:.3f}–{rng['paced_r7']['reads'][1]:.3f} "
                   f"| {rng['paced_r7']['writes'][0]:,.0f}–{rng['paced_r7']['writes'][1]:,.0f} |")
    out.append("")

    out += ["**`perf c2c`**, top three shared cache lines by load HITM. Symbol shares are of that line's "
            "local HITMs, as the sampler attributes them.", "",
            "| run | cell | load HITM | line | address | share of HITM | symbols |", "|---|---|---|---|---|---|---|"]
    for label, run_id in order:
        for cell in C2C_CELLS:
            path = root / f"pin_{label}" / run_id / f"c2c_rocksdb_conc_{cell}.txt"
            rep = parse_c2c(path.read_text())
            for line in rep["lines"]:
                syms = sorted(line["symbols"].items(), key=lambda kv: -kv[1])[:4]
                out.append(f"| `{label}` {run_id} | {cell} | {rep['load_local_hitm']:,} | {line['index']} "
                           f"| {address_class(line['address'])} `{line['address']}` | {line['hitm_pct']:.2f}% "
                           f"| {'; '.join(f'`{s}` {v:.0f}%' for s, v in syms)} |")
    out.append("")
    return "\n".join(out)


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------
# Verbatim from results/counters/pin_one_sibling/34783862078/c2c_rocksdb_conc_paced_r7.txt,
# trimmed to the rows the parser reads.
C2C_EXCERPT = """\
=================================================
            Trace Event Information              
=================================================
  Total records                     :      40380
  Load Local HITM                   :       1629
  Load Remote HITM                  :          0
=================================================
           Shared Data Cache Line Table          
=================================================
#
      0  0xffff8909c25dac00     0     742   13.57%      221      221        0     1273     1169      114      104        0        0      659      266        0        23      221         0        0         0         0
      1      0x7ffe528882c0     0     995   12.03%      196      196        0     1589     1529       79       60        0        0     1101       93        0       139      196         0        0         0         0
=================================================
      Shared Cache Line Distribution Pareto      
=================================================
  ----------------------------------------------------------------------
      0        0      221      104        0        0  0xffff8909c25dac00
  ----------------------------------------------------------------------
           0.00%   40.27%    0.00%    0.00%    0.00%                 0x0     0       1  0xffffffff94a2b712         0       152       169      146         8  [k] 0xffffffff94a2b712  [unknown]         ??:0          0
           0.00%   21.27%    0.00%    0.00%    0.00%                 0x4     0       1  0xffffffff95a43493         0       179       213       54         8  [k] 0xffffffff95a43493  [unknown]         ??:0          0

  ----------------------------------------------------------------------
      1        0      196       60        0        0      0x7ffe528882c0
  ----------------------------------------------------------------------
           0.00%   58.16%    0.00%    0.00%    0.00%                 0x0     0       1      0x58948cbe7d9c         0       172       143      300         8  [.] rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&, void*, bool (*)(void*, char const*))  bench_memtable_concurrent  rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&, void*, bool (*)(void*, char const*))+204   0
           0.00%    7.65%    0.00%    0.00%    0.00%                 0x0     0       1      0x58948cbe74ef         0       178       135      250         8  [.] rocksdb::ExpanseMemTableRep::FindLeafBlockForSeek(rocksdb::Slice const&, char const*) const       bench_memtable_concurrent  rocksdb::ExpanseMemTableRep::FindLeafBlockForSeek(rocksdb::Slice const&, char const*) const+543        0
"""


def _fake_cell(events: dict) -> dict:
    return {"events": {e: {"per_op_mean": m, "ci_lower": lo, "ci_upper": hi}
                       for e, (m, lo, hi) in events.items()}}


def self_test() -> int:
    fails: list[str] = []

    def check(name, got, want):
        if got != want:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    # --- rule 18 over two runs per pin ---
    def runs(*vals):
        return [{"paced_r7": _fake_cell({"e": v})} for v in vals]
    up_w, up_n = runs((10, 9.8, 10.2), (10.1, 9.9, 10.3)), runs((12, 11.8, 12.2), (12.2, 12.0, 12.4))
    rep, spr = pin_effects(up_w, up_n, ["e"], cells=("paced_r7",))
    check("both narrow runs above both wide runs replicates", len(rep), 1)
    check("no spread in that case", spr, [])
    one_overlap = runs((12, 11.8, 12.2), (10.1, 9.9, 10.3))
    check("one narrow run overlapping a wide run does not replicate",
          len(pin_effects(up_w, one_overlap, ["e"], cells=("paced_r7",))[0]), 0)
    mixed = runs((12, 11.8, 12.2), (8, 7.8, 8.2))
    rep, spr = pin_effects(up_w, mixed, ["e"], cells=("paced_r7",))
    check("narrow runs on both sides of the wide runs does not replicate", len(rep), 0)
    check("the separated narrow pair is spread", [(s[0], s[1]) for s in spr], [(NARROW, "paced_r7")])

    # --- the pin a run must have run on ---
    def pinned(launch, allowed):
        return {c: {"provenance": {"pin": ["taskset", "-c", launch]},
                    "rounds_raw": [{"round": 0, "harness_row": {"cpus_allowed": allowed}}]} for c in CELLS}
    # Run 34783092421: launched and ran on 0-15 while dispatched one sibling per core.
    if not pin_problems(pinned("0-15", "0-15"), PIN_SETS[NARROW]):
        fails.append("a run on 0-15 was accepted under the one-sibling pin")
    check("a run on its pin", pin_problems(pinned("0,2,4,6,8,10,12,14", "0,2,4,6,8,10,12,14"), PIN_SETS[NARROW]), [])
    check("cpulist spellings of one set", pin_problems(pinned("0-3", "0,1,2,3"), "0,1,2,3"), [])
    if not pin_problems(pinned("0,2,4,6,8,10,12,14", "0-15"), PIN_SETS[NARROW]):
        fails.append("a harness that reported 0-15 under a one-sibling launch was accepted")
    if not layout_problems({WIDE: {"a": {}}, NARROW: {"b": {}, "c": {}}}):
        fails.append("a pin with one run was accepted for comparison")
    check("two runs per pin", layout_problems({WIDE: {"a": {}, "b": {}}, NARROW: {"c": {}, "d": {}}}), [])

    # --- perf c2c, on a verbatim excerpt ---
    rep = parse_c2c(C2C_EXCERPT)
    check("load local HITM", rep["load_local_hitm"], 1629)
    check("top lines", [(ln["index"], address_class(ln["address"]), ln["hitm_pct"], ln["hitm"]) for ln in rep["lines"]],
          [(0, "kernel", 13.57, 221), (1, "user", 12.03, 196)])
    check("kernel line symbols", {k: round(v, 2) for k, v in rep["lines"][0]["symbols"].items()},
          {"kernel (unresolved)": 61.54})
    check("user line symbols", {k: round(v, 2) for k, v in rep["lines"][1]["symbols"].items()},
          {"ExpanseMemTableRep::Get": 58.16, "ExpanseMemTableRep::FindLeafBlockForSeek": 7.65})

    # --- the committed tree renders every section (AGENTS.md section 8.20.7) ---
    if ROOT.is_dir():
        text = render(ROOT, RESULTS)
        for needle in ("Pin effects that replicate", "Between-run spread", "Achieved rates", "perf c2c"):
            if needle not in text:
                fails.append(f"the committed tree rendered no `{needle}` section")
        c2c_rows = [ln for ln in text.splitlines() if re.match(r"\| `(0-15|one_sibling)` \d+ \| (idle|paced)_r7 \|", ln)]
        if len(c2c_rows) < 4 * len(C2C_CELLS):
            fails.append(f"the c2c table has {len(c2c_rows)} rows; every run's reports should contribute")
    else:
        print(f"note: {ROOT} does not exist; the rendered-tree check did not run")

    if fails:
        print("counters_report.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("counters_report.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", type=Path, default=ROOT)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    print(render(args.root, RESULTS))
    return 0


if __name__ == "__main__":
    sys.exit(main())
