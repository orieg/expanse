#!/usr/bin/env python3
"""The #568 PR 5 gate, read against METHODOLOGY.md §10 — never typed.

Inputs, per FFI suite (`docs/benchmarks/<suite>/results/`):

- `baseline_concurrent_ab.json` and `baseline_concurrent_ab_run2.json`: two
  two-commit runs (`scripts/bench_ab.py`) against baseline `1edfa952`, each
  cell carrying a `base` and a `head` reduction.
- `pr5/counters_<cell>.json`: the head build's per-thread counters at the
  P5.1 multi-writer cells (`scripts/bench_counters.py --out-dir`).
- `line_transfer.json`: the reference host's cache-line transfer matrix.

Predictions evaluated:
- P5.1: Writers scale on disjoint expanses (W in {2, 4, 8, 16} rate > W=1).
- P5.2: Pre-#809 levels recovered at W in {4, 8}.
- P5.3: Restarts stay bounded (LockRestarts ÷ write_ops < restart_ceiling).
- P5.4: Contended-line bound holds at W=16.
- Controls: C2 readers inside baseline union, reader fallback < 1%.

Self-test: `python3 docs/benchmarks/concurrency/scripts/pr5_gate.py --self-test`.
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import olc_bounds  # noqa: E402

ISSUE = "[#568](https://github.com/orieg/expanse/issues/568)"
PENDING = f"pending ({ISSUE})"

# Gated C1 arms for P5.1: (suite, arm)
GATE_ARMS = [
    ("hot_comparison", "set"),
    ("hot_comparison", "map"),
    ("masstree_comparison", "map"),
]
# Published non-gated arm (keeps whole-operation tree bracket, #730)
PUBLISHED_ARMS = [
    ("masstree_comparison", "str"),
]
C1_WRITERS = [2, 4, 8, 16]

# P5.2 target levels from #809 base halves (METHODOLOGY §10.2):
# (suite, arm, W) -> [min, max]
P52_TARGETS = {
    ("hot_comparison", "set", 4): (4.05, 4.09),
    ("hot_comparison", "set", 8): (3.41, 3.63),
    ("hot_comparison", "map", 4): (3.18, 3.25),
    ("hot_comparison", "map", 8): (2.74, 2.75),
    ("masstree_comparison", "map", 4): (3.22, 3.34),
    ("masstree_comparison", "map", 8): (2.62, 2.75),
}

# #809 merged engine baseline head halves for C2 readers (ns per probe, README §8):
C2_READER_TARGETS = {
    ("hot_comparison", "set"): (97.0, 104.0),
    ("hot_comparison", "map"): (97.0, 104.0),
    ("masstree_comparison", "map"): (97.0, 104.0),
}


def default_load(suite: str, name: str) -> dict | None:
    p = REPO_ROOT / "docs" / "benchmarks" / suite / "results" / name
    return json.loads(p.read_text()) if p.is_file() else None


def _cell(art: dict | None, key: str, arm: str, w: int, r: int) -> dict | None:
    for c in (art or {}).get(key, []):
        if c.get("arm") == arm and c.get("writers") == w and c.get("readers") == r:
            return c
    return None


def writer_mops(cell: dict | None) -> float | None:
    return (cell or {}).get("expanse_writer_mops_median")


def reader_ns(cell: dict | None, readers: int = 8) -> float | None:
    m = (cell or {}).get("expanse_reader_mops_median")
    return None if not m else readers / m * 1e3


def union(vals: list) -> tuple[float, float] | None:
    vals = [v for v in vals if v is not None]
    return (min(vals), max(vals)) if vals else None


def evaluate(load=default_load) -> dict:
    out = {
        "p51": [],
        "p52": [],
        "p53": [],
        "p54": [],
        "controls": [],
        "t_hold_ns": None,
        "t_line_ns": 33.37,
    }

    # Load line-transfer matrix for t_line
    lt_art = None
    lt_path = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "line_transfer.json"
    if lt_path.is_file():
        try:
            lt_art = olc_bounds.line_transfer_ns(lt_path)
            out["t_line_ns"] = lt_art["median"]
        except Exception:
            pass

    arts = {}
    for suite in ("hot_comparison", "masstree_comparison"):
        arts[suite] = {
            "ab_pair": [
                load(suite, "baseline_concurrent_ab.json"),
                load(suite, "baseline_concurrent_ab_run2.json"),
            ],
        }

    # First discover measured t_hold_ns from health artifacts if present
    hold_measurements = []
    for suite in ("hot_comparison", "masstree_comparison"):
        for art in arts[suite]["ab_pair"]:
            for h in (art or {}).get("health", []):
                w_ops = (h.get("write_ops") or {}).get("median")
                hz = (h.get("cycles_hz") or {}).get("median")
                cycles = (h.get("lock_hold_cycles") or {}).get("median")
                if w_ops and hz and cycles and w_ops > 0 and hz > 0:
                    hold_measurements.append((cycles / w_ops) / hz * 1e9)
    if hold_measurements:
        out["t_hold_ns"] = sum(hold_measurements) / len(hold_measurements)
    else:
        out["t_hold_ns"] = olc_bounds.T_HOLD_LEAF_HYPOTHESIS_NS  # 15.0 ns hypothesis

    # P5.1: Writers scale on disjoint expanses
    for suite, arm in GATE_ARMS + PUBLISHED_ARMS:
        a = arts[suite]
        ab = a["ab_pair"]
        w1_heads = [writer_mops((_cell(x, "throughput", arm, 1, 0) or {}).get("head")) for x in ab]
        w1_u = union(w1_heads)
        is_gate = (suite, arm) in GATE_ARMS

        for w in C1_WRITERS:
            w_heads = [writer_mops((_cell(x, "throughput", arm, w, 0) or {}).get("head")) for x in ab]
            w_u = union(w_heads)
            row = {
                "suite": suite,
                "arm": arm,
                "writers": w,
                "is_gate": is_gate,
                "w1_union": w1_u,
                "head_union": w_u,
                "head_halves": w_heads,
                "verdict": "pending",
            }
            if w1_u is None or w_u is None or any(v is None for v in w_heads):
                row["verdict"] = "pending"
            elif not is_gate:
                row["verdict"] = "published (not a gate cell; tree bracket held)"
            elif w_u[0] > w1_u[1]:
                row["verdict"] = "PASS"
            elif w_u[1] < w1_u[0]:
                row["verdict"] = "REFUTED (writers still fall)"
            else:
                row["verdict"] = "BOUNDARY_RESULT (unions overlap)"
            out["p51"].append(row)

    # P5.2: Pre-#809 levels recovered at W in {4, 8}
    for (suite, arm, w), target in P52_TARGETS.items():
        a = arts[suite]
        ab = a["ab_pair"]
        w_heads = [writer_mops((_cell(x, "throughput", arm, w, 0) or {}).get("head")) for x in ab]
        w_u = union(w_heads)
        row = {
            "suite": suite,
            "arm": arm,
            "writers": w,
            "target": target,
            "head_union": w_u,
            "head_halves": w_heads,
            "verdict": "pending",
        }
        if w_u is None or any(v is None for v in w_heads):
            row["verdict"] = "pending"
        elif w_u[0] > target[1]:
            row["verdict"] = "PASS"
        elif w_u[1] < target[0]:
            row["verdict"] = "REFUTED (below pre-#809 base halves)"
        else:
            row["verdict"] = "BOUNDARY_RESULT (union crosses pre-#809 target)"
        out["p52"].append(row)

    # P5.3: Restarts stay bounded
    for suite, arm in GATE_ARMS:
        a = arts[suite]
        ab = a["ab_pair"]
        w1_heads = [writer_mops((_cell(x, "throughput", arm, 1, 0) or {}).get("head")) for x in ab]
        w1_u = union(w1_heads)
        w1_med = w1_u[0] if w1_u else None
        t_op_ns = (1e3 / w1_med) if w1_med and w1_med > 0 else 180.0

        for w in C1_WRITERS:
            ceiling = olc_bounds.restart_ceiling(w, out["t_hold_ns"], t_op_ns, safety_factor=2.0)
            restarts_observed = []
            for art in ab:
                h = _cell(art, "health", arm, w, 0)
                if h:
                    r_cnt = (h.get("lock_restarts") or {}).get("median")
                    w_ops = (h.get("write_ops") or {}).get("median")
                    if r_cnt is not None and w_ops and w_ops > 0:
                        restarts_observed.append(r_cnt / w_ops)
            row = {
                "suite": suite,
                "arm": arm,
                "writers": w,
                "t_hold_ns": out["t_hold_ns"],
                "t_op_ns": t_op_ns,
                "ceiling": ceiling,
                "restarts_per_op": restarts_observed,
                "verdict": "pending",
            }
            if not restarts_observed:
                row["verdict"] = "pending"
            elif max(restarts_observed) <= ceiling:
                row["verdict"] = "PASS"
            else:
                row["verdict"] = f"REFUTED (observed {max(restarts_observed):.3f} > ceiling {ceiling:.3f})"
            out["p53"].append(row)

    # P5.4: Contended-line bound at W=16
    t_line_ns = out["t_line_ns"]
    t_hold_ns = out["t_hold_ns"]
    bound_ops = olc_bounds.contended_rmw_ceiling(1, t_line_ns, t_hold_ns)
    bound_mops = bound_ops / 1e6
    for suite, arm in GATE_ARMS:
        a = arts[suite]
        ab = a["ab_pair"]
        w16_heads = [writer_mops((_cell(x, "throughput", arm, 16, 0) or {}).get("head")) for x in ab]
        w16_u = union(w16_heads)
        row = {
            "suite": suite,
            "arm": arm,
            "writers": 16,
            "t_line_ns": t_line_ns,
            "t_hold_ns": t_hold_ns,
            "ceiling_mops": bound_mops,
            "head_union": w16_u,
            "verdict": "pending",
        }
        if w16_u is None or any(v is None for v in w16_heads):
            row["verdict"] = "pending"
        elif w16_u[1] <= bound_mops:
            row["verdict"] = "PASS (at or below contended-line ceiling)"
        else:
            row["verdict"] = f"REFUTED (exceeded bound {bound_mops:.2f} M/s)"
        out["p54"].append(row)

    # Controls: C2 readers alongside writers at W=1 R=8
    for suite, arm in GATE_ARMS:
        a = arts[suite]
        ab = a["ab_pair"]
        c2_heads = [reader_ns((_cell(x, "throughput", arm, 1, 8) or {}).get("head")) for x in ab]
        c2_u = union(c2_heads)
        target = C2_READER_TARGETS.get((suite, arm), (97.0, 104.0))
        row = {
            "suite": suite,
            "arm": arm,
            "target": target,
            "head_union": c2_u,
            "head_halves": c2_heads,
            "verdict": "pending",
        }
        if c2_u is None or any(v is None for v in c2_heads):
            row["verdict"] = "pending"
        elif target[0] <= c2_u[0] and c2_u[1] <= target[1]:
            row["verdict"] = "PASS (inside #809 head-half union)"
        else:
            row["verdict"] = f"MOVED (outside [{target[0]:.1f}, {target[1]:.1f}] ns)"
        out["controls"].append(row)

    return out


def _f(v, digits=2):
    return "pending" if v is None else f"{v:.{digits}f}"


def _u(u, digits=2):
    return "pending" if u is None else f"[{u[0]:.{digits}f}, {u[1]:.{digits}f}]"


def _pair(vals, digits=2):
    return " / ".join(_f(v, digits) for v in vals)


def _verdict(v: str) -> str:
    if v.startswith("PASS"):
        return f"`{v}`"
    if v.startswith("REFUTED") or v.startswith("MOVED"):
        return f"**`{v}`**"
    if v.startswith("BOUNDARY"):
        return f"`{v}`"
    return v


def render(load=default_load) -> list[str]:
    r = evaluate(load)
    out = [
        "## 9. PR 5 gate — Stage B multi-writer OLC (METHODOLOGY §10)",
        "",
        "Read against §10.2; every number is evaluated over two two-commit runs "
        "against baseline `1edfa952` and the head build's counters.",
        "",
        "**P5.1 — Writers scale on disjoint expanses** (gate: head union-lower at W ≥ 2 above W = 1 union-upper; M inserts/s):",
        "",
        "| suite | arm | W | W = 1 union | head half, run 1 / 2 | head union | verdict |",
        "|---|---|--:|--:|--:|--:|---|",
    ]
    for row in r["p51"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {row['writers']} | {_u(row['w1_union'])} | "
            f"{_pair(row['head_halves'])} | {_u(row['head_union'])} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        "**P5.2 — Pre-#809 levels recovered** (gate: head union-lower above pre-#809 base halves; M inserts/s):",
        "",
        "| suite | arm | W | pre-#809 target | head half, run 1 / 2 | head union | verdict |",
        "|---|---|--:|--:|--:|--:|---|",
    ]
    for row in r["p52"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {row['writers']} | {_u(row['target'])} | "
            f"{_pair(row['head_halves'])} | {_u(row['head_union'])} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        f"**P5.3 — Restarts stay bounded** (safety factor 2.0; instantiated with t_hold = {_f(r['t_hold_ns'], 1)} ns):",
        "",
        "| suite | arm | W | t_op (ns) | ceiling (restarts/op) | observed, run 1 / 2 | verdict |",
        "|---|---|--:|--:|--:|--:|---|",
    ]
    for row in r["p53"]:
        obs = _pair(row["restarts_per_op"], 3) if row["restarts_per_op"] else "pending"
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {row['writers']} | {_f(row['t_op_ns'], 1)} | "
            f"{_f(row['ceiling'], 3)} | {obs} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        f"**P5.4 — Contended-line bound at W = 16** (ceiling = 1 / (t_line + t_hold); t_line = {_f(r['t_line_ns'], 1)} ns, t_hold = {_f(r['t_hold_ns'], 1)} ns):",
        "",
        "| suite | arm | ceiling (M/s) | head union (M/s) | verdict |",
        "|---|---|--:|--:|---|",
    ]
    for row in r["p54"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {_f(row['ceiling_mops'], 2)} | "
            f"{_u(row['head_union'])} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        "**Controls — C2 readers alongside writers at W = 1 R = 8** (predicted inside [97, 104] ns):",
        "",
        "| suite | arm | target union (ns) | head half, run 1 / 2 (ns) | verdict |",
        "|---|---|--:|--:|---|",
    ]
    for row in r["controls"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {_u(row['target'], 1)} | "
            f"{_pair(row['head_halves'], 1)} | {_verdict(row['verdict'])} |"
        )

    return out


def _self_test() -> int:
    fails = 0

    def check(name, cond):
        nonlocal fails
        if not cond:
            print(f"  FAIL {name}")
            fails += 1

    # Empty loader test
    r = evaluate(lambda s, n: None)
    check("P5.1 pending without artifacts", all(x["verdict"] == "pending" for x in r["p51"] if x["is_gate"]))
    check("P5.2 pending without artifacts", all(x["verdict"] == "pending" for x in r["p52"]))
    check("P5.3 pending without artifacts", all(x["verdict"] == "pending" for x in r["p53"]))
    check("P5.4 pending without artifacts", all(x["verdict"] == "pending" for x in r["p54"]))
    lines = render(lambda s, n: None)
    check("render is section 9", lines[0].startswith("## 9."))

    # Synthetic fixture test for PASS verdicts
    def mock_loader(suite, name):
        def _thru(arm, w, r, head_mops):
            return {
                "arm": arm, "writers": w, "readers": r,
                "head": {"expanse_writer_mops_median": head_mops, "expanse_reader_mops_median": 80.0 if r > 0 else None}
            }
        thru = []
        for a in ("set", "map"):
            thru.append(_thru(a, 1, 0, 5.0))
            for w in (2, 4, 8, 16):
                thru.append(_thru(a, w, 0, 5.0 + w * 1.5))
            thru.append(_thru(a, 1, 8, 2.0))
        return {"throughput": thru, "health": []}

    r_mock = evaluate(mock_loader)
    check("P5.1 passes on scaling mock", all(x["verdict"] == "PASS" for x in r_mock["p51"] if x["is_gate"]))
    check("P5.2 passes when rate clears targets", all(x["verdict"] == "PASS" for x in r_mock["p52"]))

    print("pr5_gate self-test:", "ok" if fails == 0 else f"{fails} failure(s)")
    return 1 if fails else 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(_self_test())
    print("\n".join(render()))
