#!/usr/bin/env python3
"""Reader-scaling bounds for the readers-only string cell (#730, AGENTS.md §8.8 commit 1).

The arithmetic #730's plan rests on, computed from the committed artifacts
rather than narrated, and unit-tested against pinned reference values. Every
input is read from a named artifact at run time; the one input no artifact
carries -- what one counted event costs in cycles -- is a stated hypothesis,
and every ceiling that depends on it says so.

What this module does not do: claim a level, attribute a mechanism, or
evaluate a prediction. The #730 pre-registration is not written yet; when it
is, it invokes these functions instead of restating their outputs.

Artifacts read (all under `docs/benchmarks/masstree_comparison/results/`):
  `step0/baseline_concurrent.json`, `step0/baseline_concurrent_run2.json`
      tree `a1982ff2`, one harness process running every round of a cell.
  `at_6f8d6ba5/baseline_concurrent.json`, `at_6f8d6ba5/baseline_concurrent_run2.json`
      tree `6f8d6ba5`, the same procedure. The suite's live
      `baseline_concurrent*.json` names are re-measured in place, so this
      reduction reads the pair it was written over from the commit-named copy.
  `baseline_concurrent_ab.json`, `baseline_concurrent_ab_run2.json`
      `scripts/bench_ab.py`: base `55b511df` and head `38fb2b1e` engines under
      one harness, one process per round, builds alternating.
  `counters_masstree_conc_str_w0_r1.json`, `counters_masstree_conc_str_w0_r8.json`
      `scripts/bench_counters.py` per-thread cells at `a1982ff2`.
  `counters_strmap_hugepage_off_1m.json`, `counters_strmap_hugepage_on_1m.json`
      `scripts/perf_counters.py` single-threaded pair at `b1868813`.

Sources for the estimators:
  Efron, "Better Bootstrap Confidence Intervals", JASA 82 (1987) -- the BCa
    interval, through `scripts/bca_bootstrap.py`.
  Hampel, "The Influence Curve and its Role in Robust Estimation", JASA 69
    (1974) -- the median absolute deviation, scaled by 1.4826 to estimate a
    normal standard deviation.
  Cohen, Statistical Power Analysis for the Behavioral Sciences, 2nd ed.
    (1988), ch. 2 -- the two-sample minimum detectable difference
    (z_{1-alpha/2} + z_{1-beta}) * sigma * sqrt(2 / n).

Usage:
    python3 scripts/reader_scaling_bounds.py              # the reduction, then the unit tests
    python3 scripts/reader_scaling_bounds.py --self-test  # the unit tests only
    python3 scripts/reader_scaling_bounds.py --table      # the README §13 block
    python3 scripts/reader_scaling_bounds.py --write-readme
"""

from __future__ import annotations

import json
import math
import statistics
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

MT = REPO_ROOT / "docs" / "benchmarks" / "masstree_comparison" / "results"
README = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "README.md"
README_BEGIN = "<!-- BEGIN GENERATED: scripts/reader_scaling_bounds.py --table -->"
README_END = "<!-- END GENERATED: scripts/reader_scaling_bounds.py --table -->"

# The cell #730 is about: the string arm, no writer, eight readers.
CELL = {"arm": "str", "writers": 0, "readers": 8}
READER_KEY = "expanse_reader_mops"

PROCEDURE_ONE_PROCESS = "one process, every round"
PROCEDURE_PER_ROUND = "one process per round"

