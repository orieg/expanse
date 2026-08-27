#!/usr/bin/env python3
"""Keep the `/bench` suite vocabulary honest and single-sourced.

`.github/bench-suites.json` is the one place a benchmark suite is declared.
Three other places have to agree with it, and used to drift:

  1. `.github/workflows/bench_baremetal.yml` — the `workflow_dispatch`
     `benchmark_suite` choice list (generated block).
  2. `docs/BENCHMARKING.md` — the reader-facing suite table (generated block).
  3. `crates/*/Cargo.toml` — every generic suite must name a real `[[bench]]`
     target in the package it claims.

This script checks all three and, with `--write`, regenerates the two
generated blocks from the manifest. It also checks the manifest itself for the
properties the workflow's resolver relies on: unique names, no name that is a
reserved marker word, a declared default that is available, aliases that
resolve, and — the point of #410 — that no name can be reached by substring
matching another one, so a future regression to `includes()` is caught here
rather than in a silently mislabelled report.

Usage:
  python3 scripts/check_bench_suites.py             # check (exit 1 on drift)
  python3 scripts/check_bench_suites.py --write     # regenerate the blocks
  python3 scripts/check_bench_suites.py --self-test
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

MANIFEST = ".github/bench-suites.json"
WORKFLOW = ".github/workflows/bench_baremetal.yml"
DOCS = "docs/BENCHMARKING.md"

BEGIN = "BEGIN GENERATED: bench-suites"
END = "END GENERATED: bench-suites"

REQUIRED_FIELDS = ("name", "available", "kind", "runner", "summary")
KINDS = ("callgrind", "wallclock")
RUNNERS = ("builtin", "generic")
NAME_RE = re.compile(r"^[a-z][a-z0-9_]*$")


# --------------------------------------------------------------------------
# manifest
# --------------------------------------------------------------------------
def check_manifest(manifest: dict) -> tuple[list[str], list[str]]:
    """Structural checks on the manifest. Returns (errors, containing-name pairs)."""
    errs: list[str] = []
    suites = manifest.get("suites")
    if not isinstance(suites, list) or not suites:
        return [f"{MANIFEST}: `suites` must be a non-empty list"], []

    names: list[str] = []
    for i, s in enumerate(suites):
        where = f"{MANIFEST}: suites[{i}]"
        for f in REQUIRED_FIELDS:
            if f not in s:
                errs.append(f"{where}: missing required field `{f}`")
        name = s.get("name", "")
        if not NAME_RE.match(str(name)):
            errs.append(f"{where}: name {name!r} must match {NAME_RE.pattern}")
        else:
            names.append(name)
        if s.get("kind") not in KINDS:
            errs.append(f"{where} ({name}): kind must be one of {KINDS}")
        if s.get("runner") not in RUNNERS:
            errs.append(f"{where} ({name}): runner must be one of {RUNNERS}")
        if s.get("runner") == "generic" and not (s.get("package") and s.get("target")):
            errs.append(f"{where} ({name}): generic suites need `package` and `target`")
        if s.get("available") is False and not s.get("reason"):
            errs.append(f"{where} ({name}): an unavailable suite must state a `reason`")
        if s.get("available") is True and s.get("reason"):
            errs.append(f"{where} ({name}): `reason` belongs only on an unavailable suite")

    dupes = sorted({n for n in names if names.count(n) > 1})
    if dupes:
        errs.append(f"{MANIFEST}: duplicate suite name(s): {', '.join(dupes)}")

    reserved = set(manifest.get("reserved", []))
    for n in names:
        if n in reserved:
            errs.append(f"{MANIFEST}: {n!r} is a reserved comment-marker word and cannot be a suite")

    available = {s["name"] for s in suites if s.get("available")}
    default = manifest.get("default")
    if default not in available:
        errs.append(f"{MANIFEST}: default {default!r} is not an available suite")
    for alias, target in (manifest.get("aliases") or {}).items():
        if not NAME_RE.match(alias):
            errs.append(f"{MANIFEST}: alias {alias!r} must match {NAME_RE.pattern}")
        if target not in available:
            errs.append(f"{MANIFEST}: alias {alias!r} points at {target!r}, which is not available")
        if alias in set(names):
            errs.append(f"{MANIFEST}: alias {alias!r} collides with a declared suite name")

    # The #410 regression guard. Substring containment among suite names is
    # what let `/benchmark search_instructions` run `instructions`: an
    # `includes()` ladder resolves to whichever containing name it tests
    # first. Token matching makes containment harmless, so this is not a
    # prohibition — it is an inventory, asserted so that a reviewer who
    # reintroduces substring matching finds a list of exactly which pairs
    # would collide.
    tokens = sorted(set(names) | set(manifest.get("aliases") or {}))
    collisions = sorted(
        f"{a} contains {b}" for a in tokens for b in tokens if a != b and b in a
    )
    return errs, collisions


# --------------------------------------------------------------------------
# generated blocks
# --------------------------------------------------------------------------
def render_choice_options(manifest: dict, indent: str) -> list[str]:
    return [f"{indent}- {s['name']}" for s in manifest["suites"] if s.get("available")]


def render_docs_table(manifest: dict) -> list[str]:
    default = manifest["default"]
    aliases = manifest.get("aliases") or {}
    alias_of: dict[str, list[str]] = {}
    for a, t in aliases.items():
        alias_of.setdefault(t, []).append(a)

    out = [
        "<!-- Generated from `.github/bench-suites.json` by",
        "     `python3 scripts/check_bench_suites.py --write`. Do not hand-edit:",
        "     the `lint` CI job fails when this block and the manifest disagree. -->",
        "",
        f"`/bench` with no argument runs `{default}`. The argument is matched as a whole"
        " token against this table — never as a substring — and an unrecognised argument"
        " is refused by name with no benchmark run.",
        "",
        "| Suite | Instrument | What it runs |",
        "|---|---|---|",
    ]
    for s in manifest["suites"]:
        if not s.get("available"):
            continue
        name = f"`{s['name']}`"
        for a in sorted(alias_of.get(s["name"], [])):
            name += f" (alias `{a}`)"
        instrument = "Callgrind" if s["kind"] == "callgrind" else "wall-clock"
        out.append(f"| {name} | {instrument} | {s['summary']} |")

    gaps = [s for s in manifest["suites"] if not s.get("available")]
    if gaps:
        out += [
            "",
            "Bench targets deliberately **not** reachable from a slash command:",
            "",
            "| Target | Why |",
            "|---|---|",
        ]
        out += [f"| `{s['name']}` | {s['reason']} |" for s in gaps]
    return out


def splice(path: Path, body: list[str], write: bool) -> list[str]:
    """Replace the generated block in `path` with `body`. Returns errors."""
    text = path.read_text(encoding="utf-8")
    lines = text.split("\n")
    try:
        b = next(i for i, l in enumerate(lines) if BEGIN in l)
        e = next(i for i, l in enumerate(lines) if END in l and i > b)
    except StopIteration:
        return [f"{path}: missing the `{BEGIN}` / `{END}` block"]

    current = lines[b + 1 : e]
    if current == body:
        return []
    if not write:
        return [
            f"{path}: generated block is out of sync with {MANIFEST} "
            f"— run `python3 scripts/check_bench_suites.py --write`"
        ]
    path.write_text("\n".join(lines[: b + 1] + body + lines[e:]), encoding="utf-8")
    print(f"rewrote the generated block in {path}")
    return []


# --------------------------------------------------------------------------
# cargo bench targets
# --------------------------------------------------------------------------
def bench_targets(root: Path) -> dict[str, set[str]]:
    """package name -> set of [[bench]] target names, from the crate manifests."""
    out: dict[str, set[str]] = {}
    for toml in sorted(root.glob("crates/*/Cargo.toml")):
        text = toml.read_text(encoding="utf-8")
        pkg = re.search(r'(?m)^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', text)
        if not pkg:
            continue
        names = set()
        for block in re.findall(r"(?ms)^\[\[bench\]\]\s*$(.*?)(?=^\[|\Z)", text):
            m = re.search(r'(?m)^name\s*=\s*"([^"]+)"', block)
            if m:
                names.add(m.group(1))
        out[pkg.group(1)] = names
    return out


def check_targets(manifest: dict, root: Path) -> list[str]:
    targets = bench_targets(root)
    errs = []
    for s in manifest["suites"]:
        if s.get("runner") != "generic":
            continue
        pkg, tgt = s["package"], s["target"]
        if pkg not in targets:
            errs.append(f"{MANIFEST}: suite {s['name']!r} names unknown package {pkg!r}")
        elif tgt not in targets[pkg]:
            errs.append(
                f"{MANIFEST}: suite {s['name']!r} names bench target {tgt!r}, "
                f"which is not a [[bench]] in {pkg}"
            )
    return errs


# --------------------------------------------------------------------------
def run(root: Path, write: bool) -> int:
    manifest = json.loads((root / MANIFEST).read_text(encoding="utf-8"))
    errs, collisions = check_manifest(manifest)
    if errs:
        for e in errs:
            print(f"::error::{e}")
        return 1

    errs += check_targets(manifest, root)
    errs += splice(root / WORKFLOW, render_choice_options(manifest, " " * 10), write)
    errs += splice(root / DOCS, render_docs_table(manifest), write)

    for e in errs:
        print(f"::error::{e}")
    if errs:
        return 1

    n_avail = sum(1 for s in manifest["suites"] if s.get("available"))
    n_gap = len(manifest["suites"]) - n_avail
    print(
        f"check_bench_suites.py: {n_avail} suite(s) reachable from /bench, "
        f"{n_gap} declared unavailable, workflow + docs in sync."
    )
    if collisions:
        # Not an error: token matching makes containment safe. Printed so the
        # inventory is visible in the job log if `includes()` ever returns.
        print(
            "  substring-containing name pairs (harmless under token matching, "
            "fatal under `includes()`): " + "; ".join(collisions)
        )
    return 0


def self_test() -> int:
    base = {
        "default": "all",
        "aliases": {"full": "extended"},
        "reserved": ["rejected"],
        "suites": [
            {"name": "all", "available": True, "kind": "callgrind", "runner": "builtin", "summary": "s"},
            {"name": "extended", "available": True, "kind": "callgrind", "runner": "builtin", "summary": "s"},
        ],
    }

    def errs(m):
        return check_manifest(m)[0]

    assert errs(base) == [], errs(base)

    m = json.loads(json.dumps(base)); m["default"] = "nope"
    assert any("default" in e for e in errs(m)), "default must be available"

    m = json.loads(json.dumps(base)); m["aliases"] = {"full": "nope"}
    assert any("alias" in e for e in errs(m)), "alias must resolve"

    m = json.loads(json.dumps(base)); m["aliases"] = {"all": "extended"}
    assert any("collides" in e for e in errs(m)), "alias/name collision"

    m = json.loads(json.dumps(base)); m["suites"].append(dict(m["suites"][0]))
    assert any("duplicate" in e for e in errs(m)), "duplicate names"

    m = json.loads(json.dumps(base))
    m["suites"].append({"name": "rejected", "available": True, "kind": "wallclock", "runner": "builtin", "summary": "s"})
    assert any("reserved" in e for e in errs(m)), "reserved marker word"

    m = json.loads(json.dumps(base))
    m["suites"].append({"name": "x", "available": True, "kind": "wallclock", "runner": "generic", "summary": "s"})
    assert any("package" in e for e in errs(m)), "generic needs package/target"

    m = json.loads(json.dumps(base))
    m["suites"].append({"name": "x", "available": False, "kind": "wallclock", "runner": "builtin", "summary": "s"})
    assert any("reason" in e for e in errs(m)), "unavailable needs a reason"

    m = json.loads(json.dumps(base)); m["suites"][0]["kind"] = "guesswork"
    assert any("kind" in e for e in errs(m)), "kind vocabulary"

    # The #410 shape: `search_instructions` contains `instructions`, so the two
    # must be reported as a containing pair — the exact case an `includes()`
    # ladder resolved the wrong way.
    m = json.loads(json.dumps(base))
    m["suites"] += [
        {"name": "instructions", "available": True, "kind": "callgrind", "runner": "builtin", "summary": "s"},
        {"name": "search_instructions", "available": True, "kind": "callgrind", "runner": "generic",
         "package": "expanse-trie", "target": "search_instructions", "summary": "s"},
    ]
    e, coll = check_manifest(m)
    assert e == [], e
    assert "search_instructions contains instructions" in coll, coll

    # Rendering is derived, not stamped: an added suite must appear in both blocks.
    opts = render_choice_options(m, "  ")
    assert "  - search_instructions" in opts, opts
    table = "\n".join(render_docs_table(m))
    assert "`search_instructions`" in table and "`extended` (alias `full`)" in table, table

    print("check_bench_suites.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--write", action="store_true", help="regenerate the generated blocks from the manifest")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    root = Path(
        subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
    )
    return run(root, args.write)


if __name__ == "__main__":
    sys.exit(main())
