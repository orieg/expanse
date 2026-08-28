#!/usr/bin/env python3
"""
scripts/bench_baseline.py — Criterion sample harvest, BCa confidence intervals,
committed baseline artifacts, and CI-lower-bound gating.

This is the consumer `scripts/bca_bootstrap.py` was missing. It closes the path
AGENTS.md §8.4 / §8.7 require for a wall-clock claim:

    criterion sample.json  ->  per-iteration samples
                           ->  BCa 95% CI (>= 1000 resamples)
                           ->  committed results/baseline_*.json
                           ->  CI rendered in the PR report
                           ->  gate on the CI's conservative bound, not the point

Criterion 0.8 layout (verified against criterion-0.8.2 `src/analysis/mod.rs`):

    target/criterion/<directory_name>/{new,base,<saved-baseline>}/
        benchmark.json   {group_id, function_id, value_str, throughput, full_id,
                          directory_name, title}
        sample.json      {sampling_mode: "Linear"|"Flat",
                          iters: [f64; n], times: [f64 ns; n]}
        estimates.json   {mean,median,median_abs_dev,slope,std_dev} each
                          {confidence_interval:{confidence_level,lower_bound,
                          upper_bound}, point_estimate, standard_error}
        tukey.json       [f64; 4] outlier fences

`sample.json` is the raw data: `times[i]` is the wall time for `iters[i]`
iterations of the routine. Criterion itself analyses `avg_times[i] =
times[i] / iters[i]` (`analysis/mod.rs`), so that same quotient is the honest
per-iteration sample, and `mean(avg_times)` reproduces criterion's own
`estimates.json -> mean.point_estimate` bit for bit. The harvest asserts that
equality per arm and fails loudly when it does not hold: a mismatch means the
on-disk layout changed underneath us, and silently publishing a CI over
misparsed numbers is exactly the §8.1 failure this repo forbids.

Nothing about the benches had to change to expose samples — criterion has been
writing `sample.json` all along; nothing read it.

Usage:
  # harvest a completed `cargo bench` run into a baseline artifact
  python3 scripts/bench_baseline.py --harvest \
      --criterion-dir target/criterion \
      --out results/baseline_comparative.json \
      --host-desc "AMD Ryzen 9 7950X, 16C/32T, 64 MiB L3, Linux 6.8" \
      --run-id "https://github.com/orieg/expanse/actions/runs/<id>"

  # render the interval table for a PR comment
  python3 scripts/bench_baseline.py --input results/baseline_comparative.json

  # gate declared claims on the CI's conservative bound
  python3 scripts/bench_baseline.py --input results/baseline_comparative.json \
      --floors docs/benchmarks/floors/issue-430.json --fail-on-gate

  # compare a head run against the committed baseline (speedup CI vs a floor)
  python3 scripts/bench_baseline.py --input results/quick/head.json \
      --against results/baseline_comparative.json --floor-speedup 1.05

  python3 scripts/bench_baseline.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import socket
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))

from bca_bootstrap import bca_bootstrap_ci, bca_bootstrap_ratio_ci  # noqa: E402

SCHEMA = "expanse.baseline.v1"

# AGENTS.md §8.4: "BCa 95% bootstrap CI lower bound >= floor (>= 1,000
# resamples)". The resample count is a property of the estimator, not of the
# data, so it is enforced here rather than left to a default.
MIN_RESAMPLES = 1000

# BCa needs n >= 3 to form a jackknife (bca_bootstrap.py raises below that). An
# interval over 3 samples is not a publishable claim, so arms below --min-n are
# labelled INSUFFICIENT_SAMPLES and reported, never silently dropped and never
# silently gated. Criterion's own default sample_size is 100.
DEFAULT_MIN_N = 10

# Gate verdicts. The two overlap labels are fixed by AGENTS.md §8.4 and
# docs/BENCHMARKING.md rule 12 -- do not add synonyms.
PASS = "PASS"
FAIL = "FAIL"
BOUNDARY_RESULT = "BOUNDARY_RESULT"
INTERMEDIATE = "INTERMEDIATE_floor_within_ci"
INSUFFICIENT_SAMPLES = "INSUFFICIENT_SAMPLES"

PASSING_VERDICTS = frozenset({PASS})

LOWER_IS_BETTER = "lower_is_better"
HIGHER_IS_BETTER = "higher_is_better"


class HarvestError(RuntimeError):
    """Raised when criterion output cannot be parsed as its documented shape."""


# --------------------------------------------------------------------------
# Harvest
# --------------------------------------------------------------------------


def _load_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError) as exc:
        raise HarvestError(f"cannot read {path}: {exc}") from exc


def per_iteration_samples(sample_json: Dict[str, Any], path: Path) -> List[float]:
    """Converts criterion's (iters, times) pairs into per-iteration nanoseconds.

    This is the same quotient criterion analyses internally, so the mean of the
    result equals criterion's own `mean.point_estimate`.
    """
    iters = sample_json.get("iters")
    times = sample_json.get("times")
    if not isinstance(iters, list) or not isinstance(times, list):
        raise HarvestError(f"{path}: expected list-valued 'iters' and 'times'")
    if len(iters) != len(times):
        raise HarvestError(
            f"{path}: 'iters' ({len(iters)}) and 'times' ({len(times)}) length mismatch"
        )
    if not iters:
        raise HarvestError(f"{path}: empty sample")
    out: List[float] = []
    for i, (it, tm) in enumerate(zip(iters, times)):
        if it <= 0:
            raise HarvestError(f"{path}: non-positive iteration count at index {i}: {it}")
        out.append(float(tm) / float(it))
    return out


def _cross_check_against_criterion(
    samples: Sequence[float], estimates: Optional[Dict[str, Any]], arm_id: str
) -> Optional[Dict[str, Any]]:
    """Asserts the derived mean reproduces criterion's own point estimate.

    Returns criterion's own mean estimate block for the record, or None when
    `estimates.json` is absent. Criterion's interval is a plain bootstrap over
    the same samples; it is stored for cross-reference and is NOT the §8.4
    interval -- that one is the BCa computed here.
    """
    if not estimates:
        return None
    mean_block = estimates.get("mean")
    if not isinstance(mean_block, dict):
        return None
    criterion_mean = mean_block.get("point_estimate")
    if criterion_mean is None:
        return None
    derived = sum(samples) / len(samples)
    # Same float ops in the same order, so this is an exact-equality check with
    # only accumulated-rounding slack.
    if abs(derived - float(criterion_mean)) > max(1e-9, abs(float(criterion_mean)) * 1e-12):
        raise HarvestError(
            f"{arm_id}: derived mean {derived!r} does not reproduce criterion's "
            f"mean.point_estimate {criterion_mean!r}. The on-disk criterion layout "
            f"has changed; refusing to publish a CI over misparsed samples."
        )
    interval = mean_block.get("confidence_interval") or {}
    return {
        "point_estimate_ns": float(criterion_mean),
        "lower_bound_ns": interval.get("lower_bound"),
        "upper_bound_ns": interval.get("upper_bound"),
        "confidence_level": interval.get("confidence_level"),
        "note": "criterion's own bootstrap interval, kept for cross-reference only",
    }


def harvest_criterion(
    criterion_dir: Path,
    baseline_dir: str = "new",
    include: Optional[str] = None,
    newer_than: Optional[float] = None,
    allow_empty: bool = False,
) -> List[Dict[str, Any]]:
    """Walks `<criterion_dir>/**/<baseline_dir>/sample.json` and returns raw arms.

    `baseline_dir` is "new" for a plain `cargo bench` run, "base" (or the name
    passed to `--save-baseline`) for a saved one; criterion copies `new/` into
    the saved directory verbatim, so both carry the same four files.

    `newer_than` is a POSIX timestamp taken before the run started. A
    self-hosted runner keeps `target/` between jobs, so `target/criterion` can
    hold arms from an earlier suite at an earlier commit; harvesting those into
    an artifact stamped with this run's commit would attach the wrong
    provenance to real numbers. Arms whose `sample.json` predates the stamp are
    excluded and named in the skip list.
    """
    if not criterion_dir.is_dir():
        raise HarvestError(
            f"{criterion_dir} does not exist. Run the criterion suite first "
            f"(e.g. `cargo bench --bench comparative -p expanse-trie`)."
        )
    arms: List[Dict[str, Any]] = []
    stale: List[str] = []
    for sample_path in sorted(criterion_dir.glob(f"**/{baseline_dir}/sample.json")):
        run_dir = sample_path.parent
        if newer_than is not None and sample_path.stat().st_mtime < newer_than:
            stale.append(str(run_dir.relative_to(criterion_dir)))
            continue
        bench_path = run_dir / "benchmark.json"
        if not bench_path.is_file():
            raise HarvestError(
                f"{run_dir}: sample.json without benchmark.json. Criterion writes both; "
                f"a lone sample.json means the directory was hand-edited or truncated."
            )
        meta = _load_json(bench_path)
        sample = _load_json(sample_path)
        arm_id = meta.get("full_id") or meta.get("title")
        if not arm_id:
            raise HarvestError(f"{bench_path}: no 'full_id' -- unrecognised criterion layout")
        if include and include not in arm_id:
            continue
        samples = per_iteration_samples(sample, sample_path)
        estimates_path = run_dir / "estimates.json"
        estimates = _load_json(estimates_path) if estimates_path.is_file() else None
        arms.append(
            {
                "id": arm_id,
                "group_id": meta.get("group_id"),
                "function_id": meta.get("function_id"),
                "value_str": meta.get("value_str"),
                "throughput": meta.get("throughput"),
                "directory_name": meta.get("directory_name"),
                "sampling_mode": sample.get("sampling_mode"),
                "samples_ns": samples,
                "criterion_mean_estimate": _cross_check_against_criterion(
                    samples, estimates, arm_id
                ),
            }
        )
    if stale:
        print(
            f"note: skipped {len(stale)} criterion arm(s) predating this run "
            f"(left over in {criterion_dir} from an earlier job): "
            + ", ".join(sorted(stale)[:8])
            + (" …" if len(stale) > 8 else ""),
            file=sys.stderr,
        )
    if not arms:
        message = (
            f"no criterion arms found under {criterion_dir} with baseline directory "
            f"'{baseline_dir}'"
            + (f" matching {include!r}" if include else "")
            + (f"; {len(stale)} arm(s) were excluded as predating this run" if stale else "")
        )
        if allow_empty:
            # Explicit, not silent: the caller asked to tolerate a suite with no
            # criterion arms (a Callgrind-only suite on a runner whose target/
            # still holds another suite's output). No artifact is written.
            print(f"note: {message}; no wall-clock interval is claimed.", file=sys.stderr)
            return []
        raise HarvestError(message)
    return arms


# --------------------------------------------------------------------------
# Statistics
# --------------------------------------------------------------------------


def arm_interval(
    arm: Dict[str, Any],
    confidence: float,
    num_resamples: int,
    seed: int,
    min_n: int,
) -> Dict[str, Any]:
    """Adds the BCa interval to one harvested arm.

    The point estimate is the mean of the per-iteration samples and the interval
    is the BCa interval of that same mean -- one definition, per §8.4 -- so the
    point estimate is enclosed by its interval. Enclosure is asserted rather
    than assumed.
    """
    if num_resamples < MIN_RESAMPLES:
        raise ValueError(
            f"num_resamples={num_resamples} is below the §8.4 floor of {MIN_RESAMPLES}"
        )
    samples = arm["samples_ns"]
    n = len(samples)
    out = dict(arm)
    out["n"] = n
    out["unit"] = "ns_per_iteration"
    if n < max(3, min_n):
        out.update(
            {
                "point_ns": sum(samples) / n,
                "ci_lower_ns": None,
                "ci_upper_ns": None,
                "status": INSUFFICIENT_SAMPLES,
                "status_detail": (
                    f"n={n} is below the minimum of {max(3, min_n)} samples required for "
                    f"a published interval; raise criterion's sample_size for this arm"
                ),
            }
        )
        return out
    point, lo, hi = bca_bootstrap_ci(
        samples, confidence=confidence, num_resamples=num_resamples, seed=seed
    )
    if not (lo <= point <= hi):
        raise HarvestError(
            f"{arm['id']}: point estimate {point!r} is not enclosed by its BCa interval "
            f"[{lo!r}, {hi!r}]. Point estimate and interval must share one definition "
            f"(§8.4); refusing to publish."
        )
    out.update(
        {
            "point_ns": point,
            "ci_lower_ns": lo,
            "ci_upper_ns": hi,
            "ci_width_ns": hi - lo,
            "ci_rel_width_pct": ((hi - lo) / point * 100.0) if point else None,
            "status": "OK",
        }
    )
    return out


# --------------------------------------------------------------------------
# Gating
# --------------------------------------------------------------------------


def gate_absolute(
    arm: Dict[str, Any], threshold: float, direction: str
) -> Tuple[str, str]:
    """Gates one arm's interval against an absolute threshold.

    §8.4 states the rule for a higher-is-better metric: pass iff the CI *lower*
    bound clears the floor. For a lower-is-better metric (latency in ns/iter)
    the same rule applied to the conservative end of the interval is: pass iff
    the CI *upper* bound clears the ceiling. Both gate on the interval's
    unfavourable end, never on the point estimate. This is the mirror of §8.4,
    not a second rule.
    """
    if direction not in (LOWER_IS_BETTER, HIGHER_IS_BETTER):
        raise ValueError(f"unknown direction {direction!r}")
    if arm.get("status") == INSUFFICIENT_SAMPLES:
        return INSUFFICIENT_SAMPLES, arm.get("status_detail", "insufficient samples")
    lo = arm["ci_lower_ns"]
    hi = arm["ci_upper_ns"]
    point = arm["point_ns"]
    span = f"95% CI [{lo:.2f}, {hi:.2f}] ns/iter, n={arm['n']}"

    if direction == HIGHER_IS_BETTER:
        if lo >= threshold:
            return PASS, f"CI lower bound {lo:.2f} >= floor {threshold:.2f} ({span})"
        point_passes = point >= threshold
        floor_inside = lo <= threshold <= hi
    else:
        if hi <= threshold:
            return PASS, f"CI upper bound {hi:.2f} <= ceiling {threshold:.2f} ({span})"
        point_passes = point <= threshold
        floor_inside = lo <= threshold <= hi

    if floor_inside and point_passes:
        return (
            INTERMEDIATE,
            f"point estimate {point:.2f} clears {threshold:.2f} but the interval spans it ({span})",
        )
    if floor_inside:
        return (
            BOUNDARY_RESULT,
            f"point estimate {point:.2f} does not clear {threshold:.2f} and the interval spans it ({span})",
        )
    return (
        FAIL,
        f"the whole interval is on the failing side of {threshold:.2f} ({span})",
    )


def gate_speedup(
    head: Dict[str, Any],
    base: Dict[str, Any],
    floor_speedup: float,
    direction: str = LOWER_IS_BETTER,
    confidence: float = 0.95,
    num_resamples: int = 2000,
    seed: int = 42,
) -> Dict[str, Any]:
    """Gates a head-vs-baseline speedup on the CI lower bound of the ratio.

    Speedup is defined so higher is always better: for a latency metric it is
    base/head, for a throughput metric head/base. §8.4 then applies verbatim --
    pass iff the speedup interval's lower bound clears the floor.
    """
    for arm in (head, base):
        if arm.get("status") == INSUFFICIENT_SAMPLES:
            return {
                "verdict": INSUFFICIENT_SAMPLES,
                "rationale": arm.get("status_detail", "insufficient samples"),
            }
    if direction == LOWER_IS_BETTER:
        numerator, denominator = base["samples_ns"], head["samples_ns"]
    else:
        numerator, denominator = head["samples_ns"], base["samples_ns"]
    point, lo, hi = bca_bootstrap_ratio_ci(
        numerator,
        denominator,
        confidence=confidence,
        num_resamples=num_resamples,
        seed=seed,
    )
    span = (
        f"speedup {point:.4f}x, 95% CI [{lo:.4f}x, {hi:.4f}x], "
        f"n_head={head['n']}, n_base={base['n']}"
    )
    if lo >= floor_speedup:
        verdict, rationale = PASS, f"CI lower bound clears the {floor_speedup:.4f}x floor ({span})"
    elif lo <= floor_speedup <= hi and point >= floor_speedup:
        verdict, rationale = INTERMEDIATE, f"point clears the floor, interval spans it ({span})"
    elif lo <= floor_speedup <= hi:
        verdict, rationale = BOUNDARY_RESULT, f"point below the floor, interval spans it ({span})"
    else:
        verdict, rationale = FAIL, f"the whole interval is below the floor ({span})"
    return {
        "verdict": verdict,
        "rationale": rationale,
        "speedup": point,
        "ci_lower": lo,
        "ci_upper": hi,
        "floor": floor_speedup,
        "direction": direction,
    }


def load_floors(path: Path) -> List[Dict[str, Any]]:
    """Reads a floors declaration.

    Shape:
      {"floors": [{"arm": "<criterion full_id>",
                   "direction": "lower_is_better" | "higher_is_better",
                   "threshold_ns": 1234.0,
                   "claim": "prose the gate is standing behind"}]}
    """
    doc = _load_json(path)
    floors = doc.get("floors")
    if not isinstance(floors, list) or not floors:
        raise HarvestError(f"{path}: expected a non-empty 'floors' array")
    for entry in floors:
        for key in ("arm", "direction", "threshold_ns"):
            if key not in entry:
                raise HarvestError(f"{path}: floor entry missing {key!r}: {entry}")
        if entry["direction"] not in (LOWER_IS_BETTER, HIGHER_IS_BETTER):
            raise HarvestError(f"{path}: unknown direction {entry['direction']!r}")
    return floors


def apply_floors(
    artifact: Dict[str, Any], floors: Sequence[Dict[str, Any]]
) -> List[Dict[str, Any]]:
    """Evaluates every declared floor against the artifact's arms."""
    by_id = {arm["id"]: arm for arm in artifact["arms"]}
    results: List[Dict[str, Any]] = []
    for entry in floors:
        arm = by_id.get(entry["arm"])
        if arm is None:
            # A floor naming an arm the run did not produce is a broken gate,
            # not a pass. Surface it rather than skipping it.
            results.append(
                {
                    "arm": entry["arm"],
                    "verdict": FAIL,
                    "rationale": "declared floor names an arm absent from this run",
                    "claim": entry.get("claim", ""),
                    "threshold_ns": entry["threshold_ns"],
                    "direction": entry["direction"],
                }
            )
            continue
        verdict, rationale = gate_absolute(
            arm, float(entry["threshold_ns"]), entry["direction"]
        )
        results.append(
            {
                "arm": entry["arm"],
                "verdict": verdict,
                "rationale": rationale,
                "claim": entry.get("claim", ""),
                "threshold_ns": float(entry["threshold_ns"]),
                "direction": entry["direction"],
            }
        )
    return results


