"""scripts/bench_pin.py — the core pin for harnesses that are invoked directly.

`scripts/bench_pin.sh` is *sourced* by a suite runner and sets that shell's
affinity, so every process it spawns inherits the pin. A harness started as
`python3 bindings/python/bench_concurrency.py` has no such shell, so nothing
pinned it and `scripts/check_bench_pin.py` — which discovers runners by globbing
`docs/benchmarks/*/run.sh` — never looked at it. That gap shipped an unpinned
`results/baseline_python_concurrency.json` and kept it through a full
withhold-and-replace cycle (#755, #774, #779).

This module is the same rule for that lane. Call `apply()` once, before any
measurement:

    import bench_pin
    bench_pin.apply("bench_concurrency.py")

**Same contract as the shell helper, deliberately.** Same sysfs source of truth
(`/sys/devices/<pmu>/cpus`), same `EXPANSE_BENCH_PIN` vocabulary, same
`EXPANSE_BENCH_PIN_APPLIED` provenance key, and the same fail-loud refusals
(AGENTS.md §8.1). `scripts/check_bench_pin.py --self-test` drives both against
one synthetic topology and asserts they choose the same mask, so the two cannot
drift onto different answers — the failure this file would otherwise introduce.

Environment:
  EXPANSE_BENCH_PIN unset   auto — pin to the kernel's `cpu_core` list on a
                            hybrid host; no pin on a uniform one, where there
                            is nothing to pin away from.
  EXPANSE_BENCH_PIN=<list>  pin to exactly this CPU list, on any host.
  EXPANSE_BENCH_PIN=off     do not pin, and say so loudly.

A shell that already sourced `bench_pin.sh` exports `EXPANSE_BENCH_PIN_APPLIED`.
This module then verifies the inherited affinity rather than re-applying it: the
runner's pin wins, and a harness cannot silently widen it.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

__all__ = ["apply", "expand", "read_pmu_cpus", "PinRefused"]


class PinRefused(SystemExit):
    """Raised as a non-zero exit: no measurement is taken (§8.1)."""

    def __init__(self, message: str) -> None:
        sys.stderr.write(f"refusing to start: {message}\n")
        sys.stderr.write("No benchmark was run and no numbers were produced.\n")
        super().__init__(1)


def expand(cpu_list: str) -> list[int]:
    """`"0-3,8"` -> `[0, 1, 2, 3, 8]`, sorted and deduplicated."""
    out: set[int] = set()
    for part in cpu_list.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            lo, _, hi = part.partition("-")
            out.update(range(int(lo), int(hi) + 1))
        else:
            out.add(int(part))
    return sorted(out)


def read_pmu_cpus(pmu: str) -> str | None:
    """The kernel's CPU list for `pmu`, e.g. `0-15` for `cpu_core`.

    `EXPANSE_PIN_SYSFS_ROOT` exists so the refusal paths can be exercised
    against a synthetic topology; it is never set on a real run. Same variable
    `bench_pin.sh` honours, so the self-test can point both at one tree.
    """
    root = Path(os.environ.get("EXPANSE_PIN_SYSFS_ROOT", "/sys/devices"))
    try:
        return (root / pmu / "cpus").read_text().strip() or None
    except OSError:
        return None


def _publish(value: str) -> str:
    os.environ["EXPANSE_BENCH_PIN_APPLIED"] = value
    return value


def _verify(requested: list[int], e_cpus: str | None, who: str) -> None:
    """Read the affinity back; a mask the kernel narrowed means no pin held."""
    actual = sorted(os.sched_getaffinity(0))
    if actual != requested:
        raise PinRefused(
            f"{who} asked for CPUs {requested} but the process reports {actual}. "
            "The pin did not hold, so core placement is unknown for this run."
        )
    if e_cpus:
        clash = sorted(set(actual) & set(expand(e_cpus)))
        if clash:
            raise PinRefused(
                f"the requested CPU list includes efficiency core(s) {clash}. "
                "Wall-clock arms measured across two core classes are not "
                "comparable run to run. Set EXPANSE_BENCH_PIN=off to do it anyway."
            )


def apply(who: str = "benchmark") -> str:
    """Confine this process to the performance cores. Returns the applied list.

    The return value is also published as `EXPANSE_BENCH_PIN_APPLIED` so a
    harness can copy it straight into its artifact's provenance block (§8.7),
    which is what makes an unpinned run visible in the committed JSON instead of
    only in whoever remembers how it was launched.
    """
    p_cpus = read_pmu_cpus("cpu_core")
    e_cpus = read_pmu_cpus("cpu_atom")
    requested = os.environ.get("EXPANSE_BENCH_PIN", "")

    inherited = os.environ.get("EXPANSE_BENCH_PIN_APPLIED")
    if inherited and requested.lower() != "off":
        # A runner sourced bench_pin.sh and spawned us. Its pin wins; check it
        # actually reached this process rather than trusting the variable.
        if inherited not in ("none", "off"):
            _verify(expand(inherited), e_cpus, "the inherited pin")
        print(f"core pin: inherited {inherited} from the runner ({who})")
        return inherited

    if requested.lower() == "off":
        sys.stderr.write(
            "core pin: DISABLED by EXPANSE_BENCH_PIN=off — this run may migrate\n"
            "between core classes, and its wall-clock figures are not publishable\n"
            "on a hybrid host without saying so (#639).\n"
        )
        return _publish("off")

    if not requested:
        if not e_cpus:
            # Uniform host: nothing to pin away from, so a pin would only
            # shrink the machine. Same rule the shell helper applies.
            print(f"core pin: not needed — this host exposes one core class ({who})")
            return _publish("none")
        if not p_cpus:
            raise PinRefused(
                "an efficiency-core PMU is present but the kernel publishes no "
                "/sys/devices/cpu_core/cpus, so the performance cores cannot be "
                "named. Set EXPANSE_BENCH_PIN=<cpu list> explicitly."
            )
        mask = p_cpus
    else:
        mask = requested

    cpus = expand(mask)
    if not cpus:
        raise PinRefused(f"EXPANSE_BENCH_PIN={mask!r} names no CPUs.")
    try:
        os.sched_setaffinity(0, cpus)
    except (OSError, AttributeError) as exc:
        raise PinRefused(
            f"could not confine this process to CPUs {mask} ({exc}). A cgroup or "
            "affinity policy may be overriding it, or the platform has no "
            "affinity call. Set EXPANSE_BENCH_PIN=off to measure unpinned "
            "deliberately."
        ) from exc
    _verify(cpus, e_cpus, who)
    print(f"core pin: {who} confined to CPUs {mask} (AGENTS.md section 8.4, #639)")
    return _publish(mask)
