#!/usr/bin/env python3
"""Fine-grained write brackets gate, read against METHODOLOGY.md §8 — never typed.

Inputs, per FFI suite (`docs/benchmarks/<suite>/results/`):

- `baseline_concurrent.json` and `baseline_concurrent_run2.json`: the
  committed pair at the base of the program (`a1982ff2`), whose union is the
  baseline every threshold in §8.2 is derived from.
- `baseline_concurrent_ab.json` and `baseline_concurrent_ab_run2.json`: two
  two-commit runs (`scripts/bench_ab.py`), each cell carrying a `base` and a
  `head` reduction.
- `fine_grained_brackets/counters_<cell>.json`: the head build's per-thread counters at the
  H3.2 cells (`scripts/bench_counters.py --out-dir`).

`evaluate(load)` takes a loader `load(suite, filename) -> dict | None` so the
self-test can hand it fixtures; `render(load)` is the Markdown `tables.py`
prints. Verdict labels are §8.2's: `PASS`, `BOUNDARY_RESULT`, `REFUTED`; a
cell whose base half does not reproduce the committed union is `VOID` (§8.4)
and is never read further.

Self-test: `python3 docs/benchmarks/concurrency/scripts/fine_grained_brackets_gate.py --self-test`.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ISSUE = "[#568](https://github.com/orieg/expanse/issues/568)"
PENDING = f"pending ({ISSUE})"

# The H3.1 gate cells: (label, suite, arm, counters cell for H3.2).
GATE_CELLS = [
    ("hot_conc_set_w1_r8", "hot_comparison", "set", "hot_conc_set_w1_r8"),
    ("hot_conc_map_w1_r8", "hot_comparison", "map", "hot_conc_map_w1_r8"),
    ("masstree_conc_map_w1_r8", "masstree_comparison", "map", "masstree_conc_map_w1_r8"),
]
# C1 controls, predicted inside the baseline union (§8.2 H3.4).
CONTROL_ARMS = [("hot_comparison", "set"), ("hot_comparison", "map"),
                ("masstree_comparison", "map"), ("masstree_comparison", "str")]
CONTROL_W = [1, 2, 4, 8, 16]
READERS = 8
H31_FACTOR = 0.5          # head union-upper below 0.5 × baseline union-lower
H32_PASS, H32_REFUTED = 6.0, 10.0
H33_CEILING = 0.30
# The string wrapper's C2 cell is not a gate cell (it keeps the whole-operation
# tree bracket, METHODOLOGY §8.3) but it is a non-targeted arm, so its two
# halves are published per round: the cell is bimodal per process, and a
# median hides which mode a run landed in. The split is descriptive — the gap
# between the two observed modes — never a threshold a verdict rests on.
STR_CELL = ("masstree_conc_str_w1_r8", "masstree_comparison", "str")
STR_HIGH_MODE_MOPS = 3.0


def default_load(suite: str, name: str) -> dict | None:
    p = Path(__file__).resolve().parents[3] / "benchmarks" / suite / "results" / name
    return json.loads(p.read_text()) if p.is_file() else None


def _cell(art: dict | None, key: str, arm: str, w: int, r: int) -> dict | None:
    for c in (art or {}).get(key, []):
        if c.get("arm") == arm and c.get("writers") == w and c.get("readers") == r:
            return c
    return None


def reader_ns(cell: dict | None, readers: int = READERS) -> float | None:
    """Per-probe ns of one reader from a reduction's aggregate reader rate."""
    m = (cell or {}).get("expanse_reader_mops_median")
    return None if not m else readers / m * 1e3


def writer_mops(cell: dict | None) -> float | None:
    return (cell or {}).get("expanse_writer_mops_median")


def union(vals: list) -> tuple[float, float] | None:
    vals = [v for v in vals if v is not None]
    return (min(vals), max(vals)) if vals else None


def _rounds(cell: dict | None, build: str) -> list[float]:
    """Per-round reader M/s of one build in a two-commit cell."""
    rows = (cell or {}).get("rounds_raw") or []
    return [r["expanse_reader_mops"] for r in rows
            if r.get("build") == build and r.get("expanse_reader_mops") is not None]


def _modes(vals: list[float]) -> dict:
    return {"n": len(vals), "high": sum(v >= STR_HIGH_MODE_MOPS for v in vals),
            "min": min(vals) if vals else None, "max": max(vals) if vals else None}