# --------------------------------------------------------------------------
# Artifact
# --------------------------------------------------------------------------


def _git_commit(root: Path) -> Optional[str]:
    try:
        proc = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return proc.stdout.strip() or None


def _load_average() -> Optional[List[float]]:
    try:
        return [round(v, 2) for v in os.getloadavg()]
    except (OSError, AttributeError):
        return None


def validate_host_description(host_desc: str) -> str:
    """Rejects host descriptions that would leak private infrastructure identity.

    AGENTS.md §7 Privacy: benchmark results reference the host by anonymised
    hardware description -- CPU model, cores, cache, OS -- never a personal
    hostname or a home path.
    """
    lowered = host_desc.lower()
    hostname = socket.gethostname().split(".")[0].lower()
    if hostname and len(hostname) > 2 and hostname in lowered:
        raise ValueError(
            "--host-desc contains this machine's hostname. Describe the hardware "
            "(CPU model, cores/threads, cache, OS), not the host identity (§7 Privacy)."
        )
    for marker in ("/users/", "/home/", "192.168.", "10.0.", "@"):
        if marker in lowered:
            raise ValueError(
                f"--host-desc contains {marker!r}, which reads as a private "
                f"infrastructure identifier (§7 Privacy)."
            )
    return host_desc


def build_artifact(
    arms: List[Dict[str, Any]],
    *,
    suite: str,
    host_desc: str,
    commit: Optional[str],
    run_id: str,
    confidence: float,
    num_resamples: int,
    seed: int,
    min_n: int,
    fixture: bool,
    store_samples: bool = True,
) -> Dict[str, Any]:
    """Assembles the committed baseline artifact."""
    priced = [
        arm_interval(arm, confidence, num_resamples, seed, min_n) for arm in arms
    ]
    if not store_samples:
        for arm in priced:
            arm.pop("samples_ns", None)
    return {
        "schema": SCHEMA,
        "kind": "wall_clock_bca",
        "suite": suite,
        "fixture": fixture,
        "provenance": {
            # Anonymised hardware description, never a hostname (§7 Privacy).
            "host_description": host_desc,
            "commit": commit,
            "run_id": run_id,
            "os": platform.system().lower(),
            "arch": platform.machine(),
            "load_average_at_harvest": _load_average(),
            "generated_by": "scripts/bench_baseline.py",
            "source": "criterion 0.8 sample.json (times[i] / iters[i])",
        },
        "statistics": {
            "estimator": "mean of per-iteration samples (ns/iter)",
            "method": "BCa bootstrap",
            "confidence": confidence,
            "num_resamples": num_resamples,
            "seed": seed,
            "min_n": min_n,
            "point_and_interval_share_one_definition": True,
        },
        "arms": priced,
    }