# (label, artifact, build filter, procedure). The build filter selects one half
# of a two-commit artifact; `None` reads every round of a single-build artifact.
LEVEL_SOURCES: tuple[tuple[str, Path, str | None, str], ...] = (
    ("a1982ff2 run 1", MT / "step0" / "baseline_concurrent.json", None, PROCEDURE_ONE_PROCESS),
    ("a1982ff2 run 2", MT / "step0" / "baseline_concurrent_run2.json", None, PROCEDURE_ONE_PROCESS),
    ("6f8d6ba5 run 1", MT / "at_6f8d6ba5" / "baseline_concurrent.json", None, PROCEDURE_ONE_PROCESS),
    ("6f8d6ba5 run 2", MT / "at_6f8d6ba5" / "baseline_concurrent_run2.json", None, PROCEDURE_ONE_PROCESS),
    ("ab run 1, base 55b511df", MT / "baseline_concurrent_ab.json", "base", PROCEDURE_PER_ROUND),
    ("ab run 1, head 38fb2b1e", MT / "baseline_concurrent_ab.json", "head", PROCEDURE_PER_ROUND),
    ("ab run 2, base 55b511df", MT / "baseline_concurrent_ab_run2.json", "base", PROCEDURE_PER_ROUND),
    ("ab run 2, head 38fb2b1e", MT / "baseline_concurrent_ab_run2.json", "head", PROCEDURE_PER_ROUND),
)
# Same procedure, same tree, two runs: the pairs whose agreement is a
# between-run spread rather than a procedure or engine difference.
SAME_PROCEDURE_PAIRS: tuple[tuple[str, tuple[str, ...], tuple[str, ...]], ...] = (
    ("a1982ff2, one process", ("a1982ff2 run 1",), ("a1982ff2 run 2",)),
    ("6f8d6ba5, one process", ("6f8d6ba5 run 1",), ("6f8d6ba5 run 2",)),
    ("ab, both halves pooled", ("ab run 1, base 55b511df", "ab run 1, head 38fb2b1e"),
     ("ab run 2, base 55b511df", "ab run 2, head 38fb2b1e")),
)

COUNTERS_R1 = MT / "counters_masstree_conc_str_w0_r1.json"
COUNTERS_R8 = MT / "counters_masstree_conc_str_w0_r8.json"
HUGEPAGE_OFF = MT / "counters_strmap_hugepage_off_1m.json"
HUGEPAGE_ON = MT / "counters_strmap_hugepage_on_1m.json"
HUGEPAGE_CELL = "strmap_get/pop=1000000/hit=100"

# HYPOTHESIS, not measured: the cycles one counted snoop hit or RFO miss costs
# the thread that takes it. No committed artifact prices a single event. The
# bracket is chosen to contain the host's measured one-way spinning line
# transfer converted at the R = 8 core clock (`line_transfer_cycles`, reported
# beside it); every ceiling computed from it is labelled a hypothesis.
EVENT_COST_CYCLES_LO = 100.0
EVENT_COST_CYCLES_HI = 400.0
LINE_TRANSFER = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "line_transfer.json"

# Two-sided 5% test at 80% power.
Z_ALPHA_2 = 1.959963984540054
Z_BETA_80 = 0.8416212335729143
MAD_TO_SIGMA = 1.4826


# ---------------------------------------------------------------------------
# Artifact readers
# ---------------------------------------------------------------------------

def cell_rounds(path: Path, build: str | None = None, key: str = READER_KEY) -> list[float]:
    """The per-round `key` series of the W = 0, R = 8 string cell, in round order.

    Reads `throughput[*].rounds_raw` of a `masstree_concurrent` sweep artifact.
    With `build`, keeps only that half of a two-commit artifact. Refuses a
    missing cell, a duplicate round or a gap, so a series is never silently
    shorter than the artifact says (AGENTS.md §8.1).
    """
    data = json.loads(path.read_text())
    cells = [c for c in data["throughput"]
             if all(c.get(k) == v for k, v in CELL.items())]
    if len(cells) != 1:
        raise ValueError(f"{path.name}: expected one {CELL} cell, found {len(cells)}")
    rows = [r for r in cells[0]["rounds_raw"] if build is None or r.get("build") == build]
    if not rows:
        raise ValueError(f"{path.name}: no rounds for build {build!r}")
    rounds = sorted(int(r["round"]) for r in rows)
    if rounds != list(range(len(rows))):
        raise ValueError(f"{path.name} build {build!r}: rounds {rounds} are not 0..{len(rows) - 1}")
    by_round = {int(r["round"]): float(r[key]) for r in rows}
    return [by_round[i] for i in range(len(rows))]


def artifact_commit(path: Path, build: str | None) -> str:
    """The engine commit a level was measured on: the artifact's, or the named half's."""
    prov = json.loads(path.read_text())["provenance"]
    if build is None:
        return str(prov["commit"])
    return str(prov["ab"][f"{build}_commit"])


def reader_event(path: Path, event: str) -> dict[str, float]:
    """`events["reader/<event>"]` of a per-thread counter cell: mean and BCa bounds per probe."""
    data = json.loads(path.read_text())
    e = data["events"][f"reader/{event}"]
    return {"mean": float(e["per_op_mean"]), "lo": float(e["ci_lower"]), "hi": float(e["ci_upper"])}


