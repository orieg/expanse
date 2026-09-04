#!/usr/bin/env python3
"""Measure what core placement costs a wall-clock benchmark on a hybrid host (#639).

The reference host is a hybrid part: performance cores at ~5.1 GHz and
efficiency cores at ~3.8 GHz. `scripts/perf_counters.py` already refuses to
collect unpinned counters on such a host and prefixes its workload with
`taskset`. The wall-clock lane had no pin at all until #639, and AGENTS.md
§8.4 gates those arms on a BCa 95% interval whose width is assumed to reflect
measurement noise rather than core-class assignment.

This script answers the question #639 pre-registers: **run the same criterion
arms under three conditions — pinned to the P-cores, pinned to the E-cores,
and unpinned — interleaved, and compare the BCa intervals.** The three
conditions answer two different questions, and they are reported separately:

  * **E-cores ÷ P-cores** is the *exposure ceiling*: the whole distance the
    scheduler could move an arm across, whether or not it ever does. This is
    the arm that produced the published ceiling for #639.
  * **unpinned ÷ P-cores** is whether the hazard *fired* in this session. It
    can overlap while the ceiling separates — an idle host keeps the work on
    the P-cores — and that overlap is not evidence the hazard cannot fire.

    python3 scripts/pin_exposure.py --bench compare -p expanse-trie --rounds 6
    python3 scripts/pin_exposure.py --bench compare -p expanse-trie --json pin.json
    python3 scripts/pin_exposure.py --self-test

Rounds rotate which condition runs first, so a monotonic drift over the
session (thermal, background load) cancels across the conditions instead of
loading onto one. Over a multiple of three rounds each condition leads, runs
second and runs last an equal number of times.

Preflight is fail-loud (§8.1): a non-hybrid host, a missing `taskset`, a
kernel that publishes no P-core or E-core CPU list, or an affinity mask this
process already carries all refuse to run rather than silently measuring
something else. There is no reduced-condition fallback — the comparison needs
all three halves, and an inherited mask would make the unpinned condition a
pinned one and the E-core condition impossible.

The `--json` document is *not* the schema of `results/pin_exposure_639.json`:
that file records per-round summaries, which is all that was retained from the
session it documents, and says so in its own `statistics.limitation`. This
script emits pooled per-iteration samples bootstrapped with BCa, which is the
re-analysable form that file points at, so it carries its own schema name.
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Set, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ci  # noqa: E402

SYS_PMU_ROOT = Path("/sys/devices")
P_CORE_PMU = "cpu_core"
E_CORE_PMU = "cpu_atom"
CRITERION_ROOT = REPO_ROOT / "target" / "criterion"
DEFAULT_ROUNDS = 6
RESAMPLES = 2000
# The three conditions, in the cyclic order a round rotates through.
CONDITIONS: Tuple[str, ...] = ("p_cores", "unpinned", "e_cores")
# Distinct from `results/pin_exposure_639.json` (`expanse.pin_exposure.v1`),
# which holds per-round summaries rather than the per-iteration samples this
# script pools. Two shapes, two names, so neither can be read as the other.
SCHEMA = "expanse.pin_exposure.samples.v1"
ISSUE_URL = "https://github.com/orieg/expanse/issues/639"


class Preflight(Exception):
    """A precondition that makes the measurement meaningless if ignored."""


def fail(msg: str) -> None:
    print(f"pin_exposure.py: {msg}", file=sys.stderr)
    sys.exit(1)


def pmu_cpus(pmu: str) -> Optional[str]:
    """The kernel's CPU list for `pmu`, e.g. `0-15`. `None` if absent."""
    path = SYS_PMU_ROOT / pmu / "cpus"
    try:
        return path.read_text(encoding="utf-8").strip() or None
    except OSError:
        return None


def parse_cpu_list(spec: str) -> Set[int]:
    """`0-15,20` -> {0..15, 20}. A malformed list is a Preflight, never a guess."""
    out: Set[int] = set()
    for part in spec.split(","):
        part = part.strip()
        if not part:
            continue
        try:
            if "-" in part:
                lo, hi = part.split("-", 1)
                out.update(range(int(lo), int(hi) + 1))
            else:
                out.add(int(part))
        except ValueError as e:
            raise Preflight(f"cannot parse the kernel CPU list {spec!r}: {e}") from e
    if not out:
        raise Preflight(f"the kernel CPU list {spec!r} names no CPU")
    return out


