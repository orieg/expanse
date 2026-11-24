#!/usr/bin/env python3
"""#1682 step A2, static half: code size and layout of the Judy1 entry paths.

For libexpanse (`--expanse <libexpanse.so>`): starting from the exported
Judy1Test, Judy1Set and Judy1Unset, follows direct `call`/`jmp` targets that
land inside the library's own functions (objdump -d), and reports for each
reached function its size, entry address and alignment, the number of 64-byte
lines and 32-byte windows it spans, its direct-call and indirect-jump counts,
plus the path's totals and the library's .text size.

For stock libJudy (`--stock <libJudy.so>`): symbol-table facts only (sizes
and entry addresses of the exported Judy1Test, Judy1Set and Judy1Unset, and
the .text size). It never disassembles stock: AGENTS.md §3 forbids
decompiling libjudy, and a black-box comparison needs only the symbol table.

The reached set is an over-approximation of the colour-map path (it follows
every direct branch, including cold error and growth paths), and it misses
code reached only through an indirect jump or call. Both are stated in the
output.

Usage:
    python3 scripts/colour_code_layout.py --objdump x86_64-linux-gnu-objdump \\
        --expanse target/release/libexpanse.so --stock /usr/lib/.../libJudy.so.1 \\
        --out results/<name>.json
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ENTRIES = ("Judy1Test", "Judy1Set", "Judy1Unset")
LINE, WINDOW = 64, 32


def run(argv: list[str]) -> str:
    p = subprocess.run(argv, capture_output=True, text=True, check=False)
    if p.returncode != 0:
        raise SystemExit(f"{' '.join(argv)} exited {p.returncode}: {p.stderr.strip()[:300]}")
    return p.stdout


def symbols(objdump: str, lib: str) -> dict[int, tuple[str, int]]:
    """{address: (name, size)} for every function symbol, dynamic and static."""
    out: dict[int, tuple[str, int]] = {}
    for flag in ("-t", "-T"):
        for line in run([objdump, flag, "-C", lib]).splitlines():
            m = re.match(r"^([0-9a-f]+)\s.*\sF\s+\.text\s+([0-9a-f]+)\s+(.+)$", line)
            if m:
                addr, size, name = int(m.group(1), 16), int(m.group(2), 16), m.group(3).strip()
                # Only the dynamic table (-T) has a version column before the
                # name; a demangled Rust name contains spaces, so stripping a
                # first token from a -t line would cut the name itself.
                if flag == "-T":
                    name = re.sub(r"^(?:\(?Base\)?|\S*@\S*|\(\S+\))\s+", "", name)
                if size and (addr not in out or len(name) < len(out[addr][0])):
                    out[addr] = (name, size)
    return out


def text_size(objdump: str, lib: str) -> int:
    for line in run([objdump, "-h", lib]).splitlines():
        parts = line.split()
        if len(parts) > 2 and parts[1] == ".text":
            return int(parts[2], 16)
    raise SystemExit(f"{lib}: no .text section")


def spans(addr: int, size: int, unit: int) -> int:
    return (addr + size - 1) // unit - addr // unit + 1


def disasm(objdump: str, lib: str, addr: int, size: int) -> list[str]:
    return run([objdump, "-d", "--no-show-raw-insn", f"--start-address={addr:#x}",
                f"--stop-address={addr + size:#x}", lib]).splitlines()


def expanse(objdump: str, lib: str) -> dict:
    syms = symbols(objdump, lib)
    by_name = {n: a for a, (n, _) in syms.items()}
    missing = [e for e in ENTRIES if e not in by_name]
    if missing:
        raise SystemExit(f"{lib}: entry symbols not found: {missing}")
    starts = sorted(syms)
    todo = [by_name[e] for e in ENTRIES]
    seen: dict[int, dict] = {}
    edges: dict[int, set[int]] = {}
    while todo:
        a = todo.pop()
        if a in seen:
            continue
        name, size = syms[a]
        calls = ijmps = icalls = 0
        callees: set[int] = set()
        for line in disasm(objdump, lib, a, size):
            m = re.match(r"^\s*[0-9a-f]+:\s+(\S+)\s+(.*)$", line)
            if not m:
                continue
            op, arg = m.group(1), m.group(2)
            if op.startswith("call") or op.startswith("jmp"):
                if "*" in arg:
                    if op.startswith("jmp"):
                        ijmps += 1
                    else:
                        icalls += 1
                    continue
                t = re.match(r"^([0-9a-f]+)\b", arg)
                if t:
                    target = int(t.group(1), 16)
                    if op.startswith("call"):
                        calls += 1
                    if target in syms and target != a and not (a <= target < a + size):
                        todo.append(target)
                        callees.add(target)
        edges[a] = callees
        seen[a] = {"name": name, "addr": hex(a), "size": size, "entry_align_64": a % LINE,
                   "lines_64": spans(a, size, LINE), "windows_32": spans(a, size, WINDOW),
                   "direct_calls": calls, "indirect_jumps": ijmps, "indirect_calls": icalls}
    fns = sorted(seen.values(), key=lambda f: -f["size"])

    def reach(entry: str) -> dict:
        stack, got = [by_name[entry]], set()
        while stack:
            x = stack.pop()
            if x not in got:
                got.add(x)
                stack.extend(edges.get(x, ()))
        sub = [seen[x] for x in got]
        return {"functions": sorted(f["name"] for f in sub),
                "bytes": sum(f["size"] for f in sub),
                "lines_64": sum(f["lines_64"] for f in sub),
                "indirect_jumps": sum(f["indirect_jumps"] for f in sub)}

    return {
        "reach_per_entry": {e: reach(e) for e in ENTRIES},
        "library": Path(lib).name,
        "text_size": text_size(objdump, lib),
        "entries": {e: next(f for f in fns if f["addr"] == hex(by_name[e])) for e in ENTRIES},
        "reached_functions": fns,
        "reached_total_bytes": sum(f["size"] for f in fns),
        "reached_total_lines_64": sum(f["lines_64"] for f in fns),
        "reached_indirect_jumps": sum(f["indirect_jumps"] for f in fns),
        "scope_note": "every direct call/jmp target from the three entries, cold paths "
                      "included; code reached only indirectly is not followed",
        "function_count_in_library": len(starts),
    }


def stock(objdump: str, lib: str) -> dict:
    syms = symbols(objdump, lib)
    found = {n: (a, s) for a, (n, s) in syms.items() if n in ENTRIES}
    return {
        "library": Path(lib).name,
        "text_size": text_size(objdump, lib),
        "entries": {n: {"addr": hex(a), "size": s, "entry_align_64": a % LINE,
                        "lines_64": spans(a, s, LINE)} for n, (a, s) in sorted(found.items())},
        "scope_note": "symbol table only; stock is never disassembled (AGENTS.md §3)",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--objdump", default="objdump")
    ap.add_argument("--expanse", required=True)
    ap.add_argument("--stock", required=True)
    ap.add_argument("--commit", default="")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    doc = {"issue": 1682, "step": "A2 static layout", "commit": a.commit,
           "objdump": run([a.objdump, "--version"]).splitlines()[0],
           "expanse": expanse(a.objdump, a.expanse), "stock": stock(a.objdump, a.stock)}
    Path(a.out).write_text(json.dumps(doc, indent=1) + "\n")
    e = doc["expanse"]
    print(f"libexpanse .text {e['text_size']} B; reached {len(e['reached_functions'])} fns, "
          f"{e['reached_total_bytes']} B, {e['reached_total_lines_64']} lines, "
          f"{e['reached_indirect_jumps']} indirect jmps")
    for f in e["reached_functions"][:25]:
        print(f"  {f['size']:6d} B {f['lines_64']:3d} lines align {f['entry_align_64']:2d} "
              f"ij {f['indirect_jumps']} {f['name'][:90]}")
    s = doc["stock"]
    print(f"stock .text {s['text_size']} B; entries {json.dumps(s['entries'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
