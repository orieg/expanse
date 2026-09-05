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
from bench_baseline import validate_host_description  # noqa: E402

SYS_PMU_ROOT = Path("/sys/devices")
P_CORE_PMU = "cpu_core"
E_CORE_PMU = "cpu_atom"
CRITERION_ROOT = REPO_ROOT / "target" / "criterion"
DEFAULT_ROUNDS = 6
RESAMPLES = 2000
# The three conditions, in the cyclic order a round rotates through.
CONDITIONS: Tuple[str, ...] = ("p_cores", "unpinned", "e_cores")
# `--condition NAME=CPULIST` (repeatable) replaces the three fixed conditions
# with caller-chosen affinity masks — the #680 question is `0-15` (both SMT
# siblings of every P-core) against `0,2,4,6,8,10,12,14` (one sibling per
# core). The first condition named is the reference every other one is read
# against; `unpinned` as the CPULIST means no mask. Its artifact carries its
# own schema so a masks comparison can never be read as a core-class one.
CONDITIONS_SCHEMA = "expanse.pin_exposure.conditions.v1"
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


def detect_commit() -> Optional[str]:
    """The commit of the tree being measured, or None if this is not a checkout.

    Exit-status discrimination per AGENTS.md §8.1: 0 is an answer, 128 is
    "not a git repository / no HEAD" and is a legitimate None (the reference
    rig is an rsync'd tree, not a checkout), and anything else is a tool
    failure that must not be read as "no commit".
    """
    try:
        proc = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
        )
    except OSError:
        return None
    if proc.returncode == 0:
        return proc.stdout.strip() or None
    if proc.returncode == 128:
        return None
    raise RuntimeError(f"`git rev-parse` exited {proc.returncode}: {proc.stderr.strip()[:200]}")


def resolve_commit(declared: Optional[str], detected: Optional[str]) -> Tuple[str, str]:
    """(commit, how_it_is_known). Refuses rather than mislabelling an artifact.

    A declared commit that contradicts the checkout is the dangerous case: it
    publishes numbers under a commit they were not taken at. Neither source is
    also a refusal — an artifact that cannot say what it measured is not
    provenance (AGENTS.md §8.7).
    """
    if declared and detected and not (declared.startswith(detected) or detected.startswith(declared)):
        raise Preflight(
            f"--commit {declared!r} contradicts the checkout at {REPO_ROOT}, which is at "
            f"{detected!r}. One of them is wrong, and publishing either would label the "
            "numbers with a commit they were not taken at."
        )
    if declared:
        return declared, "declared" if not detected else "declared (agrees with the checkout)"
    if detected:
        return detected, "git"
    raise Preflight(
        "no commit: this tree is not a git checkout, so what is being measured cannot be "
        "detected. Pass --commit <sha>. An artifact that cannot say which commit it measured "
        "is not provenance (AGENTS.md §8.7)."
    )


def describe_host() -> str:
    """An anonymised hardware description (AGENTS.md §7): never the hostname."""
    model = None
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("model name"):
                model = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    parts = [model or platform.machine(), f"{os.cpu_count()} logical CPUs", f"{platform.system()} {platform.release()}"]
    return ", ".join(parts)


def toolchain() -> Optional[str]:
    try:
        proc = subprocess.run(["rustc", "--version"], capture_output=True, text=True)
    except OSError:
        return None
    return proc.stdout.strip() or None if proc.returncode == 0 else None


def resolve_provenance(
    declared_commit: Optional[str], declared_host_desc: Optional[str]
) -> Tuple[str, str, str]:
    """(commit, commit_source, host_description) for the artifact's provenance.

    The whole assembly, not its pieces, so `--self-test` exercises what `main`
    actually calls: a privacy validator that a call site forgot to apply is
    the failure this exists to catch.
    """
    commit, commit_source = resolve_commit(declared_commit, detect_commit())
    return commit, commit_source, validate_host_description(declared_host_desc or describe_host())


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