def check_output_path(out: Path, fixture: bool) -> None:
    """Refuses to write fixture numbers to a canonical baseline path.

    §8.5 keeps scratch out of the committed baselines; the same guard keeps
    synthetic fixtures out of them.
    """
    name = out.name
    if fixture and name.startswith("baseline_") and "fixture" not in name:
        raise ValueError(
            f"refusing to write fixture data to {out}: a results/baseline_*.json is a "
            f"published measurement. Name it baseline_fixture_*.json or write it to a "
            f"gitignored scratch path (results/quick/)."
        )


# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------


def _fmt(value: Optional[float], digits: int = 2) -> str:
    return "—" if value is None else f"{value:,.{digits}f}"


def render_markdown(
    artifact: Dict[str, Any], gates: Optional[List[Dict[str, Any]]] = None
) -> str:
    """Renders the interval table. Every cell is derived from the artifact (§8.2)."""
    prov = artifact.get("provenance", {})
    stats = artifact.get("statistics", {})
    arms = artifact.get("arms", [])
    conf_pct = float(stats.get("confidence", 0.95)) * 100.0

    lines: List[str] = [
        "### 📐 Wall-Clock BCa Confidence Intervals",
        "",
    ]
    if artifact.get("fixture"):
        lines.extend(
            [
                "> ⚠️ **FIXTURE ARTIFACT — the figures below are synthetic, not measured.** "
                "It exercises the harvest → BCa → artifact → gate path and must never be "
                "cited as a result.",
                "",
            ]
        )
    lines.extend(
        [
            f"> **Estimator**: {stats.get('estimator', 'unknown')} · "
            f"**Method**: {stats.get('method', 'unknown')}, "
            f"{stats.get('num_resamples', '?')} resamples, seed {stats.get('seed', '?')}",
            "> The point estimate and the interval are the same statistic, so the point is "
            "enclosed by its interval (AGENTS.md §8.4).",
            f"> **Provenance**: (measured: `{prov.get('host_description', 'unknown')}`, "
            f"`{(prov.get('commit') or 'unknown')[:12]}`) · run `{prov.get('run_id', 'unknown')}` "
            f"· source `{prov.get('source', 'unknown')}`",
            "",
            f"| Arm | n | Mean (ns/iter) | {conf_pct:.0f}% BCa CI (ns/iter) | CI width (% of mean) | Status |",
            "|---|---:|---:|---:|---:|---|",
        ]
    )
    for arm in arms:
        # Full width, not a ± half-width: a BCa interval is asymmetric about the
        # point estimate by construction, and printing ±x% would assert a
        # symmetry the estimator does not have.
        width = (
            f"{arm['ci_rel_width_pct']:.2f}%"
            if arm.get("ci_rel_width_pct") is not None
            else "—"
        )
        interval = (
            f"[{_fmt(arm.get('ci_lower_ns'))}, {_fmt(arm.get('ci_upper_ns'))}]"
            if arm.get("ci_lower_ns") is not None
            else "—"
        )
        lines.append(
            f"| `{arm['id']}` | {arm.get('n', 0)} | {_fmt(arm.get('point_ns'))} | "
            f"{interval} | {width} | {arm.get('status', '?')} |"
        )
    lines.append("")

    if gates:
        lines.extend(
            [
                f"#### Declared gates ({conf_pct:.0f}% CI conservative bound vs floor)",
                "",
                "| Arm | Claim | Threshold (ns/iter) | Direction | Verdict | Why |",
                "|---|---|---:|---|---|---|",
            ]
        )
        for gate in gates:
            lines.append(
                f"| `{gate['arm']}` | {gate.get('claim', '') or '—'} | "
                f"{_fmt(gate.get('threshold_ns'))} | `{gate.get('direction', '')}` | "
                f"**{gate['verdict']}** | {gate['rationale']} |"
            )
        lines.append("")
        lines.append(
            "<sub>A claim passes only when the interval's unfavourable bound clears the "
            "threshold, never on the point estimate alone (AGENTS.md §8.4).</sub>"
        )
        lines.append("")

    if not arms:
        lines.append("<sub>No arms harvested; no interval is claimed.</sub>")
        lines.append("")
    return "\n".join(lines)


