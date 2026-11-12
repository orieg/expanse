#!/usr/bin/env python3
"""`lock`-prefixed instructions per function in two `objdump -d -C` dumps
(README.md, "Diagnostic: unlocked accounting on the delete phase").

Reads the dumps of D (no feature) and A (`ablation-unlocked-plain-acct`) and
writes, beside them:
- `lock_summary.json`: per function, its `lock` instructions in D and in A;
- `map_remove_plain_{D,A}.dis`: the full disassembly of
  `map_remove::<false, false>` in each build.

It prints the functions whose `lock` count differs between the builds, and
every `lock` instruction left in A, and stops if `map_remove::<false, false>`
is missing from either dump.

With `--promotion` the two dumps are M (the promotion's merge base) and P
(its head), and it adds the promotion's disassembly gate (README.md,
"Promotion: unlocked plain-tree accounting"): every `lock` instruction left in
a plain instantiation is listed with the calls within `WINDOW` instructions on
either side, so one on the system-allocator branch shows the allocator call
beside it, and the deferred instantiations' `lock` counts are compared.

Usage: acct_disasm.py [--promotion] <D.dis> <A.dis> <out_dir>
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

FUNC = re.compile(r"^[0-9a-f]+ <(.+)>:$")
INSN = re.compile(r"^\s*([0-9a-f]+):\s+(.*\S)\s*$")
TARGET = "expanse_trie::mutate_map::map_remove::<false, false>"
WINDOW = 12
CALL = re.compile(r"call\s+[0-9a-f]+\s+<(.+?)>")


def plain(name):
    """A plain-tree instantiation: a `<false…>` generic or a C ABI entry point."""
    return "<false" in name or name.startswith(("JudyL", "Judy1", "JudySL", "JudyHS"))


def deferred(name):
    return "<true" in name


def promotion(builds, out):
    """The promotion's disassembly gate over M (old) and P (new)."""
    m, p = builds["D"], builds["A"]
    rows = []
    for f in sorted(p):
        if not plain(f):
            continue
        insns = p[f]
        for i, (a, x) in enumerate(insns):
            if x.startswith("lock "):
                near = [CALL.search(y).group(1) for _, y in insns[max(0, i - WINDOW):i + WINDOW + 1]
                        if CALL.search(y)]
                rows.append({"function": f, "addr": a, "insn": x, "calls_within_window": near})
    (out / "promotion_plain_locks.json").write_text(json.dumps(rows, indent=1) + "\n")
    print(f"plain-instantiation lock instructions in P: {len(rows)} "
          f"(in M: {sum(len(locks(m[f])) for f in m if plain(f))})")
    for r in rows:
        print(f"  {r['function']}: {r['addr']} {r['insn']}  calls near: {r['calls_within_window']}")
    changed = [(f, len(locks(m.get(f, []))), len(locks(p.get(f, []))))
               for f in sorted(set(m) | set(p)) if deferred(f)
               and len(locks(m.get(f, []))) != len(locks(p.get(f, [])))]
    print(f"deferred instantiations whose lock count changed: {len(changed)}")
    for f, a, b in changed:
        print(f"  {a} -> {b}  {f}")


def parse(path):
    """Function name -> list of (address, instruction) lines."""
    funcs, cur = defaultdict(list), None
    for line in Path(path).read_text().splitlines():
        m = FUNC.match(line)
        if m:
            cur = m.group(1)
            continue
        i = INSN.match(line)
        if i and cur:
            funcs[cur].append((i.group(1), i.group(2)))
    return funcs


def locks(insns):
    return [(a, x) for a, x in insns if x.startswith("lock ")]


def main():
    args = sys.argv[1:]
    promo = args[:1] == ["--promotion"]
    if promo:
        args = args[1:]
    d_path, a_path, out = args[0], args[1], Path(args[2])
    out.mkdir(parents=True, exist_ok=True)
    builds = {"D": parse(d_path), "A": parse(a_path)}
    for arm, funcs in builds.items():
        hits = [f for f in funcs if f.startswith(TARGET)]
        if not hits:
            sys.exit(f"STOP: {TARGET} is not in {arm}'s dump")
        (out / f"map_remove_plain_{arm}.dis").write_text(
            "".join(f"{f}:\n" + "".join(f"  {a}: {x}\n" for a, x in funcs[f]) for f in hits))
    names = sorted(set(builds["D"]) | set(builds["A"]))
    summary = {f: {arm: [f"{a}: {x}" for a, x in locks(builds[arm].get(f, []))] for arm in builds}
               for f in names if any(locks(builds[arm].get(f, [])) for arm in builds)}
    (out / "lock_summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    print("functions whose lock count differs (D -> A):")
    for f, v in summary.items():
        if len(v["D"]) != len(v["A"]):
            print(f"  {len(v['D'])} -> {len(v['A'])}  {f}")
    print("lock instructions left in A:")
    for f, v in summary.items():
        for x in v["A"]:
            print(f"  {f}: {x}")
    for arm in builds:
        n = sum(len(locks(builds[arm][f])) for f in builds[arm] if f.startswith(TARGET))
        print(f"{arm}: {n} lock instructions in {TARGET}")
    if promo:
        promotion(builds, out)


if __name__ == "__main__":
    main()
