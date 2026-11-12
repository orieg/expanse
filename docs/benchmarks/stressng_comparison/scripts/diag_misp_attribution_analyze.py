#!/usr/bin/env python3
"""Reading of the #1680 mispredict and L1D-miss attribution (README.md,
"Diagnostic: where the #1680 shapes mispredict and miss").

Reads `diag_ms_attribution.py`'s two runs (`SNG_ATTR=misp`), writes
`results/diagnostic_misp_attribution_<host>.json` and prints, per shape and
kind, each arm's events per operation (every recorded sample x period /
operations), its largest functions, and N's instructions holding at least 1%
of the annotated samples. A sample outside an annotatable symbol (libc,
stock's unexported functions) counts in the rate and in `annotated_fraction`,
not in the instruction shares; `share_of_all` gives each instruction's share
of every sample.

Source lines: N's library has no line tables. An N instruction is mapped
through the line-table twin (`twin`, built from the same commit at N's build
paths) by function name and offset within the function, and only when that
function's body equals N's once addresses, jump and call targets,
RIP-relative displacements and crate hashes are normalised. `addr2line -i`
on the twin gives the inline chain. Each N instruction at or above 5% is
assigned to a construct by `construct()`, from that chain's innermost frame.

Stability, as the design states it: in both runs, the same constructs hold
N's three largest instruction shares on each shape and kind.

A census, one process per (shape, arm, kind) and run: no interval.

Usage: diag_misp_attribution_analyze.py <host> <run1_json> <run2_json> <N_lib> <twin_lib>
Needs objdump, nm and addr2line (binutils).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

from analyze import RESULTS, shown

PERIOD = 503
ASSIGN = 0.05
TOP = 3


def norm_name(sym):
    sym = re.sub(r"::h[0-9a-f]{16}$", "", sym)
    return re.sub(r"\[[0-9a-f]+\]", "", sym)


def functions(path):
    """Normalised function name -> (start, normalised body), from objdump -d -C."""
    out = subprocess.run(["objdump", "-d", "-C", "--no-show-raw-insn", "-j", ".text", path],
                         capture_output=True, text=True, check=True).stdout
    fs, cur = {}, None
    for line in out.splitlines():
        m = re.match(r"^([0-9a-f]+) <(.+)>:$", line)
        if m:
            cur = norm_name(m.group(2))
            fs[cur] = (int(m.group(1), 16), [])
            continue
        m = re.match(r"^\s+([0-9a-f]+):\s+(.*)$", line)
        if m and cur is not None:
            ins = re.sub(r"<[^>]*>", "<T>", m.group(2))
            ins = re.sub(r"\b(j\w+|call)\s+[0-9a-f]+", r"\1 X", ins)
            ins = re.sub(r"0x[0-9a-f]+\(%rip\)", "R(%rip)", ins)
            fs[cur][1].append(re.sub(r"#.*$", "", ins).strip())
    if not fs:
        sys.exit(f"STOP: objdump found no functions in {path}")
    return fs


def containing(fs, addr):
    """The function whose start is the greatest at or below `addr`."""
    best = None
    for name, (start, body) in fs.items():
        if start <= addr and (best is None or start > fs[best][0]):
            best = name
    return best


def inline_chain(twin, addr):
    out = subprocess.run(["addr2line", "-f", "-i", "-C", "-e", twin, hex(addr)],
                         capture_output=True, text=True, check=True).stdout.splitlines()
    frames = []
    for fn, loc in zip(out[0::2], out[1::2]):
        loc = re.sub(r"^.*?/(crates/|library/)", r"\1", loc)
        frames.append(f"{fn} @ {loc}")
    return frames


# The design's constructs. Innermost source frame -> construct, first match
# wins: (construct, file pattern, pattern the source line must contain or
# None). Anything else is `other`, reported with its chain, never folded into
# a named construct.
CONSTRUCTS = [
    ("tag dispatch", r"crates/expanse/src/(get|mutate_map|mutate)\.rs", r"\btag\b|tag_byte"),
    ("leaf search", r"crates/expanse/src/leaf\.rs", None),
    ("branch digit search", r"crates/expanse/src/(node|bits|bitmap)\.rs", None),
    ("insert bookkeeping", r"crates/expanse/src/(mutate_map|mutate)\.rs", None),
    ("allocation", r"crates/expanse/src/(alloc|slab)\.rs", None),
]
LIBC_ALLOC = re.compile(r"malloc|calloc|realloc|free|_int_")
N_COMMIT = "5f4892932"
_SOURCES = {}


def construct(row):
    """Assigns an N instruction to one of the design's constructs."""
    if "expanse" not in (row["dso"] or ""):
        return "allocation" if LIBC_ALLOC.search(row["symbol"] or "") else "other"
    frames = row.get("inline_chain") or []
    if not frames:
        return "other"
    for name, file_re, text_re in CONSTRUCTS:
        if re.search(file_re, frames[0]) and (text_re is None or re.search(text_re, row.get("source", ""))):
            return name
    return "other"