def reader_thread_task_clocks(path: Path) -> list[list[float]]:
    """Per round, each reader thread's own `task-clock` (ms), from the per-thread rows."""
    data = json.loads(path.read_text())
    out = []
    for r in sorted(data["rounds_raw"], key=lambda x: x["round"]):
        clocks = [float(t["rows"]["task-clock"]["value"])
                  for t in r["threads"].values() if t.get("role") == "reader"]
        if len(clocks) != int(data["roles"]["reader"]["threads"]):
            raise ValueError(f"{path.name} round {r['round']}: {len(clocks)} reader threads, "
                             f"the cell declares {data['roles']['reader']['threads']}")
        out.append(clocks)
    return out


def hugepage_cycles_per_probe(path: Path, cell_id: str = HUGEPAGE_CELL) -> float:
    """Cycles per probe of a `perf_counters.py` cell: the artifact's point over its probe count."""
    data = json.loads(path.read_text())
    cell = next(c for c in data["cells"] if c["id"] == cell_id)
    probes = int(cell["distinct_probes"]) * int(cell["passes"])
    return float(cell["counters"]["cycles"]["point"]) / probes


def line_transfer_median_ns(path: Path = LINE_TRANSFER) -> float:
    """The host's one-way spinning line transfer between P-cores: median of the cell means.

    The same estimator as `olc_bounds.line_transfer_ns`.
    """
    d = json.loads(path.read_text())
    means = [c["ns_per_transfer"]["mean"] for c in d["cells"]
             if c.get("mode") == "spin" and c.get("cpu_b") is not None]
    if not means:
        raise ValueError(f"{path}: no spinning pair cells")
    return statistics.median(means)


# ---------------------------------------------------------------------------
# Bounds and reductions
# ---------------------------------------------------------------------------

def per_arm_interval(samples: list[float], n_resamples: int = 2000, seed: int = 42) -> dict[str, object]:
    """Mean over rounds with its BCa 95% interval, and the construction label.

    The committed sweep artifacts carry a ratio interval only; this is the
    per-arm reduction they lack. Rounds are resampled as exchangeable, which
    `round_outliers` checks rather than assumes.
    """
    if len(samples) < 3:
        raise ValueError(f"need at least 3 rounds, got {len(samples)}")
    mean, lo, hi, method = bca_bootstrap_ci_with_method(samples, 0.95, n_resamples, seed)
    return {"mean": mean, "lo": lo, "hi": hi, "method": method, "n": len(samples)}


def round_outliers(samples: list[float], k: float = 3.0) -> list[int]:
    """Round indices more than `k` scaled MADs from the median (Hampel, 1974).

    The exchangeability check behind `per_arm_interval`: a round flagged here
    is not a draw from the same distribution as the others, and a BCa interval
    over i.i.d. resamples does not model it.
    """
    if k <= 0:
        raise ValueError("k must be positive")
    if len(samples) < 3:
        raise ValueError(f"need at least 3 rounds, got {len(samples)}")
    med = statistics.median(samples)
    mad = statistics.median(abs(x - med) for x in samples)
    if mad == 0:
        return [i for i, x in enumerate(samples) if x != med]
    scale = MAD_TO_SIGMA * mad
    return [i for i, x in enumerate(samples) if abs(x - med) > k * scale]


def mde_from_rounds(samples: list[float], z_alpha: float = Z_ALPHA_2, z_beta: float = Z_BETA_80) -> dict[str, float]:
    """Minimum detectable difference between two arms of `len(samples)` rounds each (Cohen, 1988).

    `sigma` is the per-round standard deviation of `samples`, assumed equal in
    both arms. Returned absolute and relative to the mean.
    """
    n = len(samples)
    if n < 2:
        raise ValueError(f"need at least 2 rounds, got {n}")
    sigma = statistics.stdev(samples)
    mde = (z_alpha + z_beta) * sigma * math.sqrt(2.0 / n)
    return {"sigma": sigma, "mde": mde, "relative": mde / statistics.fmean(samples), "n": float(n)}


