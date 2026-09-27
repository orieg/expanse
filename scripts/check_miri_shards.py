#!/usr/bin/env python3
"""scripts/check_miri_shards.py — Miri shard census for `expanse-trie`.

Covers both sharded Miri lanes: the nightly Tier-3 lane (below) and the
per-PR Tier-1 lane (`ci.yml`, job `miri`; see "Tier 1" at the end of this
docstring).

The nightly Miri lane (`.github/workflows/nightly.yml`, job `miri-full`) runs
the crate's tests as a matrix of shards because one job never fits the hosted
runner's cap. A shard scheme has two ways to rot silently:

1. A lib test can belong to no shard, or to two. libtest filters are substring
   matches, so a shard filter `map::` also selects `blobmap::`, `bytesmap::`,
   `strmap::` and `mutate_map::`, and `map32::` selects `blobmap32::` — the
   scheme this script replaced ran `blobmap` twice and put `strmap` in the
   set/map shard by accident. Lib shards are therefore defined here as a map
   from **top-level module** to shard, and each shard runs its tests by exact
   name (`--exact`), never by substring.
2. An integration target (`crates/expanse/tests/*.rs`) can be added without a
   shard. Every such file must either be a matrix entry or carry a file-level
   `#![cfg(not(miri))]`.

Rules (each fatal):
- Every lib test (from `cargo test -p expanse-trie --lib -- --list`) is
  assigned to exactly one lib shard; `lib-rest` is the complement of the
  named shards, so a new module lands there rather than nowhere.
- Every module named in `LIB_SHARDS` has at least one test (no stale entries).
- The matrix `shard:` list in `nightly.yml` equals the lib shard names plus
  the integration targets that are not `#![cfg(not(miri))]`.
- Every entry in the matrix that is not a lib shard is an existing file under
  `crates/expanse/tests/`.

Tier 1 (per-PR, `ci.yml` job `miri`) is a matrix whose shards carry libtest
*substring* filters, written out again in `scripts/gate.sh` and `AGENTS.md`
§5 as one unsharded command. Rules (each fatal):
- The shards are named `1 of N` .. `N of N`, and no filter word is in two.
- The union of the shards' filter words equals the `gate.sh` list and the
  `AGENTS.md` list.
- Every lib test any filter word selects is selected by exactly one shard,
  every shard selects a test, and every filter word matches a test.
- The `ablation_` step's `if: matrix.shard == '...'` names a shard.

Usage:
  python3 scripts/check_miri_shards.py                 # census (runs cargo)
  python3 scripts/check_miri_shards.py --list-file F   # census from a saved --list
  python3 scripts/check_miri_shards.py --select SHARD  # print the exact test
                                                       # names of a lib shard,
                                                       # reading `--list` output
                                                       # on stdin (used by the
                                                       # workflow)
  python3 scripts/check_miri_shards.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NIGHTLY = ROOT / ".github" / "workflows" / "nightly.yml"
CI = ROOT / ".github" / "workflows" / "ci.yml"
GATE = ROOT / "scripts" / "gate.sh"
AGENTS = ROOT / "AGENTS.md"
TESTS_DIR = ROOT / "crates" / "expanse" / "tests"

# Top-level lib module -> shard. Anything not listed runs in `lib-rest`.
# Sizing reference: a full run on a 5 GHz core (docs/CI.md §5, Tier 3).
LIB_SHARDS: dict[str, list[str]] = {
    "lib-cursor": ["cursor", "cursor32"],
    "lib-set": ["set", "set32"],
    "lib-map": ["map", "map32"],
    "lib-sync": ["sync", "sync32"],
    "lib-blobmap-bits": ["blobmap", "bits"],
}
REST_SHARD = "lib-rest"

FILE_LEVEL_NOT_MIRI = re.compile(r"^#!\[cfg\(not\(miri\)\)\]", re.M)
SHARD_LINE = re.compile(r"^\s*shard:\s*\[([^\]]*)\]", re.M)


def parse_list_output(text: str) -> list[str]:
    """Names from `cargo test -- --list` (`path::to::test: test` lines)."""
    return [ln[: -len(": test")] for ln in text.splitlines() if ln.endswith(": test")]


def module_of(test_name: str) -> str:
    return test_name.split("::", 1)[0]


def shard_of(test_name: str) -> str:
    mod = module_of(test_name)
    for shard, mods in LIB_SHARDS.items():
        if mod in mods:
            return shard
    return REST_SHARD


def assign(tests: list[str]) -> dict[str, list[str]]:
    out: dict[str, list[str]] = {s: [] for s in [*LIB_SHARDS, REST_SHARD]}
    for t in tests:
        out[shard_of(t)].append(t)
    return out


def matrix_shards(nightly_text: str) -> list[str]:
    m = SHARD_LINE.search(nightly_text)
    if not m:
        raise RuntimeError("nightly.yml: no `shard: [...]` matrix line found")
    return [s.strip() for s in m.group(1).split(",") if s.strip()]


def integration_targets(tests_dir: Path) -> tuple[list[str], list[str]]:
    """(miri-runnable targets, file-level not(miri) targets)."""
    runnable, excluded = [], []
    for f in sorted(tests_dir.glob("*.rs")):
        if FILE_LEVEL_NOT_MIRI.search(f.read_text(encoding="utf-8")):
            excluded.append(f.stem)
        else:
            runnable.append(f.stem)
    return runnable, excluded


def census(tests: list[str], nightly_text: str, tests_dir: Path) -> list[str]:
    errors: list[str] = []
    by_shard = assign(tests)

    # Each test in exactly one shard is structural in `assign`; what can rot
    # is the module map itself.
    present = {module_of(t) for t in tests}
    for shard, mods in LIB_SHARDS.items():
        for mod in mods:
            if mod not in present:
                errors.append(f"{shard}: module `{mod}` has no tests — stale LIB_SHARDS entry")
    seen: dict[str, str] = {}
    for shard, mods in LIB_SHARDS.items():
        for mod in mods:
            if mod in seen:
                errors.append(f"module `{mod}` listed in both {seen[mod]} and {shard}")
            seen[mod] = shard
    for shard, names in by_shard.items():
        if not names:
            errors.append(f"{shard}: selects no tests")

    runnable, _excluded = integration_targets(tests_dir)
    expected = [*LIB_SHARDS, REST_SHARD, *runnable]
    actual = matrix_shards(nightly_text)
    for s in expected:
        if s not in actual:
            kind = "lib shard" if s.startswith("lib-") else "integration target"
            errors.append(f"nightly.yml matrix is missing {kind} `{s}`")
    for s in actual:
        if s.startswith("lib-") and s not in (*LIB_SHARDS, REST_SHARD):
            errors.append(f"nightly.yml matrix names unknown lib shard `{s}`")
        elif not s.startswith("lib-") and not (tests_dir / f"{s}.rs").exists():
            errors.append(f"nightly.yml matrix names `{s}` but crates/expanse/tests/{s}.rs does not exist")
        elif not s.startswith("lib-") and s not in runnable:
            errors.append(f"nightly.yml matrix lists `{s}`, which is `#![cfg(not(miri))]` and cannot run")
    return errors


TIER1_JOB = re.compile(r"^  miri:\n(.*?)(?=^  \S)", re.M | re.S)
TIER1_SHARD = re.compile(r'^\s*- shard:\s*(.+?)\s*\n\s*filter:\s*"([^"]*)"', re.M)
TIER1_ABLATION_IF = re.compile(r"^\s*if:\s*matrix\.shard\s*==\s*'([^']*)'", re.M)
# The unsharded Tier-1 command in gate.sh and AGENTS.md: the `--lib --` form
# with no `--features`, continued over backslash-newlines.
TIER1_CMD = re.compile(r"cargo miri test -p expanse-trie --lib -- ((?:[^\n\\]|\\\n)*)")


def tier1_shards(ci_text: str) -> tuple[list[tuple[str, list[str]]], list[str]]:
    """([(shard name, filter words)], ablation-step shard names) from ci.yml."""
    m = TIER1_JOB.search(ci_text)
    if not m:
        raise RuntimeError("ci.yml: no `miri:` job found")
    body = m.group(1)
    shards = [(name, words.split()) for name, words in TIER1_SHARD.findall(body)]
    if not shards:
        raise RuntimeError("ci.yml: the `miri` job has no `- shard:` / `filter:` matrix entries")
    return shards, TIER1_ABLATION_IF.findall(body)


def tier1_command_words(text: str, where: str) -> list[str]:
    m = TIER1_CMD.search(text)
    if not m:
        raise RuntimeError(f"{where}: no unsharded Tier-1 `cargo miri test -p expanse-trie --lib -- ...` command found")
    return m.group(1).replace("\\\n", " ").split()


def tier1_census(tests: list[str], ci_text: str, gate_text: str, agents_text: str) -> list[str]:
    errors: list[str] = []
    shards, ablation_ifs = tier1_shards(ci_text)
    names = [n for n, _ in shards]
    n = len(shards)
    if names != [f"{i} of {n}" for i in range(1, n + 1)]:
        errors.append(f"Tier-1 shards must be named `1 of {n}` .. `{n} of {n}` in order, found {names}")
    owner: dict[str, str] = {}
    for name, words in shards:
        for w in words:
            if w in owner:
                errors.append(f"Tier-1 filter `{w}` is in both `{owner[w]}` and `{name}`")
            owner.setdefault(w, name)
    union = set(owner)
    for where, text in (("scripts/gate.sh", gate_text), ("AGENTS.md", agents_text)):
        listed = set(tier1_command_words(text, where))
        for w in sorted(union - listed):
            errors.append(f"Tier-1 filter `{w}` is in ci.yml but not in {where}")
        for w in sorted(listed - union):
            errors.append(f"Tier-1 filter `{w}` is in {where} but in no ci.yml shard")
    for w in sorted(union):
        if not any(w in t for t in tests):
            errors.append(f"Tier-1 filter `{w}` matches no lib test — stale")
    for name, words in shards:
        if not any(w in t for t in tests for w in words):
            errors.append(f"Tier-1 shard `{name}` selects no tests")
    for t in tests:
        hit = [name for name, words in shards if any(w in t for w in words)]
        if len(hit) > 1:
            errors.append(f"Tier-1 test `{t}` is selected by shards {hit}")
    for a in ablation_ifs:
        if a not in names:
            errors.append(f"ci.yml `miri` step runs on shard `{a}`, which is not a Tier-1 shard")
    return errors


def cargo_list() -> list[str]:
    cp = subprocess.run(
        ["cargo", "test", "-q", "-p", "expanse-trie", "--lib", "--", "--list"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if cp.returncode != 0:
        raise RuntimeError(f"cargo test --list failed ({cp.returncode}):\n{cp.stderr}")
    return parse_list_output(cp.stdout)


def self_test() -> None:
    tests = [
        "cursor::tests::a",
        "cursor32::tests::b",
        "set::tests::c",
        "map::tests::d",
        "set32::tests::e",
        "map32::tests::f",
        "sync::tests::g",
        "sync32::tests::h",
        "blobmap::tests::i",
        "bits::tests::j",
        "strmap::tests::k",
        "bytesmap::tests::l",
        "mutate_map::tests::m",
        "blobmap32::tests::n",
        "leaf::tests::o",
    ]
    # The motivating defect: substring filters put strmap/bytesmap/mutate_map in
    # the set/map shard and blobmap32 in it too, and ran blobmap twice. By
    # module they are `lib-rest`, and blobmap is in one shard only.
    by = assign(tests)
    assert shard_of("strmap::tests::k") == REST_SHARD
    assert shard_of("bytesmap::tests::l") == REST_SHARD
    assert shard_of("mutate_map::tests::m") == REST_SHARD
    assert shard_of("blobmap32::tests::n") == REST_SHARD
    assert sum("blobmap::tests::i" in v for v in by.values()) == 1
    assert sorted(sum(by.values(), [])) == sorted(tests), "every test in exactly one shard"

    import tempfile

    with tempfile.TemporaryDirectory() as td:
        d = Path(td)
        (d / "linearizability.rs").write_text("fn main(){}\n")
        (d / "proptest_model.rs").write_text("//! doc\n#![cfg(not(miri))]\n")
        good_yaml = "    shard: [lib-cursor, lib-set, lib-map, lib-sync, lib-blobmap-bits, lib-rest, linearizability]\n"
        assert census(tests, good_yaml, d) == [], census(tests, good_yaml, d)

        # 1. New integration file with no shard and no not(miri) gate → fatal.
        (d / "test_new_thing.rs").write_text("#[test] fn t(){}\n")
        errs = census(tests, good_yaml, d)
        assert any("missing integration target `test_new_thing`" in e for e in errs), errs
        (d / "test_new_thing.rs").unlink()

        # 2. Matrix names a file that does not exist → fatal.
        errs = census(tests, good_yaml.replace("linearizability", "linearizability, ghost"), d)
        assert any("ghost.rs does not exist" in e for e in errs), errs

        # 3. Matrix lists a not(miri) target → fatal.
        errs = census(tests, good_yaml.replace("linearizability", "linearizability, proptest_model"), d)
        assert any("proptest_model" in e and "cannot run" in e for e in errs), errs

        # 4. Matrix drops a lib shard → fatal.
        errs = census(tests, good_yaml.replace("lib-sync, ", ""), d)
        assert any("missing lib shard `lib-sync`" in e for e in errs), errs

        # 5. Stale module in the map → fatal.
        errs = census([t for t in tests if not t.startswith("cursor32::")], good_yaml, d)
        assert any("`cursor32` has no tests" in e for e in errs), errs

        # 6. Matrix line absent → loud failure, not a pass.
        try:
            census(tests, "steps: []\n", d)
        except RuntimeError as e:
            assert "no `shard:" in str(e)
        else:
            raise AssertionError("missing matrix line must raise")

    # --select output is exact names, one per line, for the named shard only.
    sel = select(tests, "lib-blobmap-bits")
    assert sel == ["blobmap::tests::i", "bits::tests::j"], sel
    assert "strmap::tests::k" in select(tests, REST_SHARD)
    # A shard whose modules are all gated off Miri selects nothing; that is
    # reported, not fatal (the workflow warns and skips).
    assert select([t for t in tests if not t.startswith(("sync::", "sync32::"))], "lib-sync") == []
    # Tier 1. A test two shards select, a filter word the other lists lack,
    # a stale word, a misnamed shard and a dangling ablation `if:` are each
    # fatal on their own.
    t1 = ["map::tests::compact_a", "map::tests::occ_engine_x", "blobmap::tests::deferred_y", "leaf::tests::z"]
    ci = (
        "jobs:\n  miri:\n    strategy:\n      matrix:\n        include:\n"
        '          - shard: 1 of 2\n            filter: "map::tests::compact_ leaf::"\n'
        '          - shard: 2 of 2\n            filter: "map::tests::occ_engine_x blobmap::tests::deferred"\n'
        "    steps:\n      - name: ab\n        if: matrix.shard == '2 of 2'\n  next:\n    x: 1\n"
    )
    cmd = "cargo miri test -p expanse-trie --lib -- leaf:: map::tests::compact_ \\\n  map::tests::occ_engine_x blobmap::tests::deferred\n"
    assert tier1_census(t1, ci, cmd, cmd) == [], tier1_census(t1, ci, cmd, cmd)
    errs = tier1_census(t1, ci.replace('"map::tests::occ_engine_x blob', '"map:: blob'), cmd.replace("occ_engine_x", "occ_engine_x map::"), cmd.replace("occ_engine_x", "occ_engine_x map::"))
    assert any("`map::tests::compact_a` is selected by shards" in e for e in errs), errs
    errs = tier1_census(t1, ci, cmd.replace(" leaf::", ""), cmd)
    assert any("`leaf::` is in ci.yml but not in scripts/gate.sh" in e for e in errs), errs
    errs = tier1_census(t1, ci, cmd, cmd.replace("leaf::", "leaf:: node::"))
    assert any("`node::` is in AGENTS.md but in no ci.yml shard" in e for e in errs), errs
    errs = tier1_census([x for x in t1 if not x.startswith("leaf")], ci, cmd, cmd)
    assert any("`leaf::` matches no lib test" in e for e in errs), errs
    errs = tier1_census(t1, ci.replace("shard: 2 of 2", "shard: 2 of 3"), cmd, cmd)
    assert any("must be named" in e for e in errs), errs
    errs = tier1_census(t1, ci.replace("== '2 of 2'", "== '3 of 3'"), cmd, cmd)
    assert any("shard `3 of 3`, which is not a Tier-1 shard" in e for e in errs), errs
    try:
        tier1_census(t1, "jobs:\n  lint:\n    x: 1\n", cmd, cmd)
    except RuntimeError as e:
        assert "no `miri:` job" in str(e)
    else:
        raise AssertionError("missing Tier-1 job must raise")
    print("check_miri_shards.py --self-test: ok")


def select(tests: list[str], shard: str) -> list[str]:
    if shard not in (*LIB_SHARDS, REST_SHARD):
        raise SystemExit(f"--select: `{shard}` is not a lib shard (lib shards: {', '.join([*LIB_SHARDS, REST_SHARD])})")
    return assign(tests)[shard]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--list-file", type=Path, help="saved `cargo test -- --list` output instead of running cargo")
    ap.add_argument("--select", metavar="SHARD", help="print the exact test names of a lib shard; reads `--list` output on stdin")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return 0
    if args.select:
        # The census runs on the native list; the workflow feeds this the
        # Miri-visible list, which is smaller (`sync::tests` is
        # `cfg(all(test, not(miri)))`, so `lib-sync` is `sync32` alone under
        # the interpreter). An empty selection is therefore a legitimate
        # outcome of gating, not an error: print nothing and say so on stderr,
        # and let the workflow surface the skip as a warning annotation.
        names = select(parse_list_output(sys.stdin.read()), args.select)
        if not names:
            print(f"--select {args.select}: no tests visible to this build", file=sys.stderr)
        else:
            print("\n".join(names))
        return 0

    tests = parse_list_output(args.list_file.read_text()) if args.list_file else cargo_list()
    errors = census(tests, NIGHTLY.read_text(encoding="utf-8"), TESTS_DIR)
    errors += tier1_census(
        tests,
        CI.read_text(encoding="utf-8"),
        GATE.read_text(encoding="utf-8"),
        AGENTS.read_text(encoding="utf-8"),
    )
    by = assign(tests)
    for shard in [*LIB_SHARDS, REST_SHARD]:
        mods = sorted({module_of(t) for t in by[shard]})
        print(f"  {shard:<18} {len(by[shard]):3d} tests  modules: {', '.join(mods)}")
    if errors:
        for e in errors:
            print(f"::error::check_miri_shards.py: {e}")
        print(f"check_miri_shards.py: {len(errors)} fatal finding(s)")
        return 1
    shards, _ = tier1_shards(CI.read_text(encoding="utf-8"))
    for name, words in shards:
        picked = [t for t in tests if any(w in t for w in words)]
        print(f"  Tier-1 {name:<11} {len(picked):3d} tests  filters: {' '.join(words)}")
    print(f"check_miri_shards.py: {len(tests)} lib tests in {len(LIB_SHARDS) + 1} shards, integration targets covered; Tier-1 shards partition their filter; ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