def render_comparison_markdown(
    head: Dict[str, Any],
    base: Dict[str, Any],
    floor_speedup: float,
    direction: str,
    confidence: float,
    num_resamples: int,
    seed: int,
) -> Tuple[str, List[str]]:
    """Renders head-vs-baseline speedup intervals; returns (markdown, verdicts)."""
    base_by_id = {arm["id"]: arm for arm in base.get("arms", [])}
    conf_pct = confidence * 100.0
    lines = [
        "### 📐 Wall-Clock Speedup vs Committed Baseline",
        "",
        f"> Baseline artifact: `{base.get('suite', 'unknown')}` "
        f"(measured: `{base.get('provenance', {}).get('host_description', 'unknown')}`, "
        f"`{(base.get('provenance', {}).get('commit') or 'unknown')[:12]}`)",
        f"> Speedup is defined so higher is better ({direction}); "
        f"the gate is the {conf_pct:.0f}% BCa CI lower bound vs a {floor_speedup:.4f}x floor.",
        "",
        f"| Arm | Head mean (ns/iter) | Base mean (ns/iter) | Speedup | {conf_pct:.0f}% BCa CI | Verdict |",
        "|---|---:|---:|---:|---:|---|",
    ]
    verdicts: List[str] = []
    for arm in head.get("arms", []):
        counterpart = base_by_id.get(arm["id"])
        if counterpart is None:
            lines.append(
                f"| `{arm['id']}` | {_fmt(arm.get('point_ns'))} | — | — | — | "
                f"**{FAIL}** (absent from baseline) |"
            )
            verdicts.append(FAIL)
            continue
        result = gate_speedup(
            arm,
            counterpart,
            floor_speedup,
            direction=direction,
            confidence=confidence,
            num_resamples=num_resamples,
            seed=seed,
        )
        verdicts.append(result["verdict"])
        if "speedup" in result:
            lines.append(
                f"| `{arm['id']}` | {_fmt(arm.get('point_ns'))} | "
                f"{_fmt(counterpart.get('point_ns'))} | {result['speedup']:.4f}x | "
                f"[{result['ci_lower']:.4f}x, {result['ci_upper']:.4f}x] | "
                f"**{result['verdict']}** |"
            )
        else:
            lines.append(
                f"| `{arm['id']}` | {_fmt(arm.get('point_ns'))} | "
                f"{_fmt(counterpart.get('point_ns'))} | — | — | **{result['verdict']}** |"
            )
    lines.append("")
    return "\n".join(lines), verdicts


