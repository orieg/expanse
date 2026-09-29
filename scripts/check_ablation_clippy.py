#!/usr/bin/env python3
"""scripts/check_ablation_clippy.py — keep every ablation feature compiling.

AGENTS.md §2.7 keeps a promoted default re-testable through an
`ablation-un…` inverse, and the inverse is only re-testable while it builds.
The workspace `clippy` in the `lint` job compiles default features alone, so a
change that leaves an item dead under an ablation feature fails nowhere until
someone next reaches for the ablation. #1280 found `ablation-blob-serial-writers`
and `ablation-str-serial-writers` both failing `clippy -D warnings` on `main`.

The plan is derived from the tree, never enumerated by hand:

- **Features**: every `ablation-*` feature of `crates/expanse/Cargo.toml`.
- **Retired names**: a feature with a single-feature `compile_error!` guard in
  `crates/expanse/src/lib.rs` (§2.7 item 2). Each is checked to still fail, with
  a diagnostic naming the feature (AGENTS.md §5, string-gated negative
  controls), never run as a clippy configuration.
- **Exclusive pairs**: a two-feature `all(...)` `compile_error!` guard in
  `lib.rs`. A combination containing one is skipped.
- **Combinations**: two live features named in one `cfg`/`cfg_attr` predicate
  under `crates/expanse/src` interact, since the predicate compiles an item for
  some of their combinations and not others. The features that interact form
  connected groups, and every subset of two or more members of a group, less
  any containing an exclusive pair, is a configuration. Every live feature also
  runs alone.

Residual: an item reached by two consumers gated on features that are never
named together in one predicate is covered only when both features fall in
one group. On the tree that introduced this script, the one such item
(`with_locked_pre`, reached by the blob and bytes wrappers) did.

Usage:
  python3 scripts/check_ablation_clippy.py            # run the plan
  python3 scripts/check_ablation_clippy.py --list     # print the plan only
  python3 scripts/check_ablation_clippy.py --self-test
"""

from __future__ import annotations

import argparse
import itertools
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CRATE = REPO_ROOT / "crates" / "expanse"
CARGO_TOML = CRATE / "Cargo.toml"
LIB_RS = CRATE / "src" / "lib.rs"
SRC = CRATE / "src"
PACKAGE = "expanse-trie"
PREFIX = "ablation-"

_FEATURE = re.compile(r'feature\s*=\s*"([^"]+)"')
_CFG_OPEN = re.compile(r"#!?\[cfg(?:_attr)?\(")
_ANSI = re.compile(r"\x1b\[[0-9;]*m")


def ablation_features(cargo_toml: str) -> list[str]:
    """The `ablation-*` keys of the `[features]` table, in declaration order."""
    out, in_features = [], False
    for line in cargo_toml.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_features = stripped == "[features]"
            continue
        if not in_features or stripped.startswith("#"):
            continue
        m = re.match(r"([A-Za-z0-9_-]+)\s*=", stripped)
        if m and m.group(1).startswith(PREFIX):
            out.append(m.group(1))
    return out


def cfg_predicates(source: str) -> list[str]:
    """The full text of every `#[cfg(...)]` / `#[cfg_attr(...)]` attribute,
    multi-line ones included, matched by parenthesis depth."""
    preds = []
    for m in _CFG_OPEN.finditer(source):
        i, depth = m.end(), 1
        while depth and i < len(source):
            depth += (source[i] == "(") - (source[i] == ")")
            i += 1
        if depth:
            raise ValueError(f"unbalanced cfg attribute at offset {m.start()}")
        preds.append(source[m.start():i])
    return preds


def compile_error_guards(lib_rs: str) -> list[list[str]]:
    """The feature sets whose `cfg` guards a `compile_error!` in `lib.rs`: the
    predicate immediately preceding each `compile_error!`, if it names features
    and nothing else (a target guard is not a feature guard)."""
    guards = []
    for m in _CFG_OPEN.finditer(lib_rs):
        pred = cfg_predicates(lib_rs[m.start():])[0]
        after = lib_rs[m.start() + len(pred):].lstrip()
        after = after[1:].lstrip() if after.startswith("]") else after
        if not after.startswith("compile_error!"):
            continue
        feats = _FEATURE.findall(pred)
        if feats and "target_" not in pred and "not(" not in pred:
            guards.append(sorted(set(feats)))
    return guards


