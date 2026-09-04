#!/usr/bin/env python3
"""scripts/check_bench_pin.py — every wall-clock runner takes the core pin (#639).

The bare-metal reference host is a hybrid part: performance cores at 5.0-5.1
GHz and efficiency cores at 3.8 GHz. Measured on that host, the same criterion
arm run on the E-cores takes 1.576x the time it takes on the P-cores, with
separated intervals. `scripts/perf_counters.py` has always refused to collect
unpinned counters there; the wall-clock lane had no pin at all until #639.

`scripts/bench_pin.sh` is the pin. It is sourced by the runner, not called by
the harness, so it is inherited by everything the runner spawns and no
benchmark added later has to remember it. This gate is what makes that true:
a `docs/benchmarks/*/run.sh` that does not source it, or a bare-metal workflow
that does not, fails the `lint` job by name.

Fail-loud (AGENTS.md section 8.1): an unreadable file or a missing runner is an
error, never a silently skipped check.

Usage:
  python3 scripts/check_bench_pin.py
  python3 scripts/check_bench_pin.py --self-test
"""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

PIN_HELPER = "scripts/bench_pin.sh"

# Runners that must source the pin. `docs/benchmarks/*/run.sh` is discovered
# rather than listed, so a new suite is covered the moment it lands.
# `bench_aarch64.yml` is deliberately absent: it runs `--quick` on a
# GitHub-hosted macOS runner, which has no `taskset` and publishes nothing.
WORKFLOWS = [
    ".github/workflows/bench_baremetal.yml",
    ".github/workflows/bench_avx512.yml",
]

# Suites the pin cannot help, for one of two reasons: the timed region is not on
# this host's CPUs at all, or the suite's published metric is not wall clock in
# the first place. The exemption is per-suite and carries its reason here; it is
# not a way to opt out of the pin for a host-side wall-clock arm.
EXEMPT_SUITES = {
    "stm32h747": "on-device timing — the measured region runs on the MCU, not on a host core",
    "wasm": "deterministic fuel counts — wasm_fuel.py publishes exact wasmtime fuel integers, "
            "which do not vary with the core that executes them; the suite times nothing",
}

# `. "$REPO_ROOT/scripts/bench_pin.sh"` and `source .../bench_pin.sh` both count.
SOURCE_TOKENS = ("bench_pin.sh",)
SOURCE_VERBS = (".", "source")


def sources_pin(text: str) -> bool:
    """True iff some line of `text` sources the pin helper.

    Matching the verb as well as the file name keeps a comment mentioning
    `bench_pin.sh` from satisfying the gate.
    """
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("#"):
            continue
        if not any(tok in line for tok in SOURCE_TOKENS):
            continue
        first = line.split(maxsplit=1)[0] if line.split() else ""
        if first in SOURCE_VERBS:
            return True
        # Inside a workflow the source line is one line of a `run:` block and
        # may be indented under YAML; the strip above already handled that.
    return False


def read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as e:
        print(f"::error::cannot read {path}: {e}", file=sys.stderr)
        raise SystemExit(1)


def check(root: Path) -> list[str]:
    problems: list[str] = []

    helper = root / PIN_HELPER
    if not helper.is_file():
        problems.append(f"{PIN_HELPER} is missing; nothing pins the wall-clock arms (#639).")
        return problems

    runners = sorted((root / "docs" / "benchmarks").glob("*/run.sh"))
    if not runners:
        problems.append("no docs/benchmarks/*/run.sh found — the discovery glob is wrong, not the tree.")
    for runner in runners:
        suite = runner.parent.name
        if suite in EXEMPT_SUITES:
            continue
        if not sources_pin(read(runner)):
            rel = runner.relative_to(root)
            problems.append(
                f"{rel} runs a wall-clock suite without sourcing {PIN_HELPER}. On the hybrid "
                f"reference host its arms can land on an efficiency core, which measures 1.576x "
                f"the P-core time (#639). Add `. \"$REPO_ROOT/{PIN_HELPER}\"` after the bench lock, "
                f"or record an exemption with its reason in EXEMPT_SUITES."
            )

    for wf in WORKFLOWS:
        path = root / wf
        if not path.is_file():
            problems.append(f"{wf} is missing; a wall-clock lane cannot be checked for the core pin.")
            continue
        if not sources_pin(read(path)):
            problems.append(
                f"{wf} runs benchmarks without sourcing {PIN_HELPER}. Source it once in the "
                f"bench step, after the host lock, so every `cargo bench` in that step inherits "
                f"the affinity (#639)."
            )

    return problems


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------
FAKE_TASKSET = """#!/bin/sh
# Stub `taskset` for the pin self-test. `-c -p <list> <pid>` records the list;
# `-c -p <pid>` reads it back. STATE names the file; FORCE_ACTUAL, when set,
# makes the read-back disagree with what was written, which is the "the pin did
# not hold" path.
if [ "$1" = "-c" ] && [ "$2" = "-p" ] && [ $# -eq 4 ]; then
    printf '%s' "$3" > "$STATE"
    exit ${SET_RC:-0}
fi
if [ "$1" = "-c" ] && [ "$2" = "-p" ] && [ $# -eq 3 ]; then
    if [ -n "${FORCE_ACTUAL:-}" ]; then
        echo "pid $3's current affinity list: $FORCE_ACTUAL"
    else
        echo "pid $3's current affinity list: $(cat "$STATE")"
    fi
    exit 0
fi
exit 2
"""