def frequency_share(cycles_r1: float, ref_r1: float, cycles_rk: float, ref_rk: float,
                    ns_r1: float, ns_rk: float) -> dict[str, float]:
    """How much of the R = 1 -> R = k growth in ns per probe the core clock accounts for.

    Holding R = 1's core cycles per ns fixed, R = k's cycles per probe predict
    `cycles_rk * ns_r1 / cycles_r1` ns; the measured ns above that is the part
    of the growth a slower clock explains. `droop` is the fall in
    `cycles / ref-cycles` between the two cells (AGENTS.md §8.20.1).
    """
    for name, v in (("cycles_r1", cycles_r1), ("ref_r1", ref_r1), ("cycles_rk", cycles_rk),
                    ("ref_rk", ref_rk), ("ns_r1", ns_r1), ("ns_rk", ns_rk)):
        if not v > 0:
            raise ValueError(f"{name} must be positive, got {v}")
    if ns_rk <= ns_r1:
        raise ValueError("ns_rk must exceed ns_r1: there is no growth to attribute")
    predicted = cycles_rk * ns_r1 / cycles_r1
    clock_ns = ns_rk - predicted
    growth = ns_rk - ns_r1
    return {
        "droop": 1.0 - (cycles_rk / ref_rk) / (cycles_r1 / ref_r1),
        "predicted_ns": predicted,
        "clock_ns": clock_ns,
        "growth_ns": growth,
        "share": clock_ns / growth,
    }


def event_cycle_ceiling(per_probe_r1: float, per_probe_rk: float, cost_cycles: float) -> float:
    """Cycles per probe an event's growth can account for at `cost_cycles` per event.

    An upper bound when `cost_cycles` is an upper bound on one event's cost;
    negative growth explains nothing and returns 0.
    """
    if cost_cycles < 0 or per_probe_r1 < 0 or per_probe_rk < 0:
        raise ValueError("event rates and costs must be non-negative")
    return max(0.0, per_probe_rk - per_probe_r1) * cost_cycles


def unexplained_cycles(growth_cycles: float, ceilings: list[float]) -> float:
    """The growth no listed counter can cover even at its ceiling: a floor on the unexplained part.

    Never attributed to any mechanism by subtraction (AGENTS.md §8.20.4); it
    is reported as unexplained.
    """
    if any(c < 0 for c in ceilings):
        raise ValueError("ceilings must be non-negative")
    return max(0.0, growth_cycles - sum(ceilings))


def line_transfer_cycles(t_line_ns: float, cycles_per_ns: float) -> float:
    """One measured line transfer, expressed in core cycles at a given clock."""
    if t_line_ns <= 0 or cycles_per_ns <= 0:
        raise ValueError("inputs must be positive")
    return t_line_ns * cycles_per_ns


def max_over_mean_bias(thread_times: list[float]) -> float:
    """`max_r(T_r) / mean_r(T_r) - 1`: how far a slowest-thread estimator sits above the mean.

    A harness that divides the join-of-all-readers elapsed time by the probes
    reads the slowest reader; a per-thread counter reads the mean.
    """
    if not thread_times or any(t <= 0 for t in thread_times):
        raise ValueError("need positive thread times")
    return max(thread_times) / statistics.fmean(thread_times) - 1.0


def hugepage_ceiling(cycles_off: float, cycles_on: float) -> float:
    """Relative change in cycles per probe from 4 KiB to huge pages (negative is a saving)."""
    if cycles_off <= 0 or cycles_on <= 0:
        raise ValueError("cycles must be positive")
    return cycles_on / cycles_off - 1.0


def implied_reader_mops(readers: int, ns_per_probe: float) -> float:
    """Aggregate reader throughput, M probes/s, that a per-reader ns per probe implies."""
    if readers <= 0 or ns_per_probe <= 0:
        raise ValueError("inputs must be positive")
    return readers * 1e9 / ns_per_probe / 1e6


# ---------------------------------------------------------------------------
# The reduction and its README block
# ---------------------------------------------------------------------------

def levels() -> list[dict[str, object]]:
    out = []
    for label, path, build, procedure in LEVEL_SOURCES:
        s = cell_rounds(path, build)
        iv = per_arm_interval(s)
        low = min(range(len(s)), key=lambda i: s[i])
        out.append({
            "label": label, "artifact": path.relative_to(MT).as_posix(), "commit": artifact_commit(path, build),
            "procedure": procedure, "samples": s, "interval": iv, "sd": statistics.stdev(s),
            "low_round": low, "low_value": s[low], "outliers": round_outliers(s),
            "mde": mde_from_rounds(s),
        })
    return out