def evaluate(load) -> dict:
    out = {"h31": [], "h32": [], "h33": [], "controls": [], "writer_c2": [], "str_c2": []}
    arts = {}
    for suite in ("hot_comparison", "masstree_comparison"):
        arts[suite] = {
            "base_pair": [load(suite, "baseline_concurrent.json"), load(suite, "baseline_concurrent_run2.json")],
            "ab_pair": [load(suite, "baseline_concurrent_ab.json"), load(suite, "baseline_concurrent_ab_run2.json")],
        }

    for label, suite, arm, ccell in GATE_CELLS:
        a = arts[suite]
        base_ns = union([reader_ns(_cell(x, "throughput", arm, 1, READERS)) for x in a["base_pair"]])
        ab = [_cell(x, "throughput", arm, 1, READERS) for x in a["ab_pair"]]
        row = {"label": label, "baseline_union": base_ns, "threshold": None,
               "base_half": [reader_ns(c.get("base")) if c else None for c in ab],
               "head_half": [reader_ns(c.get("head")) if c else None for c in ab],
               "verdict": "pending"}
        if base_ns is None:
            row["verdict"] = "pending (no baseline pair)"
        else:
            row["threshold"] = H31_FACTOR * base_ns[0]
            if any(v is None for v in row["base_half"] + row["head_half"]):
                row["verdict"] = "pending"
            elif not all(base_ns[0] <= v <= base_ns[1] for v in row["base_half"]):
                row["verdict"] = "VOID (base half outside the committed union, §8.4)"
            else:
                right = all(h < b for h, b in zip(row["head_half"], row["base_half"]))
                if not right:
                    row["verdict"] = "REFUTED (a run in the wrong direction)"
                elif max(row["head_half"]) < row["threshold"]:
                    row["verdict"] = "PASS"
                else:
                    row["verdict"] = "BOUNDARY_RESULT (both runs improved; union crosses the threshold)"
        out["h31"].append(row)

        c = load(suite, f"fine_grained_brackets/counters_{ccell}.json")
        ev = ((c or {}).get("roles", {}).get("writer", {}).get("events", {}) or {}).get("l2_rqsts.rfo_miss")
        step0 = load(suite, f"counters_{ccell}.json")
        ev0 = ((step0 or {}).get("roles", {}).get("writer", {}).get("events", {}) or {}).get("l2_rqsts.rfo_miss")
        m = (ev or {}).get("per_op_mean")
        if m is None:
            v = "pending"
        elif m <= H32_PASS:
            v = "PASS"
        elif m >= H32_REFUTED:
            v = "REFUTED"
        else:
            v = "BOUNDARY_RESULT"
        out["h32"].append({"label": label, "step0": ev0, "head": ev, "verdict": v})

        shares = [(_cell(x, "health", arm, 1, READERS) or {}).get("restart_share", {}).get("median") for x in a["ab_pair"]]
        if any(s is None for s in shares):
            v = "pending"
        else:
            v = "PASS" if max(shares) <= H33_CEILING else "ABOVE CEILING (reconsider before shipping, §8.2)"
        out["h33"].append({"label": label, "restart_share": shares, "verdict": v})

        # The writer at the gate cell: predicted above the baseline union-upper, confidence low.
        wu = union([writer_mops(_cell(x, "throughput", arm, 1, READERS)) for x in a["base_pair"]])
        heads = [writer_mops(c.get("head")) if c else None for c in ab]
        if wu is None or any(h is None for h in heads):
            v = "pending"
        elif min(heads) > wu[1]:
            v = "above the baseline union-upper (as predicted, confidence low)"
        elif max(heads) < wu[0]:
            v = "below the baseline union (unpredicted)"
        else:
            v = "inside the baseline union"
        out["writer_c2"].append({"label": label, "baseline_union": wu, "head": heads, "verdict": v})

    label, suite, arm = STR_CELL
    a = arts[suite]
    for i, x in enumerate(a["ab_pair"]):
        c = _cell(x, "throughput", arm, 1, READERS)
        base, head = _rounds(c, "base"), _rounds(c, "head")
        h = _cell(x, "health", arm, 1, READERS) or {}
        row = {"label": label, "run": i + 1,
               "base": _modes(base), "head": _modes(head),
               "restart_share": (h.get("restart_share") or {}).get("median"),
               "locked_share": (h.get("locked_share") or {}).get("median"),
               "verdict": "pending"}
        if base and head:
            if row["base"]["high"] and not row["head"]["high"]:
                row["verdict"] = "the high mode is absent from the head half (non-targeted arm; §6b names the mechanism)"
            elif row["base"]["high"] == row["head"]["high"]:
                row["verdict"] = "same mode count in both halves"
            else:
                row["verdict"] = "mode counts differ (direction only)"
        out["str_c2"].append(row)

    for suite, arm in CONTROL_ARMS:
        a = arts[suite]
        for w in CONTROL_W:
            wu = union([writer_mops(_cell(x, "throughput", arm, w, 0)) for x in a["base_pair"]])
            ab = [_cell(x, "throughput", arm, w, 0) for x in a["ab_pair"]]
            heads = [writer_mops(c.get("head")) if c else None for c in ab]
            bases = [writer_mops(c.get("base")) if c else None for c in ab]
            if wu is None or any(h is None for h in heads):
                v = "pending"
            elif all(wu[0] <= h <= wu[1] for h in heads):
                v = "unchanged (inside the baseline union)"
            else:
                v = "moved (unpredicted; both runs)" if all(not (wu[0] <= h <= wu[1]) for h in heads) else "moved in one run (direction only)"
            out["controls"].append({"suite": suite, "arm": arm, "writers": w, "baseline_union": wu,
                                    "base": bases, "head": heads, "verdict": v})
    return out


