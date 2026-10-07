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

And four are about the gate itself:

  6. **Everything is behind the gate.** Every job other than `release-gate`
     needs it, directly or through its `needs`. No job or step condition uses
     `always()`, `failure()` or `cancelled()`, which would run it past a
     failed gate or a failed smoke test.
  7. **A condition is one of the known ones, exactly.** A pre-release guard
     is compared with the expressions listed in `JOB_GUARDS` and
     `STEP_GUARDS` after normalising whitespace. Containing the guard's text
     is not enough: `<guard> || true` contains it.
  8. **`promote` does what it says, in order.** Its named steps are exactly
     `PROMOTE_STEPS`: the Go tag first, while nothing is public, then the
     release, then PyPI.
  9. **The gate scripts are called as written.** Each line in `CALL_SITES`
     must appear in the workflow, so an argument cannot be dropped and a
     check cannot be swapped for `true` without this failing. The workflow's
     default permission is read-only, and a manual run cannot publish.

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
PROMOTE_STEPS = ("Push Go nested-module tag", "Publish the release", "Dispatch PyPI publish (python.yml)")
PRERELEASE_GUARD = "contains(github.ref_name, '-')"
_NOT_PRERELEASE_TAG = f"!(startsWith(github.ref, 'refs/tags/v') && {PRERELEASE_GUARD})"
# The only conditions accepted as excluding a pre-release tag, compared whole.
JOB_GUARDS = frozenset({
    _NOT_PRERELEASE_TAG,
    f"github.event_name == 'push' && {_NOT_PRERELEASE_TAG}",
})
STEP_GUARDS = frozenset({
    f"github.event_name == 'push' && !{PRERELEASE_GUARD}",
    f"!(github.event_name == 'workflow_dispatch' && inputs.dry_run) && !{PRERELEASE_GUARD}",
})
ESCAPING = ("always()", "failure()", "cancelled()")
# Lines the workflow must contain, as written: the gate scripts' call sites.
CALL_SITES = (
    "if: ${{ github.event_name == 'workflow_dispatch' && !inputs.dry_run }}",
    "on_main=(--require-ancestor-of main)",
    '--sha "${GITHUB_SHA}" \\',
    '"${on_main[@]}" \\',
    'python3 scripts/verify_tag_signature.py --tag "${GITHUB_REF_NAME}" --sha "${GITHUB_SHA}"',
    'python3 scripts/bump_version.py --check "${BASE_VERSION}"',
    "run: python3 scripts/release_assets.py check-local --dir artifacts",
    'python3 scripts/release_notes.py --tag "${GITHUB_REF_NAME}" --out curated-notes.md',
    "draft: true",
    "python3 scripts/release_assets.py check-readback --dir readback --local artifacts",
    'git push origin "${GITHUB_SHA}:refs/tags/bindings/go/${GITHUB_REF_NAME}"',
    "cancel-in-progress: false",
)
SHA_PINNED_RE = re.compile(r"^[^@\s]+@[0-9a-f]{40}$")
EXACT_VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
TOOLCHAIN_REF = "${{ env.RELEASE_TOOLCHAIN }}"
# The one job whose checkout keeps the token: it pushes the Go module tag.
KEEPS_CREDENTIALS = frozenset({"promote"})


def needs(job: dict) -> set[str]:
    n = job.get("needs", [])
    return {n} if isinstance(n, str) else set(n)


def normalise(condition) -> str:
    """A condition without its `${{ }}` wrapper and with single spaces."""
    text = " ".join(str(condition or "").split())
    if text.startswith("${{") and text.endswith("}}"):
        text = text[3:-2].strip()
    return text


def guarded(condition, allowed: frozenset = JOB_GUARDS | STEP_GUARDS) -> bool:
    """True when the condition is, exactly, one known to exclude a pre-release tag."""
    return normalise(condition) in allowed


def environment_name(job: dict):
    env = job.get("environment")
    return env.get("name") if isinstance(env, dict) else env


def reaches(jobs: dict, start: str, target: str) -> bool:
    seen, stack = set(), [start]
    while stack:
        cur = stack.pop()
        if cur == target:
            return True
        if cur in seen or cur not in jobs:
            continue
        seen.add(cur)
        stack.extend(needs(jobs[cur]))
    return False