def same_procedure_agreement(rows: list[dict[str, object]]) -> list[dict[str, object]]:
    by = {r["label"]: r for r in rows}
    out = []
    for name, a, b in SAME_PROCEDURE_PAIRS:
        ma = statistics.fmean([x for lab in a for x in by[lab]["samples"]])
        mb = statistics.fmean([x for lab in b for x in by[lab]["samples"]])
        out.append({"pair": name, "run1": ma, "run2": mb, "abs_diff": abs(ma - mb)})
    return out


def counter_budget() -> dict[str, float]:
    ev = {name: (reader_event(COUNTERS_R1, name), reader_event(COUNTERS_R8, name))
          for name in ("cycles", "ref-cycles", "instructions", "task-clock", "LLC-load-misses",
                       "l2_rqsts.rfo_miss", "mem_load_l3_hit_retired.xsnp_hitm")}
    # perf reports `task-clock` in milliseconds, so its per-op value is ms per
    # probe (191.5 ms over 1,048,576 probes is 0.000183 in round 0 at R = 1).
    ns = (ev["task-clock"][0]["mean"] * 1e6, ev["task-clock"][1]["mean"] * 1e6)
    fs = frequency_share(ev["cycles"][0]["mean"], ev["ref-cycles"][0]["mean"],
                         ev["cycles"][1]["mean"], ev["ref-cycles"][1]["mean"], ns[0], ns[1])
    growth = ev["cycles"][1]["mean"] - ev["cycles"][0]["mean"]
    hitm = [event_cycle_ceiling(ev["mem_load_l3_hit_retired.xsnp_hitm"][0]["mean"],
                                ev["mem_load_l3_hit_retired.xsnp_hitm"][1]["mean"], c)
            for c in (EVENT_COST_CYCLES_LO, EVENT_COST_CYCLES_HI)]
    rfo = [event_cycle_ceiling(ev["l2_rqsts.rfo_miss"][0]["mean"], ev["l2_rqsts.rfo_miss"][1]["mean"], c)
           for c in (EVENT_COST_CYCLES_LO, EVENT_COST_CYCLES_HI)]
    llc = event_cycle_ceiling(ev["LLC-load-misses"][0]["mean"], ev["LLC-load-misses"][1]["mean"], EVENT_COST_CYCLES_HI)
    biases = [max_over_mean_bias(t) for t in reader_thread_task_clocks(COUNTERS_R8)]
    return {
        "ns_r1": ns[0], "ns_r8": ns[1],
        "cycles_r1": ev["cycles"][0]["mean"], "cycles_r8": ev["cycles"][1]["mean"],
        "cycles_growth": growth,
        "droop": fs["droop"], "clock_ns": fs["clock_ns"], "growth_ns": fs["growth_ns"], "clock_share": fs["share"],
        "instructions_r1": ev["instructions"][0]["mean"], "instructions_r8": ev["instructions"][1]["mean"],
        "instructions_r8_hi_below_r1_lo": float(ev["instructions"][1]["hi"] < ev["instructions"][0]["lo"]),
        "llc_r1": ev["LLC-load-misses"][0]["mean"], "llc_r8": ev["LLC-load-misses"][1]["mean"],
        "llc_ceiling": llc,
        "hitm_lo": hitm[0], "hitm_hi": hitm[1], "rfo_lo": rfo[0], "rfo_hi": rfo[1],
        "unexplained_floor": unexplained_cycles(growth, [hitm[1], rfo[1], llc]),
        "line_transfer_cycles_r8": line_transfer_cycles(line_transfer_median_ns(),
                                                        ev["cycles"][1]["mean"] / ns[1]),
        "implied_mops_r8": implied_reader_mops(8, ns[1]),
        "bias_min": min(biases), "bias_max": max(biases),
        "hugepage": hugepage_ceiling(hugepage_cycles_per_probe(HUGEPAGE_OFF), hugepage_cycles_per_probe(HUGEPAGE_ON)),
        "hugepage_off": hugepage_cycles_per_probe(HUGEPAGE_OFF), "hugepage_on": hugepage_cycles_per_probe(HUGEPAGE_ON),
    }