def _f(v, digits=0):
    return "pending" if v is None else f"{v:.{digits}f}"


def _pair(vals, digits=0):
    return " / ".join(_f(v, digits) for v in vals)


def _u(u, digits=0):
    return "pending" if u is None else f"[{u[0]:.{digits}f}, {u[1]:.{digits}f}]"


def _verdict(v: str) -> str:
    if v == "PASS":
        return "`PASS`"
    if v.startswith(("REFUTED", "VOID", "ABOVE")):
        return f"**`{v}`**"
    if v.startswith("BOUNDARY"):
        return f"`{v}`"
    return v


def render(load=default_load) -> list[str]:
    r = evaluate(load)
    out = ["## 8. Fine-grained write brackets gate (METHODOLOGY §8)", "",
           "Read against §8.2; every number is the runner's own estimator over the two-commit "
           "artifacts (`results/baseline_concurrent_ab*.json` of each FFI suite) and the head "
           "build's counters (`results/fine_grained_brackets/`). A cell whose base half falls outside the "
           "committed `a1982ff2` union is `VOID` (§8.4) and decides nothing.", "",
           "**H3.1 — reader ns per probe at C2 W = 1 R = 8** (gate: head union-upper below "
           f"{H31_FACTOR:.1f} × the baseline union-lower):", "",
           "| cell | baseline union (ns) | threshold (ns) | base half, run 1 / 2 | head half, run 1 / 2 | verdict |",
           "|---|--:|--:|--:|--:|---|"]
    for row in r["h31"]:
        out.append(f"| `{row['label']}` | {_u(row['baseline_union'])} | {_f(row['threshold'])} | "
                   f"{_pair(row['base_half'])} | {_pair(row['head_half'])} | {_verdict(row['verdict'])} |")
    out += ["", f"**H3.2 — writer `l2_rqsts.rfo_miss` per insert at C2 W = 1 R = 8** (gate: ≤ {H32_PASS:.1f}; "
            f"refuted at ≥ {H32_REFUTED:.1f}):", "",
            "| cell | Step 0 (base) | head | verdict |", "|---|--:|--:|---|"]
    for row in r["h32"]:
        def iv(e):
            if not e or e.get("per_op_mean") is None:
                return "pending"
            return f"{e['per_op_mean']:.2f} [{e['ci_lower']:.2f}, {e['ci_upper']:.2f}]"
        out.append(f"| `{row['label']}` | {iv(row['step0'])} | {iv(row['head'])} | {_verdict(row['verdict'])} |")
    out += ["", f"**H3.3 — restart share at C2 W = 1 R = 8, head build** (ceiling {H33_CEILING:.0%}):", "",
            "| cell | restart share, run 1 / 2 | verdict |", "|---|--:|---|"]
    for row in r["h33"]:
        shares = " / ".join("pending" if s is None else f"{s:.2%}" for s in row["restart_share"])
        out.append(f"| `{row['label']}` | {shares} | {_verdict(row['verdict'])} |")
    out += ["", "**The writer at the gate cell** (published beside H3.1, not a gate; M inserts/s):", "",
            "| cell | baseline union | head, run 1 / 2 | reading |", "|---|--:|--:|---|"]
    for row in r["writer_c2"]:
        out.append(f"| `{row['label']}` | {_u(row['baseline_union'], 2)} | {_pair(row['head'], 2)} | {row['verdict']} |")
    out += ["", "**H3.4 controls — C1 aggregate M inserts/s, predicted inside the baseline union** "
            "(the Callgrind half of H3.4 is the PR's own `instruction-counts` run):", "",
            "| suite | arm | W | baseline union | base half, run 1 / 2 | head half, run 1 / 2 | reading |",
            "|---|---|--:|--:|--:|--:|---|"]
    for row in r["controls"]:
        out.append(f"| {row['suite']} | {row['arm']} | {row['writers']} | {_u(row['baseline_union'], 2)} | "
                   f"{_pair(row['base'], 2)} | {_pair(row['head'], 2)} | {row['verdict']} |")
    out += ["", "**The string wrapper's C2 cell, per round** (not a gate cell — it keeps the "
            "whole-operation tree bracket, §8.3 — but a non-targeted arm; each half is the "
            f"harness's per-round lookup rate, split at {STR_HIGH_MODE_MOPS:.0f} M/s, the gap between "
            "the two modes every run has shown; §6b is the ablation that names the mechanism):", "",
            "| cell | run | base half: rounds ≥ 3 M/s, min–max | head half: rounds ≥ 3 M/s, min–max | "
            "head restart share | head locked share | reading |",
            "|---|--:|--:|--:|--:|--:|---|"]
    for row in r["str_c2"]:
        def modes(m):
            if not m["n"]:
                return "pending"
            return f"{m['high']} / {m['n']}, {m['min']:.2f}–{m['max']:.2f}"
        rs = "pending" if row["restart_share"] is None else f"{row['restart_share']:.1%}"
        ls = "pending" if row["locked_share"] is None else f"{row['locked_share']:.2%}"
        out.append(f"| `{row['label']}` | {row['run']} | {modes(row['base'])} | {modes(row['head'])} | "
                   f"{rs} | {ls} | {row['verdict']} |")
    if all(row["verdict"].startswith("pending") for row in r["h31"]):
        out += ["", f"The two-commit artifacts are not committed yet: {PENDING}."]
    return out


