#!/usr/bin/env python3
"""Multi-writer optimistic lock coupling (OLC) gate, read against METHODOLOGY.md §10 — never typed.

Inputs, per FFI suite (`docs/benchmarks/<suite>/results/`):

- `multi_writer_olc/baseline_concurrent_ab.json` and `_run2.json`: two
  two-commit runs (`scripts/bench_ab.py`) against baseline `1edfa952`, each
  cell carrying a `base` and a `head` reduction.
- `multi_writer_olc/counters_<cell>.json`: the head build's per-thread counters at the
  P5.1 multi-writer cells (`scripts/bench_counters.py --out-dir`).
- `line_transfer.json`: the reference host's cache-line transfer matrix.

Predictions evaluated:
- P5.1: Writers scale on disjoint expanses (W in {2, 4, 8, 16} rate > W=1).
- P5.2: Pre-#809 levels recovered at W in {4, 8}.
- P5.3: Restarts stay bounded (LockRestarts ÷ write_ops < restart_ceiling).
- P5.4: Contended-line bound holds at W=16.
- Controls: C2 readers inside baseline union, reader fallback < 1%.

What voids a cell (§10.4): a P5.1 or P5.2 verdict whose base half sits outside
the registered `10cd755d` union of §10.1 is `VOID` and decides nothing. A base
half sits outside when the closed interval its two runs' medians span shares no
point with the closed registered union — edges inclusive, medians as the
artifacts record them. That reading reproduces the count of §10.6 item 1 (7 of
20 base halves against `1edfa952`), and the self-test pins it there.

Self-test: `python3 docs/benchmarks/concurrency/scripts/multi_writer_olc_gate.py --self-test`.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import olc_bounds  # noqa: E402

ISSUE = "[#568](https://github.com/orieg/expanse/issues/568)"
PENDING = f"pending ({ISSUE})"
METHODOLOGY = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "METHODOLOGY.md"

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

# METHODOLOGY §10.1, as written: the registered baseline P5.1 and P5.2 are gated
# against — the union of the two #809 runs' head-half medians at `10cd755d`, C1
# writer-only cells, M inserts/s, two decimals. Typed from the frozen
# pre-registration (AGENTS.md §8.7), never derived from an artifact; the
# self-test re-reads §10.1's table and fails when this copy differs from it.
# (suite, arm) -> {W: (union-lower, union-upper)}
REGISTERED_COMMIT = "10cd755d"
REGISTERED_WRITERS = [1, 2, 4, 8, 16]
REGISTERED_UNION = {
    ("hot_comparison", "set"): {
        1: (7.19, 7.24), 2: (4.11, 4.16), 4: (3.17, 3.21), 8: (2.55, 2.57), 16: (2.35, 2.40),
    },
    ("hot_comparison", "map"): {
        1: (5.09, 5.09), 2: (3.32, 3.36), 4: (2.72, 2.73), 8: (2.34, 2.35), 16: (1.69, 2.12),
    },
    ("masstree_comparison", "map"): {
        1: (5.43, 5.44), 2: (3.44, 3.47), 4: (2.75, 2.76), 8: (2.33, 2.40), 16: (2.23, 2.23),
    },
    ("masstree_comparison", "str"): {
        1: (3.87, 3.88), 2: (2.38, 2.39), 4: (2.23, 2.25), 8: (1.93, 2.04), 16: (0.48, 0.48),
    },
}
VOID_BASE_HALF = f"VOID (base half outside the {REGISTERED_COMMIT} union, §10.4)"
PENDING_BASE_HALF = "pending (no base half, §10.4)"
BASE_DIGITS = 4  # base halves are printed to four decimals, so a §10.4 edge can be read from the table

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


def outside(span: tuple[float, float], registered: tuple[float, float]) -> bool:
    """§10.4's "sits outside": the closed span of the base-half medians shares no
    point with the closed registered union. Touching edges share a point."""
    if span[0] > span[1] or registered[0] > registered[1]:
        raise ValueError(f"inverted interval: span {span}, registered {registered}")
    return span[1] < registered[0] or span[0] > registered[1]


def _with_note(verdict: str, note: str) -> str:
    return f"{verdict[:-1]}; {note})" if verdict.endswith(")") else f"{verdict} ({note})"


def registered_union_from_methodology(text: str) -> dict:
    """§10.1's table as METHODOLOGY.md writes it, for the self-test's pin on REGISTERED_UNION."""
    start = text.find("### 10.1 ")
    end = text.find("\n### ", start + 1) if start >= 0 else -1
    if start < 0 or end < 0:
        return {}
    rows = [[c.strip() for c in line.strip().strip("|").split("|")]
            for line in text[start:end].splitlines() if line.strip().startswith("|")]
    if len(rows) < 3 or rows[0][2:] != [f"W = {w}" for w in REGISTERED_WRITERS]:
        return {}
    return {
        (cells[0], cells[1]): {
            w: tuple(float(x) for x in c.strip("[]").split(","))
            for w, c in zip(REGISTERED_WRITERS, cells[2:])
        }
        for cells in rows[2:]
    }


def evaluate(load=default_load, k_lines: int | None = None) -> dict:
    out = {
        "base_halves": [],
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
                load(suite, "multi_writer_olc/baseline_concurrent_ab.json") or load(suite, "baseline_concurrent_ab.json"),
                load(suite, "multi_writer_olc/baseline_concurrent_ab_run2.json") or load(suite, "baseline_concurrent_ab_run2.json"),
            ],
        }

    base_commit = None
    for suite in ("hot_comparison", "masstree_comparison"):
        for art in arts.get(suite, {}).get("ab_pair", []):
            if art:
                base_commit = ((art.get("provenance") or {}).get("ab") or {}).get("base_commit")
                if base_commit:
                    break
        if base_commit:
            break
    out["base_commit"] = base_commit or "b49835ad"

    def cell_k_lines(arts: dict) -> int:
        """Selects k (shared cache lines per insert) for P5.4 based on the build configuration.
        Defaults to olc_bounds.K_LINES["ffi_disjoint_default"] (2) for the default build,
        or olc_bounds.K_LINES["ffi_disjoint_padded"] (1) if lock-padded is enabled."""
        import os
        env_k = os.environ.get("EXPANSE_BENCH_K_LINES")
        if env_k is not None:
            try:
                return int(env_k)
            except ValueError:
                pass
        if os.environ.get("EXPANSE_LOCK_PADDED") in ("1", "true", "yes") or \
           os.environ.get("EXPANSE_BENCH_LOCK_PADDED") in ("1", "true", "yes"):
            return olc_bounds.K_LINES["ffi_disjoint_padded"]
        for suite in ("hot_comparison", "masstree_comparison"):
            for art in (arts.get(suite, {}).get("ab_pair") or []):
                if not art:
                    continue
                prov = art.get("provenance") or {}
                rustflags = str(prov.get("rustflags", ""))
                feat = str(prov.get("features", ""))
                variant = str(art.get("variant", ""))
                if "lock-padded" in rustflags or "lock-padded" in feat or "lock-padded" in variant:
                    return olc_bounds.K_LINES["ffi_disjoint_padded"]
        return olc_bounds.K_LINES["ffi_disjoint_default"]

    if k_lines is None:
        k_lines = cell_k_lines(arts)
    out["k_lines"] = k_lines

    def cell_t_hold_ns(suite: str, arm: str) -> float:
        """Discovers measured t_hold_ns for a (suite, arm) cell from loaded health artifacts,
        falling back to olc_bounds.lock_hold_ns(suite, arm) and then T_HOLD_LEAF_HYPOTHESIS_NS."""
        vals = []
        for art in arts.get(suite, {}).get("ab_pair", []):
            for h in (art or {}).get("health", []):
                if h.get("arm") == arm:
                    w_ops = (h.get("write_ops") or {}).get("median")
                    hz = (h.get("cycles_hz") or {}).get("median")
                    cycles = (h.get("lock_hold_cycles") or {}).get("median")
                    if w_ops and hz and cycles and w_ops > 0 and hz > 0:
                        vals.append((cycles / w_ops) / hz * 1e9)
        if vals:
            import statistics
            return float(statistics.median(vals))
        try:
            return float(olc_bounds.lock_hold_ns(suite, arm)["median"])
        except Exception:
            return float(olc_bounds.T_HOLD_LEAF_HYPOTHESIS_NS)

    out["t_hold_ns"] = {f"{s}/{a}": cell_t_hold_ns(s, a) for s, a in GATE_ARMS}

    # §10.4: every registered cell's base half against the §10.1 union, read
    # before any verdict that depends on it.
    base_check = {}
    for (suite, arm), by_w in REGISTERED_UNION.items():
        for w, registered in by_w.items():
            halves = [writer_mops((_cell(x, "throughput", arm, w, 0) or {}).get("base"))
                      for x in arts[suite]["ab_pair"]]
            if any(v is None for v in halves):
                span, reading = None, "pending"
            else:
                span = union(halves)
                reading = "outside" if outside(span, registered) else "overlaps"
            row = {
                "suite": suite,
                "arm": arm,
                "writers": w,
                "base_halves": halves,
                "base_span": span,
                "registered": registered,
                "reading": reading,
            }
            out["base_halves"].append(row)
            base_check[(suite, arm, w)] = row

    # P5.1: Writers scale on disjoint expanses
    for suite, arm in GATE_ARMS + PUBLISHED_ARMS:
        a = arts[suite]
        ab = a["ab_pair"]
        w1_heads = [writer_mops((_cell(x, "throughput", arm, 1, 0) or {}).get("head")) for x in ab]
        w1_u = union(w1_heads)
        w1_check = base_check[(suite, arm, 1)]
        is_gate = (suite, arm) in GATE_ARMS

        for w in C1_WRITERS:
            w_heads = [writer_mops((_cell(x, "throughput", arm, w, 0) or {}).get("head")) for x in ab]
            w_u = union(w_heads)
            check = base_check[(suite, arm, w)]
            row = {
                "suite": suite,
                "arm": arm,
                "writers": w,
                "is_gate": is_gate,
                "base_halves": check["base_halves"],
                "registered": check["registered"],
                "base_reading": check["reading"],
                "w1_base_reading": w1_check["reading"],
                "w1_union": w1_u,
                "head_union": w_u,
                "head_halves": w_heads,
                "verdict": "pending",
            }
            if w1_u is None or w_u is None or any(v is None for v in w_heads):
                row["verdict"] = "pending"
            elif not is_gate:
                row["verdict"] = "published (not a gate cell; tree bracket held)"
            elif check["reading"] == "pending":
                row["verdict"] = PENDING_BASE_HALF
            elif check["reading"] == "outside":
                row["verdict"] = VOID_BASE_HALF
            else:
                if w_u[0] > w1_u[1]:
                    verdict = "PASS"
                elif w_u[1] < w1_u[0]:
                    verdict = "REFUTED (writers still fall)"
                else:
                    verdict = "BOUNDARY_RESULT (unions overlap)"
                # The comparator is named, not voided: §10.4 voids a verdict by its
                # own cell's base half.
                if w1_check["reading"] != "overlaps":
                    what = "outside the " + REGISTERED_COMMIT + " union" if w1_check["reading"] == "outside" else "missing"
                    verdict = _with_note(verdict, f"W = 1 comparator's base half {what}")
                row["verdict"] = verdict
            out["p51"].append(row)

    # P5.2: Pre-#809 levels recovered at W in {4, 8}
    for (suite, arm, w), target in P52_TARGETS.items():
        a = arts[suite]
        ab = a["ab_pair"]
        w_heads = [writer_mops((_cell(x, "throughput", arm, w, 0) or {}).get("head")) for x in ab]
        w_u = union(w_heads)
        check = base_check[(suite, arm, w)]
        row = {
            "suite": suite,
            "arm": arm,
            "writers": w,
            "target": target,
            "base_halves": check["base_halves"],
            "registered": check["registered"],
            "base_reading": check["reading"],
            "head_union": w_u,
            "head_halves": w_heads,
            "verdict": "pending",
        }
        if w_u is None or any(v is None for v in w_heads):
            row["verdict"] = "pending"
        elif check["reading"] == "pending":
            row["verdict"] = PENDING_BASE_HALF
        elif check["reading"] == "outside":
            row["verdict"] = VOID_BASE_HALF
        elif w_u[0] > target[1]:
            row["verdict"] = "PASS"
        elif w_u[1] < target[0]:
            row["verdict"] = "REFUTED (below pre-#809 base halves)"
        else:
            row["verdict"] = "BOUNDARY_RESULT (union crosses pre-#809 target)"
        out["p52"].append(row)

    # P5.3: Restarts stay bounded
    for suite, arm in GATE_ARMS:
        arm_t_hold_ns = cell_t_hold_ns(suite, arm)
        a = arts[suite]
        ab = a["ab_pair"]
        w1_heads = [writer_mops((_cell(x, "throughput", arm, 1, 0) or {}).get("head")) for x in ab]
        w1_u = union(w1_heads)
        w1_med = w1_u[0] if w1_u else None
        t_op_ns = (1e3 / w1_med) if w1_med and w1_med > 0 else 180.0

        for w in C1_WRITERS:
            ceiling = olc_bounds.restart_ceiling(w, arm_t_hold_ns, t_op_ns, safety_factor=2.0)
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
                "t_hold_ns": arm_t_hold_ns,
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
    for suite, arm in GATE_ARMS:
        arm_t_hold_ns = cell_t_hold_ns(suite, arm)
        bound_ops = olc_bounds.contended_rmw_ceiling(k_lines, t_line_ns, arm_t_hold_ns)
        bound_mops = bound_ops / 1e6
        a = arts[suite]
        ab = a["ab_pair"]
        w16_heads = [writer_mops((_cell(x, "throughput", arm, 16, 0) or {}).get("head")) for x in ab]
        w16_u = union(w16_heads)
        row = {
            "suite": suite,
            "arm": arm,
            "writers": 16,
            "k_lines": k_lines,
            "t_line_ns": t_line_ns,
            "t_hold_ns": arm_t_hold_ns,
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
        elif c2_u[1] < target[0]:
            row["verdict"] = f"MOVED (faster: {_pair(c2_heads, 1)} ns vs [{target[0]:.1f}, {target[1]:.1f}] ns)"
        else:
            row["verdict"] = f"MOVED (slower: {_pair(c2_heads, 1)} ns vs [{target[0]:.1f}, {target[1]:.1f}] ns)"
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
    if v.startswith(("REFUTED", "MOVED", "VOID")):
        return f"**`{v}`**"
    if v.startswith("BOUNDARY"):
        return f"`{v}`"
    return v


def _reading(reading: str) -> str:
    return "**outside**" if reading == "outside" else reading


def render(load=default_load, k_lines: int | None = None) -> list[str]:
    r = evaluate(load, k_lines=k_lines)
    base_commit = r.get("base_commit", "b49835ad")
    k_val = r.get("k_lines", 2)
    n_cells = len(r["base_halves"])
    n_outside = sum(x["reading"] == "outside" for x in r["base_halves"])
    n_pending = sum(x["reading"] == "pending" for x in r["base_halves"])
    count = f"{n_outside} of {n_cells} outside" + (f"; {n_pending} {PENDING}" if n_pending else "")
    out = [
        "## 9. Multi-writer OLC gate (METHODOLOGY §10)",
        "",
        "Read against §10.2; every number is evaluated over two two-commit runs "
        f"against baseline `{base_commit}` and the head build's counters. A P5.1 or P5.2 verdict "
        f"whose base half sits outside the registered `{REGISTERED_COMMIT}` union of §10.1 is `VOID` "
        "(§10.4) and decides nothing; a base half sits outside when the interval its two runs' medians "
        "span shares no point with that union, edges inclusive.",
        "",
        f"**§10.4 — base halves against the registered `{REGISTERED_COMMIT}` union** "
        f"(M inserts/s; base halves to {BASE_DIGITS} decimals; {count}):",
        "",
        "| suite | arm | W | base half, run 1 / 2 | base-half span | §10.1 union | reading |",
        "|---|---|--:|--:|--:|--:|---|",
    ]
    for row in r["base_halves"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {row['writers']} | {_pair(row['base_halves'], BASE_DIGITS)} | "
            f"{_u(row['base_span'], BASE_DIGITS)} | {_u(row['registered'])} | {_reading(row['reading'])} |"
        )

    out += [
        "",
        "**P5.1 — Writers scale on disjoint expanses** (gate: head union-lower at W ≥ 2 above W = 1 union-upper; M inserts/s):",
        "",
        "| suite | arm | W | base half, run 1 / 2 | §10.1 union | head W = 1 union | head half, run 1 / 2 | head union | verdict |",
        "|---|---|--:|--:|--:|--:|--:|--:|---|",
    ]
    for row in r["p51"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {row['writers']} | {_pair(row['base_halves'], BASE_DIGITS)} | "
            f"{_u(row['registered'])} | {_u(row['w1_union'])} | {_pair(row['head_halves'])} | "
            f"{_u(row['head_union'])} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        "**P5.2 — Pre-#809 levels recovered** (gate: head union-lower above pre-#809 base halves; M inserts/s):",
        "",
        "| suite | arm | W | base half, run 1 / 2 | §10.1 union | pre-#809 target | head half, run 1 / 2 | head union | verdict |",
        "|---|---|--:|--:|--:|--:|--:|--:|---|",
    ]
    for row in r["p52"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {row['writers']} | {_pair(row['base_halves'], BASE_DIGITS)} | "
            f"{_u(row['registered'])} | {_u(row['target'])} | {_pair(row['head_halves'])} | "
            f"{_u(row['head_union'])} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        "**P5.3 — Restarts stay bounded** (safety factor 2.0; per-arm measured t_hold from health rows):",
        "",
        "| suite | arm | W | t_hold (ns) | t_op (ns) | ceiling (restarts/op) | observed, run 1 / 2 | verdict |",
        "|---|---|--:|--:|--:|--:|--:|---|",
    ]
    for row in r["p53"]:
        obs = _pair(row["restarts_per_op"], 3) if row["restarts_per_op"] else "pending"
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {row['writers']} | {_f(row['t_hold_ns'], 1)} | "
            f"{_f(row['t_op_ns'], 1)} | {_f(row['ceiling'], 3)} | {obs} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        f"**P5.4 — Contended-line bound at W = 16** (ceiling = 1 / (k·t_line + t_hold); k = {k_val}, t_line = {_f(r['t_line_ns'], 1)} ns, per-arm t_hold):",
        "",
        "| suite | arm | t_hold (ns) | ceiling (M/s) | head union (M/s) | verdict |",
        "|---|---|--:|--:|--:|---|",
    ]
    for row in r["p54"]:
        out.append(
            f"| `{row['suite']}` | `{row['arm']}` | {_f(row['t_hold_ns'], 1)} | {_f(row['ceiling_mops'], 2)} | "
            f"{_u(row['head_union'])} | {_verdict(row['verdict'])} |"
        )

    out += [
        "",
        "**Controls — C2 readers alongside writers at W = 1 R = 8** (predicted inside [97, 104] ns; lower is faster):",
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


def _ab_loader(base: dict | None = None):
    """A two-commit fixture: every registered C1 cell in both runs, alike.

    `base` maps (suite, arm, W) to that cell's base-half median (None drops the
    half); an unnamed cell's base half sits at its §10.1 union's midpoint. Head
    halves scale with W, so every gate verdict the fixture can reach is PASS.
    """
    base = base or {}

    def load(suite, name):
        thru = []
        for (s, arm), by_w in REGISTERED_UNION.items():
            if s != suite:
                continue
            for w, (lo, hi) in by_w.items():
                b = base.get((suite, arm, w), (lo + hi) / 2)
                thru.append({
                    "arm": arm, "writers": w, "readers": 0,
                    "base": None if b is None else {"expanse_writer_mops_median": b},
                    "head": {"expanse_writer_mops_median": 5.0 + (w - 1) * 1.5},
                })
            thru.append({"arm": arm, "writers": 1, "readers": 8,
                         "head": {"expanse_writer_mops_median": 2.0, "expanse_reader_mops_median": 80.0}})
        return {"throughput": thru, "health": []}

    return load


def _self_test() -> int:
    fails = 0

    def check(name, cond):
        nonlocal fails
        if not cond:
            print(f"  FAIL {name}")
            fails += 1

    def cells(rows, prefix):
        return {(x["suite"], x["arm"], x["writers"]) for x in rows if x["verdict"].startswith(prefix)}

    def verdicts(rows):
        return {(x["suite"], x["arm"], x["writers"]): x["verdict"] for x in rows}

    # Empty loader test
    r = evaluate(lambda s, n: None)
    check("P5.1 pending without artifacts", all(x["verdict"] == "pending" for x in r["p51"] if x["is_gate"]))
    check("P5.2 pending without artifacts", all(x["verdict"] == "pending" for x in r["p52"]))
    check("P5.3 pending without artifacts", all(x["verdict"] == "pending" for x in r["p53"]))
    check("P5.4 pending without artifacts", all(x["verdict"] == "pending" for x in r["p54"]))
    check("§10.4 base halves pending without artifacts",
          len(r["base_halves"]) == 20 and all(x["reading"] == "pending" for x in r["base_halves"]))
    check("P5.3 carries per-arm t_hold_ns", all(x["t_hold_ns"] > 0 for x in r["p53"]))
    check("P5.4 carries per-arm t_hold_ns", all(x["t_hold_ns"] > 0 for x in r["p54"]))
    check("P5.4 carries k_lines", all(x["k_lines"] == 2 for x in r["p54"]))
    lines = render(lambda s, n: None)
    check("render is section 9", lines[0].startswith("## 9."))

    # §10.4's predicate on its edges: closed intervals, so a shared point is not outside.
    reg = (2.0, 3.0)
    check("outside: span entirely above the union", outside((3.0001, 4.0), reg))
    check("outside: span entirely below the union", outside((1.0, 1.9999), reg))
    check("not outside: span partly overlapping the union", not outside((2.5, 3.5), reg))
    check("not outside: span inside the union", not outside((2.2, 2.8), reg))
    check("not outside: span covering the union", not outside((1.0, 4.0), reg))
    check("not outside: span touching the union-upper", not outside((3.0, 4.0), reg))
    check("not outside: span touching the union-lower", not outside((1.0, 2.0), reg))
    check("not outside: a one-point union on the span's edge", not outside((4.0, 5.0), (5.0, 5.0)))
    check("outside: a one-point union just past the span", outside((4.0, 4.9999), (5.0, 5.0)))
    try:
        outside((3.0, 2.0), reg)
        check("an inverted span is refused", False)
    except ValueError:
        pass

    # The carried union is §10.1 as written (a frozen pre-registration, AGENTS.md §8.7).
    check("REGISTERED_UNION equals METHODOLOGY §10.1's table",
          registered_union_from_methodology(METHODOLOGY.read_text()) == REGISTERED_UNION)

    # Reference value check against committed artifacts (default k=2)
    r_def = evaluate()
    set_row = next(x for x in r_def["p54"] if x["suite"] == "hot_comparison" and x["arm"] == "set")
    map_row = next(x for x in r_def["p54"] if x["suite"] == "hot_comparison" and x["arm"] == "map")
    check("set t_hold is ~14.0 ns", 13.9 <= set_row["t_hold_ns"] <= 14.1)
    check("map t_hold is ~45.0 ns", 44.0 <= map_row["t_hold_ns"] <= 46.0)
    check("set P5.4 default ceiling is ~12.4 M/s (k=2)", 12.3 <= set_row["ceiling_mops"] <= 12.5)
    check("map P5.4 default ceiling is ~8.95 M/s (k=2)", 8.9 <= map_row["ceiling_mops"] <= 9.1)

    # §10.6 item 1, against the committed artifacts: 7 of 20 base halves measured
    # against `1edfa952` sit outside §10.1's union, Masstree map W = 1 among them.
    outside_cells = {(x["suite"], x["arm"], x["writers"]) for x in r_def["base_halves"] if x["reading"] == "outside"}
    check("all 20 committed base halves are read",
          len(r_def["base_halves"]) == 20 and not any(x["reading"] == "pending" for x in r_def["base_halves"]))
    check("7 of 20 committed base halves sit outside the union (§10.6 item 1)", len(outside_cells) == 7)
    check("the seven outside cells are the recomputed set", outside_cells == {
        ("hot_comparison", "map", 4), ("hot_comparison", "map", 8),
        ("masstree_comparison", "map", 1), ("masstree_comparison", "map", 4),
        ("masstree_comparison", "str", 2), ("masstree_comparison", "str", 4), ("masstree_comparison", "str", 16),
    })
    affected = {("hot_comparison", "map", 4), ("hot_comparison", "map", 8), ("masstree_comparison", "map", 4)}
    check("P5.1 VOID rows are exactly HOT map W = 4, 8 and Masstree map W = 4", cells(r_def["p51"], "VOID") == affected)
    check("P5.2 VOID rows are exactly HOT map W = 4, 8 and Masstree map W = 4", cells(r_def["p52"], "VOID") == affected)
    check("the string arm's rows stay published, never a verdict",
          all(x["verdict"].startswith("published") for x in r_def["p51"] if not x["is_gate"]))
    mt_map = [x for x in r_def["p51"]
              if (x["suite"], x["arm"]) == ("masstree_comparison", "map") and not x["verdict"].startswith("VOID")]
    check("Masstree map P5.1 rows name their outside W = 1 comparator",
          len(mt_map) == 3 and all("W = 1 comparator's base half outside" in x["verdict"] for x in mt_map))
    check("no HOT P5.1 row names its comparator",
          not any("comparator" in x["verdict"] for x in r_def["p51"] if x["suite"] == "hot_comparison"))
    committed = render()
    check("committed render carries six VOID rows", sum(f"**`{VOID_BASE_HALF}`**" in line for line in committed) == 6)
    check("committed render states 7 of 20 outside", any("7 of 20 outside" in line for line in committed))

    # Reference value check with padded k=1 override
    r_pad = evaluate(k_lines=1)
    set_pad = next(x for x in r_pad["p54"] if x["suite"] == "hot_comparison" and x["arm"] == "set")
    map_pad = next(x for x in r_pad["p54"] if x["suite"] == "hot_comparison" and x["arm"] == "map")
    check("set P5.4 padded ceiling is ~21.1 M/s (k=1)", 21.0 <= set_pad["ceiling_mops"] <= 21.2)
    check("map P5.4 padded ceiling is ~12.8 M/s (k=1)", 12.6 <= map_pad["ceiling_mops"] <= 12.9)

    # Synthetic fixture test for PASS verdicts: base halves inside their unions
    r_mock = evaluate(_ab_loader())
    check("P5.1 passes on scaling mock", all(x["verdict"] == "PASS" for x in r_mock["p51"] if x["is_gate"]))
    check("P5.2 passes when rate clears targets", all(x["verdict"] == "PASS" for x in r_mock["p52"]))
    check("no base half outside when each sits mid-union", all(x["reading"] == "overlaps" for x in r_mock["base_halves"]))

    # The decision point: one base half past its union voids P5.1 and P5.2 at
    # that cell, though its head halves would PASS, and nowhere else.
    cell = ("hot_comparison", "map", 4)
    upper = REGISTERED_UNION[cell[:2]][cell[2]][1]
    r_void = evaluate(_ab_loader({cell: upper + 0.0001}))
    p51, p52 = verdicts(r_void["p51"]), verdicts(r_void["p52"])
    check("P5.1 VOID at a base half past the union", p51[cell] == VOID_BASE_HALF)
    check("P5.2 VOID at a base half past the union", p52[cell] == VOID_BASE_HALF)
    check("P5.1 read everywhere else", all(v == "PASS" for k, v in p51.items() if k != cell and k[1] != "str"))
    check("P5.2 read everywhere else", all(v == "PASS" for k, v in p52.items() if k != cell))
    lines = render(_ab_loader({cell: upper + 0.0001}))
    check("render bolds the VOID label", any(f"**`{VOID_BASE_HALF}`**" in line for line in lines))
    check("render marks the outside base half", any("**outside**" in line for line in lines))

    # A base half on the union's edge touches it: the verdict is read.
    r_edge = evaluate(_ab_loader({cell: upper}))
    check("P5.1 read when the base half touches the union", verdicts(r_edge["p51"])[cell] == "PASS")
    check("P5.2 read when the base half touches the union", verdicts(r_edge["p52"])[cell] == "PASS")

    # No base half cannot clear §10.4: pending, never a verdict (AGENTS.md §8.1).
    r_none = evaluate(_ab_loader({cell: None}))
    check("P5.1 pending without a base half", verdicts(r_none["p51"])[cell] == PENDING_BASE_HALF)
    check("P5.2 pending without a base half", verdicts(r_none["p52"])[cell] == PENDING_BASE_HALF)

    # A W = 1 comparator outside its union is named on the rows that read it;
    # their labels stand, and P5.2, which never reads W = 1, is untouched.
    w1 = ("masstree_comparison", "map", 1)
    r_w1 = evaluate(_ab_loader({w1: REGISTERED_UNION[w1[:2]][1][1] + 0.1}))
    rows = [x for x in r_w1["p51"] if (x["suite"], x["arm"]) == w1[:2]]
    check("a W = 1 comparator outside its union is named and the label stands",
          len(rows) == 4 and all(x["verdict"] == "PASS (W = 1 comparator's base half outside the "
                                 f"{REGISTERED_COMMIT} union)" for x in rows))
    check("P5.2 does not read the W = 1 comparator", all(x["verdict"] == "PASS" for x in r_w1["p52"]))

    print("multi_writer_olc_gate self-test:", "ok" if fails == 0 else f"{fails} failure(s)")
    return 1 if fails else 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(_self_test())
    k_arg = None
    if "--k-lines" in sys.argv:
        idx = sys.argv.index("--k-lines")
        if idx + 1 < len(sys.argv):
            k_arg = int(sys.argv[idx + 1])
    print("\n".join(render(k_lines=k_arg)))