def affinity_gap(p_cpus: str, e_cpus: str, inherited: Set[int]) -> List[int]:
    """CPUs the three conditions need that this process is not allowed to use.

    Split out from `preflight` so the refusal is checkable without a hybrid
    host: the caller supplies the mask instead of the kernel.
    """
    return sorted((parse_cpu_list(p_cpus) | parse_cpu_list(e_cpus)) - inherited)


def round_order(rnd: int, conditions: Sequence[str] = CONDITIONS) -> List[str]:
    """The order round `rnd` runs its conditions in.

    A rotation by `rnd % n`, so over any multiple of n rounds each condition
    leads, runs second and runs last equally often and a monotonic session
    drift cannot load onto whichever condition always goes first. For the two
    conditions this script started with, the rotation is exactly the
    flip-on-odd-rounds it replaces.
    """
    n = len(conditions)
    if n == 0:
        raise ValueError("no conditions to order")
    k = rnd % n
    return list(conditions[k:]) + list(conditions[:k])


def preflight(
    system: Optional[str] = None,
    has_taskset: Optional[bool] = None,
    affinity: Optional[Set[int]] = None,
) -> Tuple[str, str]:
    """Returns (p_core_cpus, e_core_cpus) or raises.

    The three environment facts are injectable so every refusal below is
    reachable from `--self-test` on a host that is not the reference one;
    `main` passes none of them and reads the real host.
    """
    system = platform.system() if system is None else system
    if system != "Linux":
        raise Preflight(
            f"this host is {system}, and core pinning here is a Linux "
            "`taskset` question about the bare-metal reference host. Run it there."
        )
    if has_taskset is None:
        has_taskset = shutil.which("taskset") is not None
    if not has_taskset:
        raise Preflight("`taskset` is not on PATH (install `util-linux`); the pinned halves cannot run.")
    p_cpus, e_cpus = pmu_cpus(P_CORE_PMU), pmu_cpus(E_CORE_PMU)
    if p_cpus is None:
        raise Preflight(
            f"the kernel publishes no CPU list at {SYS_PMU_ROOT / P_CORE_PMU / 'cpus'}, so this "
            "host exposes no performance-core PMU. Either it is not hybrid — in which case #639 "
            "does not apply to it and the measurement would compare a pin against itself — or the "
            "PMU is named differently and this script needs teaching."
        )
    if e_cpus is None:
        raise Preflight(
            f"a `{P_CORE_PMU}` PMU exists but no `{E_CORE_PMU}`, so the scheduler has no second "
            "core class to migrate onto and pinning cannot change the result. Nothing to measure."
        )
    # An inherited affinity mask is the one failure that would produce numbers
    # rather than an error: the unpinned condition would silently be a pinned
    # one, and `taskset -c <e_cpus>` cannot widen a mask it is nested inside.
    gap = affinity_gap(p_cpus, e_cpus, os.sched_getaffinity(0) if affinity is None else affinity)
    if gap:
        raise Preflight(
            f"this process already carries an affinity mask that excludes CPU(s) "
            f"{','.join(str(c) for c in gap)}. The unpinned condition would silently be a "
            "pinned one and the E-core condition cannot run at all, since `taskset` narrows a "
            "mask and never widens it. Run this outside the runner's pin — "
            "`EXPANSE_BENCH_PIN=off` is what scripts/bench_pin.sh reads to stand down."
        )
    return p_cpus, e_cpus


def bench_argv(
    bench: str,
    package: str,
    pin: Optional[str],
    filter_: Optional[str],
    criterion_args: Optional[Sequence[str]] = None,
) -> List[str]:
    """The exact command one condition runs.

    Split out from `run_bench` so the assembled command — which decides what
    is being measured — is checkable in `--self-test` without invoking cargo.

    Criterion's own flags (`--measurement-time`, `--warm-up-time`,
    `--sample-size`) go after the `--`, alongside the filter. They are part of
    the comparison's definition, not a detail: a run at a different warm-up
    is a different measurement, and a claim paired against an earlier one has
    to hold them equal (AGENTS.md §8.3).
    """
    argv: List[str] = []
    if pin:
        argv += ["taskset", "-c", pin]
    argv += ["cargo", "bench", "--bench", bench, "-p", package]
    tail = ([filter_] if filter_ else []) + list(criterion_args or [])
    if tail:
        argv += ["--"] + tail
    return argv