def render_table() -> str:
    rows = levels()
    agree = same_procedure_agreement(rows)
    b = counter_budget()
    lines = [
        README_BEGIN,
        "",
        "**Levels.** Expanse reader throughput of the W = 0, R = 8 string cell, M lookups/s, "
        "per artifact half; mean over rounds with its BCa 95% interval.",
        "",
        "| source | engine commit | procedure | rounds | mean [95%] | construction | per-round sd | lowest round | rounds beyond 3 MADs | two-arm MDE (80% power) |",
        "|---|---|---|--:|---|---|--:|---|---|--:|",
    ]
    for r in rows:
        iv = r["interval"]
        out = ", ".join(str(i) for i in r["outliers"]) or "none"
        lines.append(
            f"| `{r['artifact']}` ({r['label']}) | `{r['commit']}` | {r['procedure']} | {iv['n']} | "
            f"{iv['mean']:.3f} [{iv['lo']:.3f}, {iv['hi']:.3f}] | `{iv['method']}` | {r['sd']:.3f} | "
            f"{r['low_round']} ({r['low_value']:.3f}) | {out} | {r['mde']['mde']:.3f} |")
    lines += [
        "",
        "**Between two runs of one procedure on one tree** (difference of the means over all rounds):",
        "",
        "| pair | run 1 mean | run 2 mean | difference |",
        "|---|--:|--:|--:|",
    ]
    for a in agree:
        lines.append(f"| {a['pair']} | {a['run1']:.3f} | {a['run2']:.3f} | {a['abs_diff']:.3f} |")
    lines += [
        "",
        "**Per-thread counters, R = 1 against R = 8** (`counters_masstree_conc_str_w0_r{1,8}.json`, "
        "tree `a1982ff2`, means per probe):",
        "",
        "| quantity | value |",
        "|---|--:|",
        f"| reader cycles per probe, R = 1 / R = 8 | {b['cycles_r1']:.2f} / {b['cycles_r8']:.2f} |",
        f"| cycles growth per probe | {b['cycles_growth']:.2f} |",
        f"| reader `task-clock` ns per probe, R = 1 / R = 8 | {b['ns_r1']:.2f} / {b['ns_r8']:.2f} |",
        f"| fall in `cycles ÷ ref-cycles` (`frequency_share`) | {b['droop'] * 100:.2f}% |",
        f"| ns of the growth the clock accounts for | {b['clock_ns']:.2f} of {b['growth_ns']:.2f} ({b['clock_share'] * 100:.1f}%) |",
        f"| instructions per probe, R = 1 / R = 8 | {b['instructions_r1']:.2f} / {b['instructions_r8']:.2f} |",
        f"| R = 8 instructions interval entirely below R = 1's | {'yes' if b['instructions_r8_hi_below_r1_lo'] else 'no'} |",
        f"| `LLC-load-misses` per probe, R = 1 / R = 8 | {b['llc_r1']:.3f} / {b['llc_r8']:.3f} |",
        f"| `xsnp_hitm` growth at {EVENT_COST_CYCLES_LO:.0f}–{EVENT_COST_CYCLES_HI:.0f} cycles per event (hypothesis) | {b['hitm_lo']:.2f}–{b['hitm_hi']:.2f} cycles |",
        f"| `l2_rqsts.rfo_miss` growth at {EVENT_COST_CYCLES_LO:.0f}–{EVENT_COST_CYCLES_HI:.0f} cycles per event (hypothesis) | {b['rfo_lo']:.3f}–{b['rfo_hi']:.3f} cycles |",
        f"| growth no listed counter covers at the upper cost (`unexplained_cycles`) | ≥ {b['unexplained_floor']:.2f} cycles |",
        f"| one measured line transfer at the R = 8 clock (`line_transfer_cycles`) | {b['line_transfer_cycles_r8']:.1f} cycles |",
        f"| slowest over mean reader `task-clock`, R = 8, across rounds (`max_over_mean_bias`) | {b['bias_min'] * 100:.2f}%–{b['bias_max'] * 100:.2f}% |",
        f"| aggregate rate the R = 8 `task-clock` implies (`implied_reader_mops`) | {b['implied_mops_r8']:.2f} M/s |",
        f"| single-threaded `strmap_get` cycles per probe, 4 KiB / huge pages (`hugepage_ceiling`, `b1868813`) | {b['hugepage_off']:.2f} / {b['hugepage_on']:.2f} ({b['hugepage'] * 100:+.1f}%) |",
        "",
        README_END,
    ]
    return "\n".join(lines)


