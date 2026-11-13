#!/usr/bin/env python3
"""Parsing of `perf report` and `perf annotate` text for `diag_ms_attribution.py`.

Separate so that `diag_ms_attribution_analyze.py` can parse the recorder's raw
text again without the recorder's host environment.
"""
import re

MIN_INSN_SHARE = 0.01
HEADER = re.compile(r"Disassembly of (\S+) for .*\((\d+) samples")
INSN = re.compile(r"^\s*([0-9]+\.[0-9]+)\s*:\s+([0-9a-f]+):\s+(\S.*?)\s*$")
# A string instruction under a `rep` prefix: the hypothesis the design names.
REP_STRING = re.compile(r"^rep\w*\s+(movs|stos|cmps|scas|lods)")
# perf 6.8 prints a function's first line as `: <line>  <address> <name>:`.
SYM = re.compile(r"^\s*:\s*(?:\d+\s+)?[0-9a-f]+\s+<(.+)>:\s*$")


def parse(rep, ann):
    """Function shares from `perf report` text, instruction shares from `perf annotate` text."""
    functions = []
    for line in rep.splitlines():
        parts = line.split()
        if len(parts) >= 4 and parts[0].endswith("%"):
            functions.append({"pct": float(parts[0][:-1]), "dso": parts[1], "symbol": " ".join(parts[3:])})
    insns, dso, sym, n = [], None, None, 0
    for line in ann.splitlines():
        h = HEADER.search(line)
        if h:
            dso, n, sym = h.group(1), int(h.group(2)), None
            continue
        m = SYM.match(line)
        if m and sym is None:
            sym = m.group(1)
            continue
        i = INSN.match(line)
        if i and n:
            insns.append({"dso": dso, "symbol": sym, "local_pct": float(i.group(1)),
                          "samples": float(i.group(1)) / 100 * n, "addr": i.group(2),
                          "insn": i.group(3)})
    total = sum(x["samples"] for x in insns)
    for x in insns:
        x["share"] = x["samples"] / total if total else 0.0
    top = sorted((x for x in insns if x["share"] >= MIN_INSN_SHARE), key=lambda x: -x["share"])
    rep = sum(x["share"] for x in insns if REP_STRING.match(x["insn"]))
    return functions, top, total, rep