def run_bench(
    bench: str,
    package: str,
    pin: Optional[str],
    filter_: Optional[str],
    criterion_args: Optional[Sequence[str]] = None,
) -> None:
    argv = bench_argv(bench, package, pin, filter_, criterion_args)
    proc = subprocess.run(argv, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr[-4000:])
        fail(f"`{' '.join(argv)}` exited {proc.returncode}")


def collect_samples() -> Dict[str, List[float]]:
    """Per-iteration times from every criterion `new/sample.json` under target/criterion.

    Criterion overwrites `new/` on each run, so this is called immediately
    after each run and the values are accumulated by the caller.
    """
    out: Dict[str, List[float]] = {}
    if not CRITERION_ROOT.exists():
        fail(f"{CRITERION_ROOT} does not exist — did the bench run?")
    for sample_path in sorted(CRITERION_ROOT.glob("**/new/sample.json")):
        arm = str(sample_path.parent.parent.relative_to(CRITERION_ROOT))
        try:
            data = json.loads(sample_path.read_text(encoding="utf-8"))
            times, iters = data["times"], data["iters"]
        except (OSError, KeyError, json.JSONDecodeError) as e:
            fail(f"cannot read {sample_path}: {e}")
        if len(times) != len(iters) or not times:
            fail(f"{sample_path}: {len(times)} times for {len(iters)} iters")
        out[arm] = [t / i for t, i in zip(times, iters) if i]
    if not out:
        fail("no criterion sample.json found — the bench produced no samples")
    return out


def overlap(a: Tuple[float, float], b: Tuple[float, float]) -> bool:
    return a[0] <= b[1] and b[0] <= a[1]


def _interval(samples: Sequence[float]) -> Tuple[float, float, float, float]:
    """(mean, lo, hi, width%) — one condition's BCa 95% interval."""
    mean, lo, hi = bca_bootstrap_ci(list(samples), num_resamples=RESAMPLES)
    return mean, lo, hi, ((hi - lo) / mean * 100 if mean else float("nan"))


def analyse(
    p_cores: Dict[str, List[float]],
    unpinned: Dict[str, List[float]],
    e_cores: Dict[str, List[float]],
) -> List[Dict[str, Any]]:
    """One row per arm present under all three conditions.

    An arm missing from any condition is dropped rather than half-reported:
    a ratio needs both of its halves and the ceiling needs the third.
    """
    rows: List[Dict[str, Any]] = []
    for arm in sorted(set(p_cores) & set(unpinned) & set(e_cores)):
        p, u, e = p_cores[arm], unpinned[arm], e_cores[arm]
        p_mean, p_lo, p_hi, p_width = _interval(p)
        u_mean, u_lo, u_hi, u_width = _interval(u)
        e_mean, e_lo, e_hi, e_width = _interval(e)
        rows.append(
            {
                "arm": arm,
                "p_cores_ns": p_mean,
                "unpinned_ns": u_mean,
                "e_cores_ns": e_mean,
                "p_cores_ci": [p_lo, p_hi],
                "unpinned_ci": [u_lo, u_hi],
                "e_cores_ci": [e_lo, e_hi],
                "p_cores_ci_width_pct": p_width,
                "unpinned_ci_width_pct": u_width,
                "e_cores_ci_width_pct": e_width,
                "ratio_e_over_p": e_mean / p_mean if p_mean else float("nan"),
                "ratio_unpinned_over_p": u_mean / p_mean if p_mean else float("nan"),
                "e_p_intervals_overlap": overlap((p_lo, p_hi), (e_lo, e_hi)),
                "unpinned_p_intervals_overlap": overlap((p_lo, p_hi), (u_lo, u_hi)),
                "n_p_cores": len(p),
                "n_unpinned": len(u),
                "n_e_cores": len(e),
            }
        )
    return rows


