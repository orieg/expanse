#!/usr/bin/env python3
"""Hold the pinned CPUs at the `performance` governor for a benchmark run (#1213).

    bench_governor.py apply   --state STATE.json [--json OUT.json]
    bench_governor.py restore --state STATE.json [--json OUT.json]
    bench_governor.py --self-test

## Why

The reference host's CPUs run intel_pstate's `powersave` governor with the
`balance_performance` energy/performance preference, and something else on the
host sometimes switches them to `performance`. Every wall-clock figure
therefore carried whatever frequency policy the host was in at the time,
including a ramp-up at the start of a short timed window. The policy is now a
controlled variable: the pinned CPUs run `performance` for the whole run, the
run verifies it, the host guard records it every sample and discards a run in
which it changes (`bench_host_guard.py summarize`), and afterwards each CPU is
put back to exactly the governor and preference it had.

## How

Only root can write cpufreq policy, so the change goes through one helper
installed by the host's administrator at `/usr/local/sbin/expanse-governor`
and a NOPASSWD sudoers line naming exactly that path (docs/BENCHMARKING.md,
"Host setup"). Its reference source is `scripts/host/expanse-governor`; the
checkout's copy is never what runs.

- `apply` refuses a host whose turbo is disabled or whose performance range is
  capped, since those change the delivered clock whatever the governor says.
  It records each pinned CPU's governor and preference in STATE, *then* sets
  `performance`, reads every CPU back, and fails (`governor_unavailable`) when
  the helper is missing, sudo asks for a password, or a CPU did not change.
- `restore` puts back the recorded values, governor first, then preference,
  since a preference cannot be written under `performance`. It is idempotent:
  STATE records that it ran. It also reports the thermal-throttle counters'
  change over the run.

Both run inside the host lock, so a restore never lands in someone else's run.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bench_host_guard import resolve_pin

SYSFS = Path("/sys/devices/system/cpu")
HELPER = "/usr/local/sbin/expanse-governor"
SUDO = ["/usr/bin/sudo", "-n"]


def _read(path: Path) -> str | None:
    try:
        return path.read_text().strip()
    except OSError:  # discipline:allow(error-swallowing): an absent sysfs knob is recorded as null, and apply refuses on the ones it needs
        return None


def cpulist(cpus: list[int]) -> str:
    return ",".join(str(c) for c in cpus)


def snapshot(pin: list[int]) -> dict:
    """Everything that sets the pinned CPUs' clock, per CPU where it is per CPU."""
    per_cpu = {}
    for c in pin:
        base = SYSFS / f"cpu{c}"
        per_cpu[str(c)] = {
            "governor": _read(base / "cpufreq" / "scaling_governor"),
            "epp": _read(base / "cpufreq" / "energy_performance_preference"),
            "core_throttle_count": _read(base / "thermal_throttle" / "core_throttle_count"),
            "package_throttle_count": _read(base / "thermal_throttle" / "package_throttle_count"),
        }
    pstate = SYSFS / "intel_pstate"
    return {
        "pin": pin,
        "per_cpu": per_cpu,
        "no_turbo": _read(pstate / "no_turbo"),
        "min_perf_pct": _read(pstate / "min_perf_pct"),
        "max_perf_pct": _read(pstate / "max_perf_pct"),
        "hwp_dynamic_boost": _read(pstate / "hwp_dynamic_boost"),
    }


