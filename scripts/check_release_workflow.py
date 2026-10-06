#!/usr/bin/env python3
"""Structural check of `release.yml`: nothing irreversible can run early or from a pre-release.

`release.yml` publishes to registries that cannot take a version back. Two
properties keep that safe, and both are properties of the job graph, which a
review of one changed job does not see:

  1. **Smoke before promote, promote before publish.** `promote` needs
     `smoke`; `smoke` needs `github-release`; every publishing job needs
     `promote`.
  2. **A pre-release publishes nothing.** Every publishing job, and
     `verify-registries`, carries a job-level condition that is false for a
     `vX.Y.Z-rc.N` tag, and the three steps of `promote` do too.

A publishing job is one declared in `PUBLISH_JOBS`. So that a new one cannot
be added outside the list, any job that enters the `release` environment, or
whose id starts with `publish-` or `package-`, must be in it.

Three further properties are about what a job can leak or pull in:

  3. **Every action is pinned by commit.** A `uses:` names a 40-hex commit,
     never a tag or a branch, which their owner can move.
  4. **No checkout leaves the token behind.** Every `actions/checkout` sets
     `persist-credentials: false`, except the one in `promote`, which pushes
     the Go module tag with it.
  5. **The compiler is pinned, and write jobs are observed.** No step asks for
     a toolchain channel (`stable`); each takes `env.RELEASE_TOOLCHAIN`, which
     is an exact version. Every job with a `write` permission or the `release`
     environment starts with `step-security/harden-runner`.

Run:  check_release_workflow.py [--workflow FILE]
      check_release_workflow.py --self-test
"""

from __future__ import annotations

import argparse
import copy
import re
import sys
from pathlib import Path

import yaml

PUBLISH_JOBS = frozenset({
    "publish-crates", "publish-npm", "publish-wasm", "publish-gem",
    "package-nuget", "package-maven", "publish-homebrew", "publish-pages",
})
PROMOTE_STEPS = ("Publish the release", "Push Go nested-module tag", "Dispatch PyPI publish (python.yml)")
# What a condition must contain to be false on a pre-release tag.
PRERELEASE_GUARD = "contains(github.ref_name, '-')"
SHA_PINNED_RE = re.compile(r"^[^@\s]+@[0-9a-f]{40}$")
EXACT_VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
TOOLCHAIN_REF = "${{ env.RELEASE_TOOLCHAIN }}"
# The one job whose checkout keeps the token: it pushes the Go module tag.
KEEPS_CREDENTIALS = frozenset({"promote"})


def needs(job: dict) -> set[str]:
    n = job.get("needs", [])
    return {n} if isinstance(n, str) else set(n)


def guarded(condition) -> bool:
    """True when the condition excludes a pre-release tag."""
    text = str(condition or "")
    return f"!{PRERELEASE_GUARD}" in text or f"!(startsWith(github.ref, 'refs/tags/v') && {PRERELEASE_GUARD})" in text


def holds_credentials(job: dict) -> bool:
    """True for a job with a `write` permission or the `release` environment."""
    perms = job.get("permissions") or {}
    writes = perms == "write-all" or (isinstance(perms, dict) and "write" in perms.values())
    return writes or job.get("environment") == "release"


def supply_chain_problems(workflow: dict) -> list[str]:
    out: list[str] = []
    pinned = str((workflow.get("env") or {}).get("RELEASE_TOOLCHAIN", ""))
    if not EXACT_VERSION_RE.match(pinned):
        out.append(f"`env.RELEASE_TOOLCHAIN` is {pinned!r}, not an exact version (X.Y.Z)")
    for name, job in sorted(workflow.get("jobs", {}).items()):
        steps = job.get("steps", [])
        for step in steps:
            uses = step.get("uses", "")
            label = step.get("name") or uses
            if uses and not uses.startswith("./") and not SHA_PINNED_RE.match(uses):
                out.append(f"`{name}`: `{uses}` is not pinned by commit")
            if uses.startswith("actions/checkout@"):
                persist = (step.get("with") or {}).get("persist-credentials")
                if name in KEEPS_CREDENTIALS:
                    if persist is not True:
                        out.append(f"`{name}`: its checkout must state `persist-credentials: true`")
                elif persist is not False:
                    out.append(f"`{name}`: a checkout without `persist-credentials: false`")
            toolchain = (step.get("with") or {}).get("toolchain")
            if toolchain is not None and toolchain != TOOLCHAIN_REF:
                out.append(f"`{name}`: step `{label}` installs toolchain {toolchain!r}, not RELEASE_TOOLCHAIN")
        if holds_credentials(job):
            first = steps[0].get("uses", "") if steps else ""
            if not first.startswith("step-security/harden-runner@"):
                out.append(f"`{name}` holds a write permission or the release environment "
                           f"and does not start with harden-runner")
    return out


