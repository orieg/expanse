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
INDEX = "docs/benchmarks/README.md"
SUITE_ROOT = "docs/benchmarks"

BEGIN = "BEGIN GENERATED: bench-suites"
END = "END GENERATED: bench-suites"

REQUIRED_FIELDS = ("name", "available", "kind", "runner", "summary")
KINDS = ("callgrind", "wallclock", "counters", "fuel")
RUNNERS = ("builtin", "generic")
NAME_RE = re.compile(r"^[a-z][a-z0-9_]*$")
SUITE_RE = re.compile(r"^[a-z][a-z0-9_]*$")

# Tokens with no `suite`: core engine instruments that no comparative suite
# directory owns. Listed under their own heading in the index rather than
# dropped, so a token missing a suite is visible rather than silent.
CORE_HEADING = "Core engine instruments (no comparative suite directory)"


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
        suite = s.get("suite")
        if suite is not None and not SUITE_RE.match(str(suite)):
            errs.append(f"{where}: suite {suite!r} must match {SUITE_RE.pattern}")
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
        instrument = {
            "callgrind": "Callgrind",
            "counters": "`perf stat`",
        }.get(s["kind"], "wall-clock")
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


def check_suite_dirs(manifest: dict, root: Path) -> list[str]:
    """Every declared `suite` must be a real docs/benchmarks/<dir> with a README.

    A token pointing at a directory that does not exist would render an index
    row whose link 404s, which is the class of silent breakage AGENTS.md 8.1
    forbids -- so it is fatal here rather than discovered by a reader.
    """
    errs: list[str] = []
    for s in manifest["suites"]:
        suite = s.get("suite")
        if suite is None:
            continue
        d = root / SUITE_ROOT / suite
        if not d.is_dir():
            errs.append(f"{MANIFEST}: `{s['name']}` names suite {suite!r}, but {SUITE_ROOT}/{suite}/ does not exist")
        elif not (d / "README.md").is_file():
            errs.append(f"{MANIFEST}: {SUITE_ROOT}/{suite}/ has no README.md for `{s['name']}` to link to")
    return errs


def suite_title(root: Path, suite: str) -> str:
    """The suite's own `# ` heading, used as its one-line description.

    Taken from the suite rather than from a member token's `summary`: a suite
    with several tokens has no single representative arm, and picking one
    described `art_comparison` as insertion-only and `redis_zset_engine` as
    memory-only. Sourcing it from the README keeps the index honest and lets a
    suite reword its own description without touching the manifest.
    """
    readme = root / SUITE_ROOT / suite / "README.md"
    try:
        for line in readme.read_text(encoding="utf-8").split("\n"):
            if line.startswith("# "):
                return line[2:].strip()
    except OSError:
        pass
    return suite.replace("_", " ")


def render_index(manifest: dict, root: Path) -> list[str]:
    """The docs/benchmarks/README.md body: one section per suite directory."""
    by_suite: dict[str, list[dict]] = {}
    core: list[dict] = []
    for s in manifest["suites"]:
        (by_suite.setdefault(s["suite"], []) if s.get("suite") else core).append(s)

    out = [
        "<!-- Generated from `.github/bench-suites.json` by",
        "     `python3 scripts/check_bench_suites.py --write`. Do not hand-edit:",
        "     the `lint` CI job fails when this block and the manifest disagree. -->",
        "",
        "| Suite | `/benchmark` tokens | Instrument | What it covers |",
        "|---|---|---|---|",
    ]

    def instrument(entries: list[dict]) -> str:
        kinds = sorted({e["kind"] for e in entries})
        label = {"callgrind": "Callgrind", "counters": "`perf stat`", "fuel": "wasm fuel"}
        return " + ".join(label.get(k, "wall-clock") for k in kinds)

    for suite in sorted(by_suite):
        entries = by_suite[suite]
        toks = ", ".join(f"`{e['name']}`" for e in sorted(entries, key=lambda e: e["name"]))
        readme = f"[`{suite}/`]({suite}/README.md)"
        out.append(f"| {readme} | {toks} | {instrument(entries)} | {suite_title(root, suite)} |")

    out += [
        "",
        f"### {CORE_HEADING}",
        "",
        "These run from the engine crates and publish into `docs/BENCHMARKING.md`",
        "rather than a suite directory. They are listed so that a token without a",
        "suite is visible rather than silently absent from this index.",
        "",
        "| Token | Instrument | What it runs |",
        "|---|---|---|",
    ]
    for e in sorted(core, key=lambda e: e["name"]):
        out.append(f"| `{e['name']}` | {instrument([e])} | {e['summary']} |")
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


