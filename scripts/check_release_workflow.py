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
     never a tag or a branch, which their owner can move. This holds for every
     file under `.github/workflows/` and every composite action under
     `.github/actions/`, not only `release.yml` (`all_pin_problems`); a
     `dtolnay/rust-toolchain` pinned by commit also carries a `toolchain:`
     input, because the action reads the channel from its ref otherwise.
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
     failed gate or a failed smoke test. One condition is exempt, compared
     whole: `verify-registries` runs on `!cancelled()` once `promote`
     succeeded (`VERIFY_REGISTRIES_GUARD`), so that a failed registry job
     does not take the report of every registry with it. It is read-only.
  7. **A condition is one of the known ones, exactly.** A pre-release guard
     is compared with the expressions listed in `JOB_GUARDS` and
     `STEP_GUARDS` after normalising whitespace. Containing the guard's text
     is not enough: `<guard> || true` contains it.
  8. **`promote` does what it says, in order.** Its named steps are exactly
     `PROMOTE_STEPS`: the Go tag first, while nothing is public, then the
     release, then PyPI.
  9. **A channel neither redeploys nor skips silently.** The steps in
     `STEP_CONDITIONS` carry exactly their listed condition: Maven Central's
     deploy is skipped only when the `central` probe, which runs first, found
     the version already served, and the tap push runs on every tag push.
     The probe's answers and the two unset-secret failures are call sites.
 10. **The gate scripts are called as written.** Each line in `CALL_SITES`
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
import shutil
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
# A job in the `release` environment runs only on a pushed tag: the environment
# admits release tags alone, and a canary dispatched on a branch is refused by it.
ENVIRONMENT_JOB_GUARDS = frozenset({f"github.event_name == 'push' && {_NOT_PRERELEASE_TAG}"})
STEP_GUARDS = frozenset({
    f"github.event_name == 'push' && !{PRERELEASE_GUARD}",
    f"!(github.event_name == 'workflow_dispatch' && inputs.dry_run) && !{PRERELEASE_GUARD}",
})
# Publishing jobs whose channel `verify-registries` does not probe (a Pages deploy
# is not a registry); every other one must be in its needs.
VERIFY_UNWATCHED = frozenset({"publish-pages"})
ESCAPING = ("always()", "failure()", "cancelled()")
# The one condition allowed to name `cancelled()`: `verify-registries` reports
# every registry even when one registry job failed, and only once the release
# is published, which keeps it behind the gate and the smoke job.
VERIFY_REGISTRIES_GUARD = (f"!cancelled() && needs.promote.result == 'success' && "
                           f"github.event_name == 'push' && {_NOT_PRERELEASE_TAG}")