def parse_conditions(
    specs: Sequence[str], p_cpus: str, e_cpus: str
) -> List[Tuple[str, Optional[str]]]:
    """`NAME=CPULIST` pairs -> [(name, cpulist | None)], validated against the host.

    Refusals are Preflights, never guesses: fewer than two conditions (there is
    nothing to compare), a repeated or non-identifier name, a mask naming a CPU
    the host does not expose, or a mask that mixes the two core classes. A
    condition spanning both classes measures where the scheduler put the work,
    not the mask — that is what the three fixed conditions are for.
    """
    if len(specs) < 2:
        raise Preflight("--condition needs at least two NAME=CPULIST entries; one mask compares against nothing")
    p_set, e_set = parse_cpu_list(p_cpus), parse_cpu_list(e_cpus)
    out: List[Tuple[str, Optional[str]]] = []
    seen: Set[str] = set()
    for spec in specs:
        name, sep, mask = spec.partition("=")
        name, mask = name.strip(), mask.strip()
        if not sep or not name or not mask:
            raise Preflight(f"--condition {spec!r} is not NAME=CPULIST")
        if not name.isidentifier():
            raise Preflight(f"--condition name {name!r} must be an identifier (it names a JSON field)")
        if name in seen:
            raise Preflight(f"--condition name {name!r} is given twice")
        seen.add(name)
        if mask.lower() == "unpinned":
            out.append((name, None))
            continue
        cpus = parse_cpu_list(mask)
        unknown = sorted(cpus - p_set - e_set)
        if unknown:
            raise Preflight(
                f"--condition {name}={mask} names CPU(s) {','.join(map(str, unknown))} that neither "
                f"the `{P_CORE_PMU}` list ({p_cpus}) nor the `{E_CORE_PMU}` list ({e_cpus}) contains"
            )
        if cpus & p_set and cpus & e_set:
            raise Preflight(
                f"--condition {name}={mask} spans both core classes; a mask like that measures "
                "scheduler placement, which the default three-condition run already does. "
                "Give each condition CPUs from one class."
            )
        out.append((name, mask))
    return out


def analyse_conditions(
    collected: Dict[str, Dict[str, List[float]]], names: Sequence[str]
) -> List[Dict[str, Any]]:
    """One row per arm present under every condition; `names[0]` is the reference.

    Each condition gets its own BCa interval and its ratio over, and overlap
    with, the reference. An arm missing from any condition is dropped rather
    than half-reported, exactly as `analyse` does.
    """
    ref = names[0]
    arms = set(collected.get(ref, {}))
    for name in names[1:]:
        arms &= set(collected.get(name, {}))
    rows: List[Dict[str, Any]] = []
    for arm in sorted(arms):
        r_mean, r_lo, r_hi, _ = _interval(collected[ref][arm])
        conds: Dict[str, Dict[str, Any]] = {}
        for name in names:
            mean, lo, hi, width = _interval(collected[name][arm])
            conds[name] = {
                "mean_ns": mean,
                "ci": [lo, hi],
                "ci_width_pct": width,
                "n": len(collected[name][arm]),
                "ratio_over_reference": mean / r_mean if r_mean else float("nan"),
                "overlaps_reference": overlap((r_lo, r_hi), (lo, hi)),
            }
        rows.append({"arm": arm, "reference": ref, "conditions": conds})
    return rows


def verdict_conditions(rows: Sequence[Dict[str, Any]], names: Sequence[str]) -> Tuple[str, str]:
    """(label, sentence) for the masks comparison. An overlap bounds, it does not prove zero."""
    if not rows:
        return "NO_DATA", "no arm produced samples under every condition; nothing was measured."
    ref = names[0]
    pairs = [(r, n) for r in rows for n in names[1:]]
    separated = [(r, n) for r, n in pairs if not r["conditions"][n]["overlaps_reference"]]
    if separated:
        r, n = max(separated, key=lambda rn: abs(rn[0]["conditions"][rn[1]]["ratio_over_reference"] - 1))
        return (
            "SEPARATED",
            f"{len(separated)} of {len(pairs)} arm×condition pairs have a BCa 95% interval that does not "
            f"overlap the reference `{ref}`; the widest is `{r['arm']}` under `{n}` at "
            f"{r['conditions'][n]['ratio_over_reference']:.3f}× the reference. The mask moved a measured arm.",
        )
    widest = max(
        (r["conditions"][n]["ci_width_pct"] for r, n in pairs), default=float("nan")
    )
    return (
        "OVERLAP",
        f"every condition's BCa 95% interval overlaps the reference `{ref}` on all {len(rows)} arm(s), so "
        f"the masks are indistinguishable to this instrument at this sample size. That bounds any effect "
        f"below roughly the interval width (widest {widest:.2f}%); it does not show the effect is zero.",
    )