GROUP_DECL = re.compile(
    r"library_benchmark_group!\s*\(\s*name\s*=\s*(\w+)\s*;\s*benchmarks\s*=\s*([^;)]+)\)"
)


def declared_arms(source: str) -> dict[str, list[str]]:
    """`{library_benchmark_group! name -> [benchmark fn, ...]}` from a bench source."""
    out: dict[str, list[str]] = {}
    for group, body in GROUP_DECL.findall(source):
        out[group] = [n.strip() for n in body.split(",") if n.strip()]
    return out


def check_arms(manifest: dict, root: Path) -> list[str]:
    """Every callgrind arm must say what it is.

    `scripts/perf_report.py` reads the `arms` block to tell an Expanse arm from
    a third-party baseline — it never infers that from the arm's name. So the
    block has to be a complete partition of the arms the bench source actually
    declares: an arm added to the source without a line here would otherwise
    enter the report as if it were our code, carrying a `vs main` column
    against a dependency and leaving its twin ratio uncomputed. That is the
    silent-untwinning this check exists to make loud.

    A baseline may be named by more than one twin — one competitor kernel is
    the symmetric comparison for each of our paths to it (#417) — but a subject
    is named once, and no arm may be both.
    """
    errs: list[str] = []
    for s in manifest["suites"]:
        arms = s.get("arms")
        needs_arms = s.get("kind") == "callgrind" and s.get("runner") == "generic"
        if not needs_arms and not arms:
            continue
        name = s["name"]
        if needs_arms and not arms:
            errs.append(
                f"{MANIFEST}: suite {name!r} is a callgrind suite run from a bench "
                "target but declares no `arms` block; perf_report.py cannot tell its "
                "third-party baselines from our own arms"
            )
            continue
        if not s.get("target"):
            errs.append(f"{MANIFEST}: suite {name!r} declares `arms` but no `target`")
            continue

        matches = sorted(root.glob(f"crates/*/benches/{s['target']}.rs"))
        if not matches:
            errs.append(
                f"{MANIFEST}: suite {name!r} declares `arms` but no bench source "
                f"crates/*/benches/{s['target']}.rs exists to check them against"
            )
            continue

        groups = declared_arms(matches[0].read_text(encoding="utf-8"))
        in_source = {fn for fns in groups.values() for fn in fns}
        group_of = {fn: g for g, fns in groups.items() for fn in fns}

        twins = arms.get("twins", [])
        subjects: list[str] = list(arms.get("unpaired", []))
        baselines: list[str] = []
        for t in twins:
            if not t.get("subject") or not t.get("baseline"):
                errs.append(f"{MANIFEST}: suite {name!r}: a twin needs `subject` and `baseline`")
                continue
            subjects.append(t["subject"])
            baselines.append(t["baseline"])
            if t["subject"] in group_of and t["baseline"] in group_of:
                if group_of[t["subject"]] != group_of[t["baseline"]]:
                    errs.append(
                        f"{MANIFEST}: suite {name!r}: twin {t['subject']!r}/"
                        f"{t['baseline']!r} spans two library_benchmark_group!s "
                        f"({group_of[t['subject']]} vs {group_of[t['baseline']]}) — "
                        "a twin pair must be measured in the same group"
                    )
        if twins and not arms.get("baseline_label"):
            errs.append(
                f"{MANIFEST}: suite {name!r} declares twins but no `baseline_label`; "
                "the rendered ratio has to name what it is a ratio against"
            )

        # A subject is ours and is counted once. A baseline is a competitor
        # kernel and may be the twin of several subjects: where the competitor
        # exposes one kernel and we reach it by more than one path (composed
        # vs native), the same baseline is the symmetric comparison for each,
        # and forbidding the repeat would force a duplicate arm measuring the
        # identical function under a second name. What must never happen is an
        # arm classified as both ours and theirs — that decides the `vs main`
        # column and the regression gate, so it is an error, not a warning.
        dupes = sorted({a for a in subjects if subjects.count(a) > 1})
        if dupes:
            errs.append(f"{MANIFEST}: suite {name!r}: arm(s) declared twice: {', '.join(dupes)}")
        both = sorted(set(subjects) & set(baselines))
        if both:
            errs.append(
                f"{MANIFEST}: suite {name!r}: arm(s) declared as both a subject and a "
                f"baseline: {', '.join(both)}"
            )

        declared = set(subjects) | set(baselines)
        missing = sorted(in_source - declared)
        if missing:
            errs.append(
                f"{MANIFEST}: suite {name!r}: {len(missing)} arm(s) in "
                f"{matches[0].relative_to(root)} are not classified in `arms` — add each "
                f"to a `twins` pair or to `unpaired`: {', '.join(missing)}"
            )
        stale = sorted(declared - in_source)
        if stale:
            errs.append(
                f"{MANIFEST}: suite {name!r}: `arms` names {len(stale)} arm(s) that "
                f"{matches[0].relative_to(root)} no longer declares: {', '.join(stale)}"
            )
    return errs


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
    errs += check_arms(manifest, root)
    errs += splice(root / WORKFLOW, render_choice_options(manifest, " " * 10), write)
    errs += splice(root / DOCS, render_docs_table(manifest), write)
    errs += check_suite_dirs(manifest, root)
    if not (root / INDEX).is_file():
        errs.append(f"{INDEX}: missing -- the suite index is generated into it; create it with the BEGIN/END block")
    else:
        errs += splice(root / INDEX, render_index(manifest, root), write)

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

    # A `counters` suite is a third instrument, not a wall-clock one wearing a
    # different name: the docs table has to say which instrument produced the
    # numbers, which is the doc-vs-reality gap #453 records.
    m = json.loads(json.dumps(base))
    m["suites"].append(
        {"name": "counter_demo", "available": True, "kind": "counters", "runner": "builtin", "summary": "s"}
    )
    assert errs(m) == [], errs(m)
    table = "\n".join(render_docs_table(m))
    assert "| `counter_demo` | `perf stat` |" in table, table

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

    # --- #413: an arm must say what it is -------------------------------
    src = """
        library_benchmark_group!(
            name = boolean;
            benchmarks = expanse_and, roaring_and
        );
        library_benchmark_group!(
            name = wand;
            benchmarks =
                expanse_wand,
                roaring_wand
        );
    """
    groups = declared_arms(src)
    assert groups == {
        "boolean": ["expanse_and", "roaring_and"],
        "wand": ["expanse_wand", "roaring_wand"],
    }, groups

    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "crates" / "expanse" / "benches").mkdir(parents=True)
        (root / "crates" / "expanse" / "benches" / "demo.rs").write_text(src, encoding="utf-8")

        def suite(arms):
            return {
                "suites": [
                    {
                        "name": "demo",
                        "available": True,
                        "kind": "callgrind",
                        "runner": "generic",
                        "package": "expanse-trie",
                        "target": "demo",
                        "summary": "s",
                        **({"arms": arms} if arms is not None else {}),
                    }
                ]
            }

        good = {
            "subject_label": "Expanse",
            "baseline_label": "Roaring",
            "twins": [
                {"subject": "expanse_and", "baseline": "roaring_and"},
                {"subject": "expanse_wand", "baseline": "roaring_wand"},
            ],
            "unpaired": [],
        }
        assert check_arms(suite(good), root) == [], check_arms(suite(good), root)

        # A callgrind bench target with no `arms` block at all.
        e = check_arms(suite(None), root)
        assert any("declares no `arms` block" in x for x in e), e

        # An arm added to the source but not classified: the silent-untwinning
        # this check exists to make loud.
        partial = json.loads(json.dumps(good))
        partial["twins"] = partial["twins"][:1]
        e = check_arms(suite(partial), root)
        assert any("not classified in `arms`" in x and "roaring_wand" in x for x in e), e

        # An arm named here that the source no longer declares.
        stale = json.loads(json.dumps(good))
        stale["unpaired"] = ["expanse_gone"]
        e = check_arms(suite(stale), root)
        assert any("no longer declares" in x and "expanse_gone" in x for x in e), e

        # A twin pair must be measured inside one library_benchmark_group!.
        crossed = json.loads(json.dumps(good))
        crossed["twins"] = [
            {"subject": "expanse_and", "baseline": "roaring_wand"},
            {"subject": "expanse_wand", "baseline": "roaring_and"},
        ]
        e = check_arms(suite(crossed), root)
        assert any("spans two library_benchmark_group" in x for x in e), e

        # #417: one competitor kernel is the honest twin of both our composed
        # and our native path, so a baseline may be repeated across twins.
        src2 = """
            library_benchmark_group!(
                name = boolean;
                benchmarks = expanse_and, expanse_native_and, roaring_and
            );
        """
        (root / "crates" / "expanse" / "benches" / "two.rs").write_text(src2, encoding="utf-8")

        def suite2(arms):
            s = suite(arms)
            s["suites"][0]["target"] = "two"
            return s

        two = {
            "subject_label": "Expanse",
            "baseline_label": "Roaring",
            "twins": [
                {"subject": "expanse_and", "baseline": "roaring_and"},
                {"subject": "expanse_native_and", "baseline": "roaring_and"},
            ],
            "unpaired": [],
        }
        assert check_arms(suite2(two), root) == [], check_arms(suite2(two), root)

        # A subject still may not repeat — that would double-count our own arm.
        dup_subject = json.loads(json.dumps(two))
        dup_subject["twins"][1]["subject"] = "expanse_and"
        e = check_arms(suite2(dup_subject), root)
        assert any("declared twice" in x and "expanse_and" in x for x in e), e

        # An arm may not be ours and theirs at once: that decides the `vs main`
        # column and the regression gate.
        both_sides = json.loads(json.dumps(two))
        both_sides["unpaired"] = ["roaring_and"]
        e = check_arms(suite2(both_sides), root)
        assert any("both a subject and a baseline" in x for x in e), e

        # Twins without a label would render a ratio against an unnamed thing.
        unlabelled = json.loads(json.dumps(good))
        del unlabelled["baseline_label"]
        e = check_arms(suite(unlabelled), root)
        assert any("baseline_label" in x for x in e), e

    # --- the `suite` field and the generated index -------------------------
    with tempfile.TemporaryDirectory() as td:
        fake = Path(td)
        bench = fake / SUITE_ROOT
        (bench / "alpha_suite").mkdir(parents=True)
        (bench / "alpha_suite" / "README.md").write_text("# Alpha suite title\n")

        def mf(entries):
            return {"default": "all", "suites": entries}

        tok = {"name": "t_one", "available": True, "kind": "wallclock",
               "runner": "builtin", "summary": "one."}

        # A suite naming a directory that exists, with a README, is accepted.
        ok = dict(tok, suite="alpha_suite")
        assert check_suite_dirs(mf([ok]), fake) == []

        # A suite naming a directory that does not exist is fatal: the index
        # would otherwise render a row whose link 404s (AGENTS.md 8.1).
        e = check_suite_dirs(mf([dict(tok, suite="nope")]), fake)
        assert any("does not exist" in x for x in e), e

        # ...and one that exists but has no README is fatal for the same reason.
        (bench / "empty_suite").mkdir()
        e = check_suite_dirs(mf([dict(tok, suite="empty_suite")]), fake)
        assert any("no README.md" in x for x in e), e

        # A token with no `suite` is legal and is NOT dropped: it must appear
        # under the core heading, so that an unowned token stays visible.
        core = dict(tok, name="t_core")
        body = "\n".join(render_index(mf([ok, core]), fake))
        assert CORE_HEADING in body, "core heading missing"
        assert "`t_core`" in body, "token without a suite was dropped from the index"

        # The suite's description comes from its own README title, never from a
        # member token's summary -- picking one arm described a whole suite by
        # its alphabetically-first token.
        suite_row = next(l for l in body.split("\n") if "alpha_suite/README.md" in l)
        assert "Alpha suite title" in suite_row, suite_row
        assert "one." not in suite_row, (
            f"suite row used a token summary as its description: {suite_row!r}"
        )
        # The core table, by contrast, DOES use each token's own summary.
        core_row = next(l for l in body.split("\n") if "`t_core`" in l)
        assert "one." in core_row, core_row

        # A missing README degrades to the directory name rather than raising.
        assert suite_title(fake, "ghost") == "ghost"

        # Structural validation of the field itself.
        errs, _ = check_manifest(mf([dict(tok, suite="Bad-Name")]))
        assert any("must match" in x for x in errs), errs

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