# Steps whose condition is fixed, compared whole after normalising: (job, step
# name) -> condition, None for a step that must have none.
MAVEN_PUBLISH_STEP = "Publish to Maven Central"
MAVEN_PROBE_STEP = "Check whether Maven Central already serves this version"
MAVEN_PROBE_ID = "central"
STEP_CONDITIONS = {
    ("package-maven", MAVEN_PROBE_STEP): None,
    ("package-maven", MAVEN_PUBLISH_STEP):
        f"!(github.event_name == 'workflow_dispatch' && inputs.dry_run) && "
        f"steps.{MAVEN_PROBE_ID}.outputs.published != 'true'",
    ("publish-homebrew", "Push formula to orieg/homebrew-tap"): "github.event_name == 'push'",
}
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
    # Maven Central: probe the POM verify-registries reads; 200 skips the deploy.
    'url="https://repo1.maven.org/maven2/io/github/orieg/expanse-java/${version}/expanse-java-${version}.pom"',
    '200) echo "published=true" >> "${GITHUB_OUTPUT}"',
    '404) echo "published=false" >> "${GITHUB_OUTPUT}" ;;',
    # An unset secret fails the real-tag path; it never warns and exits 0.
    '[ -n "${MAVEN_CENTRAL_TOKEN:-}" ] || { echo "::error::MAVEN_CENTRAL_TOKEN is unset in the release environment: '
    'expanse-java was not published to Maven Central."; exit 1; }',
    '[ -n "${HOMEBREW_TAP_DEPLOY_KEY}" ] || { echo "::error::HOMEBREW_TAP_DEPLOY_KEY is unset in the release environment: '
    'the formula was not pushed to orieg/homebrew-tap (dist/expanse.rb is attached to the release)."; exit 1; }',
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
        if environment_name(job) == "release" and normalise(job.get("if")) not in ENVIRONMENT_JOB_GUARDS:
            out.append(f"`{name}` is in the `release` environment but runs on a manual canary: the environment "
                       f"refuses a branch, so the canary would end red with the job's steps unrun")
        exempt = name == "verify-registries" and normalise(job.get("if")) == VERIFY_REGISTRIES_GUARD
        for where, cond in [(f"`{name}`", None if exempt else job.get("if"))] + [
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
            if uses.startswith("dtolnay/rust-toolchain@"):
                toolchain = (step.get("with") or {}).get("toolchain")
                if toolchain != TOOLCHAIN_REF:
                    out.append(f"`{name}`: step `{label}` installs toolchain {toolchain!r}, not RELEASE_TOOLCHAIN")
            else:
                toolchain = (step.get("with") or {}).get("toolchain")
                if toolchain is not None and toolchain != TOOLCHAIN_REF:
                    out.append(f"`{name}`: step `{label}` installs toolchain {toolchain!r}, not RELEASE_TOOLCHAIN")
        if holds_credentials(job):
            first = steps[0].get("uses", "") if steps else ""
            if not first.startswith("step-security/harden-runner@"):
                out.append(f"`{name}` holds a write permission or the release environment "
                           f"and does not start with harden-runner")
    return out


PYTHON_WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "python.yml"


def python_pin_problems(workflow: dict, where: str = "python.yml") -> list[str]:
    """Checks the pin rules on python.yml: actions pinned by commit, persist-credentials: false, exact toolchain."""
    out: list[str] = []
    prefix = f"{where}: "
    pinned = str((workflow.get("env") or {}).get("RELEASE_TOOLCHAIN", ""))
    if not EXACT_VERSION_RE.match(pinned):
        out.append(f"{prefix}`env.RELEASE_TOOLCHAIN` is {pinned!r}, not an exact version (X.Y.Z)")
    for name, job in sorted(workflow.get("jobs", {}).items()):
        steps = job.get("steps", [])
        for step in steps:
            uses = step.get("uses", "")
            label = step.get("name") or uses
            if uses and not uses.startswith("./") and not SHA_PINNED_RE.match(uses):
                out.append(f"{prefix}`{name}`: `{uses}` is not pinned by commit")
            if uses.startswith("actions/checkout@"):
                persist = (step.get("with") or {}).get("persist-credentials")
                if persist is not False:
                    out.append(f"{prefix}`{name}`: a checkout without `persist-credentials: false`")
            if uses.startswith("dtolnay/rust-toolchain@"):
                toolchain = (step.get("with") or {}).get("toolchain")
                if toolchain != TOOLCHAIN_REF:
                    out.append(f"{prefix}`{name}`: step `{label}` installs toolchain {toolchain!r}, not RELEASE_TOOLCHAIN")
            else:
                toolchain = (step.get("with") or {}).get("toolchain")
                if toolchain is not None and toolchain != TOOLCHAIN_REF:
                    out.append(f"{prefix}`{name}`: step `{label}` installs toolchain {toolchain!r}, not RELEASE_TOOLCHAIN")
    return out


REPO_ROOT = Path(__file__).resolve().parent.parent
# release.yml and python.yml are held to the same rule, with more, by
# `supply_chain_problems` and `python_pin_problems`; reporting a tag there twice
# would only repeat the line.
PIN_COVERED_ELSEWHERE = frozenset({"release.yml", "python.yml"})


def pin_problems_in(doc: dict, where: str) -> list[str]:
    """Every `uses:` in one workflow or composite action is pinned by commit.

    A reusable-workflow call (`jobs.<id>.uses`) and a composite action's
    `runs.steps` are covered with the ordinary `jobs.<id>.steps`. A local
    `./path` action is repository content, not a third-party ref.
    """
    out: list[str] = []
    units: list[tuple[str, list[dict]]] = []
    for name, job in sorted((doc.get("jobs") or {}).items()):
        if isinstance(job.get("uses"), str):
            units.append((name, [{"uses": job["uses"]}]))
        units.append((name, job.get("steps") or []))
    units.append(("runs", (doc.get("runs") or {}).get("steps") or []))
    for name, steps in units:
        for step in steps:
            uses = step.get("uses", "")
            if not uses or uses.startswith("./"):
                continue
            if not SHA_PINNED_RE.match(uses):
                out.append(f"{where}: `{name}`: `{uses}` is not pinned by commit")
            elif uses.startswith("dtolnay/rust-toolchain@") and not (step.get("with") or {}).get("toolchain"):
                out.append(f"{where}: `{name}`: `{uses}` has no `toolchain:` input; "
                           f"a commit pin carries no channel for the action to read")
    return out


def all_pin_problems(root: Path = REPO_ROOT) -> tuple[list[str], int]:
    """Pin problems over every workflow and composite action; also the file count."""
    paths = sorted((root / ".github" / "workflows").glob("*.yml"))
    paths += sorted((root / ".github" / "actions").glob("*/action.yml"))
    out: list[str] = []
    for path in paths:
        if path.parent.name == "workflows" and path.name in PIN_COVERED_ELSEWHERE:
            continue
        out += pin_problems_in(yaml.safe_load(path.read_text(encoding="utf-8")) or {},
                               str(path.relative_to(root)))
    return out, len(paths)


NPM_GLOBAL_RE = re.compile(r"npm install -g (\S+)")
EXACT_NPM_SPEC_RE = re.compile(r"^@?[^@\s]+@\d+\.\d+\.\d+$")


def toolchain_problems(workflow: dict, text: str | None) -> list[str]:
    """Tools the release installs are exact versions, and cargo builds are locked.

    `npm@latest`, a bare `wasm-pack`, and a range such as `npm@12` each resolve
    to whatever the registry holds on the day, so a rebuilt tag need not use
    the tools its canary ran. An unlocked `cargo build` or `cargo publish`
    resolves dependencies afresh instead of from `Cargo.lock`; the glibc-hwcaps
    variants that `build_hwcaps.sh` builds ship in the release archives, so it
    is held to the same rule (`main`).
    """
    out: list[str] = []
    if text is None:
        return out
    out += npm_global_problems(text, "")
    return out + lock_problems(text, "release.yml")


def npm_global_problems(text: str, where: str) -> list[str]:
    """Every `npm install -g` in `text` names an exact X.Y.Z version."""
    prefix = f"{where}: " if where else ""
    return [f"{prefix}`npm install -g {m.group(1)}` is not pinned to an exact X.Y.Z version"
            for m in NPM_GLOBAL_RE.finditer(text) if not EXACT_NPM_SPEC_RE.match(m.group(1))]


def all_npm_problems(root: Path = REPO_ROOT) -> list[str]:
    """`npm_global_problems` over every workflow and composite action but release.yml.

    release.yml is read by `toolchain_problems`; reporting it here too would
    repeat the line.
    """
    paths = sorted((root / ".github" / "workflows").glob("*.yml"))
    paths += sorted((root / ".github" / "actions").glob("*/action.yml"))
    out: list[str] = []
    for path in paths:
        if path.parent.name == "workflows" and path.name == "release.yml":
            continue
        out += npm_global_problems(path.read_text(encoding="utf-8"), str(path.relative_to(root)))
    return out


PIP_INSTALL_RE = re.compile(r"(?<![\w-])pip3? install\b(.*)")


def pip_hash_problems(text: str, where: str) -> list[str]:
    """Every `pip install` in `text` carries `--require-hashes`.

    A requirement without a hash is resolved against the index on the day, so a
    rebuilt job need not install what its last run did, and a substituted
    release is not noticed. The tools come from `.github/requirements/*.txt`,
    compiled with `--generate-hashes`. The one install that cannot take hashes
    is the repository itself (`pip install .`): `--require-hashes` rejects a
    local path, and its build requirements come from `pyproject.toml`. A line
    that names anything else besides the path, or any `-r` file, is held to the
    rule. Backslash continuations are joined first; comments and `echo` lines
    are skipped.
    """
    out: list[str] = []
    logical = re.sub(r"\\\n\s*", " ", text)
    for line in logical.splitlines():
        stripped = line.strip()
        if stripped.startswith(("#", "echo ", "printf ")):
            continue
        found = PIP_INSTALL_RE.search(stripped)
        if not found:
            continue
        args = found.group(1).split("#")[0].split()
        if "--require-hashes" in args:
            continue
        operands: list[str] = []
        skip = False
        for a in args:
            if skip:
                skip = False
            elif a in ("-r", "-c", "--requirement", "--constraint"):
                operands.append(a)
                skip = True
            elif not a.startswith("-"):
                operands.append(a)
        if operands and all(o == "." or o.startswith(("./", "../")) for o in operands):
            continue
        prefix = f"{where}: " if where else ""
        out.append(f"{prefix}`pip install` without `--require-hashes`: {stripped}")
    return out


def all_pip_problems(root: Path = REPO_ROOT) -> list[str]:
    """`pip_hash_problems` over every workflow and composite action."""
    paths = sorted((root / ".github" / "workflows").glob("*.yml"))
    paths += sorted((root / ".github" / "actions").glob("*/action.yml"))
    out: list[str] = []
    for path in paths:
        out += pip_hash_problems(path.read_text(encoding="utf-8"), str(path.relative_to(root)))
    return out


# The resolved sets of the python-bindings requirements, which must hold on
# every Python and OS the jobs that install them run on. pip-compile drops a
# requirement whose environment marker is false on the compiling machine, and
# Dependabot compiles on its own Python, so the dependencies that hold only on
# some Python or OS (colorama: Windows; exceptiongroup, tomli: Python < 3.11;
# typing-extensions: exceptiongroup on Python < 3.13) are listed in each .in
# unconditionally, and the set of each .txt is compared with the one declared
# here exactly. A change to a set is a change to what every job on that file
# installs.
#
# Two files: the main one serves Python 3.10 and newer with a pytest that
# carries the fix for GHSA-6w46-j5rx-g56g; the py38 one serves only the
# Python 3.8 row of python.yml and goes away with it.
REQUIREMENTS_DIR_NAME = ".github/requirements"
PYTHON_BINDINGS_MAIN = "python-bindings"
PYTHON_BINDINGS_PY38 = "py38/python-bindings-py38"
PYTHON_BINDINGS_UNCONDITIONAL = frozenset({"colorama", "exceptiongroup", "tomli", "typing-extensions"})
PYTHON_BINDINGS_COMMON = frozenset({"colorama", "exceptiongroup", "iniconfig", "maturin", "packaging",
                                    "pluggy", "pytest", "tomli", "typing-extensions"})
PYTHON_BINDINGS_EXPECTED = {
    PYTHON_BINDINGS_MAIN: PYTHON_BINDINGS_COMMON | {"pygments"},  # pytest 8.4+ requires pygments
    PYTHON_BINDINGS_PY38: PYTHON_BINDINGS_COMMON,
}
# The oldest Python the main file serves. The py38 file serves the oldest
# python-version in python.yml's matrix, read from the workflow.
MAIN_PYTHON_FLOOR = (3, 10)
# First pytest release with the fix for GHSA-6w46-j5rx-g56g (CVE-2025-71176):
# the main file must not pin an older one.
PYTEST_FIXED = (9, 0, 3)
# Releases of a pinned package that need a newer Python than the file serves:
# (first release, the Python it requires), from the package's `requires_python`
# on PyPI. A pin stays below the first release whose Python is above the
# file's floor.
PYTHON_FLOOR_CEILINGS = {
    "pytest": (((8, 4), (3, 9)), ((9, 0), (3, 10))),
    "typing-extensions": (((4, 14), (3, 9)),),
}
# Install sites outside python.yml that take the py38 file, with the reason:
# a site whose interpreter the workflow does not set cannot take the main file.
PY38_FILE_SITES = {
    "bench_baremetal.yml": "the benchmark host's python3 is not set by the workflow; no pytest runs there",
}
REQUIREMENT_RE = re.compile(r"^([A-Za-z0-9][A-Za-z0-9._-]*)==(\d+(?:\.\d+)*)\b")
REQUIREMENT_PATH_RE = re.compile(r"\.github/requirements/((?:py38/)?python-bindings(?:-py38)?)\.txt")


def requirement_pins(text: str) -> dict[str, tuple[int, ...]]:
    """`name==version` lines of a requirements `.in` or `.txt` file, names normalised."""
    out: dict[str, tuple[int, ...]] = {}
    for line in text.splitlines():
        found = REQUIREMENT_RE.match(line)
        if found:
            name = re.sub(r"[-_.]+", "-", found.group(1)).lower()
            out[name] = tuple(int(x) for x in found.group(2).split("."))
    return out


def python_versions(value) -> list[tuple[int, ...]]:
    return [tuple(int(x) for x in str(v).split(".")) for v in (value or [])]


def matrix_python_floor(workflow: dict) -> tuple[int, ...] | None:
    """The oldest `python-version` in any job matrix of `workflow`, or None."""
    versions: list[tuple[int, ...]] = []
    for job in (workflow.get("jobs") or {}).values():
        matrix = ((job.get("strategy") or {}).get("matrix") or {})
        versions += python_versions(matrix.get("python-version"))
    return min(versions) if versions else None


def dotted(version: tuple[int, ...]) -> str:
    return ".".join(map(str, version))


def python_requirements_problems(in_text: str, txt_text: str, floor: tuple[int, ...] | None,
                                 which: str = PYTHON_BINDINGS_MAIN) -> list[str]:
    """One requirements pair agrees with itself, holds every Python it serves, and respects its Python floor."""
    out: list[str] = []
    where = which.rsplit("/", 1)[-1]
    expected = PYTHON_BINDINGS_EXPECTED[which]
    pins_in, pins_txt = requirement_pins(in_text), requirement_pins(txt_text)
    missing = sorted(expected - pins_txt.keys())
    extra = sorted(pins_txt.keys() - expected)
    if missing or extra:
        out.append(f"{where}.txt resolves {sorted(pins_txt)}, not the expected set: "
                   f"missing {missing}, unexpected {extra}; a compile on a newer Python drops the "
                   f"requirements whose marker is false there, so list them in {where}.in")
    for name in sorted(PYTHON_BINDINGS_UNCONDITIONAL - pins_in.keys()):
        out.append(f"{where}.in does not list `{name}`, a dependency that holds only on some Python or OS")
    for name, version in sorted(pins_in.items()):
        if pins_txt.get(name) != version:
            have = dotted(pins_txt[name]) if name in pins_txt else "nothing"
            out.append(f"{where}.in pins {name}=={dotted(version)} but {where}.txt has {have}")
    if floor is None:
        out.append(f"python.yml has no python-version matrix, so the Python floor of {where}.in is unknown")
        return out
    for name, ceilings in sorted(PYTHON_FLOOR_CEILINGS.items()):
        pinned = pins_in.get(name)
        for first, needs_python in ceilings:
            if pinned is not None and floor < needs_python and pinned[:len(first)] >= first:
                out.append(f"{where}.in pins {name}=={dotted(pinned)}, which needs Python "
                           f"{dotted(needs_python)}+, but the file serves Python {dotted(floor)}: "
                           f"keep {name} below {dotted(first)}")
    if which == PYTHON_BINDINGS_MAIN:
        pytest_pin = pins_in.get("pytest")
        if pytest_pin is None or pytest_pin < PYTEST_FIXED:
            out.append(f"{where}.in pins pytest=={dotted(pytest_pin) if pytest_pin else 'nothing'}, below "
                       f"{dotted(PYTEST_FIXED)}, the first release that fixes GHSA-6w46-j5rx-g56g")
    return out


def python_requirements_sites_problems(workflows: dict[str, dict], texts: dict[str, str]) -> list[str]:
    """Each install site of the python-bindings requirements takes the file that matches its Python.

    `workflows` maps a workflow file name to its parsed YAML, `texts` to its text.
    In python.yml the file is chosen by the matrix value: the oldest version gets
    the py38 file and every other row the main one; no row between them may
    exist. Every other site takes the main file and sets up a Python at or
    above its floor, unless it is a declared py38 site.
    """
    out: list[str] = []
    py_wf = workflows.get("python.yml") or {}
    floor = matrix_python_floor(py_wf)
    for name, text in sorted(texts.items()):
        refs = sorted(set(REQUIREMENT_PATH_RE.findall(text)))
        if not refs:
            continue
        if name == "python.yml":
            if floor is None:
                out.append("python.yml installs python-bindings requirements but has no python-version matrix")
                continue
            selector = (f"matrix.python-version == '{dotted(floor)}' && "
                        f"'{REQUIREMENTS_DIR_NAME}/{PYTHON_BINDINGS_PY38}.txt' || "
                        f"'{REQUIREMENTS_DIR_NAME}/{PYTHON_BINDINGS_MAIN}.txt'")
            if selector not in text:
                out.append(f"python.yml does not choose the requirements file by the matrix value as "
                           f"`{selector}`: the Python {dotted(floor)} row takes the py38 file and every other row the main one")
            for version in {v for job in (py_wf.get("jobs") or {}).values()
                            for v in python_versions(((job.get("strategy") or {}).get("matrix") or {}).get("python-version"))}:
                if floor < version < MAIN_PYTHON_FLOOR:
                    out.append(f"python.yml tests Python {dotted(version)}, which neither the py38 file "
                               f"(Python {dotted(floor)} only) nor the main file (Python {dotted(MAIN_PYTHON_FLOOR)}+) serves")
            continue
        for ref in refs:
            if ref == PYTHON_BINDINGS_PY38 and name not in PY38_FILE_SITES:
                out.append(f"{name} installs the py38 requirements file, which only the Python {dotted(floor or (3, 8))} "
                           f"row of python.yml may use")
            if ref == PYTHON_BINDINGS_MAIN and name in PY38_FILE_SITES:
                out.append(f"{name} installs the main requirements file, but {PY38_FILE_SITES[name]}")
        if PYTHON_BINDINGS_MAIN in refs and name not in PY38_FILE_SITES:
            for job_name, job in sorted((workflows.get(name) or {}).get("jobs", {}).items()):
                steps = job.get("steps") or []
                if not any(f"{PYTHON_BINDINGS_MAIN}.txt" in str(s.get("run", "")) for s in steps):
                    continue
                versions = [v for s in steps if str(s.get("uses", "")).startswith("actions/setup-python@")
                            for v in python_versions([(s.get("with") or {}).get("python-version")])]
                if not versions or min(versions) < MAIN_PYTHON_FLOOR:
                    out.append(f"{name}: `{job_name}` installs the main requirements file, which serves Python "
                               f"{dotted(MAIN_PYTHON_FLOOR)}+, on Python {[dotted(v) for v in versions] or 'unset'}")
    return out


def all_requirements_problems(root: Path = REPO_ROOT) -> list[str]:
    req = root / ".github" / "requirements"
    wf_dir = root / ".github" / "workflows"
    workflow = yaml.safe_load((wf_dir / "python.yml").read_text(encoding="utf-8"))
    floor = matrix_python_floor(workflow)
    pairs = {w: ((req / f"{w}.in").read_text(encoding="utf-8"), (req / f"{w}.txt").read_text(encoding="utf-8"))
             for w in (PYTHON_BINDINGS_MAIN, PYTHON_BINDINGS_PY38)}
    out = python_requirements_problems(*pairs[PYTHON_BINDINGS_MAIN], MAIN_PYTHON_FLOOR, PYTHON_BINDINGS_MAIN)
    out += python_requirements_problems(*pairs[PYTHON_BINDINGS_PY38], floor, PYTHON_BINDINGS_PY38)
    maturin = {w: requirement_pins(pairs[w][0]).get("maturin") for w in pairs}
    if len(set(maturin.values())) != 1:
        out.append(f"the two python-bindings files pin different maturin versions: {maturin}")
    paths = sorted(wf_dir.glob("*.yml"))
    texts = {p.name: p.read_text(encoding="utf-8") for p in paths}
    return out + python_requirements_sites_problems({n: yaml.safe_load(t) or {} for n, t in texts.items()}, texts)


def lock_problems(text: str, where: str) -> list[str]:
    """Every `cargo build` and `cargo publish` in `text` names `--locked` on its own line."""
    out: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(("#", "echo ", "printf ")):
            continue
        # A command, not a word inside a message: `::error::cargo publish failed`
        # is preceded by a colon.
        found = re.search(r"(?<![\w:/.\"'-])cargo (build|publish)\b", stripped)
        if found and "--locked" not in stripped:
            out.append(f"{where}: `cargo {found.group(1)}` without `--locked`: {stripped}")
    return out


HWCAPS_SCRIPT = Path(__file__).resolve().parent / "build_hwcaps.sh"


def hwcaps_problems(workflow_text: str) -> list[str]:
    """The script the release calls to build the glibc-hwcaps variants is read too."""
    if "build_hwcaps.sh" not in workflow_text:
        return []
    if not HWCAPS_SCRIPT.is_file():
        return [f"release.yml calls {HWCAPS_SCRIPT.name}, which does not exist"]
    return lock_problems(HWCAPS_SCRIPT.read_text(encoding="utf-8"), HWCAPS_SCRIPT.name)


PUBLISH_COMMAND_RE = re.compile(
    r"(?<![\w-])(?:cargo\s+publish|npm\s+publish|pnpm\s+publish|yarn\s+publish|gem\s+push"
    r"|dotnet\s+nuget\s+push|mvn\b[^\n|;&]*?\bdeploy|twine\s+upload|maturin\s+(?:upload|publish)"
    r"|docker\s+push)(?![\w-])"
)


def publish_command_problems(workflow: dict) -> list[str]:
    """A registry publish command in a `run:` body of a job not in PUBLISH_JOBS.

    The job-level checks read `needs`, `if` and the job name, so a publish
    command added to a step of `smoke` or `github-release` would run with none
    of the pre-release guard or the `promote` ordering. Comment lines and
    `echo` lines are skipped: they mention a command without running it.
    """
    out: list[str] = []
    for name, job in sorted((workflow.get("jobs") or {}).items()):
        if name in PUBLISH_JOBS:
            continue
        for st in job.get("steps") or []:
            body = st.get("run")
            if not isinstance(body, str):
                continue
            for line in body.splitlines():
                code = line.strip()
                if not code or code.startswith(("#", "echo ", "printf ")):
                    continue
                m = PUBLISH_COMMAND_RE.search(code)
                if m:
                    out.append(f"`{name}` is not a publishing job but step `{st.get('name', '?')}` runs "
                               f"`{m.group(0)}`; move it to a job in PUBLISH_JOBS")
    return out


def problems(workflow: dict, text: str | None = None) -> list[str]:
    jobs = workflow.get("jobs", {})
    out: list[str] = supply_chain_problems(workflow) + gate_problems(workflow, text) + toolchain_problems(workflow, text)
    out += publish_command_problems(workflow)
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
    if normalise(jobs["verify-registries"].get("if")) != VERIFY_REGISTRIES_GUARD:
        out.append("`verify-registries` has no condition that excludes a pre-release tag and still reports "
                   f"every registry when one registry job failed: expected exactly `{VERIFY_REGISTRIES_GUARD}`")
    for name in sorted(PUBLISH_JOBS - VERIFY_UNWATCHED):
        if name in jobs and name not in needs(jobs["verify-registries"]):
            out.append(f"`verify-registries` does not need `{name}`: it would report before that channel finished")
    if "promote" not in needs(jobs["verify-registries"]):
        out.append("`verify-registries` does not need `promote`, which its condition reads")
    for (job_name, step_name), want in STEP_CONDITIONS.items():
        steps = (jobs.get(job_name) or {}).get("steps", [])
        found = [k for k, st in enumerate(steps) if st.get("name") == step_name]
        if len(found) != 1:
            out.append(f"`{job_name}` has {len(found)} steps named `{step_name}`, not exactly one")
            continue
        got = steps[found[0]].get("if")
        if (None if got is None else normalise(got)) != want:
            out.append(f"`{job_name}` step `{step_name}` runs on {got!r}, not exactly {want!r}")
    maven = (jobs.get("package-maven") or {}).get("steps", [])
    order = {st.get("name"): k for k, st in enumerate(maven)}
    probe = next((st for st in maven if st.get("name") == MAVEN_PROBE_STEP), {})
    if probe.get("id") != MAVEN_PROBE_ID:
        out.append(f"`package-maven` step `{MAVEN_PROBE_STEP}` does not have id `{MAVEN_PROBE_ID}`, "
                   f"which the deploy's condition reads")
    if order.get(MAVEN_PROBE_STEP, 1 << 30) > order.get(MAVEN_PUBLISH_STEP, -1):
        out.append(f"`package-maven` does not probe Maven Central before `{MAVEN_PUBLISH_STEP}`: "
                   f"a re-run would deploy a version Central already holds")
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
    # verify-registries reports every registry even when one registry job failed.
    check("verify-registries skipped whenever one registry job fails",
          lambda j: j["verify-registries"].__setitem__(
              "if", "${{ github.event_name == 'push' && " + _NOT_PRERELEASE_TAG + " }}"),
          "`verify-registries` has no condition")
    check("verify-registries running although the release was never published",
          lambda j: j["verify-registries"].__setitem__(
              "if", "${{ " + VERIFY_REGISTRIES_GUARD.replace("needs.promote.result == 'success' && ", "") + " }}"),
          "`verify-registries` has no condition")
    check("verify-registries on always() instead of !cancelled()",
          lambda j: j["verify-registries"].__setitem__(
              "if", "${{ " + VERIFY_REGISTRIES_GUARD.replace("!cancelled()", "always()") + " }}"),
          "`verify-registries` uses `always()`")
    check("the !cancelled() exemption borrowed by a publishing job",
          lambda j: j["publish-gem"].__setitem__("if", "${{ !cancelled() && " + _NOT_PRERELEASE_TAG + " }}"),
          "`publish-gem` uses `cancelled()`")
    check("verify-registries no longer waiting for the tap push", drop_need("verify-registries", "publish-homebrew"),
          "`verify-registries` does not need `publish-homebrew`")
    check("verify-registries no longer needing promote", drop_need("verify-registries", "promote"),
          "`verify-registries` does not need `promote`")

    # Maven Central: a re-run skips a version Central already serves.
    def maven_step(jobs, name):
        return next(s for s in jobs["package-maven"]["steps"] if s.get("name") == name)

    check("the Maven deploy no longer reading the probe",
          lambda j: maven_step(j, MAVEN_PUBLISH_STEP).__setitem__(
              "if", "${{ !(github.event_name == 'workflow_dispatch' && inputs.dry_run) }}"),
          f"`package-maven` step `{MAVEN_PUBLISH_STEP}` runs on")
    check("the Maven probe removed",
          lambda j: j["package-maven"]["steps"].remove(maven_step(j, MAVEN_PROBE_STEP)),
          f"`package-maven` has 0 steps named `{MAVEN_PROBE_STEP}`")
    check("the Maven probe given a condition",
          lambda j: maven_step(j, MAVEN_PROBE_STEP).__setitem__("if", "false"),
          f"`package-maven` step `{MAVEN_PROBE_STEP}` runs on")
    check("the Maven probe under another id",
          lambda j: maven_step(j, MAVEN_PROBE_STEP).__setitem__("id", "probe"),
          "does not have id `central`")

    def probe_after_deploy(jobs):
        steps = jobs["package-maven"]["steps"]
        probe = maven_step(jobs, MAVEN_PROBE_STEP)
        steps.remove(probe)
        steps.insert(steps.index(maven_step(jobs, MAVEN_PUBLISH_STEP)) + 1, probe)

    check("the Maven probe moved after the deploy", probe_after_deploy,
          "`package-maven` does not probe Maven Central before")
    check("the tap push switched off",
          lambda j: next(s for s in j["publish-homebrew"]["steps"]
                         if s.get("name") == "Push formula to orieg/homebrew-tap").__setitem__("if", "false"),
          "`publish-homebrew` step `Push formula to orieg/homebrew-tap` runs on")
    check("a publish command added to a non-publishing job",
          lambda j: j["smoke"]["steps"].append({"name": "Sneak", "run": "set -e\ncargo publish --locked -p x"}),
          "`smoke` is not a publishing job but step `Sneak` runs `cargo publish`")
    check("mvn deploy added to github-release",
          lambda j: j["github-release"]["steps"].append({"name": "Sneak", "run": "mvn -f p.xml -P release deploy"}),
          "`github-release` is not a publishing job")
    check("npm publish added to promote",
          lambda j: j["promote"]["steps"].append({"name": "Sneak", "run": "npm publish ./x --access public"}),
          "`promote` is not a publishing job")
    for harmless in ("# cargo publish later", 'echo "cargo publish failed"', "echo mvn deploy", "cargo package --list"):
        if publish_command_problems({"jobs": {"smoke": {"steps": [{"name": "x", "run": harmless}]}}}):
            failures.append(f"{harmless!r} was reported as a publish command")
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
    for job in ("publish-crates", "package-maven", "publish-homebrew"):
        check(f"{job}, in the release environment, left running on a manual canary",
              lambda j, job=job: j[job].__setitem__("if", "${{ " + _NOT_PRERELEASE_TAG + " }}"),
              f"`{job}` is in the `release` environment but runs on a manual canary")
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
        ("the Maven probe never reporting a published version",
         '200) echo "published=true" >> "${GITHUB_OUTPUT}"', '200) echo "published=false" >> "${GITHUB_OUTPUT}"'),
        ("the Maven probe deploying when it could not tell",
         '404) echo "published=false" >> "${GITHUB_OUTPUT}" ;;', '*) echo "published=false" >> "${GITHUB_OUTPUT}" ;;'),
        ("the Maven probe reading another URL",
         'url="https://repo1.maven.org/maven2/io/github/orieg/expanse-java/${version}/expanse-java-${version}.pom"',
         'url="https://search.maven.org/solrsearch/select?q=expanse-java"'),
        ("an unset Maven token skipped with exit 0",
         'expanse-java was not published to Maven Central."; exit 1; }',
         'expanse-java was not published to Maven Central."; exit 0; }'),
        ("an unset tap key skipped with exit 0",
         '(dist/expanse.rb is attached to the release)."; exit 1; }',
         '(dist/expanse.rb is attached to the release)."; exit 0; }'),
    ):
        check_text(label, old, new, "the workflow no longer contains")

    # --- toolchain pinning --------------------------------------------------
    for label, old, new, needle in (
        ("npm@latest", "npm install -g npm@12.2.0", "npm install -g npm@latest",
         "`npm install -g npm@latest` is not pinned"),
        ("npm given as a range", "npm install -g npm@12.2.0", "npm install -g npm@12",
         "`npm install -g npm@12` is not pinned"),
        ("wasm-pack@latest", "npm install -g wasm-pack@0.15.0", "npm install -g wasm-pack@latest",
         "`npm install -g wasm-pack@latest` is not pinned"),
        ("wasm-pack with no version", "npm install -g wasm-pack@0.15.0", "npm install -g wasm-pack",
         "`npm install -g wasm-pack` is not pinned"),
        ("cargo build without --locked", "--release --locked --target", "--release --target",
         "`cargo build` without `--locked`"),
        ("cargo publish without --locked", "cargo publish --locked -p", "cargo publish -p",
         "`cargo publish` without `--locked`"),
    ):
        check_text(label, old, new, needle)

    # `main` reads the script too: run it on the real workflow with the script
    # swapped for an unlocked copy, and with the script missing.
    import contextlib
    import io
    import tempfile

    global HWCAPS_SCRIPT
    real_script = HWCAPS_SCRIPT
    try:
        with tempfile.TemporaryDirectory(prefix="hwcaps-test-") as tmp:
            for label, content, want in (("unlocked", None, 1), ("locked", "cargo build --release --locked\n", 0),
                                         ("missing", False, 1)):
                path = Path(tmp) / f"{label}.sh"
                if content is None:
                    path.write_text(real_script.read_text(encoding="utf-8").replace("--release --locked", "--release"))
                elif content:
                    path.write_text(content)
                HWCAPS_SCRIPT = path
                saved = sys.argv
                sys.argv = ["check_release_workflow.py", "--workflow", str(real_path)]
                try:
                    with contextlib.redirect_stdout(io.StringIO()):
                        got = main()
                finally:
                    sys.argv = saved
                if got != want:
                    failures.append(f"main with a {label} build_hwcaps.sh exited {got}, want {want}")
    finally:
        HWCAPS_SCRIPT = real_script

    hwcaps = HWCAPS_SCRIPT.read_text(encoding="utf-8")
    if "cargo build --release --locked" not in hwcaps:
        failures.append("build_hwcaps.sh no longer builds with `--locked`")
    unlocked = hwcaps.replace("cargo build --release --locked", "cargo build --release")
    if not any("without `--locked`" in p for p in lock_problems(unlocked, "build_hwcaps.sh")):
        failures.append("an unlocked cargo build in build_hwcaps.sh: not reported")
    for harmless in ("# cargo build --release", "echo \"::error::cargo publish failed for x\"",
                     "run: echo cargo builds", "x=$(echo cargo-build)", "echo cargo publish failed"):
        if lock_problems(harmless, "x"):
            failures.append(f"{harmless!r} was reported as an unlocked cargo command")
    for command in ("cargo build --release", "if cargo publish -p x 2>&1; then", "run: cargo build -p a"):
        if not lock_problems(command, "x"):
            failures.append(f"{command!r} was not reported as an unlocked cargo command")

    # --- python.yml pin rules -----------------------------------------------
    real_py_path = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "python.yml"
    real_py_text = real_py_path.read_text(encoding="utf-8")
    real_py = yaml.safe_load(real_py_text)
    py_found = python_pin_problems(real_py, "python.yml")
    if py_found:
        failures.append(f"the real python.yml has pin problems: {py_found}")

    def check_py(label: str, mutate, needle: str) -> None:
        wf = copy.deepcopy(real_py)
        mutate(wf)
        got = python_pin_problems(wf, "python.yml")
        if not any(needle in p for p in got):
            failures.append(f"{label}: not reported (got {got})")

    check_py("an action pinned by tag in python.yml",
             lambda w: step_of(w, "test", "actions/checkout@").__setitem__("uses", "actions/checkout@v7"),
             "`actions/checkout@v7` is not pinned by commit")
    check_py("a checkout without persist-credentials: false in python.yml",
             lambda w: step_of(w, "test", "actions/checkout@")["with"].pop("persist-credentials"),
             "a checkout without `persist-credentials: false`")
    check_py("a toolchain channel in python.yml",
             lambda w: step_of(w, "test", "dtolnay/rust-toolchain@")["with"].__setitem__("toolchain", "stable"),
             "installs toolchain 'stable'")
    check_py("RELEASE_TOOLCHAIN set to a channel in python.yml",
             lambda w: w["env"].__setitem__("RELEASE_TOOLCHAIN", "stable"),
             "not an exact version")

    # --- every workflow and composite action: pinned by commit ---------------
    all_found, _ = all_pin_problems()
    if all_found:
        failures.append(f"the tree has unpinned actions: {all_found}")
    sha = "0" * 40
    fixture = {"jobs": {"build": {"steps": [{"uses": f"actions/checkout@{sha}"},
                                            {"uses": "./.github/actions/valgrind"}, {"run": "true"}]}}}
    if pin_problems_in(fixture, "x.yml"):
        failures.append(f"a pinned fixture was reported: {pin_problems_in(fixture, 'x.yml')}")
    for label, doc, needle in (
        ("a tag in a non-release workflow",
         {"jobs": {"build": {"steps": [{"uses": "actions/checkout@v7"}]}}},
         "x.yml: `build`: `actions/checkout@v7` is not pinned by commit"),
        ("a branch ref",
         {"jobs": {"build": {"steps": [{"uses": "dtolnay/rust-toolchain@master", "with": {"toolchain": "stable"}}]}}},
         "`dtolnay/rust-toolchain@master` is not pinned by commit"),
        ("an abbreviated sha",
         {"jobs": {"build": {"steps": [{"uses": "actions/checkout@3d3c42e"}]}}},
         "is not pinned by commit"),
        ("a reusable workflow call by tag",
         {"jobs": {"call": {"uses": "o/r/.github/workflows/w.yml@v1"}}},
         "`call`: `o/r/.github/workflows/w.yml@v1` is not pinned by commit"),
        ("a composite action step by tag",
         {"runs": {"using": "composite", "steps": [{"uses": "actions/cache@v6"}]}},
         "`runs`: `actions/cache@v6` is not pinned by commit"),
        ("a commit-pinned rust-toolchain with no toolchain input",
         {"jobs": {"build": {"steps": [{"uses": f"dtolnay/rust-toolchain@{sha}"}]}}},
         "has no `toolchain:` input"),
    ):
        got = pin_problems_in(doc, "x.yml")
        if not any(needle in p for p in got):
            failures.append(f"{label}: not reported (got {got})")
    # The same mutation through the file walk: one tag in a temp copy of the
    # tree is reported by path, and an unmutated copy is not.
    with tempfile.TemporaryDirectory() as tmp:
        tree = Path(tmp)
        shutil.copytree(REPO_ROOT / ".github", tree / ".github")
        clean, _ = all_pin_problems(tree)
        if clean:
            failures.append(f"a clean copy of .github was reported: {clean}")
        target = tree / ".github" / "workflows" / "nightly.yml"
        mutated, n = re.subn(r"(uses: actions/checkout)@[0-9a-f]{40} # v[\d.]+", r"\1@v7",
                             target.read_text(encoding="utf-8"), count=1)
        if n != 1:
            failures.append("nightly.yml has no pinned actions/checkout to mutate")
        target.write_text(mutated, encoding="utf-8")
        got, _ = all_pin_problems(tree)
        if not any(p.startswith(".github/workflows/nightly.yml: ") and "`actions/checkout@v7` is not pinned by commit" in p
                   for p in got):
            failures.append(f"an unpinned checkout in nightly.yml: not reported (got {got})")
        # An unpinned global npm tool in a workflow other than release.yml.
        if all_npm_problems(tree):
            failures.append(f"a clean copy of .github was reported by the npm rule: {all_npm_problems(tree)}")
        target.write_text(target.read_text(encoding="utf-8").replace(
            "npm install -g wasm-pack@0.15.0", "npm install -g wasm-pack"), encoding="utf-8")
        got = all_npm_problems(tree)
        if not any(p == ".github/workflows/nightly.yml: `npm install -g wasm-pack` is not pinned to an exact X.Y.Z version"
                   for p in got):
            failures.append(f"an unpinned wasm-pack in nightly.yml: not reported (got {got})")

        # `pip install` without `--require-hashes`, through the file walk: the
        # clean copy is silent, and dropping the flag from one line of each of
        # ci.yml, python.yml and nightly.yml is reported by path with the line.
        if all_pip_problems(tree):
            failures.append(f"a clean copy of .github was reported by the pip rule: {all_pip_problems(tree)}")
        for name in ("ci.yml", "python.yml", "nightly.yml"):
            wf = tree / ".github" / "workflows" / name
            original = wf.read_text(encoding="utf-8")
            mutated, n = re.subn(r"(pip install )(?:--quiet )?(?:--disable-pip-version-check )?--require-hashes ",
                                 r"\1", original, count=1)
            if n != 1:
                failures.append(f"{name} has no `pip install --require-hashes` to mutate")
                continue
            wf.write_text(mutated, encoding="utf-8")
            got = all_pip_problems(tree)
            if not any(p.startswith(f".github/workflows/{name}: `pip install` without `--require-hashes`: ")
                       for p in got):
                failures.append(f"{name} with the flag dropped: not reported (got {got})")
            wf.write_text(original, encoding="utf-8")
    for label, line, want in (
        ("an unpinned tool", "run: python -m pip install ruff", 1),
        ("an upgrade of pip itself", "run: python -m pip install --upgrade pip", 1),
        ("a requirements file without the flag", "run: pip install -r requirements.txt", 1),
        ("a requirements file with the flag", "run: pip install --require-hashes -r requirements.txt", 0),
        ("the flag on a continuation line", "run: |\n  pip install \\\n    --require-hashes -r r.txt", 0),
        ("a missing flag on a continuation line", "run: |\n  pip install \\\n    -r r.txt", 1),
        ("the local project", "run: pip install .", 0),
        ("the local project in a venv", 'run: "$VENV/bin/python" -m pip install --quiet .', 0),
        ("the local project and a package", "run: pip install . pytest", 1),
        ("a comment", "# pip install ruff", 0),
    ):
        got = pip_hash_problems(line, "x.yml")
        if len(got) != want:
            failures.append(f"pip rule, {label}: got {got}, want {want} problem(s)")

    # --- python-bindings requirements: sets, pins, Python floors, install sites ---
    req_dir = REPO_ROOT / ".github" / "requirements"
    MAIN, P38 = PYTHON_BINDINGS_MAIN, PYTHON_BINDINGS_PY38
    pairs = {w: ((req_dir / f"{w}.in").read_text(encoding="utf-8"), (req_dir / f"{w}.txt").read_text(encoding="utf-8"))
             for w in (MAIN, P38)}
    floor = matrix_python_floor(real_py)
    if floor != (3, 8):
        failures.append(f"the Python floor read from python.yml is {floor}, not (3, 8)")
    got = all_requirements_problems()
    if got:
        failures.append(f"the real python-bindings requirements have problems: {got}")

    def drop_pin(text: str, name: str) -> str:
        return "\n".join(l for l in text.splitlines() if not l.startswith(f"{name}==")) + "\n"

    def swap(which: str, old: str, new: str) -> tuple[str, str]:
        return pairs[which][0].replace(old, new), pairs[which][1].replace(old, new)

    for label, which, in_t, txt_t, fl, needle in (
        ("main: a recompile that dropped tomli from the .txt", MAIN, pairs[MAIN][0], drop_pin(pairs[MAIN][1], "tomli"),
         MAIN_PYTHON_FLOOR, "missing ['tomli']"),
        ("main: a recompile that dropped pygments", MAIN, pairs[MAIN][0], drop_pin(pairs[MAIN][1], "pygments"),
         MAIN_PYTHON_FLOOR, "missing ['pygments']"),
        ("py38: a recompile that dropped exceptiongroup and typing-extensions", P38, pairs[P38][0],
         drop_pin(drop_pin(pairs[P38][1], "exceptiongroup"), "typing-extensions"), floor,
         "missing ['exceptiongroup', 'typing-extensions']"),
        ("py38: a package added to the .txt", P38, pairs[P38][0], pairs[P38][1] + "zipp==3.0.0 \\\n", floor,
         "unexpected ['zipp']"),
        ("main: tomli no longer listed in the .in", MAIN, drop_pin(pairs[MAIN][0], "tomli"), pairs[MAIN][1],
         MAIN_PYTHON_FLOOR, "does not list `tomli`"),
        ("main: a pin moved in the .in only", MAIN, pairs[MAIN][0].replace("maturin==1.15.0", "maturin==1.16.0"),
         pairs[MAIN][1], MAIN_PYTHON_FLOOR, "pins maturin==1.16.0 but python-bindings.txt has 1.15.0"),
        ("py38: pytest 8.4 with Python 3.8 served", P38, *swap(P38, "pytest==8.3.5", "pytest==8.4.2"), floor,
         "pins pytest==8.4.2, which needs Python 3.9+, but the file serves Python 3.8"),
        ("py38: typing-extensions 4.14 with Python 3.8 served", P38,
         *swap(P38, "typing-extensions==4.13.2", "typing-extensions==4.14.0"), floor,
         "pins typing-extensions==4.14.0"),
        ("main: pytest 9 on a file that served Python 3.9", MAIN, pairs[MAIN][0], pairs[MAIN][1], (3, 9),
         "which needs Python 3.10+, but the file serves Python 3.9"),
        ("main: pytest back at the vulnerable 8.3.5", MAIN, *swap(MAIN, "pytest==9.1.1", "pytest==8.3.5"),
         MAIN_PYTHON_FLOOR, "below 9.0.3, the first release that fixes GHSA-6w46-j5rx-g56g"),
        ("main: pytest 9.0.2, one release before the fix", MAIN, *swap(MAIN, "pytest==9.1.1", "pytest==9.0.2"),
         MAIN_PYTHON_FLOOR, "below 9.0.3"),
        ("py38: no matrix in python.yml", P38, pairs[P38][0], pairs[P38][1], None, "Python floor"),
    ):
        got = python_requirements_problems(in_t, txt_t, fl, which)
        if not any(needle in p for p in got):
            failures.append(f"{label}: not reported (got {got})")
    if python_requirements_problems(*swap(P38, "pytest==8.3.5", "pytest==8.4.2"), (3, 9), P38):
        failures.append("pytest 8.4 was reported although the file serves Python 3.9")
    if matrix_python_floor({"jobs": {"a": {"strategy": {"matrix": {"python-version": ["3.12", "3.10"]}}}}}) != (3, 10):
        failures.append("matrix_python_floor does not compare versions numerically")

    # install sites: every workflow, then one mutation per clause
    wf_dir = REPO_ROOT / ".github" / "workflows"
    site_texts = {p.name: p.read_text(encoding="utf-8") for p in sorted(wf_dir.glob("*.yml"))}

    def site_check(label: str, name: str, old: str, new: str, needle: str) -> None:
        if site_texts[name].count(old) < 1:
            failures.append(f"{label}: the fixture text {old!r} is not in {name}")
            return
        texts = dict(site_texts)
        texts[name] = texts[name].replace(old, new)
        got = python_requirements_sites_problems({n: yaml.safe_load(t) or {} for n, t in texts.items()}, texts)
        if not any(needle in p for p in got):
            failures.append(f"{label}: not reported (got {got})")

    py38_file = ".github/requirements/py38/python-bindings-py38.txt"
    main_file = ".github/requirements/python-bindings.txt"
    selector = f"matrix.python-version == '3.8' && '{py38_file}' || '{main_file}'"
    site_check("python.yml installing the main file on every row", "python.yml", selector, f"'{main_file}'",
               "does not choose the requirements file by the matrix value")
    site_check("python.yml selecting on the wrong version", "python.yml", "matrix.python-version == '3.8'",
               "matrix.python-version == '3.10'", "does not choose the requirements file by the matrix value")
    site_check("python.yml with a 3.9 row", "python.yml", 'python-version: ["3.8", "3.10", "3.12"]',
               'python-version: ["3.8", "3.9", "3.10", "3.12"]', "tests Python 3.9, which neither")
    site_check("ci.yml on the py38 file", "ci.yml", "-r .github/requirements/python-bindings.txt",
               f"-r {py38_file}", "installs the py38 requirements file")
    site_check("nightly.yml on the py38 file", "nightly.yml", "-r .github/requirements/python-bindings.txt",
               f"-r {py38_file}", "installs the py38 requirements file")
    site_check("nightly.yml on Python 3.9", "nightly.yml", '          python-version: "3.12"\n',
               '          python-version: "3.9"\n', "installs the main requirements file")
    site_check("bench_baremetal.yml on the main file", "bench_baremetal.yml", f"-r {py38_file}",
               f"-r {main_file}", "installs the main requirements file")
    site_check("ci.yml test-python on Python 3.9", "ci.yml", 'python-version: "3.12"',
               'python-version: "3.9"', "installs the main requirements file")

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
    found += hwcaps_problems(text)
    if PYTHON_WORKFLOW.is_file():
        py_text = PYTHON_WORKFLOW.read_text(encoding="utf-8")
        found += python_pin_problems(yaml.safe_load(py_text), PYTHON_WORKFLOW.name)
    tree_found, n_files = all_pin_problems()
    found += tree_found + all_npm_problems() + all_pip_problems() + all_requirements_problems()
    for p in found:
        print(f"::error::{p}")
    if found:
        return 1
    print(f"check_release_workflow: {len(PUBLISH_JOBS)} publishing jobs wait for promote and skip a pre-release tag; "
          f"every job is behind the gate; {len(CALL_SITES)} call sites are as written; "
          f"actions are pinned by commit in release.yml, python.yml and {n_files - len(PIN_COVERED_ELSEWHERE)} "
          f"other workflow/action files, checkouts drop the token, the toolchain is exact; "
          f"global tools use pinned versions, pip installs carry --require-hashes, cargo build carries --locked, "
          f"python-bindings.txt resolves the expected set and respects the matrix Python floor")
    return 0


if __name__ == "__main__":
    sys.exit(main())
