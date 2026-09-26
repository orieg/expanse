#!/usr/bin/env python3
"""Acquire/release instruction census of an AArch64 `--emit asm` file (Refs #1191).

Counts, per function, the acquire loads (`ldar*`, RCsc, and `ldapr*`, RCpc),
the release stores (`stlr*`) and the out-of-line atomic calls
(`bl __aarch64_*`, which `aarch64-unknown-linux-gnu` emits for read-modify-write
atomics when `lse` is off), and lists every **straight-line `stlr` -> `ldar`
pair**: a release store followed by an RCsc acquire load in the same function
with no intervening label, branch, call or return. That pair is the ordering
the Arm architecture forbids an implementation to relax under RCsc and permits
it to relax under RCpc (`ldapr`); whether a given core stalls on it is not
something a census can show, only a timed run.

Scope and limits, stated so the output is not over-read:
- a pair is detected only inside one basic block of the linear listing; pairs
  that span a branch or a call are not counted, so the pair count is a floor;
- the census reads code the library crate emits; `#[inline]` generic code
  instantiated only in a downstream binary is not in the file;
- "hot path" is a name filter (`--filter`), not a profile.

Usage:
    python3 rcpc_asm_census.py <file.s> [--filter REGEX] [--json OUT] [--pairs N]
    python3 rcpc_asm_census.py --self-test
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import re
import shutil
import subprocess
import sys

LABEL = re.compile(r"^([A-Za-z_.$][\w.$]*):")
INSN = re.compile(r"^\s+([a-z][a-z0-9.]*)\b(.*)$")
FUNC_TYPE = re.compile(r"^\s+\.type\s+([^,]+),@function")
# Block boundaries for pair detection: any branch, call or return, and any
# local label (a branch target).
BRANCH = re.compile(r"^(b|bl|blr|br|ret|cbz|cbnz|tbz|tbnz|b\.[a-z]+)$")

DEFAULT_FILTER = r"expanse_trie(::|\.\.)(sync|sync32|occ)\b|Sync[A-Z]\w*|SharedBox|MapReader|mutate_map|mutate\b"


def classify(op: str) -> str | None:
    if op.startswith("ldapr") or op.startswith("ldapur"):
        return "ldapr"
    if op.startswith("ldar"):
        return "ldar"
    if op.startswith("stlr") or op.startswith("stlur"):
        return "stlr"
    return None


def parse(lines: list[str]) -> dict[str, dict]:
    """Per function: counts, and the straight-line stlr -> ldar pairs."""
    funcs: set[str] = set()
    for line in lines:
        m = FUNC_TYPE.match(line)
        if m:
            funcs.add(m.group(1).strip())
    out: dict[str, dict] = {}
    cur: dict | None = None
    pending_stlr: int | None = None  # index of the last stlr in this block
    block: list[str] = []  # instructions since that stlr
    idx = 0
    for line in lines:
        m = LABEL.match(line)
        if m:
            name = m.group(1)
            if name in funcs:
                cur = out.setdefault(name, {
                    "ldar": 0, "ldapr": 0, "stlr": 0, "outline_atomics": 0,
                    "pairs": [], "insns": 0,
                })
                idx = 0
            pending_stlr = None  # any label starts a new block
            continue
        if cur is None:
            continue
        m = INSN.match(line)
        if not m:
            continue
        op, rest = m.group(1), m.group(2)
        if op.startswith("."):
            continue
        idx += 1
        cur["insns"] += 1
        kind = classify(op)
        if kind:
            cur[kind] += 1
            if kind == "stlr":
                pending_stlr = idx
                block = []
            elif kind == "ldar" and pending_stlr is not None:
                block.append(f"{op}{rest}".replace("\t", " "))
                cur["pairs"].append({"stlr_at": pending_stlr, "ldar_at": idx,
                                     "distance": idx - pending_stlr,
                                     "sequence": block})
                pending_stlr = None
            elif kind == "ldapr":
                pending_stlr = None
        if pending_stlr is not None and kind != "ldar":
            block.append(f"{op}{rest}".replace("\t", " "))
        if op == "bl" and "__aarch64_" in rest:
            cur["outline_atomics"] += 1
        if BRANCH.match(op):
            pending_stlr = None
    return out


def demangle(names: list[str]) -> dict[str, str]:
    tool = shutil.which("rustfilt")
    if not tool or not names:
        return {n: n for n in names}
    res = subprocess.run([tool], input="\n".join(names), capture_output=True,
                         text=True, check=True)
    return dict(zip(names, res.stdout.splitlines()))


def summarise(per_func: dict[str, dict], filt: str) -> dict:
    names = list(per_func)
    dem = demangle(names)
    rx = re.compile(filt)
    tot = collections.Counter()
    hot = collections.Counter()
    hot_funcs = []
    for n in names:
        f = per_func[n]
        for k in ("ldar", "ldapr", "stlr", "outline_atomics"):
            tot[k] += f[k]
        tot["pairs"] += len(f["pairs"])
        d = dem[n]
        if rx.search(d):
            for k in ("ldar", "ldapr", "stlr", "outline_atomics"):
                hot[k] += f[k]
            hot["pairs"] += len(f["pairs"])
            hot["functions"] += 1
            hot_funcs.append({"symbol": d, "ldar": f["ldar"], "ldapr": f["ldapr"],
                              "stlr": f["stlr"], "outline_atomics": f["outline_atomics"],
                              "stlr_ldar_pairs": len(f["pairs"]),
                              "pair_distances": [p["distance"] for p in f["pairs"]],
                              "pair_sequences": [p["sequence"] for p in f["pairs"]]})
    hot["functions_with_stlr_and_ldar"] = sum(1 for r in hot_funcs if r["stlr"] and r["ldar"])
    hot["ldar_in_functions_with_stlr"] = sum(r["ldar"] for r in hot_funcs if r["stlr"])
    hot_funcs.sort(key=lambda r: (-r["stlr_ldar_pairs"], -r["ldar"], r["symbol"]))
    return {"filter": filt, "totals": dict(tot), "hot": dict(hot), "hot_functions": hot_funcs}


def provenance(invocation: str | None) -> dict:
    """What produced the census (AGENTS.md §8.7): commit, toolchain, command."""
    def out(cmd: list[str]) -> str | None:
        try:
            return subprocess.check_output(cmd, text=True, stderr=subprocess.DEVNULL).strip()
        except (OSError, subprocess.CalledProcessError):
            return None
    commit = os.environ.get("EXPANSE_BENCH_COMMIT") or out(["git", "rev-parse", "--short=8", "HEAD"])
    return {
        "issue": 1191,
        "commit": commit,
        "rustc": out(["rustc", "--version"]),
        "invocation": invocation,
        "demangler": "rustfilt" if shutil.which("rustfilt") else None,
    }


SELF_TEST_ASM = """\
\t.type\tf,@function
f:
\tstlr\tx1, [x0]
\tldr\tx2, [x3]
\tldar\tx4, [x5]
\tstlr\tx1, [x0]
\tb.ne\t.LBB0_2
\tldar\tx4, [x5]
.LBB0_2:
\tstlrb\tw1, [x0]
\tldapr\tx4, [x5]
\tldar\tx4, [x5]
\tbl\t__aarch64_cas8_acq_rel
\tret
"""


def self_test() -> int:
    r = parse(SELF_TEST_ASM.splitlines())["f"]
    assert (r["ldar"], r["ldapr"], r["stlr"], r["outline_atomics"]) == (3, 1, 3, 1), r
    # Only the first stlr -> ldar is straight-line: the second is cut by b.ne,
    # the third by the intervening ldapr.
    assert [p["distance"] for p in r["pairs"]] == [2], r["pairs"]
    assert r["pairs"][0]["sequence"][0].startswith("stlr"), r["pairs"]
    assert r["pairs"][0]["sequence"][-1].startswith("ldar"), r["pairs"]
    print("rcpc_asm_census self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("asm", nargs="?")
    ap.add_argument("--filter", default=DEFAULT_FILTER)
    ap.add_argument("--json")
    ap.add_argument("--invocation", default=None,
                    help="the build command that produced the file, recorded in the JSON (AGENTS.md §8.7)")
    ap.add_argument("--pairs", type=int, default=25, help="functions to print")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if not a.asm:
        ap.error("asm file required")
    with open(a.asm, encoding="utf-8", errors="replace") as fh:
        per = parse(fh.read().splitlines())
    if not per:
        sys.stderr.write(f"error: no functions parsed from {a.asm}\n")
        return 1
    s = summarise(per, a.filter)
    print(f"all functions: {s['totals']}")
    print(f"filtered ({a.filter}): {s['hot']}")
    for r in s["hot_functions"][: a.pairs]:
        print(f"  pairs={r['stlr_ldar_pairs']:3d} ldar={r['ldar']:3d} ldapr={r['ldapr']:3d} "
              f"stlr={r['stlr']:3d} ool={r['outline_atomics']:3d}  {r['symbol']}")
    if a.json:
        s = {"provenance": provenance(a.invocation), **s}
        with open(a.json, "w", encoding="utf-8") as fh:
            json.dump(s, fh, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