def interaction_groups(predicates: list[str], live: set[str]) -> list[list[str]]:
    """Connected groups of live features named together in one predicate."""
    parent = {f: f for f in live}

    def find(f):
        while parent[f] != f:
            parent[f] = parent[parent[f]]
            f = parent[f]
        return f

    for pred in predicates:
        named = sorted({f for f in _FEATURE.findall(pred) if f in live})
        for a, b in zip(named, named[1:]):
            parent[find(a)] = find(b)
    groups: dict[str, list[str]] = {}
    for f in sorted(live):
        groups.setdefault(find(f), []).append(f)
    return [g for g in groups.values() if len(g) > 1]


def plan(cargo_toml: str, lib_rs: str, sources: list[str]):
    """(configurations, retired, exclusive_pairs). Fails loudly on an empty
    plan: a parser that silently found nothing would pass every tree."""
    features = ablation_features(cargo_toml)
    if not features:
        raise ValueError(f"no {PREFIX}* features found in {CARGO_TOML}")
    guards = compile_error_guards(lib_rs)
    retired = sorted({g[0] for g in guards if len(g) == 1 and g[0] in features})
    exclusive = sorted({tuple(g) for g in guards if len(g) == 2})
    live = [f for f in features if f not in retired]
    if not live:
        raise ValueError("every ablation feature is retired; nothing to compile")
    preds = [p for s in sources for p in cfg_predicates(s)]
    configs = [(f,) for f in live]
    for group in interaction_groups(preds, set(live)):
        for k in range(2, len(group) + 1):
            for combo in itertools.combinations(group, k):
                if any(set(pair) <= set(combo) for pair in exclusive):
                    continue
                configs.append(combo)
    return configs, retired, exclusive


def load_plan():
    sources = [p.read_text(encoding="utf-8") for p in sorted(SRC.rglob("*.rs"))]
    return plan(CARGO_TOML.read_text(encoding="utf-8"), LIB_RS.read_text(encoding="utf-8"), sources)


def diagnostic_names(log: str, feat: str) -> bool:
    """True when an `error:` line of `log` names `feat`. ANSI colour is
    stripped first: CI forces cargo's colour on, and a coloured line starts
    with an escape code, not `error:`."""
    return any(
        ln.startswith("error:") and feat in ln
        for ln in _ANSI.sub("", log).splitlines()
    )


def run(cmd: list[str]) -> tuple[int, str]:
    try:
        proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    except FileNotFoundError as e:  # §8.1: a missing toolchain is a failure
        raise SystemExit(f"error: cannot run {cmd[0]}: {e}")
    return proc.returncode, proc.stdout + proc.stderr


def execute(configs, retired) -> int:
    failures = []
    for combo in configs:
        feats = ",".join(combo)
        cmd = ["cargo", "clippy", "--color", "never", "-p", PACKAGE, "--all-targets",
               "--features", feats, "--", "-D", "warnings"]
        print(f"::group::clippy --features {feats}", flush=True)
        rc, log = run(cmd)
        print(log, flush=True)
        print("::endgroup::", flush=True)
        if rc != 0:
            errs = sorted({ln for ln in _ANSI.sub("", log).splitlines()
                           if ln.startswith("error") and "could not compile" not in ln})
            failures.append((feats, errs))
            print(f"::error::clippy -D warnings fails with --features {feats}")
    for feat in retired:
        # A retired name must still fail, and fail on its own compile_error!,
        # not on an unrelated build error (AGENTS.md §5 string-gated controls).
        rc, log = run(["cargo", "check", "--color", "never", "-p", PACKAGE,
                       "--features", feat])
        named = diagnostic_names(log, feat)
        if rc == 0 or not named:
            why = "builds" if rc == 0 else "fails without a diagnostic naming it"
            failures.append((f"retired {feat}", [why]))
            print(f"::error::retired feature {feat} {why}; §2.7 needs its compile_error!")
    print(f"\n{len(configs)} clippy configurations, {len(retired)} retired names checked")
    for feats, errs in failures:
        print(f"FAIL {feats}")
        for e in errs:
            print(f"    {e}")
    if not failures:
        print("all ablation configurations pass")
    return 1 if failures else 0


