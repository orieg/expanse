#!/usr/bin/env python3
"""`lock`-prefixed instructions per function in two `objdump -d -C` dumps
(README.md, "Diagnostic: unlocked accounting on the delete phase").

Reads the dumps of D (no feature) and A (`ablation-unlocked-plain-acct`, retired
since the promotion and built from its own commit) and
writes, beside them:
- `lock_summary.json`: per function, its `lock` instructions in D and in A;
- `map_remove_plain_{D,A}.dis`: the full disassembly of
  `map_remove::<false, false>` in each build.

It prints the functions whose `lock` count differs between the builds, and
every `lock` instruction left in A, and stops if `map_remove::<false, false>`
is missing from either dump.

Usage: acct_disasm.py <D.dis> <A.dis> <out_dir>
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

FUNC = re.compile(r"^[0-9a-f]+ <(.+)>:$")
INSN = re.compile(r"^\s*([0-9a-f]+):\s+(.*\S)\s*$")
TARGET = "expanse_trie::mutate_map::map_remove::<false, false>"


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
    d_path, a_path, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
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


if __name__ == "__main__":
    main()