# --------------------------------------------------------------------------
# Fixture (synthetic; never a measurement)
# --------------------------------------------------------------------------


def write_fixture_criterion_tree(root: Path, n: int = 40) -> Path:
    """Writes a synthetic criterion 0.8 tree for exercising the harvest path.

    The numbers are generated from a fixed PRNG here in this function. They are
    NOT measurements of anything and are labelled as fixtures everywhere they
    surface.
    """
    import random

    rng = random.Random(20260827)
    root.mkdir(parents=True, exist_ok=True)
    specs = [
        ("fixture_group/fast", "fixture_group", "fast", 100.0, 4.0),
        ("fixture_group/slow", "fixture_group", "slow", 130.0, 5.0),
        ("fixture_group/tiny_n", "fixture_group", "tiny_n", 100.0, 4.0),
    ]
    for full_id, group_id, function_id, centre, spread in specs:
        count = 4 if function_id == "tiny_n" else n
        directory_name = full_id.replace("/", "_")
        run_dir = root / directory_name / "new"
        run_dir.mkdir(parents=True, exist_ok=True)
        iters = [float(1000 * (i + 1)) for i in range(count)]
        times = [
            it * max(1.0, rng.gauss(centre, spread)) for it in iters
        ]
        (run_dir / "sample.json").write_text(
            json.dumps({"sampling_mode": "Linear", "iters": iters, "times": times}),
            encoding="utf-8",
        )
        (run_dir / "benchmark.json").write_text(
            json.dumps(
                {
                    "group_id": group_id,
                    "function_id": function_id,
                    "value_str": None,
                    "throughput": None,
                    "full_id": full_id,
                    "directory_name": directory_name,
                    "title": full_id,
                }
            ),
            encoding="utf-8",
        )
        avg = [t / i for t, i in zip(times, iters)]
        mean = sum(avg) / len(avg)
        (run_dir / "estimates.json").write_text(
            json.dumps(
                {
                    "mean": {
                        "confidence_interval": {
                            "confidence_level": 0.95,
                            "lower_bound": mean * 0.99,
                            "upper_bound": mean * 1.01,
                        },
                        "point_estimate": mean,
                        "standard_error": 0.0,
                    }
                }
            ),
            encoding="utf-8",
        )
    return root


# --------------------------------------------------------------------------
# Self-test
# --------------------------------------------------------------------------