def gate_problems(workflow: dict, text: str | None) -> list[str]:
    out: list[str] = []
    jobs = workflow.get("jobs", {})
    if workflow.get("permissions") != {"contents": "read"}:
        out.append(f"the workflow's default permissions are {workflow.get('permissions')!r}, not `contents: read`")
    if (workflow.get("concurrency") or {}).get("cancel-in-progress") is not False:
        out.append("the workflow has no `concurrency` group with `cancel-in-progress: false`")
    for name, job in sorted(jobs.items()):
        if name != "release-gate" and not reaches(jobs, name, "release-gate"):
            out.append(f"`{name}` does not need `release-gate`, directly or through its needs")
        if "uses" in job:
            out.append(f"`{name}` calls a reusable workflow, which this check does not read")
        for where, cond in [(f"`{name}`", job.get("if"))] + [
                (f"`{name}` step `{st.get('name') or st.get('uses')}`", st.get("if")) for st in job.get("steps", [])]:
            for word in ESCAPING:
                if word in str(cond or ""):
                    out.append(f"{where} uses `{word}`: it would run past a failed gate or smoke test")
    if text is not None:
        for line in CALL_SITES:
            if line not in text:
                out.append(f"the workflow no longer contains `{line}`")
    return out


def holds_credentials(job: dict) -> bool:
    """True for a job with a `write` permission or the `release` environment."""
    perms = job.get("permissions") or {}
    writes = perms == "write-all" or (isinstance(perms, dict) and "write" in perms.values())
    return writes or environment_name(job) == "release"


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


def problems(workflow: dict, text: str | None = None) -> list[str]:
    jobs = workflow.get("jobs", {})
    out: list[str] = supply_chain_problems(workflow) + gate_problems(workflow, text)
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
        if not guarded(jobs[name].get("if"), JOB_GUARDS):
            out.append(f"`{name}` has no job-level condition that excludes a pre-release tag")
    if jobs["promote"].get("if") is not None:
        out.append("`promote` has a job-level condition: it must run exactly when `smoke` succeeded")
    if not guarded(jobs["verify-registries"].get("if"), JOB_GUARDS):
        out.append("`verify-registries` has no condition that excludes a pre-release tag")
    for name, job in sorted(jobs.items()):
        looks_publishing = (environment_name(job) == "release"
                            or name.startswith(("publish-", "package-")))
        if looks_publishing and name not in PUBLISH_JOBS:
            out.append(f"`{name}` looks like a publishing job (release environment, or its name) "
                       f"and is not in PUBLISH_JOBS")
    named = [s for s in jobs["promote"].get("steps", []) if "run" in s]
    names = tuple(s.get("name") for s in named)
    if names != PROMOTE_STEPS:
        out.append(f"`promote` runs {list(names)}, not exactly {list(PROMOTE_STEPS)} in that order")
    for s in named:
        if s.get("name") in PROMOTE_STEPS and not guarded(s.get("if"), STEP_GUARDS):
            out.append(f"`promote` step `{s.get('name')}` would run for a pre-release tag")
    return out