def helper(*args: str) -> tuple[bool, str]:
    try:
        r = subprocess.run([*SUDO, HELPER, *args], capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return False, str(exc)
    return r.returncode == 0, (r.stderr or r.stdout).strip()


def _gh_output(**kv: str) -> None:
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as fh:
            for k, v in kv.items():
                fh.write(f"{k}={v}\n")


def fail(msg: str) -> int:
    print(f"::error::{msg}")
    _gh_output(fail_reason="governor_unavailable")
    return 1


def cmd_apply(state_path: Path, json_out: Path | None) -> int:
    pin = resolve_pin()
    if not pin:
        return fail("no pinned CPU set could be resolved; the governor cannot be set for it")
    before = snapshot(pin)
    if any(v["governor"] is None for v in before["per_cpu"].values()):
        return fail("a pinned CPU has no cpufreq governor to set")
    if before["no_turbo"] not in (None, "0"):
        return fail(f"turbo is disabled on this host (intel_pstate/no_turbo={before['no_turbo']}); "
                    "every figure would run below the clock the published ones ran at")
    if before["max_perf_pct"] not in (None, "100"):
        return fail(f"the host's performance range is capped (intel_pstate/max_perf_pct={before['max_perf_pct']})")
    state = {"restored": False, "before": before}
    state_path.write_text(json.dumps(state, indent=2) + "\n")
    ok, msg = helper("set", "performance", cpulist(pin))
    if not ok:
        return fail(
            f"could not set the performance governor through {HELPER} ({msg or 'no output'}). "
            "The host needs the helper and its sudoers line (docs/BENCHMARKING.md, Host setup)"
        )
    after = snapshot(pin)
    wrong = [c for c, v in after["per_cpu"].items() if v["governor"] != "performance"]
    if wrong:
        return fail(f"the governor did not change on CPU(s) {', '.join(wrong)}")
    epps = sorted({v["epp"] for v in after["per_cpu"].values() if v["epp"] is not None})
    state["applied"] = after
    state_path.write_text(json.dumps(state, indent=2) + "\n")
    if json_out:
        json_out.write_text(json.dumps({"before": before, "during": after}, indent=2) + "\n")
    print(f"governor: performance on CPU(s) {cpulist(pin)} (was: "
          f"{', '.join(sorted({v['governor'] for v in before['per_cpu'].values()}))}); "
          f"energy/performance preference {', '.join(epps) or 'n/a'}; turbo on, max_perf_pct "
          f"{after['max_perf_pct']}")
    return 0


def _group(values: dict[str, str | None]) -> dict[str, list[int]]:
    out: dict[str, list[int]] = {}
    for cpu, val in values.items():
        if val is not None:
            out.setdefault(val, []).append(int(cpu))
    return {k: sorted(v) for k, v in out.items()}


def cmd_restore(state_path: Path, json_out: Path | None) -> int:
    try:
        state = json.loads(state_path.read_text())
    except (OSError, ValueError):  # discipline:allow(error-swallowing): no state means apply never changed anything
        print("governor: nothing to restore (the run never changed it)")
        return 0
    if state.get("restored"):
        print("governor: already restored")
        return 0
    # A state without `applied` may still hold a partial change (the helper
    # can fail part-way), so it is restored like a complete one.
    before = state["before"]
    problems = []
    for gov, cpus in _group({c: v["governor"] for c, v in before["per_cpu"].items()}).items():
        ok, msg = helper("set", gov, cpulist(cpus))
        if not ok:
            problems.append(f"governor {gov} on {cpulist(cpus)}: {msg}")
    for epp, cpus in _group({
        c: v["epp"] for c, v in before["per_cpu"].items() if v["governor"] != "performance"
    }).items():
        ok, msg = helper("epp", epp, cpulist(cpus))
        if not ok:
            problems.append(f"preference {epp} on {cpulist(cpus)}: {msg}")
    now = snapshot(before["pin"])
    mismatched = [
        c for c, v in before["per_cpu"].items()
        if now["per_cpu"][c]["governor"] != v["governor"]
        or (v["governor"] != "performance" and now["per_cpu"][c]["epp"] != v["epp"])
    ]
    throttled = {}
    for c, v in before["per_cpu"].items():
        for k in ("core_throttle_count", "package_throttle_count"):
            a, b = v.get(k), now["per_cpu"][c].get(k)
            if a is not None and b is not None and int(b) > int(a):
                throttled[f"cpu{c}.{k}"] = int(b) - int(a)
    state["restored"] = not problems and not mismatched
    state["after"] = now
    state_path.write_text(json.dumps(state, indent=2) + "\n")
    if json_out:
        record = json.loads(json_out.read_text()) if json_out.exists() else {}
        record.update({"after": now, "thermal_throttle_events_during_run": throttled})
        json_out.write_text(json.dumps(record, indent=2) + "\n")
    if throttled:
        print(f"::warning::the pinned CPUs were thermally throttled during the run: {throttled}")
    if problems or mismatched:
        for p in problems:
            print(f"::warning::governor restore: {p}")
        if mismatched:
            print(f"::warning::governor restore: CPU(s) {', '.join(mismatched)} did not return to their prior policy")
        return 1
    groups = _group({c: v["governor"] for c, v in before["per_cpu"].items()})
    print(f"governor: restored ({'; '.join(f'{g} on {cpulist(c)}' for g, c in groups.items())})")
    return 0


# --------------------------------------------------------------------------
# self-test: a fake sysfs, and the reference helper pointed at it
# --------------------------------------------------------------------------
def self_test() -> int:
    global SYSFS, HELPER, SUDO
    ref_helper = Path(__file__).resolve().parent / "host" / "expanse-governor"
    saved = (SYSFS, HELPER, SUDO, os.environ.get("EXPANSE_BENCH_PIN"))
    with tempfile.TemporaryDirectory() as td:
        sysfs = Path(td, "cpu")
        for c in range(4):
            d = sysfs / f"cpu{c}" / "cpufreq"
            d.mkdir(parents=True)
            (d / "scaling_governor").write_text("powersave\n")
            (d / "energy_performance_preference").write_text("balance_performance\n")
            t = sysfs / f"cpu{c}" / "thermal_throttle"
            t.mkdir()
            (t / "core_throttle_count").write_text("0\n")
            (t / "package_throttle_count").write_text("0\n")
        (sysfs / "present").write_text("0-3\n")
        ps = sysfs / "intel_pstate"
        ps.mkdir()
        (ps / "no_turbo").write_text("0\n")
        (ps / "max_perf_pct").write_text("100\n")
        (ps / "min_perf_pct").write_text("17\n")
        # The reference helper with its sysfs root moved into the fixture.
        # It also emulates intel_pstate, which forces the preference to
        # `performance` under the performance governor and refuses (EBUSY)
        # a preference written while that governor is set.
        fake = Path(td, "expanse-governor")
        src = ref_helper.read_text().replace("readonly SYS=/sys/devices/system/cpu", f"readonly SYS={sysfs}")
        src = src.replace(
            'printf \'%s\\n\' "$value" > "$SYS/cpu$c/cpufreq/$file"',
            'if [ "$file" = energy_performance_preference ] && '
            '[ "$(cat "$SYS/cpu$c/cpufreq/scaling_governor")" = performance ]; then '
            'echo "EBUSY" >&2; exit 16; fi; '
            'printf \'%s\\n\' "$value" > "$SYS/cpu$c/cpufreq/$file"; '
            'if [ "$file" = scaling_governor ] && [ "$value" = performance ]; then '
            'printf \'performance\\n\' > "$SYS/cpu$c/cpufreq/energy_performance_preference"; fi',
        )
        assert src.count(str(sysfs)) == 1 and "energy_performance_preference\"; fi" in src, "fixture rewrite did not apply"
        fake.write_text(src)
        SYSFS, HELPER, SUDO = sysfs, str(fake), ["bash"]
        os.environ["EXPANSE_BENCH_PIN"] = "0-1"
        st = Path(td, "state.json")
        out = Path(td, "gov.json")
        devnull = open(os.devnull, "w")
        real_stdout = sys.stdout
        try:
            sys.stdout = devnull
            # CPU 2 is not pinned and carries its own policy, which must survive.
            (sysfs / "cpu1" / "cpufreq" / "energy_performance_preference").write_text("power\n")
            assert cmd_apply(st, out) == 0
            assert [(sysfs / f"cpu{c}" / "cpufreq" / "scaling_governor").read_text().strip() for c in range(4)] == \
                ["performance", "performance", "powersave", "powersave"]
            # A throttle event during the run is reported.
            (sysfs / "cpu0" / "thermal_throttle" / "package_throttle_count").write_text("3\n")
            assert cmd_restore(st, out) == 0
            assert [(sysfs / f"cpu{c}" / "cpufreq" / "scaling_governor").read_text().strip() for c in range(4)] == \
                ["powersave"] * 4
            # Each CPU gets back its own preference, not a common default.
            assert (sysfs / "cpu0" / "cpufreq" / "energy_performance_preference").read_text().strip() == "balance_performance"
            assert (sysfs / "cpu1" / "cpufreq" / "energy_performance_preference").read_text().strip() == "power"
            assert json.loads(out.read_text())["thermal_throttle_events_during_run"] == {"cpu0.package_throttle_count": 3}
            # Idempotent.
            assert cmd_restore(st, out) == 0 and json.loads(st.read_text())["restored"] is True

            # A CPU that was already at `performance` (another tenant's
            # setting) is put back to `performance`, not to powersave.
            (sysfs / "cpu1" / "cpufreq" / "scaling_governor").write_text("performance\n")
            st.unlink()
            assert cmd_apply(st, None) == 0 and cmd_restore(st, None) == 0
            assert (sysfs / "cpu1" / "cpufreq" / "scaling_governor").read_text().strip() == "performance"
            (sysfs / "cpu1" / "cpufreq" / "scaling_governor").write_text("powersave\n")

            # Refusals: turbo off; capped range; no helper.
            (ps / "no_turbo").write_text("1\n")
            assert cmd_apply(Path(td, "s2.json"), None) == 1
            (ps / "no_turbo").write_text("0\n")
            (ps / "max_perf_pct").write_text("80\n")
            assert cmd_apply(Path(td, "s3.json"), None) == 1
            (ps / "max_perf_pct").write_text("100\n")
            HELPER = str(Path(td, "absent"))
            assert cmd_apply(Path(td, "s4.json"), None) == 1
            HELPER = str(fake)
            # A helper that "succeeds" without changing anything is caught
            # by the read-back.
            noop = Path(td, "noop")
            noop.write_text("exit 0\n")
            HELPER = str(noop)
            assert cmd_apply(Path(td, "s5.json"), None) == 1
            HELPER = str(fake)
            # Nothing to restore when apply never ran.
            assert cmd_restore(Path(td, "never.json"), None) == 0
        finally:
            sys.stdout = real_stdout
            devnull.close()
            SYSFS, HELPER, SUDO = saved[0], saved[1], saved[2]
            if saved[3] is None:
                os.environ.pop("EXPANSE_BENCH_PIN", None)
            else:
                os.environ["EXPANSE_BENCH_PIN"] = saved[3]

        # The helper's own argument checks, run against the fixture.
        def run_helper(*args):
            return subprocess.run(["bash", str(fake), *args], capture_output=True, text=True).returncode

        assert run_helper("set", "performance", "0-1") == 0
        assert run_helper("set", "ondemand", "0") == 64, "only performance and powersave"
        assert run_helper("epp", "turbo", "0") == 64
        assert run_helper("set", "powersave", "0-9") == 64, "a CPU beyond present is refused"
        assert run_helper("set", "powersave", "0;rm -rf /") == 64
        assert run_helper("set", "powersave", "../../x") == 64
        assert run_helper("frob", "powersave", "0") == 64
        assert run_helper("set", "powersave") == 64

    print("bench_governor.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("command", nargs="?", choices=("apply", "restore"))
    ap.add_argument("--state", type=Path)
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.command or not args.state:
        ap.error("apply|restore and --state are required")
    return cmd_apply(args.state, args.json) if args.command == "apply" else cmd_restore(args.state, args.json)


if __name__ == "__main__":
    sys.exit(main())