def source_text(frame, repo):
    """The source line a frame names, read at N's commit, not the checkout."""
    m = re.search(r"@ (crates/[^:]+):(\d+)", frame)
    if not m:
        return ""
    if m.group(1) not in _SOURCES:
        r = subprocess.run(["git", "-C", str(repo), "show", f"{N_COMMIT}:{m.group(1)}"],
                           capture_output=True, text=True)
        _SOURCES[m.group(1)] = r.stdout.splitlines() if r.returncode == 0 else []
    lines, n = _SOURCES[m.group(1)], int(m.group(2))
    return lines[n - 1].strip() if 0 < n <= len(lines) else ""


def main():
    host, j1, j2, nlib, twin = sys.argv[1:6]
    repo = Path(__file__).resolve().parents[4]
    runs = [json.loads(Path(j).read_text()) for j in (j1, j2)]
    nf, tf = functions(nlib), functions(twin)
    # Run 1's provenance at the top, as the provenance gate reads it; run 2's
    # nested beside it, so both runs' hosts, pins and load windows are kept.
    out = {"provenance": dict(runs[0]["provenance"], run2_provenance=runs[1]["provenance"]), "cells": []}
    tops = {}
    for run_no, art in enumerate(runs, 1):
        for c in art["cells"]:
            key = (c["workload_id"], c["kind"], c["arm"])
            cell = {"run": run_no, "workload_id": c["workload_id"], "kind": c["kind"], "arm": c["arm"],
                    "event": c["event"], "samples": c["perf_script_samples"],
                    "annotated_samples": c["samples"],
                    "annotated_fraction": c["samples"] / c["perf_script_samples"],
                    "ops": c["ops"], "events_per_op": c["perf_script_samples"] * PERIOD / c["ops"],
                    "functions": c["functions"][:8], "instructions": []}
            for x in c["instructions"]:
                row = {k: x[k] for k in ("dso", "symbol", "addr", "insn", "share")}
                row["share_of_all"] = x["samples"] / c["perf_script_samples"]
                if c["arm"] == "N" and "expanse" in (x["dso"] or ""):
                    addr = int(x["addr"], 16)
                    fn = containing(nf, addr)
                    row["function"] = fn
                    if fn in tf and nf[fn][1] == tf[fn][1]:
                        taddr = tf[fn][0] + (addr - nf[fn][0])
                        row["inline_chain"] = inline_chain(twin, taddr)
                        row["source"] = source_text(row["inline_chain"][0], repo) if row["inline_chain"] else ""
                    else:
                        row["inline_chain"], row["source"] = None, ""
                        row["unmapped"] = "function body differs from the twin's, or absent"
                if c["arm"] == "N" and x["share"] >= ASSIGN:
                    row["construct"] = construct(row)
                cell["instructions"].append(row)
            out["cells"].append(cell)
            if c["arm"] == "N":
                assigned = [r for r in cell["instructions"] if "construct" in r]
                tops.setdefault(key[:2], {})[run_no] = [r["construct"] for r in assigned[:TOP]]
    stable = {f"{w} {k}": (len(v) == 2 and v[1] == v[2]) for (w, k), v in tops.items()}
    out["stability"] = {"rule": f"same constructs hold N's {TOP} largest instruction shares (>= {ASSIGN:.0%}) in both runs",
                        "per_shape_kind": stable, "tops": {f"{w} {k}": v for (w, k), v in tops.items()}}
    dest = RESULTS / f"diagnostic_misp_attribution_{host}.json"
    dest.write_text(json.dumps(out, indent=1) + "\n")
    print(f"-> {shown(dest)}")
    for c in out["cells"]:
        print(f"\n## run {c['run']} {c['workload_id']} {c['kind']} {c['arm']}: {c['events_per_op']:.3f} events/op "
              f"({c['samples']:.0f} samples, {c['annotated_fraction']:.1%} annotated)")
        for f in c["functions"][:4]:
            print(f"   {f['pct']:5.1f}% {f['symbol'][:90]}")
        for r in c["instructions"][:8]:
            where = (r.get("inline_chain") or [""])[0][:110]
            print(f"   {r['share']:.3f} ({r['share_of_all']:.3f} of all) {r.get('construct', ''):20} "
                  f"{r['insn'][:36]:36} {where}")
            if r.get("source"):
                print(f"         `{r['source'][:100]}`")
    print("\nstability:", json.dumps(out["stability"]["per_shape_kind"]))
    if not all(stable.values()):
        print("UNSTABLE: at least one shape and kind changes its top constructs between runs; nothing is read from it")


if __name__ == "__main__":
    main()