def self_test() -> int:
    failures: list[str] = []
    real_path = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "release.yml"
    real_text = real_path.read_text(encoding="utf-8")
    real = yaml.safe_load(real_text)
    found = problems(real, real_text)
    if found:
        failures.append(f"the real release.yml has problems: {found}")

    wf_box: list = [None]  # the copy `check` is mutating, for `check_wf`

    def check(label: str, mutate, needle: str) -> None:
        """Requires that `mutate`, applied to the real job graph, is reported."""
        wf = copy.deepcopy(real)
        wf_box[0] = wf
        mutate(wf["jobs"])
        got = problems(wf, real_text)
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

    # --- the gate rules -----------------------------------------------------
    guard = real["jobs"]["publish-crates"]["if"]
    for label, cond in (
        ("a guard switched off with `|| true`", "${{ " + normalise(guard) + " || true }}"),
        ("a guard that is not negated", "${{ startsWith(github.ref, 'refs/tags/v') && contains(github.ref_name, '-') }}"),
        ("a guard negated twice", "${{ !" + normalise(guard) + " }}"),
        ("a guard inside a string", "${{ '" + normalise(guard).replace("'", "") + "' != '' }}"),
    ):
        check(label, lambda j, c=cond: j["publish-crates"].__setitem__("if", c),
              "`publish-crates` has no job-level condition")
    check("a guard that also runs past a failure",
          lambda j: j["publish-gem"].__setitem__("if", "${{ always() && " + normalise(guard) + " }}"),
          "`publish-gem` uses `always()`")
    check("promote running although smoke failed", lambda j: j["promote"].__setitem__("if", "${{ always() }}"),
          "`promote` has a job-level condition")
    check("a step that runs past a failure",
          lambda j: j["smoke"]["steps"][-1].__setitem__("if", "failure()"), "uses `failure()`")
    check("a job that is not behind the gate", drop_need("build-release-artifacts", "release-gate"),
          "`build-release-artifacts` does not need `release-gate`")
    check("a new job beside the gate",
          lambda j: j.__setitem__("deploy-crates", {"runs-on": "ubuntu-latest", "permissions": {"contents": "read"},
                                                     "steps": [{"run": "cargo publish"}]}),
          "`deploy-crates` does not need `release-gate`")
    check("the release environment in mapping form",
          lambda j: j.__setitem__("upload-elsewhere", {"needs": ["promote"], "environment": {"name": "release"}, "steps": []}),
          "`upload-elsewhere` looks like a publishing job")
    check("a job calling a reusable workflow",
          lambda j: j.__setitem__("reuse", {"needs": ["release-gate"], "uses": "o/r/.github/workflows/x.yml@main"}),
          "`reuse` calls a reusable workflow")
    check("an extra step in promote",
          lambda j: j["promote"]["steps"].append({"name": "Also publish", "run": "gh release edit --draft=false"}),
          "`promote` runs")
    check("a second step named like a guarded one",
          lambda j: j["promote"]["steps"].insert(2, {"name": "Publish the release", "run": "gh release edit --draft=false"}),
          "`promote` runs")

    def swap_promote(jobs):
        steps = jobs["promote"]["steps"]
        i = next(k for k, st in enumerate(steps) if st.get("name") == PROMOTE_STEPS[0])
        steps[i], steps[i + 1] = steps[i + 1], steps[i]

    check("the release published before the Go tag is pushed", swap_promote, "`promote` runs")
    check_wf("write-all as the workflow default", lambda w: w.__setitem__("permissions", "write-all"),
             "default permissions")
    check_wf("a write default with job permissions dropped",
             lambda w: w.__setitem__("permissions", {"contents": "write"}), "default permissions")
    check_wf("no concurrency group", lambda w: w.pop("concurrency"), "no `concurrency` group")
    check_wf("runs cancelled in progress",
             lambda w: w["concurrency"].__setitem__("cancel-in-progress", True), "no `concurrency` group")

    # --- call sites: the text of the workflow -------------------------------
    def check_text(label: str, old: str, new: str, needle: str) -> None:
        if real_text.count(old) < 1:
            failures.append(f"{label}: the fixture line {old!r} is not in release.yml")
            return
        text = real_text.replace(old, new)
        got = problems(yaml.safe_load(text), text)
        if not any(needle in p for p in got):
            failures.append(f"{label}: not reported (got {got})")

    for label, old, new in (
        ("the on-main assertion dropped", "on_main=(--require-ancestor-of main)", "on_main=()"),
        ("the gate run on another commit", '--sha "${GITHUB_SHA}" \\', '--sha "${OTHER_SHA}" \\'),
        ("the signature check replaced by true",
         'python3 scripts/verify_tag_signature.py --tag "${GITHUB_REF_NAME}" --sha "${GITHUB_SHA}"', "true"),
        ("the signature not bound to the commit",
         'python3 scripts/verify_tag_signature.py --tag "${GITHUB_REF_NAME}" --sha "${GITHUB_SHA}"',
         'python3 scripts/verify_tag_signature.py --tag "${GITHUB_REF_NAME}"'),
        ("the lockstep check replaced by true", 'python3 scripts/bump_version.py --check "${BASE_VERSION}"', "true"),
        ("the read-back compared with itself",
         "python3 scripts/release_assets.py check-readback --dir readback --local artifacts",
         "python3 scripts/release_assets.py check-readback --dir artifacts --local artifacts"),
        ("the release created published", "draft: true", "draft: false"),
        ("the Go tag forced", 'git push origin "${GITHUB_SHA}:refs/tags/bindings/go/${GITHUB_REF_NAME}"',
         'git push -f origin "${GITHUB_SHA}:refs/tags/bindings/go/${GITHUB_REF_NAME}"'),
        ("a manual run allowed to publish",
         "if: ${{ github.event_name == 'workflow_dispatch' && !inputs.dry_run }}", "if: ${{ false }}"),
        ("the notes read from another tag",
         'python3 scripts/release_notes.py --tag "${GITHUB_REF_NAME}" --out curated-notes.md',
         'python3 scripts/release_notes.py --tag v0.0.1 --out curated-notes.md'),
    ):
        check_text(label, old, new, "the workflow no longer contains")

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
    text = args.workflow.read_text(encoding="utf-8")
    found = problems(yaml.safe_load(text), text)
    for p in found:
        print(f"::error::{p}")
    if found:
        return 1
    print(f"check_release_workflow: {len(PUBLISH_JOBS)} publishing jobs wait for promote and skip a pre-release tag; "
          f"every job is behind the gate; {len(CALL_SITES)} call sites are as written; "
          f"actions are pinned by commit, checkouts drop the token, the toolchain is exact")
    return 0


if __name__ == "__main__":
    sys.exit(main())