def problems(workflow: dict) -> list[str]:
    jobs = workflow.get("jobs", {})
    out: list[str] = supply_chain_problems(workflow)
    missing = [r for r in ("github-release", "smoke", "promote", "verify-registries") if r not in jobs]
    if missing:
        return out + [f"job `{r}` is missing" for r in missing]
    if "github-release" not in needs(jobs["smoke"]):
        out.append("`smoke` does not need `github-release`")
    if "smoke" not in needs(jobs["promote"]):
        out.append("`promote` does not need `smoke`: the release could be published untested")
    for name in sorted(PUBLISH_JOBS):
        if name not in jobs:
            out.append(f"publishing job `{name}` is missing; update PUBLISH_JOBS if it was removed on purpose")
            continue
        if "promote" not in needs(jobs[name]):
            out.append(f"`{name}` does not need `promote`: it could publish before the smoke test")
        if not guarded(jobs[name].get("if")):
            out.append(f"`{name}` has no job-level condition that excludes a pre-release tag")
    if not guarded(jobs["verify-registries"].get("if")):
        out.append("`verify-registries` has no condition that excludes a pre-release tag")
    for name, job in sorted(jobs.items()):
        looks_publishing = (job.get("environment") == "release"
                            or name.startswith(("publish-", "package-")))
        if looks_publishing and name not in PUBLISH_JOBS:
            out.append(f"`{name}` looks like a publishing job (release environment, or its name) "
                       f"and is not in PUBLISH_JOBS")
    steps = {s.get("name"): s for s in jobs["promote"].get("steps", [])}
    for step in PROMOTE_STEPS:
        if step not in steps:
            out.append(f"`promote` has no step `{step}`")
        elif not guarded(steps[step].get("if")):
            out.append(f"`promote` step `{step}` would run for a pre-release tag")
    return out