def _fixture(base_ns, head_ns, base_pair_ns, w_base=2.0, w_head=2.5, restart=0.12):
    """Artifacts for one suite (`map`, W=1 R=8 plus one C1 cell) from ns/probe."""
    def thr(ns, wm):
        return {"arm": "map", "writers": 1, "readers": READERS,
                "expanse_reader_mops_median": READERS / ns * 1e3, "expanse_writer_mops_median": wm}

    def c1(wm):
        return {"arm": "map", "writers": 1, "readers": 0, "expanse_writer_mops_median": wm}

    base_pair = [{"throughput": [thr(ns, w_base), c1(w_base)]} for ns in base_pair_ns]
    ab_pair = [{"throughput": [{"arm": "map", "writers": 1, "readers": READERS,
                                "base": thr(b, w_base), "head": thr(h, w_head)},
                               {"arm": "map", "writers": 1, "readers": 0,
                                "base": c1(w_base), "head": c1(w_head)}],
                "health": [{"arm": "map", "writers": 1, "readers": READERS,
                            "restart_share": {"median": restart}}]}
               for b, h in zip(base_ns, head_ns)]
    return base_pair, ab_pair


def _str_fixture(base_rounds, head_rounds, restart=0.92, locked=0.01):
    """One two-commit artifact carrying the string C2 cell with per-round rows."""
    rows = [{"round": i, "build": "base", "expanse_reader_mops": v} for i, v in enumerate(base_rounds)]
    rows += [{"round": i, "build": "head", "expanse_reader_mops": v} for i, v in enumerate(head_rounds)]
    return {"throughput": [{"arm": "str", "writers": 1, "readers": READERS, "rounds_raw": rows}],
            "health": [{"arm": "str", "writers": 1, "readers": READERS,
                        "restart_share": {"median": restart}, "locked_share": {"median": locked}}]}