def _ceiling_clause(rows: Sequence[Dict[str, Any]], separated: Sequence[Dict[str, Any]]) -> str:
    """What the E-core arm says, phrased for whichever way the unpinned pair went."""
    if not separated:
        return (
            f"No arm separates E-core from P-core either, so on this workload the host's two core "
            f"classes are indistinguishable to the instrument across all {len(rows)} arms"
        )
    worst = max(separated, key=lambda r: abs(r["ratio_e_over_p"] - 1))
    return (
        f"{len(separated)} of {len(rows)} arms separate E-core from P-core, the widest `{worst['arm']}` "
        f"at {worst['ratio_e_over_p']:.3f}× E-core over P-core; that is the exposure ceiling — the whole "
        f"distance a migration could move an arm, whether or not one happened here"
    )


def _width_clause(widened: Sequence[Dict[str, Any]]) -> str:
    if not widened:
        return ""
    return (
        f" {len(widened)} arm(s) nonetheless show an unpinned interval more than twice as wide as the "
        "P-core one, which is the variance the pin would remove even where the point estimates agree."
    )


def verdict(rows: Sequence[Dict[str, Any]]) -> Tuple[str, str]:
    """(label, sentence) — the reading #639 pre-registered, over both pairs."""
    if not rows:
        return "NO_DATA", "no arm produced samples under all three conditions; nothing was measured."
    placement = [r for r in rows if not r["unpinned_p_intervals_overlap"]]
    ceiling = [r for r in rows if not r["e_p_intervals_overlap"]]
    widened = [r for r in rows if r["unpinned_ci_width_pct"] > 2 * r["p_cores_ci_width_pct"]]
    ceiling_clause = _ceiling_clause(rows, ceiling)
    if placement:
        worst = max(placement, key=lambda r: abs(r["ratio_unpinned_over_p"] - 1))
        return (
            "SEPARATED",
            f"{len(placement)} of {len(rows)} arms have non-overlapping unpinned and P-core intervals; "
            f"the widest is `{worst['arm']}` at {worst['ratio_unpinned_over_p']:.3f}× unpinned over P-core. "
            "Core placement moved a measured arm, so the wall-clock arms warrant the pin "
            f"perf_counters.py already applies. {ceiling_clause}." + _width_clause(widened),
        )
    if ceiling:
        return (
            "OVERLAP_BUT_PINNED",
            f"every one of the {len(rows)} arms has overlapping unpinned and P-core BCa 95% intervals, so "
            "the scheduler left this session's work where the pin would have put it and the hazard did not "
            f"fire in these rounds. {ceiling_clause}. An overlap here is not evidence the hazard cannot "
            "fire: placement depends on what else the host is doing, and the interval stays narrow whether "
            "or not a round migrated." + _width_clause(widened),
        )
    return (
        "NO_EXPOSURE",
        f"neither pair separates on any of the {len(rows)} arms: unpinned and E-core intervals both overlap "
        f"the P-core interval. {ceiling_clause}, and pinning changes nothing this instrument can resolve."
        + _width_clause(widened),
    )


def render(rows: Sequence[Dict[str, Any]], meta: Dict[str, Any]) -> str:
    label, sentence = verdict(rows)
    out = [
        f"### Core-placement exposure on {meta['host']} — `{meta['bench']}`, {meta['rounds']} interleaved rounds",
        "",
        f"Three conditions per round, rotating which runs first: P-cores `taskset -c {meta['p_cpus']}`, "
        f"unpinned (no affinity mask), E-cores `taskset -c {meta['e_cpus']}`. Samples are criterion "
        f"per-iteration times, pooled across rounds; intervals are BCa 95% over {RESAMPLES} resamples. "
        "`E ÷ P` is the exposure ceiling; `unpinned ÷ P` is whether it fired.",
        "",
    ]
    if meta.get("invocation"):
        out += [
            f"Invocation (the pin aside): `{meta['invocation']}`. Criterion settings are part of "
            "what was measured, so a figure here pairs only with one taken at the same ones.",
            "",
        ]
    out += [
        "| arm | P-cores ns | unpinned ns | E-cores ns | E ÷ P | unpinned ÷ P | CI width P / unpinned / E | E vs P | unpinned vs P |",
        "|---|---:|---:|---:|---:|---:|---:|---|---|",
    ]
    for r in rows:
        out.append(
            f"| `{r['arm']}` | {r['p_cores_ns']:,.2f} | {r['unpinned_ns']:,.2f} | {r['e_cores_ns']:,.2f} | "
            f"{r['ratio_e_over_p']:.3f}× | {r['ratio_unpinned_over_p']:.3f}× | "
            f"{r['p_cores_ci_width_pct']:.2f}% / {r['unpinned_ci_width_pct']:.2f}% / {r['e_cores_ci_width_pct']:.2f}% | "
            f"{'overlap' if r['e_p_intervals_overlap'] else '**separated**'} | "
            f"{'overlap' if r['unpinned_p_intervals_overlap'] else '**separated**'} |"
        )
    out += ["", f"**Verdict: {label}.** {sentence}"]
    return "\n".join(out) + "\n"