def self_test() -> int:
    failures: list[str] = []
    real_path = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "release.yml"
    real = yaml.safe_load(real_path.read_text(encoding="utf-8"))
    found = problems(real)
    if found:
        failures.append(f"the real release.yml has problems: {found}")

    wf_box: list = [None]  # the copy `check` is mutating, for `check_wf`

    def check(label: str, mutate, needle: str) -> None:
        """Requires that `mutate`, applied to the real job graph, is reported."""
        wf = copy.deepcopy(real)
        wf_box[0] = wf
        mutate(wf["jobs"])
        got = problems(wf)
        if not any(needle in p for p in got):
            failures.append(f"{label}: not reported (got {got})")

    def drop_need(job: str, dep: str):
        def f(jobs):
            jobs[job]["needs"] = [n for n in jobs[job]["needs"] if n != dep]
        return f

    # THE FALSIFIERS OF THE ISSUE: a registry job whose needs do not include
    # the smoke path, and a registry that could receive a pre-release.
    check("a registry job that does not wait for promote", drop_need("publish-crates", "promote"),
           "`publish-crates` does not need `promote`")
    check("promote without smoke", drop_need("promote", "smoke"), "`promote` does not need `smoke`")
    check("smoke without the release job", drop_need("smoke", "github-release"), "`smoke` does not need")
    check("a registry job with no pre-release guard", lambda j: j["publish-npm"].pop("if"),
           "`publish-npm` has no job-level condition")
    check("a guard that does not mention pre-releases",
           lambda j: j["publish-gem"].__setitem__("if", "github.event_name == 'push'"),
           "`publish-gem` has no job-level condition")
    check("verify-registries on a pre-release",
           lambda j: j["verify-registries"].__setitem__("if", "github.event_name == 'push'"),
           "`verify-registries` has no condition")
    check("a new publishing job outside the list",
           lambda j: j.__setitem__("publish-conda", {"needs": ["promote"], "runs-on": "ubuntu-latest", "steps": []}),
           "`publish-conda` looks like a publishing job")
    check("a new job in the release environment",
           lambda j: j.__setitem__("upload-somewhere", {"environment": "release", "steps": []}),
           "`upload-somewhere` looks like a publishing job")

    def unguard_step(name: str):
        def f(jobs):
            for s in jobs["promote"]["steps"]:
                if s.get("name") == name:
                    s["if"] = "github.event_name == 'push'"
        return f

    for step in PROMOTE_STEPS:
        check(f"promote step `{step}` on a pre-release", unguard_step(step), f"`promote` step `{step}` would run")
    check("a removed publishing job", lambda j: j.pop("publish-wasm"), "publishing job `publish-wasm` is missing")

    def check_wf(label: str, mutate, needle: str) -> None:
        """As `check`, with `mutate` applied to the whole workflow."""
        check(label, lambda _jobs: mutate(wf_box[0]), needle)

    def step_of(wf, job: str, prefix: str) -> dict:
        return next(s for s in wf["jobs"][job]["steps"] if s.get("uses", "").startswith(prefix))

    check_wf("an action pinned by tag",
              lambda w: step_of(w, "smoke", "actions/checkout@").__setitem__("uses", "actions/checkout@v7"),
              "`actions/checkout@v7` is not pinned by commit")
    check_wf("an action pinned by a short commit",
              lambda w: step_of(w, "smoke", "actions/checkout@").__setitem__("uses", "actions/checkout@3d3c42e"),
              "is not pinned by commit")
    check_wf("a checkout that keeps the token",
              lambda w: step_of(w, "publish-npm", "actions/checkout@").pop("with"),
              "`publish-npm`: a checkout without `persist-credentials: false`")
    check_wf("promote's checkout no longer stating that it keeps the token",
              lambda w: step_of(w, "promote", "actions/checkout@").pop("with"),
              "`promote`: its checkout must state")
    check_wf("a toolchain channel",
              lambda w: step_of(w, "publish-crates", "dtolnay/rust-toolchain@")["with"].__setitem__("toolchain", "stable"),
              "installs toolchain 'stable'")
    check_wf("RELEASE_TOOLCHAIN set to a channel",
              lambda w: w["env"].__setitem__("RELEASE_TOOLCHAIN", "stable"), "not an exact version")
    check_wf("a write job without harden-runner",
              lambda w: w["jobs"]["github-release"]["steps"].pop(0), "`github-release` holds a write permission")
    check_wf("harden-runner not first",
              lambda w: w["jobs"]["promote"]["steps"].reverse(), "`promote` holds a write permission")
    check_wf("a read-only job gaining a write permission",
              lambda w: w["jobs"]["smoke"]["permissions"].__setitem__("contents", "write"),
              "`smoke` holds a write permission")

    if failures:
        for f in failures:
            print(f"::error::check_release_workflow self-test: {f}")
        return 1
    print("check_release_workflow --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--workflow", type=Path,
                    default=Path(__file__).resolve().parent.parent / ".github" / "workflows" / "release.yml")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    found = problems(yaml.safe_load(args.workflow.read_text(encoding="utf-8")))
    for p in found:
        print(f"::error::{p}")
    if found:
        return 1
    print(f"check_release_workflow: {len(PUBLISH_JOBS)} publishing jobs wait for promote and skip a pre-release tag; "
          f"actions are pinned by commit, checkouts drop the token, the toolchain is exact")
    return 0


if __name__ == "__main__":
    sys.exit(main())