def _self_test() -> int:
    fails = 0

    def check(name, cond):
        nonlocal fails
        print(f"  {'ok ' if cond else 'FAIL'} {name}")
        fails += 0 if cond else 1

    def loader_for(base_pair, ab_pair, rfo=None):
        def load(suite, name):
            if suite != "masstree_comparison":
                return None
            if name == "baseline_concurrent.json":
                return base_pair[0]
            if name == "baseline_concurrent_run2.json":
                return base_pair[1]
            if name == "baseline_concurrent_ab.json":
                return ab_pair[0]
            if name == "baseline_concurrent_ab_run2.json":
                return ab_pair[1]
            if name == "fine_grained_brackets/counters_masstree_conc_map_w1_r8.json" and rfo is not None:
                return {"roles": {"writer": {"events": {"l2_rqsts.rfo_miss": {
                    "per_op_mean": rfo, "ci_lower": rfo - 0.1, "ci_upper": rfo + 0.1}}}}}
            return None
        return load

    def mt(r):
        return next(x for x in r["h31"] if x["label"] == "masstree_conc_map_w1_r8")

    # The registered numbers: baseline [273, 407] → threshold 136.5.
    bp, ab = _fixture([300.0, 350.0], [120.0, 130.0], [273.0, 407.0])
    r = evaluate(loader_for(bp, ab, rfo=5.0))
    check("threshold is 0.5 × union-lower", abs(mt(r)["threshold"] - 136.5) < 1e-9)
    check("PASS when the head union-upper is below the threshold", mt(r)["verdict"] == "PASS")
    check("H3.2 PASS at 5.0", next(x for x in r["h32"] if x["label"].startswith("masstree"))["verdict"] == "PASS")
    check("H3.3 PASS at 12%", next(x for x in r["h33"] if x["label"].startswith("masstree"))["verdict"] == "PASS")
    check("writer above union-upper reads as predicted",
          "as predicted" in next(x for x in r["writer_c2"] if x["label"].startswith("masstree"))["verdict"])
    check("C1 control moved (2.0 → 2.5 outside [2.0, 2.0])",
          any(x["verdict"].startswith("moved (unpredicted") for x in r["controls"] if x["suite"] == "masstree_comparison" and x["writers"] == 1))

    bp, ab = _fixture([300.0, 350.0], [120.0, 200.0], [273.0, 407.0])
    r = evaluate(loader_for(bp, ab, rfo=7.0))
    check("BOUNDARY_RESULT when both improved but a run crosses the threshold", mt(r)["verdict"].startswith("BOUNDARY_RESULT"))
    check("H3.2 BOUNDARY between 6 and 10", next(x for x in r["h32"] if x["label"].startswith("masstree"))["verdict"] == "BOUNDARY_RESULT")

    bp, ab = _fixture([300.0, 350.0], [120.0, 360.0], [273.0, 407.0])
    r = evaluate(loader_for(bp, ab, rfo=11.0))
    check("REFUTED when a run goes the wrong direction", mt(r)["verdict"].startswith("REFUTED"))
    check("H3.2 REFUTED at 11", next(x for x in r["h32"] if x["label"].startswith("masstree"))["verdict"] == "REFUTED")

    bp, ab = _fixture([300.0, 450.0], [120.0, 130.0], [273.0, 407.0], restart=0.35)
    r = evaluate(loader_for(bp, ab))
    check("VOID when the base half leaves the committed union", mt(r)["verdict"].startswith("VOID"))
    check("H3.3 above ceiling at 35%", next(x for x in r["h33"] if x["label"].startswith("masstree"))["verdict"].startswith("ABOVE"))
    check("H3.2 pending without counters", next(x for x in r["h32"] if x["label"].startswith("masstree"))["verdict"] == "pending")

    r = evaluate(lambda s, n: None)
    check("everything pending without artifacts", all(x["verdict"].startswith("pending") for x in r["h31"]))
    lines = render(lambda s, n: None)
    check("render names the open issue when pending", any(ISSUE in l for l in lines))
    check("render is a Markdown section", lines[0].startswith("## 8."))
    # The string cell: base bimodal, head single-mode — named, never a verdict on a median.
    def str_loader(suite, name):
        if suite == "masstree_comparison" and name.startswith("baseline_concurrent_ab"):
            return _str_fixture([1.5, 7.0, 6.5, 1.2], [1.1, 1.0, 1.3, 0.9])
        return None
    r = evaluate(str_loader)
    check("str cell: high mode absent from the head", all(
        x["verdict"].startswith("the high mode is absent") and x["base"]["high"] == 2 and x["head"]["high"] == 0
        for x in r["str_c2"]))
    check("str cell rendered", any("`masstree_conc_str_w1_r8` | 1 | 2 / 4, 1.20–7.00 | 0 / 4, 0.90–1.30 | 92.0% | 1.00%" in l
                                   for l in render(str_loader)))
    r = evaluate(lambda s, n: None)
    check("str cell pending without artifacts", all(x["verdict"] == "pending" for x in r["str_c2"]))
    print("fine_grained_brackets_gate self-test:", "ok" if fails == 0 else f"{fails} failure(s)")
    return 1 if fails else 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(_self_test())
    print("\n".join(render()))