def self_test() -> None:
    """Fail-then-pass pins of the reading, on synthetic samples."""
    import random

    rng = random.Random(0xB0A7)
    # The published #639 ratios, so the synthetic arms sit where the real ones did.
    e_over_p, clock_ratio = 1.576, 5.1 / 3.8

    def sample(centre: float, sigma: float = 1.0, n: int = 400) -> List[float]:
        return [centre + rng.gauss(0, sigma) for _ in range(n)]

    # 1. three identical distributions -> nothing separates, verdict NO_EXPOSURE
    p_a, u_a, e_a = sample(100), sample(100), sample(100)
    rows = analyse({"x": p_a}, {"x": u_a}, {"x": e_a})
    assert rows[0]["unpinned_p_intervals_overlap"], "same distribution must overlap"
    assert rows[0]["e_p_intervals_overlap"], "same distribution must overlap"
    label, sentence = verdict(rows)
    assert label == "NO_EXPOSURE", label
    assert "indistinguishable to the instrument" in sentence, sentence

    # 2. the #639 shape: E separated from P, unpinned indistinguishable from P.
    #    The two pairs must read independently — this is the case the
    #    two-condition script could not produce at all.
    e_slow = [x * e_over_p for x in sample(100)]
    rows = analyse({"x": p_a}, {"x": u_a}, {"x": e_slow})
    assert not rows[0]["e_p_intervals_overlap"], "a 57.6% shift must separate E from P"
    assert rows[0]["unpinned_p_intervals_overlap"], "the unpinned pair must overlap independently"
    assert f"{rows[0]['ratio_e_over_p']:.2f}" == "1.58", rows[0]["ratio_e_over_p"]
    label, sentence = verdict(rows)
    assert label == "OVERLAP_BUT_PINNED", label
    assert "exposure ceiling" in sentence and "`x`" in sentence, sentence
    assert "not evidence the hazard cannot" in sentence, sentence

    # 3. the hazard fires: unpinned drifts onto the E-cores too. The unpinned
    #    pair now separates, and the ceiling clause still reports alongside it.
    u_slow = [x * clock_ratio for x in sample(100)]
    rows = analyse({"x": p_a}, {"x": u_slow}, {"x": e_slow})
    assert not rows[0]["unpinned_p_intervals_overlap"], "a 34% shift must separate"
    label, sentence = verdict(rows)
    assert label == "SEPARATED", label
    assert "1.3" in f"{rows[0]['ratio_unpinned_over_p']:.3f}", rows[0]["ratio_unpinned_over_p"]
    assert "exposure ceiling" in sentence and "`x`" in sentence, sentence

    # 4. same means, far wider unpinned spread -> overlap, but the width note fires
    wide = sample(100, sigma=12)
    rows = analyse({"x": p_a}, {"x": wide}, {"x": e_slow})
    assert rows[0]["unpinned_p_intervals_overlap"], "same mean must still overlap"
    label, sentence = verdict(rows)
    assert label == "OVERLAP_BUT_PINNED" and "twice as wide" in sentence, sentence

    # 5. an arm absent from ANY condition is dropped, not half-reported. The
    #    E-core condition is a dropping condition like the other two.
    assert [r["arm"] for r in analyse({"x": p_a, "y": p_a}, {"x": u_a}, {"x": e_a})] == ["x"]
    assert [r["arm"] for r in analyse({"x": p_a, "y": p_a}, {"x": u_a, "y": u_a}, {"x": e_a})] == ["x"]

    # 6. no arm shared by all three -> NO_DATA, never a pass
    assert verdict(analyse({"x": p_a}, {"y": u_a}, {"x": e_a}))[0] == "NO_DATA"

    # 7. render carries both ratios and both interval readings for every arm
    md = render(
        analyse({"x": p_a}, {"x": u_a}, {"x": e_slow}),
        {"host": "h", "bench": "b", "rounds": 6, "p_cpus": "0-15", "e_cpus": "16-23"},
    )
    header = next(ln for ln in md.splitlines() if ln.startswith("| arm |"))
    body = [ln for ln in md.splitlines() if ln.startswith("| `x`")]
    for column in ("P-cores ns", "unpinned ns", "E-cores ns", "E ÷ P", "unpinned ÷ P", "E vs P", "unpinned vs P"):
        assert column in header, (column, header)
    assert len(body) == 1 and body[0].count("|") == header.count("|"), (header, body)
    assert md.count("**separated**") == 1 and "overlap" in md, md
    assert "16-23" in md and "0-15" in md, md

    # 8. the rotation leads with each condition equally and reproduces the
    #    two-condition flip it generalises.
    orders = [round_order(r) for r in range(6)]
    assert orders[0] == ["p_cores", "unpinned", "e_cores"], orders[0]
    assert orders[1] == ["unpinned", "e_cores", "p_cores"], orders[1]
    assert orders[2] == ["e_cores", "p_cores", "unpinned"], orders[2]
    for pos in range(len(CONDITIONS)):
        seen = sorted(o[pos] for o in orders)
        assert seen == sorted(list(CONDITIONS) * 2), (pos, seen)
    assert [round_order(r, ("a", "b")) for r in range(4)] == [
        ["a", "b"],
        ["b", "a"],
        ["a", "b"],
        ["b", "a"],
    ], "the two-condition case must still flip on odd rounds"

    # 9. the assembled command: criterion's own flags reach criterion, the pin
    #    stays outside cargo, and the three conditions differ ONLY in the pin.
    base = ("domain", "expanse-trie")
    assert bench_argv(*base, None, None, None) == ["cargo", "bench", "--bench", "domain", "-p", "expanse-trie"]
    assert bench_argv(*base, "0-15", None, None)[:3] == ["taskset", "-c", "0-15"]
    assert bench_argv(*base, None, "arm/100000", None)[-2:] == ["--", "arm/100000"]
    # criterion flags land after the `--`, after the filter, in the given order
    assert bench_argv(*base, None, "arm/100000", ["--measurement-time", "5"])[-4:] == [
        "--",
        "arm/100000",
        "--measurement-time",
        "5",
    ]
    # ... and a `--` is still emitted when there are flags but no filter, or
    # criterion would never see them
    assert bench_argv(*base, None, None, ["--warm-up-time", "2"])[-3:] == ["--", "--warm-up-time", "2"]
    # the three conditions must be the same command but for the pin: anything
    # else and the comparison is not measuring core placement (AGENTS.md §8.3)
    args = ("arm/100000", ["--measurement-time", "5"])
    per_condition = [bench_argv(*base, pin, *args) for pin in ("0-15", None, "16-23")]
    stripped = [a[3:] if a[0] == "taskset" else a for a in per_condition]
    assert stripped[0] == stripped[1] == stripped[2], per_condition

    # 10. an inherited affinity mask is a refusal, not a narrower measurement
    assert affinity_gap("0-3", "4-5", set(range(6))) == []
    assert affinity_gap("0-3", "4-5", {0, 1, 2, 3}) == [4, 5]
    assert affinity_gap("0,2", "4-5", {0, 2, 4, 5}) == []
    for bad in ("", "0-", "x", "3-1,"):
        try:
            parse_cpu_list(bad)
            raise AssertionError(f"{bad!r} must not parse to a CPU set")
        except Preflight:
            pass

    # 11. every preflight refusal, on a synthetic host so they are all reachable
    #     from any machine rather than only from the reference one.
    global pmu_cpus
    real = pmu_cpus
    hybrid = {"system": "Linux", "has_taskset": True, "affinity": set(range(24))}

    def expect(fragment: str, **over: Any) -> None:
        try:
            preflight(**{**hybrid, **over})
        except Preflight as e:
            assert fragment in str(e), (fragment, str(e))
        else:
            raise AssertionError(f"a host that {fragment!r} must refuse")

    try:
        pmu_cpus = lambda pmu: {P_CORE_PMU: "0-15", E_CORE_PMU: "16-23"}.get(pmu)  # noqa: E731
        assert preflight(**hybrid) == ("0-15", "16-23"), "a hybrid host with a full mask must not refuse"
        expect("core pinning here is a Linux", system="Darwin")
        expect("`taskset` is not on PATH", has_taskset=False)
        # `taskset` narrows a mask and never widens it, so a runner that already
        # pinned to the P-cores can neither reach the E-cores nor run unpinned.
        expect("affinity mask", affinity=set(range(16)))
        pmu_cpus = lambda pmu: "0-15" if pmu == P_CORE_PMU else None  # noqa: E731
        expect("no second core class")
        pmu_cpus = lambda pmu: None  # noqa: E731
        expect("no performance-core PMU")
    finally:
        pmu_cpus = real
    print("pin_exposure.py --self-test: all checks passed")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bench", help="criterion bench target, e.g. `compare`")
    ap.add_argument("-p", "--package", default="expanse-trie")
    ap.add_argument("--filter", help="criterion filter passed after `--`")
    ap.add_argument(
        "--criterion-arg",
        action="append",
        metavar="ARG",
        help="extra argument passed to criterion after `--`, repeatable "
        "(e.g. --criterion-arg=--measurement-time --criterion-arg=5). Recorded in "
        "the --json meta, because a run at different criterion settings is not "
        "paired with one at the defaults.",
    )
    ap.add_argument("--rounds", type=int, default=DEFAULT_ROUNDS, help=f"interleaved rounds per condition (default {DEFAULT_ROUNDS})")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return
    if not args.bench:
        ap.error("--bench is required")
    try:
        p_cpus, e_cpus = preflight()
    except Preflight as e:
        fail(str(e))
        return

    pins: Dict[str, Optional[str]] = {"p_cores": p_cpus, "unpinned": None, "e_cores": e_cpus}
    collected: Dict[str, Dict[str, List[float]]] = {name: {} for name in CONDITIONS}
    for rnd in range(args.rounds):
        # Rotate which condition runs first, so a monotonic drift over the
        # session does not load onto whichever one always goes first.
        for name in round_order(rnd):
            print(f"round {rnd + 1}/{args.rounds}: {name}", file=sys.stderr)
            run_bench(args.bench, args.package, pins[name], args.filter, args.criterion_arg)
            for arm, samples in collect_samples().items():
                collected[name].setdefault(arm, []).extend(samples)

    rows = analyse(collected["p_cores"], collected["unpinned"], collected["e_cores"])
    meta = {
        "host": platform.node() and f"{platform.machine()} Linux hybrid host",
        "bench": args.bench,
        "rounds": args.rounds,
        "p_cpus": p_cpus,
        "e_cpus": e_cpus,
        "filter": args.filter,
        "criterion_args": list(args.criterion_arg or []),
        "invocation": " ".join(bench_argv(args.bench, args.package, None, args.filter, args.criterion_arg)),
        "conditions": [
            {"condition": name, "pin": (f"taskset -c {pins[name]}" if pins[name] else None)}
            for name in CONDITIONS
        ],
        "design": (
            f"{len(CONDITIONS)} conditions ({'/'.join(CONDITIONS)}) interleaved within each of "
            f"{args.rounds} rounds, rotating which runs first"
        ),
    }
    md = render(rows, meta)
    print(md)
    if args.markdown:
        args.markdown.write_text(md, encoding="utf-8")
    if args.json:
        label, sentence = verdict(rows)
        args.json.write_text(
            json.dumps(
                {
                    "schema": SCHEMA,
                    "kind": "wall_clock_core_placement",
                    "issue": ISSUE_URL,
                    "meta": meta,
                    "statistics": {
                        "estimator": "mean of pooled criterion per-iteration times (ns)",
                        "method": f"BCa bootstrap over {RESAMPLES} resamples",
                        "confidence": 0.95,
                    },
                    "verdict": label,
                    "reading": sentence,
                    "arms": rows,
                },
                indent=1,
            )
            + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