def readme_block(text: str) -> str:
    start = text.index(README_BEGIN)
    end = text.index(README_END) + len(README_END)
    return text[start:end]


def report() -> None:
    print(render_table())


# ---------------------------------------------------------------------------
# Unit tests: synthetic hand-checkable values, then the committed artifacts
# ---------------------------------------------------------------------------

class SyntheticTests(unittest.TestCase):
    def test_frequency_share_hand_values(self):
        # R = 1: 1000 cycles in 200 ns (5 cycles/ns). R = k: 1100 cycles in 250 ns.
        # Clock held: 1100 / 5 = 220 ns predicted; 30 of the 50 ns growth is clock.
        fs = frequency_share(1000.0, 500.0, 1100.0, 600.0, 200.0, 250.0)
        self.assertAlmostEqual(fs["predicted_ns"], 220.0)
        self.assertAlmostEqual(fs["clock_ns"], 30.0)
        self.assertAlmostEqual(fs["share"], 0.6)
        self.assertAlmostEqual(fs["droop"], 1.0 - (1100 / 600) / 2.0)

    def test_event_ceiling_and_unexplained(self):
        self.assertAlmostEqual(event_cycle_ceiling(0.01, 0.11, 100.0), 10.0)
        self.assertEqual(event_cycle_ceiling(0.5, 0.4, 100.0), 0.0)
        self.assertAlmostEqual(unexplained_cycles(136.0, [31.0, 0.3]), 104.7)
        self.assertEqual(unexplained_cycles(10.0, [20.0]), 0.0)

    def test_mde_hand_value(self):
        # sd of [1, 2, 3] is 1; n = 3; (1.96 + 0.8416) * sqrt(2/3) = 2.2875.
        m = mde_from_rounds([1.0, 2.0, 3.0])
        self.assertAlmostEqual(m["sigma"], 1.0)
        self.assertAlmostEqual(m["mde"], (Z_ALPHA_2 + Z_BETA_80) * math.sqrt(2 / 3), places=12)
        self.assertAlmostEqual(m["mde"], 2.2875, places=4)

    def test_outliers(self):
        s = [10.0, 10.1, 9.9, 10.0, 10.2, 7.0, 10.05]
        self.assertEqual(round_outliers(s), [5])
        self.assertEqual(round_outliers([1.0, 1.0, 1.0]), [])

    def test_bias_hugepage_implied(self):
        self.assertAlmostEqual(max_over_mean_bias([1.0, 1.0, 1.0, 1.4]), 1.4 / 1.1 - 1)
        self.assertAlmostEqual(hugepage_ceiling(100.0, 89.0), -0.11)
        self.assertAlmostEqual(implied_reader_mops(8, 250.0), 32.0)
        self.assertAlmostEqual(line_transfer_cycles(30.0, 4.5), 135.0)

    def test_per_arm_interval_encloses_mean(self):
        iv = per_arm_interval([1.0, 2.0, 3.0, 4.0, 5.0])
        self.assertAlmostEqual(iv["mean"], 3.0)
        self.assertLessEqual(iv["lo"], iv["mean"])
        self.assertGreaterEqual(iv["hi"], iv["mean"])

    def test_invalid_arguments_raise(self):
        for fn in (lambda: frequency_share(0, 1, 1, 1, 1, 2),
                   lambda: frequency_share(1, 1, 1, 1, 2, 1),
                   lambda: event_cycle_ceiling(0.1, 0.2, -1),
                   lambda: unexplained_cycles(1.0, [-1.0]),
                   lambda: mde_from_rounds([1.0]),
                   lambda: round_outliers([1.0, 2.0]),
                   lambda: round_outliers([1.0, 2.0, 3.0], k=0),
                   lambda: max_over_mean_bias([]),
                   lambda: hugepage_ceiling(0, 1),
                   lambda: implied_reader_mops(0, 1.0),
                   lambda: line_transfer_cycles(0, 1.0),
                   lambda: per_arm_interval([1.0, 2.0])):
            with self.assertRaises(ValueError):
                fn()