def _run_pin(tmp: Path, *, hybrid: bool, taskset: bool, env: dict[str, str]) -> subprocess.CompletedProcess:
    """Source bench_pin.sh against a synthetic topology and return the result."""
    sysfs = tmp / "sysfs"
    (sysfs / "cpu_core").mkdir(parents=True, exist_ok=True)
    (sysfs / "cpu_core" / "cpus").write_text("0-15\n", encoding="utf-8")
    if hybrid:
        (sysfs / "cpu_atom").mkdir(parents=True, exist_ok=True)
        (sysfs / "cpu_atom" / "cpus").write_text("16-23\n", encoding="utf-8")

    bindir = tmp / "bin"
    bindir.mkdir(exist_ok=True)
    stub = bindir / "taskset"
    if taskset:
        stub.write_text(FAKE_TASKSET, encoding="utf-8")
        stub.chmod(stub.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    elif stub.exists():
        stub.unlink()

    # PATH is *only* this directory. Filtering `taskset` out of the real PATH
    # instead would remove /usr/bin along with it on any host where the two
    # live together, taking sh/sed/awk/tr with it — which is how this self-test
    # first passed on a host without `taskset` and failed on one with it.
    _link_tools(bindir)
    path = str(bindir)

    full = {
        **os.environ,
        "PATH": path,
        "EXPANSE_PIN_SYSFS_ROOT": str(sysfs),
        "STATE": str(tmp / "affinity"),
        **env,
    }
    return subprocess.run(
        ["sh", "-c", f'. "{REPO_ROOT / PIN_HELPER}"; echo "APPLIED=$EXPANSE_BENCH_PIN_APPLIED"'],
        capture_output=True,
        text=True,
        env=full,
    )


# Everything `bench_pin.sh` reaches for outside the shell's own builtins. The
# self-test builds a PATH containing exactly these (plus, in the cases that
# want one, the stub `taskset`), so "no taskset on PATH" is a real condition
# rather than a side effect of editing the host's PATH.
REQUIRED_TOOLS = ("sh", "tr", "sort", "uniq", "sed", "awk", "cat")


def _link_tools(bindir: Path) -> None:
    """Symlink the tools bench_pin.sh needs into `bindir`."""
    for tool in REQUIRED_TOOLS:
        found = shutil.which(tool)
        if found is None:
            raise SystemExit(f"::error::self-test needs `{tool}` on PATH and it is absent")
        link = bindir / tool
        if not link.exists():
            link.symlink_to(found)


def self_test() -> None:
    # ---- the policy check ------------------------------------------------
    assert sources_pin('. "$REPO_ROOT/scripts/bench_pin.sh"')
    assert sources_pin('source "${REPO_ROOT}/scripts/bench_pin.sh"')
    assert not sources_pin("# see scripts/bench_pin.sh for why"), "a comment must not satisfy the gate"
    assert not sources_pin("cargo bench --bench domain"), "an unpinned runner must not pass"

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        tree = tmp / "repo"
        (tree / "docs" / "benchmarks" / "suite_a").mkdir(parents=True)
        (tree / ".github" / "workflows").mkdir(parents=True)
        (tree / "scripts").mkdir(parents=True)
        (tree / PIN_HELPER).write_text("# helper\n", encoding="utf-8")
        runner = tree / "docs" / "benchmarks" / "suite_a" / "run.sh"
        wfs = [tree / w for w in WORKFLOWS]

        # fail: nothing sources the pin
        runner.write_text("cargo bench --bench a\n", encoding="utf-8")
        for wf in wfs:
            wf.write_text("run: cargo bench --bench a\n", encoding="utf-8")
        problems = check(tree)
        assert len(problems) == 1 + len(wfs), problems
        assert any("suite_a/run.sh" in p for p in problems), problems
        assert any("bench_baremetal.yml" in p for p in problems), problems

        # pass: all of them do
        runner.write_text('. "$REPO_ROOT/scripts/bench_pin.sh"\ncargo bench --bench a\n', encoding="utf-8")
        for wf in wfs:
            wf.write_text('          . "$GITHUB_WORKSPACE/scripts/bench_pin.sh"\n', encoding="utf-8")
        assert check(tree) == [], check(tree)

        # a workflow that stops sourcing it is named, not tolerated
        wfs[0].write_text("run: cargo bench --bench a\n", encoding="utf-8")
        assert len(check(tree)) == 1 and "bench_baremetal.yml" in check(tree)[0], check(tree)
        wfs[0].write_text('          . "$GITHUB_WORKSPACE/scripts/bench_pin.sh"\n', encoding="utf-8")

        # an exempt suite is not required to pin, and says why in one place
        (tree / "docs" / "benchmarks" / "stm32h747").mkdir()
        (tree / "docs" / "benchmarks" / "stm32h747" / "run.sh").write_text("sh flash.sh\n", encoding="utf-8")
        assert check(tree) == [], check(tree)

        # a missing helper is the whole gate failing, not a pass
        (tree / PIN_HELPER).unlink()
        assert any("is missing" in p for p in check(tree)), check(tree)

    # ---- the helper's own refusal paths, on a synthetic topology ---------
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)

        # uniform host: nothing to pin away from, and no pin is applied
        r = _run_pin(tmp, hybrid=False, taskset=True, env={})
        assert r.returncode == 0 and "APPLIED=none" in r.stdout, (r.returncode, r.stdout, r.stderr)

        # hybrid host: pinned to the kernel's P-core list
        r = _run_pin(tmp, hybrid=True, taskset=True, env={})
        assert r.returncode == 0 and "APPLIED=0-15" in r.stdout, (r.returncode, r.stdout, r.stderr)

        # hybrid host with no taskset: refuses, and names the fix
        r = _run_pin(tmp, hybrid=True, taskset=False, env={})
        assert r.returncode == 1, (r.returncode, r.stdout, r.stderr)
        assert "util-linux" in r.stderr, r.stderr

        # the pin does not hold (the shell reports a different mask): refuses
        r = _run_pin(tmp, hybrid=True, taskset=True, env={"FORCE_ACTUAL": "0-23"})
        assert r.returncode == 1 and "did not hold" in r.stderr, (r.returncode, r.stderr)

        # taskset itself fails: refuses rather than measuring unpinned
        r = _run_pin(tmp, hybrid=True, taskset=True, env={"SET_RC": "1"})
        assert r.returncode == 1 and "could not be confined" in r.stderr, (r.returncode, r.stderr)

        # an explicit mask that includes efficiency cores is refused by name
        r = _run_pin(tmp, hybrid=True, taskset=True, env={"EXPANSE_BENCH_PIN": "0-23"})
        assert r.returncode == 1 and "efficiency" in r.stderr, (r.returncode, r.stderr)

        # the SMT-off knob: one sibling per physical P-core is accepted
        r = _run_pin(tmp, hybrid=True, taskset=True, env={"EXPANSE_BENCH_PIN": "0,2,4,6,8,10,12,14"})
        assert r.returncode == 0 and "APPLIED=0,2,4,6,8,10,12,14" in r.stdout, (r.returncode, r.stdout, r.stderr)

        # opting out is loud, and records that it opted out
        r = _run_pin(tmp, hybrid=True, taskset=True, env={"EXPANSE_BENCH_PIN": "off"})
        assert r.returncode == 0 and "APPLIED=off" in r.stdout, (r.returncode, r.stdout)
        assert "DISABLED" in r.stderr, r.stderr

    print("check_bench_pin.py --self-test: all checks passed")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return 0

    if shutil.which("sh") is None:  # pragma: no cover - defensive
        print("::error::no POSIX shell on PATH", file=sys.stderr)
        return 1

    problems = check(REPO_ROOT)
    for p in problems:
        print(f"::error::{p}", file=sys.stderr)
    if problems:
        print(f"\ncheck_bench_pin.py: {len(problems)} runner(s) do not take the core pin.", file=sys.stderr)
        return 1
    print("check_bench_pin.py: every wall-clock runner sources scripts/bench_pin.sh")
    return 0


if __name__ == "__main__":
    sys.exit(main())