def self_test() -> int:
    fails = []

    def check(label, got, want):
        if got != want:
            fails.append(f"{label}: got {got!r}, want {want!r}")

    toml = (
        '[package]\nname = "x"\n[features]\ndefault = ["std"]\n'
        '# ablation-commented = []\nablation-a = []\nablation-b = []\n'
        'ablation-c = []\nablation-old = []\nablation-x = []\nother = []\n'
        '[dependencies]\nablation-dep = "1"\n'
    )
    check("feature table", ablation_features(toml),
          ["ablation-a", "ablation-b", "ablation-c", "ablation-old", "ablation-x"])
    lib = (
        '#[cfg(not(any(target_pointer_width = "64")))]\ncompile_error!("w");\n'
        '#[cfg(feature = "ablation-old")]\ncompile_error!("ablation-old is retired");\n'
        '#[cfg(all(\n    feature = "ablation-a",\n    feature = "ablation-b"\n))]\n'
        'compile_error!("pick one");\n'
        '#[cfg(feature = "ablation-x")]\nmod not_an_error;\n'
    )
    check("guards", compile_error_guards(lib),
          [["ablation-old"], ["ablation-a", "ablation-b"]])
    srcs = [
        '#[cfg(all(not(feature = "ablation-a"),\n  not(feature = "ablation-c")))]\nfn f() {}\n',
        '#[cfg_attr(all(feature = "ablation-b", feature = "ablation-c"), allow(x))]\n',
        '#[cfg(feature = "ablation-x")]\nfn g() {}\n',
    ]
    configs, retired, exclusive = plan(toml, lib, srcs)
    check("retired", retired, ["ablation-old"])
    check("exclusive", exclusive, [("ablation-a", "ablation-b")])
    # a-c and b-c join {a, b, c}; x stands alone; {a, b} and {a, b, c} are excluded.
    check("configs", configs, [
        ("ablation-a",), ("ablation-b",), ("ablation-c",), ("ablation-x",),
        ("ablation-a", "ablation-c"), ("ablation-b", "ablation-c"),
    ])
    # The retired-name check read CI's coloured cargo output as unnamed (#1286's
    # first run): the line began with an escape code, not `error:`.
    coloured = "\x1b[1m\x1b[91merror\x1b[0m\x1b[1m: ablation-old is retired\x1b[0m\n"
    check("coloured diagnostic", diagnostic_names(coloured, "ablation-old"), True)
    check("plain diagnostic", diagnostic_names("error: ablation-old is retired\n", "ablation-old"), True)
    check("other feature", diagnostic_names("error: ablation-new is retired\n", "ablation-old"), False)
    check("not an error line", diagnostic_names("note: ablation-old\n", "ablation-old"), False)
    try:
        plan("[features]\nstd = []\n", lib, srcs)
        fails.append("an empty feature table did not fail")
    except ValueError:
        pass

    # The live tree: the configurations #1280 found failing on `main` must be
    # in the plan, so the job cannot pass by planning around them.
    configs, retired, exclusive = load_plan()
    for must in [
        ("ablation-blob-serial-writers",),
        ("ablation-str-serial-writers",),
        ("ablation-blob-serial-writers", "ablation-str-serial-writers"),
        ("ablation-blob-shared-arena", "ablation-bytes-serial-writers"),
    ]:
        if tuple(sorted(must)) not in {tuple(sorted(c)) for c in configs}:
            fails.append(f"live plan is missing {must}")
    if "ablation-blob-writer-arenas" not in retired:
        fails.append("live plan does not treat ablation-blob-writer-arenas as retired")
    if ("ablation-blob-serial-writers", "ablation-blob-shared-arena") not in exclusive:
        fails.append("live plan lost the blob serial/shared-arena exclusion")

    for f in fails:
        print(f"FAIL {f}")
    print(f"check_ablation_clippy self-test: {'FAIL' if fails else 'ok'}")
    return 1 if fails else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--list", action="store_true", help="print the plan and exit")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    configs, retired, exclusive = load_plan()
    if args.list:
        for c in configs:
            print("clippy", ",".join(c))
        for f in retired:
            print("retired", f)
        for a, b in exclusive:
            print("exclusive", f"{a},{b}")
        return 0
    return execute(configs, retired)


if __name__ == "__main__":
    sys.exit(main())