def render_conditions(rows: Sequence[Dict[str, Any]], meta: Dict[str, Any], names: Sequence[str]) -> str:
    label, sentence = verdict_conditions(rows, names)
    pins = {c["condition"]: c["pin"] for c in meta.get("conditions", [])}
    described = ", ".join(f"`{n}` = {pins.get(n) or 'unpinned'}" for n in names)
    out = [
        f"### Affinity-mask comparison on {meta['host']} — `{meta['bench']}`, {meta['rounds']} interleaved rounds",
        "",
        f"Conditions per round, rotating which runs first: {described}. `{names[0]}` is the reference. "
        f"Samples are criterion per-iteration times pooled across rounds; intervals are BCa 95% over "
        f"{RESAMPLES} resamples.",
        "",
    ]
    if meta.get("invocation"):
        out += [
            f"Invocation (the mask aside): `{meta['invocation']}`. Criterion settings are part of "
            "what was measured, so a figure here pairs only with one taken at the same ones.",
            "",
        ]
    header = "| arm | " + " | ".join(f"`{n}` ns [BCa 95%]" for n in names)
    header += " | " + " | ".join(f"`{n}` ÷ `{names[0]}`" for n in names[1:])
    header += " | " + " | ".join(f"`{n}` vs `{names[0]}`" for n in names[1:]) + " |"
    align = "|---|" + "---:|" * len(names) + "---:|" * (len(names) - 1) + "---|" * (len(names) - 1)
    out += [header, align]
    for r in rows:
        c = r["conditions"]
        cells = [f"{c[n]['mean_ns']:,.2f} [{c[n]['ci'][0]:,.1f}, {c[n]['ci'][1]:,.1f}]" for n in names]
        ratios = [f"{c[n]['ratio_over_reference']:.3f}×" for n in names[1:]]
        reads = ["overlap" if c[n]["overlaps_reference"] else "**separated**" for n in names[1:]]
        out.append(f"| `{r['arm']}` | " + " | ".join(cells + ratios + reads) + " |")
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

    # 10. provenance: an artifact must be able to say what it measured, and
    #     must never say something the checkout contradicts (AGENTS.md §8.7).
    assert resolve_commit("c4b1817", None) == ("c4b1817", "declared")
    assert resolve_commit(None, "638f66e") == ("638f66e", "git")
    # a declared value that the checkout agrees with is recorded as agreeing,
    # and prefix-matching means a short sha and a long one do not collide
    assert resolve_commit("c4b1817f", "c4b1817")[0] == "c4b1817f"
    assert "agrees" in resolve_commit("c4b1817f", "c4b1817")[1]
    for declared, detected, why in (
        ("c4b1817", "638f66e", "a declared commit contradicting the checkout"),
        (None, None, "neither a declared nor a detectable commit"),
    ):
        try:
            resolve_commit(declared, detected)
            raise AssertionError(f"{why} must refuse")
        except Preflight:
            pass
    # the host description is anonymised: a hostname or a home path is refused
    assert describe_host()
    validate_host_description(describe_host())
    # ... and the refusal must survive the assembly `main` actually calls, not
    #     just the validator in isolation
    # A commit this tree can vouch for, so the host check is what is exercised
    # here rather than the commit refusal that correctly precedes it.
    ok_commit = detect_commit() or "c4b1817"
    for leaky in ("/home/someone/expanse rig", "bench box 192.168.1.9"):
        for call, what in (
            (lambda: validate_host_description(leaky), "the validator"),
            (lambda: resolve_provenance(ok_commit, leaky), "resolve_provenance"),
        ):
            try:
                call()
                raise AssertionError(f"{what} must refuse {leaky!r} as private infrastructure")
            except ValueError:
                pass
    assert resolve_provenance(ok_commit, None)[0] == ok_commit

    # 11. an inherited affinity mask is a refusal, not a narrower measurement
    assert affinity_gap("0-3", "4-5", set(range(6))) == []
    assert affinity_gap("0-3", "4-5", {0, 1, 2, 3}) == [4, 5]
    assert affinity_gap("0,2", "4-5", {0, 2, 4, 5}) == []
    for bad in ("", "0-", "x", "3-1,"):
        try:
            parse_cpu_list(bad)
            raise AssertionError(f"{bad!r} must not parse to a CPU set")
        except Preflight:
            pass

    # 12. every preflight refusal, on a synthetic host so they are all reachable
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

    # 13. `--condition`: the masks mode. Parsing refuses everything that would
    #     make the comparison meaningless, and accepts the #680 pair.
    ok = parse_conditions(["p_smt=0-15", "p_one_sibling=0,2,4,6,8,10,12,14", "free=unpinned"], "0-15", "16-23")
    assert ok == [("p_smt", "0-15"), ("p_one_sibling", "0,2,4,6,8,10,12,14"), ("free", None)], ok
    for bad, why in (
        (["a=0-15"], "one condition"),
        (["a=0-15", "a=0-7"], "duplicate name"),
        (["a=0-15", "b"], "missing mask"),
        (["a=0-15", "2b=0-7"], "non-identifier name"),
        (["a=0-15", "b=0-23"], "mask spanning both classes"),
        (["a=0-15", "b=40"], "CPU the host does not expose"),
    ):
        try:
            parse_conditions(bad, "0-15", "16-23")
            raise AssertionError(f"{why} must refuse")
        except Preflight:
            pass
    # identical distributions -> every pair overlaps, verdict OVERLAP, ratio ~1
    names = ("p_smt", "p_one_sibling")
    rows = analyse_conditions({"p_smt": {"x": p_a}, "p_one_sibling": {"x": sample(100)}}, names)
    assert rows[0]["reference"] == "p_smt"
    assert rows[0]["conditions"]["p_one_sibling"]["overlaps_reference"]
    assert abs(rows[0]["conditions"]["p_one_sibling"]["ratio_over_reference"] - 1) < 0.01
    label, sentence = verdict_conditions(rows, names)
    assert label == "OVERLAP" and "does not show the effect is zero" in sentence, sentence
    # a shifted second mask separates, and the verdict names the mask and the arm
    rows = analyse_conditions({"p_smt": {"x": p_a}, "p_one_sibling": {"x": [v * 1.1 for v in sample(100)]}}, names)
    assert not rows[0]["conditions"]["p_one_sibling"]["overlaps_reference"]
    label, sentence = verdict_conditions(rows, names)
    assert label == "SEPARATED" and "`p_one_sibling`" in sentence and "`x`" in sentence, sentence
    assert "1.10" in sentence, sentence
    # an arm absent from any condition is dropped; no shared arm -> NO_DATA
    rows = analyse_conditions({"p_smt": {"x": p_a, "y": p_a}, "p_one_sibling": {"x": p_a}}, names)
    assert [r["arm"] for r in rows] == ["x"]
    assert verdict_conditions(analyse_conditions({"p_smt": {"x": p_a}, "p_one_sibling": {"y": p_a}}, names), names)[0] == "NO_DATA"
    # render: one column per condition, a ratio and a reading per non-reference one
    md = render_conditions(
        analyse_conditions({"p_smt": {"x": p_a}, "p_one_sibling": {"x": [v * 1.1 for v in sample(100)]}}, names),
        {
            "host": "h", "bench": "compare", "rounds": 6,
            "conditions": [{"condition": "p_smt", "pin": "taskset -c 0-15"}, {"condition": "p_one_sibling", "pin": "taskset -c 0,2,4,6,8,10,12,14"}],
        },
        names,
    )
    header = next(ln for ln in md.splitlines() if ln.startswith("| arm |"))
    body = [ln for ln in md.splitlines() if ln.startswith("| `x`")]
    assert "`p_smt` ns" in header and "`p_one_sibling` ÷ `p_smt`" in header and "`p_one_sibling` vs `p_smt`" in header, header
    assert len(body) == 1 and body[0].count("|") == header.count("|"), (header, body)
    assert "**separated**" in md and "0,2,4,6,8,10,12,14" in md, md
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
    ap.add_argument(
        "--commit",
        help="commit being measured; auto-detected in a git checkout and REQUIRED otherwise "
        "(the reference rig is an rsync'd tree). A value contradicting the checkout is refused.",
    )
    ap.add_argument(
        "--host-desc",
        help="anonymised hardware description (CPU model, cores, cache, OS). Derived from "
        "/proc/cpuinfo when omitted; a hostname or home path in it is refused (AGENTS.md §7).",
    )
    ap.add_argument(
        "--condition",
        action="append",
        metavar="NAME=CPULIST",
        help="replace the three fixed conditions with these affinity masks (repeat; at least two). "
        "The first is the reference. `unpinned` as the CPULIST means no mask. "
        "The #680 pair: --condition p_smt=0-15 --condition p_one_sibling=0,2,4,6,8,10,12,14",
    )
    ap.add_argument(
        "--issue-url",
        default=ISSUE_URL,
        help="tracking issue recorded in the --json artifact (default: #639)",
    )
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
        commit, commit_source, host_desc = resolve_provenance(args.commit, args.host_desc)
    except (Preflight, ValueError, RuntimeError) as e:
        fail(str(e))
        return

    start_load = os.getloadavg() if hasattr(os, "getloadavg") else None
    if args.condition:
        try:
            custom = parse_conditions(args.condition, p_cpus, e_cpus)
        except Preflight as e:
            fail(str(e))
            return
        names: Tuple[str, ...] = tuple(name for name, _ in custom)
        pins: Dict[str, Optional[str]] = dict(custom)
    else:
        names = CONDITIONS
        pins = {"p_cores": p_cpus, "unpinned": None, "e_cores": e_cpus}
    collected: Dict[str, Dict[str, List[float]]] = {name: {} for name in names}
    for rnd in range(args.rounds):
        # Rotate which condition runs first, so a monotonic drift over the
        # session does not load onto whichever one always goes first.
        for name in round_order(rnd, names):
            print(f"round {rnd + 1}/{args.rounds}: {name}", file=sys.stderr)
            run_bench(args.bench, args.package, pins[name], args.filter, args.criterion_arg)
            for arm, samples in collect_samples().items():
                collected[name].setdefault(arm, []).extend(samples)

    meta = {
        "host": host_desc,
        "provenance": {
            "host_description": host_desc,
            "commit": commit,
            "commit_source": commit_source,
            "toolchain": toolchain(),
            "os": platform.system().lower(),
            "arch": platform.machine(),
            "load_average_at_start": start_load,
            "load_average_at_end": os.getloadavg() if hasattr(os, "getloadavg") else None,
        },
        "bench": args.bench,
        "rounds": args.rounds,
        "p_cpus": p_cpus,
        "e_cpus": e_cpus,
        "filter": args.filter,
        "criterion_args": list(args.criterion_arg or []),
        "invocation": " ".join(bench_argv(args.bench, args.package, None, args.filter, args.criterion_arg)),
        "conditions": [
            {"condition": name, "pin": (f"taskset -c {pins[name]}" if pins[name] else None)}
            for name in names
        ],
        "design": (
            f"{len(names)} conditions ({'/'.join(names)}) interleaved within each of "
            f"{args.rounds} rounds, rotating which runs first"
        ),
    }
    if args.condition:
        meta["reference"] = names[0]
        crows = analyse_conditions(collected, names)
        md = render_conditions(crows, meta, names)
        label, sentence = verdict_conditions(crows, names)
        schema, kind, rows_out = CONDITIONS_SCHEMA, "wall_clock_affinity_masks", crows
    else:
        rows = analyse(collected["p_cores"], collected["unpinned"], collected["e_cores"])
        md = render(rows, meta)
        label, sentence = verdict(rows)
        schema, kind, rows_out = SCHEMA, "wall_clock_core_placement", rows
    print(md)
    if args.markdown:
        args.markdown.write_text(md, encoding="utf-8")
    if args.json:
        args.json.write_text(
            json.dumps(
                {
                    "schema": schema,
                    "kind": kind,
                    "issue": args.issue_url,
                    "meta": meta,
                    "statistics": {
                        "estimator": "mean of pooled criterion per-iteration times (ns)",
                        "method": f"BCa bootstrap over {RESAMPLES} resamples",
                        "confidence": 0.95,
                    },
                    "verdict": label,
                    "reading": sentence,
                    "arms": rows_out,
                },
                indent=1,
            )
            + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