class ArtifactTests(unittest.TestCase):
    """Reference values from the committed artifacts: a changed artifact or a
    changed estimator fails here before any README number moves."""

    def test_counter_budget_reference_values(self):
        b = counter_budget()
        self.assertAlmostEqual(b["ns_r1"], 186.19, places=2)
        self.assertAlmostEqual(b["ns_r8"], 229.18, places=2)
        self.assertAlmostEqual(b["cycles_growth"], 136.15, places=2)
        self.assertAlmostEqual(b["droop"], 0.0670, places=4)
        self.assertAlmostEqual(b["clock_ns"], 15.35, places=2)
        self.assertAlmostEqual(b["clock_share"], 0.357, places=3)
        self.assertAlmostEqual(b["hitm_lo"], 7.80, places=2)
        self.assertAlmostEqual(b["hitm_hi"], 31.18, places=2)
        self.assertAlmostEqual(b["rfo_lo"], 0.0812, places=4)
        self.assertEqual(b["instructions_r8_hi_below_r1_lo"], 1.0)
        self.assertLess(b["llc_r8"], b["llc_r1"])
        self.assertEqual(b["llc_ceiling"], 0.0)
        self.assertGreater(b["unexplained_floor"], 100.0)
        self.assertAlmostEqual(b["implied_mops_r8"], 34.91, places=2)
        self.assertAlmostEqual(b["hugepage"], -0.1124, places=4)
        self.assertAlmostEqual(b["hugepage_off"], 670.08, places=2)
        self.assertAlmostEqual(b["bias_min"], 0.0052, places=4)
        self.assertAlmostEqual(b["bias_max"], 0.0084, places=4)

    def test_levels_reference_values(self):
        rows = {r["label"]: r for r in levels()}
        self.assertEqual(len(rows), len(LEVEL_SOURCES))
        self.assertAlmostEqual(rows["a1982ff2 run 1"]["interval"]["mean"], 34.888, places=3)
        self.assertAlmostEqual(rows["a1982ff2 run 2"]["interval"]["mean"], 34.895, places=3)
        self.assertEqual(rows["a1982ff2 run 1"]["low_round"], 1)
        self.assertEqual(rows["a1982ff2 run 2"]["low_round"], 1)
        self.assertIn(1, rows["a1982ff2 run 1"]["outliers"])
        self.assertIn(1, rows["a1982ff2 run 2"]["outliers"])
        self.assertEqual(rows["ab run 1, base 55b511df"]["commit"], "55b511df")
        self.assertEqual(rows["ab run 1, head 38fb2b1e"]["commit"], "38fb2b1e")
        for r in rows.values():
            self.assertEqual(r["interval"]["n"], 15)
        agree = {a["pair"]: a for a in same_procedure_agreement(list(rows.values()))}
        self.assertAlmostEqual(agree["a1982ff2, one process"]["abs_diff"], 0.007, places=3)
        self.assertAlmostEqual(agree["ab, both halves pooled"]["run1"], 28.651, places=3)
        self.assertAlmostEqual(agree["ab, both halves pooled"]["run2"], 28.713, places=3)
        self.assertAlmostEqual(agree["ab, both halves pooled"]["abs_diff"], 0.062, places=3)

    def test_cell_rounds_refuses_gaps(self):
        import tempfile
        data = json.loads((MT / "step0" / "baseline_concurrent.json").read_text())
        for c in data["throughput"]:
            if all(c.get(k) == v for k, v in CELL.items()):
                del c["rounds_raw"][3]
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "gap.json"
            p.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                cell_rounds(p)

    def test_readme_block_is_the_module_output(self):
        # The README §13 tables are this module's output, byte for byte
        # (AGENTS.md §8.2): an edited cell or a changed estimator fails here.
        self.assertEqual(readme_block(README.read_text()), render_table())


if __name__ == "__main__":
    if "--table" in sys.argv:
        print(render_table())
        sys.exit(0)
    if "--write-readme" in sys.argv:
        text = README.read_text()
        README.write_text(text.replace(readme_block(text), render_table()))
        sys.exit(0)
    if "--self-test" in sys.argv:
        sys.argv = [sys.argv[0]]
        unittest.main()
    report()
    unittest.main(argv=[sys.argv[0]])