def self_test() -> int:
    """Unit-style checks for the harvest, interval and gating helpers."""
    import tempfile

    # ---- 1. Criterion 0.8 harvest: per-iteration derivation + cross-check ----
    with tempfile.TemporaryDirectory() as tmp:
        crit = write_fixture_criterion_tree(Path(tmp) / "criterion")
        arms = harvest_criterion(crit)
        ids = {a["id"] for a in arms}
        assert ids == {
            "fixture_group/fast",
            "fixture_group/slow",
            "fixture_group/tiny_n",
        }, ids
        fast = next(a for a in arms if a["id"] == "fixture_group/fast")
        assert len(fast["samples_ns"]) == 40
        # The derived mean reproduces criterion's own point estimate exactly.
        assert fast["criterion_mean_estimate"] is not None
        assert abs(
            sum(fast["samples_ns"]) / len(fast["samples_ns"])
            - fast["criterion_mean_estimate"]["point_estimate_ns"]
        ) < 1e-9

        # Arms predating the run are excluded rather than published under this
        # run's commit; with every arm excluded, --allow-empty says so and
        # yields nothing instead of raising or inventing an artifact.
        future = 4_102_444_800.0  # 2100-01-01, after every fixture mtime
        try:
            harvest_criterion(crit, newer_than=future)
        except HarvestError as exc:
            assert "predating this run" in str(exc), exc
        else:  # pragma: no cover
            raise AssertionError("stale-only harvest must fail without --allow-empty")
        assert harvest_criterion(crit, newer_than=future, allow_empty=True) == []
        # A timestamp before the fixtures were written keeps every arm.
        assert len(harvest_criterion(crit, newer_than=0.0)) == 3

        # A layout that no longer reproduces criterion's mean fails loudly
        # rather than publishing a CI over misparsed numbers.
        bad = crit / "fixture_group_fast" / "new" / "estimates.json"
        bad.write_text(
            json.dumps({"mean": {"point_estimate": 1.0, "confidence_interval": {}}}),
            encoding="utf-8",
        )
        try:
            harvest_criterion(crit)
        except HarvestError as exc:
            assert "does not reproduce criterion" in str(exc), exc
        else:  # pragma: no cover
            raise AssertionError("mismatched criterion mean must fail loudly")

        # A sample.json with no sibling benchmark.json is a truncated directory.
        (crit / "fixture_group_fast" / "new" / "benchmark.json").unlink()
        try:
            harvest_criterion(crit)
        except HarvestError as exc:
            assert "without benchmark.json" in str(exc), exc
        else:  # pragma: no cover
            raise AssertionError("missing benchmark.json must fail loudly")

    # per_iteration_samples is the quotient criterion itself analyses.
    got = per_iteration_samples(
        {"iters": [10.0, 20.0], "times": [1000.0, 2400.0]}, Path("x")
    )
    assert got == [100.0, 120.0], got
    for broken in (
        {"iters": [1.0], "times": [1.0, 2.0]},
        {"iters": [], "times": []},
        {"iters": [0.0], "times": [5.0]},
        {"iters": "nope", "times": [1.0]},
    ):
        try:
            per_iteration_samples(broken, Path("x"))
        except HarvestError:
            pass
        else:  # pragma: no cover
            raise AssertionError(f"malformed sample must fail loudly: {broken}")

    # ---- 2. n < 3 and n below the published minimum are explicit ----
    tiny = {"id": "t", "samples_ns": [10.0, 11.0]}
    priced = arm_interval(tiny, 0.95, 2000, 42, min_n=3)
    assert priced["status"] == INSUFFICIENT_SAMPLES, priced
    assert priced["ci_lower_ns"] is None and priced["ci_upper_ns"] is None
    assert "n=2" in priced["status_detail"], priced["status_detail"]

    small = {"id": "s", "samples_ns": [10.0, 11.0, 12.0, 13.0, 14.0]}
    priced_small = arm_interval(small, 0.95, 2000, 42, min_n=DEFAULT_MIN_N)
    assert priced_small["status"] == INSUFFICIENT_SAMPLES, priced_small
    # ... and the same arm does get an interval once the minimum is lowered.
    priced_small_ok = arm_interval(small, 0.95, 2000, 42, min_n=3)
    assert priced_small_ok["status"] == "OK", priced_small_ok

    # An INSUFFICIENT_SAMPLES arm is never silently gated as a pass.
    verdict, _ = gate_absolute(priced, 100.0, LOWER_IS_BETTER)
    assert verdict == INSUFFICIENT_SAMPLES
    assert verdict not in PASSING_VERDICTS

    # ---- 3. The resample floor is enforced, not merely defaulted ----
    try:
        arm_interval({"id": "r", "samples_ns": [1.0] * 20}, 0.95, 999, 42, 3)
    except ValueError as exc:
        assert "below the §8.4 floor" in str(exc), exc
    else:  # pragma: no cover
        raise AssertionError("num_resamples < 1000 must be rejected")
    # Exactly the floor is allowed.
    assert (
        arm_interval({"id": "r", "samples_ns": [1.0, 2.0, 3.0, 4.0]}, 0.95, MIN_RESAMPLES, 42, 3)[
            "status"
        ]
        == "OK"
    )

    # ---- 4. The point estimate is enclosed by its interval ----
    import random as _random

    rng = _random.Random(7)
    for trial in range(8):
        data = [rng.gauss(50.0, 3.0) for _ in range(60)]
        arm = arm_interval({"id": f"e{trial}", "samples_ns": data}, 0.95, 1000, 42, 3)
        assert (
            arm["ci_lower_ns"] <= arm["point_ns"] <= arm["ci_upper_ns"]
        ), f"point not enclosed: {arm}"
        assert arm["ci_width_ns"] > 0.0

    # ---- 5. Gating is on the CI bound, never the point estimate ----
    # Latency arm: point 100, interval straddling the 101 ceiling.
    straddle = {
        "id": "g",
        "n": 50,
        "status": "OK",
        "point_ns": 100.0,
        "ci_lower_ns": 95.0,
        "ci_upper_ns": 105.0,
    }
    # Point clears the ceiling, the interval does not -> not a PASS.
    verdict, why = gate_absolute(straddle, 101.0, LOWER_IS_BETTER)
    assert verdict == INTERMEDIATE, (verdict, why)
    assert verdict not in PASSING_VERDICTS
    # Point fails, interval spans the threshold -> BOUNDARY_RESULT.
    verdict, _ = gate_absolute(straddle, 97.0, LOWER_IS_BETTER)
    assert verdict == BOUNDARY_RESULT, verdict
    # Whole interval clears the ceiling -> PASS.
    verdict, _ = gate_absolute(straddle, 110.0, LOWER_IS_BETTER)
    assert verdict == PASS, verdict
    # Whole interval on the failing side -> FAIL.
    verdict, _ = gate_absolute(straddle, 80.0, LOWER_IS_BETTER)
    assert verdict == FAIL, verdict

    # Same four cases for a higher-is-better metric, gated on the lower bound.
    hib = {
        "id": "h",
        "n": 50,
        "status": "OK",
        "point_ns": 100.0,
        "ci_lower_ns": 95.0,
        "ci_upper_ns": 105.0,
    }
    assert gate_absolute(hib, 90.0, HIGHER_IS_BETTER)[0] == PASS
    assert gate_absolute(hib, 99.0, HIGHER_IS_BETTER)[0] == INTERMEDIATE
    assert gate_absolute(hib, 103.0, HIGHER_IS_BETTER)[0] == BOUNDARY_RESULT
    assert gate_absolute(hib, 120.0, HIGHER_IS_BETTER)[0] == FAIL
    try:
        gate_absolute(hib, 1.0, "sideways")
    except ValueError:
        pass
    else:  # pragma: no cover
        raise AssertionError("unknown direction must be rejected")

    # ---- 6. Speedup gate: CI lower bound vs floor ----
    rng2 = _random.Random(11)
    head_arm = arm_interval(
        {"id": "sp", "samples_ns": [rng2.gauss(80.0, 2.0) for _ in range(60)]},
        0.95,
        1000,
        42,
        3,
    )
    base_arm = arm_interval(
        {"id": "sp", "samples_ns": [rng2.gauss(100.0, 2.0) for _ in range(60)]},
        0.95,
        1000,
        42,
        3,
    )
    res = gate_speedup(head_arm, base_arm, 1.05, num_resamples=1000)
    assert res["verdict"] == PASS, res
    assert res["ci_lower"] <= res["speedup"] <= res["ci_upper"], res
    # An unreachable floor fails on the interval, not on the point.
    res_fail = gate_speedup(head_arm, base_arm, 2.0, num_resamples=1000)
    assert res_fail["verdict"] == FAIL, res_fail
    # A floor inside the interval is an overlap label, never a PASS.
    res_edge = gate_speedup(
        head_arm, base_arm, res["ci_lower"] + (res["ci_upper"] - res["ci_lower"]) * 0.5,
        num_resamples=1000,
    )
    assert res_edge["verdict"] in (INTERMEDIATE, BOUNDARY_RESULT), res_edge
    assert res_edge["verdict"] not in PASSING_VERDICTS

    # ---- 7. Floors naming a missing arm fail rather than vanish ----
    artifact = build_artifact(
        [{"id": "present", "samples_ns": [10.0] * 20}],
        suite="unit",
        host_desc="synthetic fixture host",
        commit="0" * 40,
        run_id="self-test",
        confidence=0.95,
        num_resamples=1000,
        seed=42,
        min_n=3,
        fixture=True,
    )
    gates = apply_floors(
        artifact,
        [
            {"arm": "absent", "direction": LOWER_IS_BETTER, "threshold_ns": 1.0},
            {"arm": "present", "direction": LOWER_IS_BETTER, "threshold_ns": 1000.0},
        ],
    )
    assert gates[0]["verdict"] == FAIL and "absent from this run" in gates[0]["rationale"]
    assert gates[1]["verdict"] == PASS

    # ---- 8. Artifact shape and provenance guards ----
    assert artifact["schema"] == SCHEMA
    assert artifact["fixture"] is True
    assert artifact["statistics"]["num_resamples"] >= MIN_RESAMPLES
    assert artifact["arms"][0]["samples_ns"] == [10.0] * 20
    for bad_host in (f"benchmarks on {socket.gethostname()}", "/Users/someone/bench"):
        try:
            validate_host_description(bad_host)
        except ValueError:
            pass
        else:  # pragma: no cover
            raise AssertionError(f"host description must be rejected: {bad_host!r}")
    assert validate_host_description("AMD Ryzen 9 7950X, 16C/32T, 64 MiB L3, Linux 6.8")
    try:
        check_output_path(Path("results/baseline_comparative.json"), fixture=True)
    except ValueError:
        pass
    else:  # pragma: no cover
        raise AssertionError("fixture data must not be writable to a canonical baseline path")
    check_output_path(Path("results/baseline_fixture_demo.json"), fixture=True)
    check_output_path(Path("results/quick/anything.json"), fixture=True)

    # ---- 9. Rendering is derived, labelled, and carries the interval ----
    md = render_markdown(artifact, gates)
    assert "FIXTURE ARTIFACT" in md, md
    assert "BCa" in md and "95% BCa CI" in md
    assert "`present`" in md and "`absent`" in md
    assert "**PASS**" in md and "**FAIL**" in md
    for fabricated in ("faster than", "outperforms", "4× to 10×", "Key Findings"):
        assert fabricated not in md, f"narrative prose leaked into the report: {fabricated!r}"
    comp_md, comp_verdicts = render_comparison_markdown(
        build_artifact(
            [{"id": "sp", "samples_ns": head_arm["samples_ns"]}],
            suite="head",
            host_desc="synthetic fixture host",
            commit=None,
            run_id="self-test",
            confidence=0.95,
            num_resamples=1000,
            seed=42,
            min_n=3,
            fixture=True,
        ),
        build_artifact(
            [{"id": "sp", "samples_ns": base_arm["samples_ns"]}],
            suite="base",
            host_desc="synthetic fixture host",
            commit=None,
            run_id="self-test",
            confidence=0.95,
            num_resamples=1000,
            seed=42,
            min_n=3,
            fixture=True,
        ),
        1.05,
        LOWER_IS_BETTER,
        0.95,
        1000,
        42,
    )
    assert comp_verdicts == [PASS], comp_verdicts
    assert "Speedup vs Committed Baseline" in comp_md

    print("bench_baseline.py --self-test: all checks passed")
    return 0


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(
        description=(
            "Harvest criterion samples, compute BCa 95% CIs, emit a committed "
            "results/baseline_*.json, render the interval table, and gate claims "
            "on the CI's conservative bound (AGENTS.md §8.4/§8.7)."
        )
    )
    ap.add_argument("--harvest", action="store_true", help="harvest a criterion run into an artifact")
    ap.add_argument(
        "--criterion-dir",
        default="target/criterion",
        help="criterion output root (default: target/criterion)",
    )
    ap.add_argument(
        "--baseline-dir",
        default="new",
        help="criterion run directory to read: 'new', 'base', or a --save-baseline name",
    )
    ap.add_argument("--include", help="only harvest arms whose full_id contains this substring")
    ap.add_argument(
        "--newer-than",
        type=float,
        help=(
            "POSIX timestamp taken before the run; arms whose sample.json predates it "
            "are left-over output from an earlier job and are excluded"
        ),
    )
    ap.add_argument(
        "--allow-empty",
        action="store_true",
        help="print a notice and write no artifact when the run produced no criterion arms",
    )
    ap.add_argument("--suite", default="unnamed", help="suite label recorded in the artifact")
    ap.add_argument("--out", help="write the artifact here (JSON)")
    ap.add_argument("--input", "-i", help="read an existing artifact instead of harvesting")
    ap.add_argument("--against", help="baseline artifact to compare --input against")
    ap.add_argument(
        "--floor-speedup",
        type=float,
        default=1.05,
        help="speedup floor for --against (default: 1.05)",
    )
    ap.add_argument(
        "--direction",
        choices=[LOWER_IS_BETTER, HIGHER_IS_BETTER],
        default=LOWER_IS_BETTER,
        help="metric direction (criterion ns/iter is lower_is_better)",
    )
    ap.add_argument("--floors", help="JSON file declaring absolute floors to gate")
    ap.add_argument(
        "--fail-on-gate",
        action="store_true",
        help="exit non-zero when any declared gate does not PASS",
    )
    ap.add_argument("--host-desc", help="anonymised hardware description (§7 Privacy)")
    ap.add_argument("--commit", help="commit the run measures (default: git rev-parse HEAD)")
    ap.add_argument("--run-id", default="local", help="CI run URL or identifier")
    ap.add_argument("--confidence", type=float, default=0.95)
    ap.add_argument("--num-resamples", type=int, default=2000)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--min-n", type=int, default=DEFAULT_MIN_N)
    ap.add_argument(
        "--fixture",
        action="store_true",
        help="label the artifact as synthetic; refuses canonical baseline_*.json paths",
    )
    ap.add_argument(
        "--emit-fixture-criterion-dir",
        help="write a synthetic criterion tree here and exit (for exercising the path)",
    )
    ap.add_argument(
        "--summary-only",
        action="store_true",
        help="omit raw samples from the artifact (the CI is then not recomputable)",
    )
    ap.add_argument("--format", choices=["markdown", "json"], default="markdown")
    ap.add_argument("--self-test", action="store_true")

    args = ap.parse_args()
    if args.self_test:
        return self_test()

    if args.emit_fixture_criterion_dir:
        path = write_fixture_criterion_tree(Path(args.emit_fixture_criterion_dir))
        print(f"synthetic criterion fixture tree written to {path}", file=sys.stderr)
        return 0

    root = Path(__file__).resolve().parent.parent

    try:
        if args.input:
            artifact = _load_json(Path(args.input))
            if artifact.get("schema") != SCHEMA:
                raise HarvestError(
                    f"{args.input}: schema {artifact.get('schema')!r} is not {SCHEMA!r}"
                )
        elif args.harvest:
            if not args.host_desc:
                raise ValueError(
                    "--host-desc is required: a published number carries an anonymised "
                    "hardware description, never a hostname (§7 Privacy, §8.7 provenance)."
                )
            validate_host_description(args.host_desc)
            arms = harvest_criterion(
                Path(args.criterion_dir),
                args.baseline_dir,
                args.include,
                newer_than=args.newer_than,
                allow_empty=args.allow_empty,
            )
            if not arms:
                return 0
            artifact = build_artifact(
                arms,
                suite=args.suite,
                host_desc=args.host_desc,
                commit=args.commit or _git_commit(root),
                run_id=args.run_id,
                confidence=args.confidence,
                num_resamples=args.num_resamples,
                seed=args.seed,
                min_n=args.min_n,
                fixture=args.fixture,
                store_samples=not args.summary_only,
            )
        else:
            ap.error("one of --harvest, --input, --emit-fixture-criterion-dir or --self-test is required")
            return 2
    except (HarvestError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.out:
        out = Path(args.out)
        try:
            check_output_path(out, artifact.get("fixture", False))
        except ValueError as exc:
            print(f"error: {exc}", file=sys.stderr)
            return 1
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(artifact, indent=2) + "\n", encoding="utf-8")
        print(f"baseline artifact written to {out}", file=sys.stderr)

    failed = False
    if args.against:
        base = _load_json(Path(args.against))
        rendered, verdicts = render_comparison_markdown(
            artifact,
            base,
            args.floor_speedup,
            args.direction,
            args.confidence,
            args.num_resamples,
            args.seed,
        )
        failed = any(v not in PASSING_VERDICTS for v in verdicts)
    else:
        gates: Optional[List[Dict[str, Any]]] = None
        if args.floors:
            try:
                gates = apply_floors(artifact, load_floors(Path(args.floors)))
            except HarvestError as exc:
                print(f"error: {exc}", file=sys.stderr)
                return 1
            failed = any(g["verdict"] not in PASSING_VERDICTS for g in gates)
        rendered = (
            render_markdown(artifact, gates)
            if args.format == "markdown"
            else json.dumps(artifact, indent=2) + "\n"
        )

    print(rendered, end="" if rendered.endswith("\n") else "\n")

    if args.fail_on_gate and failed:
        print("error: at least one declared gate did not PASS", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
