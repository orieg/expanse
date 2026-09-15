#!/usr/bin/env python3
"""The single-threaded bound on the optimistic-seek implementation (#802, #900).

`docs/benchmarks/rocksdb_memtable/METHODOLOGY.md` section 5.16 fixes, before
any wall-clock run of the optimistic-seek arm, a bound on what the change costs
the default scope (`kFullLocate`) on one thread. This file executes it.

**The instrument.** Callgrind `Ir`, inclusive, per entry point, from one
`bench_memtable --arm <phase> --round 0` process per phase, run with
`--collect-atstart=no`. The harness is built with `-DEXPANSE_BENCH_CALLGRIND`,
under which `CALLGRIND_TOGGLE_COLLECT` requests bracket the selected phase's
`ExpanseMemTable` timed loop and, separately, the `ApproximateMemoryUsage` call,
so the fixture fill every process runs is not counted in a read phase.

**Base, head and the control.** Each build comes from its own checkout in its
own directory, with its own release `libexpanse`. The code under test -- the
memtable source, its header and `libexpanse` -- comes from that checkout; the
harness, which is the instrument, is one file compiled into every build, and
it compiles against a header from before `kOptimistic`
(`EXPANSE_MEMTABLE_HAS_OPTIMISTIC_SEEK`). Two independent builds of the base
must give identical inclusive `Ir` and calls per entry point: Callgrind is
deterministic, so this checks the pipeline, and a non-zero control leaves the
bound unmet rather than widened.

**The bound.** No entry point's inclusive `Ir` rises by more than 0.1% (AGENTS.md
section 6). A change beyond 0.1% in either direction is listed for per-function
attribution. Each row publishes calls, inclusive `Ir`, `Ir` per call and the
per-call budget (`ir_budget_per_call` in `scripts/rocksdb_locate_bound.py`)
beside the head-minus-base difference.

**No thread-local access on the default paths (R7).**

- On the object file: every TLS relocation, and every relocation against
  `__tls_get_addr`, a `_ZTW*` or a `_ZTH*` symbol, lies inside the noinline
  lookup function. Read twice, from `readelf` and from `objdump -dr`, and the
  two must agree.
- In the call graph: in every default-scope cell the lookup function and every
  `expanse_sync_map_reader_*` symbol have 0 calls. The same names must have
  calls in the `kOptimistic` cells, so a matcher that finds nothing cannot pass.

The `kOptimistic` cells are measured and reported, never bounded.

Usage:
    python3 docs/benchmarks/rocksdb_memtable/scripts/locate_ir_bound.py run \\
        --base-a DIR --base-b DIR --head DIR --out-dir DIR
    python3 docs/benchmarks/rocksdb_memtable/scripts/locate_ir_bound.py tls-scan --object expanse_memtable.o
    python3 docs/benchmarks/rocksdb_memtable/scripts/locate_ir_bound.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from rocksdb_locate_bound import ir_budget_per_call  # noqa: E402

#: Section 5.16's budget: 0.1% of an entry point's inclusive `Ir`.
BOUND_FRACTION = 0.001
#: The Makefile's flags (`integrations/rocksdb/Makefile`), used for every build.
BENCH_CXXFLAGS = ("-std=c++20", "-O3", "-Wall", "-Wextra", "-pthread")
#: Phase -> the entry points section 5.16 reads in that process. Matched on the
#: demangled name up to its parameter list, so a signature change is refused by
#: name rather than silently matching nothing.
PHASE_ENTRY_POINTS = {
    "fillrandom": ("rocksdb::ExpanseMemTableRep::Insert",),
    "readrandom": ("rocksdb::ExpanseMemTableRep::Get",),
    "seekrandom": ("rocksdb::ExpanseMemTableRep::IteratorImpl::Seek",),
    "prefixscan": ("rocksdb::ExpanseMemTableRep::IteratorImpl::Next",
                   "rocksdb::ExpanseMemTableRep::IteratorImpl::ScanBatch"),
}
#: Called once after its phase by every process, bracketed separately.
MEMORY_ENTRY_POINT = "rocksdb::ExpanseMemTableRep::ApproximateMemoryUsage"
#: The noinline per-thread handle lookup (section 5.16, R7).
LOOKUP_DEMANGLED = "rocksdb::ExpanseMemTableRep::OptimisticReaderHandle"
LOOKUP_MANGLED_PART = "OptimisticReaderHandle"
READER_SYMBOL_PREFIX = "expanse_sync_map_reader_"
#: Section 5.16's list, and the 64-bit forms of the same models.
TLS_RELOCATION = re.compile(
    r"^R_X86_64_(TPOFF32|TPOFF64|GOTTPOFF|TLSGD|TLSLD|DTPOFF32|DTPOFF64|DTPMOD64|"
    r"GOTPC32_TLSDESC|TLSDESC_CALL|TLSDESC)$")


def is_tls_relocation(rtype: str, symbol: str) -> bool:
    """A TLS relocation type, or a relocation against a TLS access helper."""
    return bool(TLS_RELOCATION.match(rtype)) or symbol == "__tls_get_addr" \
        or symbol.startswith("_ZTW") or symbol.startswith("_ZTH")


# --- Callgrind output -----------------------------------------------------------

def parse_callgrind(text: str) -> dict:
    """Self `Ir`, call-line `Ir` and call counts per function, from a `callgrind.out`.

    Follows the format's name compression (`fn=(id) name`, then `fn=(id)`) and
    subposition compression (`+n`, `-n`, `*`). A cost line directly after a
    `calls=` line is the inclusive cost of that call and is attributed to the
    caller's call cost, not its self cost. Returns
    `{"self": {fn: Ir}, "call_cost": {fn: Ir}, "calls_in": {fn: count},
    "recursive": set(fn), "totals": Ir or None}`.
    """
    names: dict[str, str] = {}
    positions = 1
    event_index = None
    fn = None
    cfn = None
    pending_call = None
    self_ir: dict[str, int] = {}
    call_cost: dict[str, int] = {}
    calls_in: dict[str, int] = {}
    recursive: set[str] = set()
    totals = None

    def resolve(spec: str) -> str:
        m = re.match(r"^\((\d+)\)(?:\s+(.*))?$", spec.strip())
        if not m:
            return spec.strip()
        ident, name = m.group(1), m.group(2)
        if name is not None:
            names[ident] = name
            return name
        if ident not in names:
            raise ValueError(f"compressed name ({ident}) used before it was defined")
        return names[ident]

    for raw in text.splitlines():
        line = raw.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        if line.startswith("positions:"):
            positions = len(line.split(":", 1)[1].split())
            continue
        if line.startswith("events:"):
            events = line.split(":", 1)[1].split()
            if "Ir" not in events:
                raise ValueError(f"no Ir event in {line!r}")
            event_index = events.index("Ir")
            continue
        if line.startswith("totals:"):
            totals = int(line.split(":", 1)[1].split()[0])
            continue
        if line.startswith("fn="):
            fn = resolve(line[3:])
            cfn = None
            continue
        if line.startswith("cfn="):
            cfn = resolve(line[4:])
            continue
        if line.startswith("calls="):
            if fn is None or cfn is None:
                raise ValueError(f"calls= line without fn= and cfn=: {line!r}")
            pending_call = int(line[6:].split()[0])
            continue
        if line[0].isdigit() or line[0] in "+-*":
            if event_index is None:
                raise ValueError("cost line before the events: header")
            if fn is None:
                raise ValueError(f"cost line outside any fn=: {line!r}")
            parts = line.split()
            ev = parts[positions:]
            ir = int(ev[event_index]) if len(ev) > event_index else 0
            if pending_call is not None:
                call_cost[fn] = call_cost.get(fn, 0) + ir
                calls_in[cfn] = calls_in.get(cfn, 0) + pending_call
                if cfn == fn:
                    recursive.add(fn)
                pending_call = None
            else:
                self_ir[fn] = self_ir.get(fn, 0) + ir
            continue
        # fl=, fi=, fe=, ob=, cfi=, cob=, jump=, jcnd=, version:, cmd:, ... carry no cost.
    return {"self": self_ir, "call_cost": call_cost, "calls_in": calls_in,
            "recursive": recursive, "totals": totals}


def function_names(parsed: dict) -> set[str]:
    return set(parsed["self"]) | set(parsed["call_cost"]) | set(parsed["calls_in"])


def match_function(parsed: dict, qualified: str) -> str | None:
    """The one function whose demangled name is `qualified` up to its parameter list.

    `None` when no function matches; raises when more than one does, because an
    overload or a clone would make the inclusive count ambiguous.
    """
    hits = sorted(n for n in function_names(parsed) if n.split("(", 1)[0] == qualified)
    if len(hits) > 1:
        raise ValueError(f"{qualified}: {len(hits)} functions match: {hits}")
    return hits[0] if hits else None


def entry_point(parsed: dict, qualified: str) -> dict:
    """`{"name", "calls", "inclusive_ir"}` for one entry point, or raises."""
    name = match_function(parsed, qualified)
    if name is None:
        raise ValueError(f"{qualified}: not in the call graph")
    if name in parsed["recursive"]:
        raise ValueError(f"{name} calls itself; its inclusive Ir would count the recursion twice")
    calls = parsed["calls_in"].get(name, 0)
    if calls < 1:
        raise ValueError(f"{name}: no call record reaches it inside the collected region")
    return {"name": name, "calls": calls,
            "inclusive_ir": parsed["self"].get(name, 0) + parsed["call_cost"].get(name, 0)}


def reader_symbol_calls(parsed: dict) -> dict[str, int]:
    """Calls, inside the collected region, to the lookup and each `expanse_sync_map_reader_*` symbol."""
    out = {}
    for name, count in parsed["calls_in"].items():
        if name.split("(", 1)[0] == LOOKUP_DEMANGLED or name.startswith(READER_SYMBOL_PREFIX):
            out[name] = count
    return out


# --- Relocations on the object file -----------------------------------------------

def parse_readelf_sections(text: str) -> dict[int, dict]:
    """`readelf -SW`: `{index: {"name", "type", "offset", "info"}}`."""
    out = {}
    for line in text.splitlines():
        m = re.match(r"^\s*\[\s*(\d+)\]\s+(\S*)\s+(\S+)\s+([0-9a-f]+)\s+([0-9a-f]+)\s+([0-9a-f]+)\s+"
                     r"([0-9a-f]+)\s+([A-Za-z]*)\s+(\d+)\s+(\d+)\s+(\d+)\s*$", line)
        if m:
            out[int(m.group(1))] = {"name": m.group(2), "type": m.group(3),
                                    "offset": int(m.group(5), 16), "info": int(m.group(10))}
    return out


def parse_readelf_symbols(text: str) -> list[dict]:
    """`readelf -sW`: FUNC symbols with a numeric section index."""
    out = []
    for line in text.splitlines():
        m = re.match(r"^\s*\d+:\s+([0-9a-f]+)\s+(\d+)\s+FUNC\s+\S+\s+\S+\s+(\d+)\s+(\S+)", line)
        if m:
            out.append({"value": int(m.group(1), 16), "size": int(m.group(2)),
                        "section": int(m.group(3)), "name": m.group(4)})
    return out


def parse_readelf_relocations(text: str, sections: dict[int, dict]) -> list[dict]:
    """`readelf -rW`: every relocation with the section it patches, by index."""
    by_offset = {s["offset"]: idx for idx, s in sections.items()}
    out = []
    target = None
    for line in text.splitlines():
        m = re.match(r"^Relocation section '([^']+)' at offset 0x([0-9a-f]+) contains", line)
        if m:
            rel_idx = by_offset.get(int(m.group(2), 16))
            if rel_idx is None:
                raise ValueError(f"relocation section {m.group(1)} at 0x{m.group(2)} has no section header")
            target = sections[rel_idx]["info"]
            continue
        m = re.match(r"^([0-9a-f]+)\s+[0-9a-f]+\s+(R_\S+)\s*(?:[0-9a-f]+\s+(\S+))?", line)
        if m and target is not None:
            out.append({"section": target, "offset": int(m.group(1), 16), "type": m.group(2),
                        "symbol": m.group(3) or ""})
    return out


def containing_function(symbols: list[dict], section: int, offset: int) -> str | None:
    hits = [s["name"] for s in symbols
            if s["section"] == section and s["value"] <= offset < s["value"] + s["size"]]
    return hits[0] if len(hits) == 1 else (None if not hits else "|".join(sorted(hits)))


def tls_sites_from_readelf(sections_txt: str, symbols_txt: str, relocs_txt: str) -> list[tuple[str, str, str]]:
    """`(function, relocation type, symbol)` for every TLS relocation, from `readelf`."""
    sections = parse_readelf_sections(sections_txt)
    symbols = parse_readelf_symbols(symbols_txt)
    sites = []
    for r in parse_readelf_relocations(relocs_txt, sections):
        if is_tls_relocation(r["type"], r["symbol"]):
            fn = containing_function(symbols, r["section"], r["offset"]) or "<no function>"
            sites.append((fn, r["type"], r["symbol"]))
    return sorted(sites)


def tls_sites_from_objdump(text: str) -> list[tuple[str, str, str]]:
    """The same list from `objdump -dr`, keyed by the enclosing `<function>:` label."""
    sites = []
    fn = None
    for line in text.splitlines():
        m = re.match(r"^[0-9a-f]+ <([^>]+)>:\s*$", line)
        if m:
            fn = m.group(1)
            continue
        m = re.match(r"^\s+[0-9a-f]+:\s+(R_\S+)\s+(\S+)", line)
        if m and is_tls_relocation(m.group(1), m.group(2).split("+", 1)[0].split("-", 1)[0]):
            sites.append((fn or "<no function>", m.group(1), m.group(2).split("+", 1)[0].split("-", 1)[0]))
    return sorted(sites)


def tls_verdict(readelf_sites: list[tuple[str, str, str]], objdump_sites: list[tuple[str, str, str]]) -> dict:
    """R7 on the object file: every TLS site is in the lookup function, and both tools agree."""
    problems = []
    if readelf_sites != objdump_sites:
        problems.append(f"readelf and objdump disagree: {readelf_sites} vs {objdump_sites}")
    if not readelf_sites:
        problems.append("no TLS relocation found at all: the lookup's thread-local was not seen, so a "
                        "scan that finds nothing elsewhere proves nothing")
    outside = [s for s in readelf_sites if LOOKUP_MANGLED_PART not in s[0]]
    if outside:
        problems.append(f"TLS access outside the lookup function: {outside}")
    return {"verdict": "MET" if not problems else "UNMET", "sites": readelf_sites, "problems": problems}


def tls_scan(obj: Path) -> dict:
    def run(*cmd: str) -> str:
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            raise RuntimeError(f"{' '.join(cmd)} exited {res.returncode}: {res.stderr}")
        return res.stdout
    return tls_verdict(tls_sites_from_readelf(run("readelf", "-SW", str(obj)), run("readelf", "-sW", str(obj)),
                                              run("readelf", "-rW", str(obj))),
                       tls_sites_from_objdump(run("objdump", "-dr", str(obj))))


# --- The bound --------------------------------------------------------------------

def compare_entry(base: dict, base_b: dict, head: dict, fraction: float = BOUND_FRACTION) -> dict:
    """One row of section 5.16's table: base, control, head, budget and verdict."""
    per_call, budget = ir_budget_per_call(base["inclusive_ir"], base["calls"], fraction)
    delta = head["inclusive_ir"] - base["inclusive_ir"]
    rel = delta / base["inclusive_ir"] if base["inclusive_ir"] else 0.0
    control_equal = (base_b["inclusive_ir"], base_b["calls"]) == (base["inclusive_ir"], base["calls"])
    if head["calls"] != base["calls"]:
        verdict = "CALLS_DIFFER"
    elif rel > fraction:
        verdict = "RISES_BEYOND_BOUND"
    elif rel < -fraction:
        verdict = "FALLS_BEYOND_BOUND"
    else:
        verdict = "WITHIN_BOUND"
    return {"calls": base["calls"], "base_ir": base["inclusive_ir"], "control_ir": base_b["inclusive_ir"],
            "control_equal": control_equal, "head_calls": head["calls"], "head_ir": head["inclusive_ir"],
            "ir_per_call": per_call, "budget_per_call": budget, "delta_ir": delta,
            "delta_per_call": delta / base["calls"], "relative": rel, "verdict": verdict}


def bound_verdict(rows: list[dict], zero_call_hits: dict, positive_hits: dict, tls: dict) -> dict:
    """Section 5.16's bound over every entry point, the call graph and the object file."""
    reasons = []
    if not rows:
        reasons.append("no entry point was measured")
    for r in rows:
        where = f"{r['phase']} {r['entry']}"
        if not r["control_equal"]:
            reasons.append(f"{where}: the two base builds differ ({r['base_ir']} vs {r['control_ir']} Ir), "
                           f"so the pipeline is not reproducible and the bound is unmet")
        if r["verdict"] == "CALLS_DIFFER":
            reasons.append(f"{where}: head made {r['head_calls']} calls, base {r['calls']}")
        if r["verdict"] == "RISES_BEYOND_BOUND":
            reasons.append(f"{where}: inclusive Ir rose {r['relative']:.4%}, above the {BOUND_FRACTION:.1%} bound")
    for cell, hits in sorted(zero_call_hits.items()):
        if hits:
            reasons.append(f"{cell}: default-scope cell calls {hits}")
    if not positive_hits.get("lookup"):
        reasons.append("no kOptimistic cell calls the lookup function, so the zero-call check matched nothing")
    if not positive_hits.get("reader"):
        reasons.append(f"no kOptimistic cell calls an {READER_SYMBOL_PREFIX}* symbol, so the zero-call check "
                       f"matched nothing")
    if tls.get("verdict") != "MET":
        reasons.extend(f"object file: {p}" for p in tls.get("problems", ["TLS scan did not run"]))
    attribute = [f"{r['phase']} {r['entry']}" for r in rows if r["verdict"] in ("RISES_BEYOND_BOUND", "FALLS_BEYOND_BOUND")]
    return {"verdict": "MET" if not reasons else "UNMET", "reasons": reasons, "attribute_per_function": attribute}


def render_table(rows: list[dict]) -> str:
    out = ["| phase | entry point | calls | base inclusive Ir | control | head inclusive Ir | Ir/call | "
           "0.1% budget/call | head − base | per call | relative | verdict |",
           "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in rows:
        name = r["entry"].split("(", 1)[0].rsplit("::", 1)[-1]
        control = "identical" if r["control_equal"] else f"{r['control_ir']:,}"
        out.append(f"| `{r['phase']}` | `{name}` | {r['calls']:,} | {r['base_ir']:,} | {control} | "
                   f"{r['head_ir']:,} | {r['ir_per_call']:.2f} | {r['budget_per_call']:.3f} | {r['delta_ir']:+,} | "
                   f"{r['delta_per_call']:+.3f} | {r['relative']:+.4%} | {r['verdict']} |")
    return "\n".join(out)


def render_reported(rows: list[dict]) -> str:
    out = ["| phase | entry point | calls | `kFullLocate` inclusive Ir | `kOptimistic` inclusive Ir | relative |",
           "|---|---|---|---|---|---|"]
    for r in rows:
        out.append(f"| `{r['phase']}` | `{r['entry'].rsplit('::', 1)[-1]}` | {r['calls']:,} | {r['full_ir']:,} | "
                   f"{r['opt_ir']:,} | {r['relative']:+.4%} |")
    return "\n".join(out)


# --- Running it -------------------------------------------------------------------

def sh(cmd: list[str], cwd: Path | None = None, env: dict | None = None, log: Path | None = None) -> None:
    res = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True)
    if log is not None:
        log.write_text(f"$ {' '.join(cmd)}\n{res.stdout}\n{res.stderr}")
    if res.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)} (cwd {cwd}) exited {res.returncode}\n{res.stdout[-4000:]}\n{res.stderr[-4000:]}")


def build(checkout: Path, harness: Path, out: Path, cxx: str) -> dict:
    """Release libexpanse, the memtable object and the harness, all from `checkout` except the harness."""
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(checkout / "target"))
    sh(["cargo", "build", "--release", "-p", "expanse-capi"], cwd=checkout, env=env, log=out / "cargo.log")
    inc = [f"-I{checkout / 'integrations' / 'rocksdb' / 'include'}", f"-I{checkout / 'include'}"]
    memtable_o = out / "expanse_memtable.o"
    sh([cxx, *BENCH_CXXFLAGS, *inc, "-c", str(checkout / "integrations" / "rocksdb" / "src" / "expanse_memtable.cc"),
        "-o", str(memtable_o)], log=out / "memtable.log")
    harness_o = out / "bench_memtable.o"
    sh([cxx, *BENCH_CXXFLAGS, "-DEXPANSE_BENCH_CALLGRIND", *inc, "-c", str(harness), "-o", str(harness_o)],
       log=out / "harness.log")
    exe = out / "bench_memtable"
    sh([cxx, *BENCH_CXXFLAGS, str(harness_o), str(memtable_o), str(checkout / "target" / "release" / "libexpanse.a"),
        "-lpthread", "-ldl", "-lm", "-o", str(exe)], log=out / "link.log")
    return {"exe": exe, "memtable_o": memtable_o}


def measure(exe: Path, phase: str, scope: str, out: Path) -> dict:
    cg = out / f"{phase}.{scope}.callgrind.out"
    cmd = ["valgrind", "--tool=callgrind", "--collect-atstart=no", f"--callgrind-out-file={cg}",
           str(exe), "--arm", phase, "--round", "0"]
    if scope != "full":
        cmd += ["--scope", scope]
    sh(cmd, log=out / f"{phase}.{scope}.log")
    return parse_callgrind(cg.read_text())


def run(args: argparse.Namespace) -> int:
    # Absolute, because cargo runs with each checkout as its working directory
    # and a relative CARGO_TARGET_DIR would resolve inside it a second time.
    args.base_a, args.base_b, args.head = (p.resolve() for p in (args.base_a, args.base_b, args.head))
    for p in (args.base_a, args.base_b, args.head):
        if not (p / "integrations" / "rocksdb" / "src" / "expanse_memtable.cc").is_file():
            print(f"::error::{p} is not an expanse checkout with the RocksDB integration", file=sys.stderr)
            return 2
    if len({len(p.name) for p in (args.base_a, args.base_b, args.head)}) != 1:
        print("::error::the three checkout directories must have names of equal length, so no path "
              "string embedded in a build differs in length between them", file=sys.stderr)
        return 2
    out = args.out_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)
    harness = args.head / "integrations" / "rocksdb" / "benches" / "bench_memtable.cc"
    builds = {name: build(path, harness, out / name, args.cxx)
              for name, path in (("base-a", args.base_a), ("base-b", args.base_b), ("head", args.head))}
    parsed = {name: {phase: measure(b["exe"], phase, "full", out / name) for phase in PHASE_ENTRY_POINTS}
              for name, b in builds.items()}
    parsed_opt = {phase: measure(builds["head"]["exe"], phase, "opt", out / "head") for phase in PHASE_ENTRY_POINTS}

    rows, reported = [], []
    for phase, entries in PHASE_ENTRY_POINTS.items():
        for entry in (*entries, MEMORY_ENTRY_POINT):
            base = entry_point(parsed["base-a"][phase], entry)
            row = compare_entry(base, entry_point(parsed["base-b"][phase], entry),
                                entry_point(parsed["head"][phase], entry))
            rows.append({"phase": phase, "entry": base["name"], **row})
            full = entry_point(parsed["head"][phase], entry)
            opt = entry_point(parsed_opt[phase], entry)
            reported.append({"phase": phase, "entry": base["name"], "calls": opt["calls"],
                             "full_ir": full["inclusive_ir"], "opt_ir": opt["inclusive_ir"],
                             "relative": (opt["inclusive_ir"] - full["inclusive_ir"]) / full["inclusive_ir"]})
    zero_hits = {f"{name} {phase}": reader_symbol_calls(p) for name in parsed for phase, p in parsed[name].items()}
    opt_hits = {phase: reader_symbol_calls(p) for phase, p in parsed_opt.items()}
    positive = {"lookup": any(n.split("(", 1)[0] == LOOKUP_DEMANGLED for h in opt_hits.values() for n in h),
                "reader": any(n.startswith(READER_SYMBOL_PREFIX) for h in opt_hits.values() for n in h)}
    tls = tls_scan(builds["head"]["memtable_o"])
    verdict = bound_verdict(rows, zero_hits, positive, tls)

    artifact = {"schema": "expanse.rocksdb_locate_ir_bound.v1",
                "pre_registration": "docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section 5.16",
                "instrument": "callgrind Ir, inclusive per entry point, --collect-atstart=no, "
                              "CALLGRIND_TOGGLE_COLLECT around the timed loop",
                "bound_fraction": BOUND_FRACTION, "checkouts": {k: str(v) for k, v in
                                                                (("base-a", args.base_a), ("base-b", args.base_b),
                                                                 ("head", args.head))},
                "rows": rows, "reported_optimistic": reported, "zero_call_cells": zero_hits,
                "optimistic_reader_calls": opt_hits, "tls": tls, **verdict}
    (out / "locate_ir_bound.json").write_text(json.dumps(artifact, indent=2, default=list) + "\n")
    text = ["## Single-threaded bound (kFullLocate, base vs head)", "", render_table(rows), "",
            "## Reported, not bounded (head: kFullLocate vs kOptimistic)", "", render_reported(reported), "",
            f"TLS relocations ({tls['verdict']}): {tls['sites']}", "",
            f"Zero-call check: default-scope cells with reader calls: "
            f"{ {k: v for k, v in zero_hits.items() if v} or 'none'}; kOptimistic cells: {opt_hits}", "",
            f"**Bound: {verdict['verdict']}**"]
    text += [f"- {r}" for r in verdict["reasons"]]
    if verdict["attribute_per_function"]:
        text.append(f"- beyond 0.1% in either direction, to attribute per function: {verdict['attribute_per_function']}")
    (out / "locate_ir_bound.md").write_text("\n".join(text) + "\n")
    print("\n".join(text))
    return 0 if verdict["verdict"] == "MET" else 1


# --- Self-test --------------------------------------------------------------------

def self_test() -> int:
    fails: list[str] = []

    def check(name, got, want):
        if got != want:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    def raises(name, call, exc=ValueError):
        try:
            call()
        except exc:
            return
        except Exception as e:  # noqa: BLE001
            fails.append(f"{name}: raised {type(e).__name__}, expected {exc.__name__}")
            return
        fails.append(f"{name}: did not raise")

    # A call graph: main calls Get twice (inclusive 30 + 34), Get costs 10 + 12
    # itself and calls the lookup (3 per call) and a reader symbol (5 per call).
    cg = """# callgrind format
version: 1
positions: line
events: Ir
summary: 200
fl=(1) bench.cc
fn=(1) main
10 7
cfl=(2) memtable.cc
cfn=(2) rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&, void*, bool (*)(void*, char const*))
calls=2 40
+2 64
fl=(2)
fn=(2)
40 10
+1 12
cfn=(3) rocksdb::ExpanseMemTableRep::OptimisticReaderHandle() const
calls=2 90
* 6
cfl=(3) libexpanse
cfn=(4) expanse_sync_map_reader_prev_at_or_before
calls=2 5
* 10
fn=(3)
90 6
fn=(4)
5 10
totals: 87
"""
    p = parse_callgrind(cg)
    get = entry_point(p, "rocksdb::ExpanseMemTableRep::Get")
    check("Get calls", get["calls"], 2)
    check("Get inclusive = self 22 + calls 6 + 10", get["inclusive_ir"], 38)
    check("main inclusive = self 7 + call 64", p["self"]["main"] + p["call_cost"]["main"], 71)
    check("totals", p["totals"], 87)
    check("reader calls", reader_symbol_calls(p),
          {"rocksdb::ExpanseMemTableRep::OptimisticReaderHandle() const": 2,
           "expanse_sync_map_reader_prev_at_or_before": 2})
    raises("absent entry point", lambda: entry_point(p, "rocksdb::ExpanseMemTableRep::Insert"))
    raises("undefined compressed name", lambda: parse_callgrind("events: Ir\nfn=(9)\n1 1\n"))
    raises("self-recursive entry point", lambda: entry_point(parse_callgrind(
        "events: Ir\nfn=(1) rocksdb::ExpanseMemTableRep::Get(x)\n1 5\ncfn=(1)\ncalls=1 1\n1 5\n"
        "fn=(2) main\ncfn=(1)\ncalls=1 1\n1 10\n"), "rocksdb::ExpanseMemTableRep::Get"))
    raises("two overloads", lambda: match_function(parse_callgrind(
        "events: Ir\nfn=(1) a::F(int)\n1 1\nfn=(2) a::F(long)\n1 1\n"), "a::F"))
    check("instr+line positions read the right column",
          parse_callgrind("positions: instr line\nevents: Ir Dr\nfn=(1) f\n0x10 3 7 2\n")["self"]["f"], 7)

    # Relocations: one TLS site in the lookup, one PLT call elsewhere.
    sections = """  [Nr] Name              Type            Address          Off    Size   ES Flg Lk Inf Al
  [ 0]                   NULL            0000000000000000 000000 000000 00      0   0  0
  [ 5] .text._ZN7rocksdb18ExpanseMemTableRep22OptimisticReaderHandleEv PROGBITS 0000000000000000 000100 000080 00 AXG  0   0 16
  [ 6] .rela.text._ZN7rocksdb18ExpanseMemTableRep22OptimisticReaderHandleEv RELA 0000000000000000 000900 000030 18  IG 20   5  8
  [ 7] .text             PROGBITS        0000000000000000 000200 000100 00  AX  0   0 16
  [ 8] .rela.text        RELA            0000000000000000 000a00 000030 18   I 20   7  8
"""
    symbols = """Symbol table '.symtab' contains 3 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
    10: 0000000000000000   128 FUNC    GLOBAL DEFAULT    5 _ZN7rocksdb18ExpanseMemTableRep22OptimisticReaderHandleEv
    11: 0000000000000040   192 FUNC    GLOBAL DEFAULT    7 _ZN7rocksdb18ExpanseMemTableRep3GetEv
"""
    relocs = """Relocation section '.rela.text._ZN7rocksdb18ExpanseMemTableRep22OptimisticReaderHandleEv' at offset 0x900 contains 1 entry:
    Offset             Info             Type               Symbol's Value  Symbol's Name + Addend
0000000000000010  0000000c00000017 R_X86_64_TPOFF32       0000000000000000 _ZL16t_reader_handles + 0

Relocation section '.rela.text' at offset 0xa00 contains 1 entry:
    Offset             Info             Type               Symbol's Value  Symbol's Name + Addend
0000000000000050  0000000d00000004 R_X86_64_PLT32         0000000000000000 pthread_mutex_lock - 4
"""
    objdump = """0000000000000000 <_ZN7rocksdb18ExpanseMemTableRep22OptimisticReaderHandleEv>:
   0:	64 48 8b 04 25 00 00 	mov    %fs:0x0,%rax
			10: R_X86_64_TPOFF32	_ZL16t_reader_handles
0000000000000040 <_ZN7rocksdb18ExpanseMemTableRep3GetEv>:
  50:	e8 00 00 00 00       	call   55 <x>
			51: R_X86_64_PLT32	pthread_mutex_lock-0x4
"""
    re_sites = tls_sites_from_readelf(sections, symbols, relocs)
    check("readelf TLS sites", re_sites,
          [("_ZN7rocksdb18ExpanseMemTableRep22OptimisticReaderHandleEv", "R_X86_64_TPOFF32", "_ZL16t_reader_handles")])
    check("objdump TLS sites agree", tls_sites_from_objdump(objdump), re_sites)
    check("TLS in the lookup only is MET", tls_verdict(re_sites, re_sites)["verdict"], "MET")
    stray = relocs.replace("R_X86_64_PLT32         0000000000000000 pthread_mutex_lock - 4",
                           "R_X86_64_PLT32         0000000000000000 __tls_get_addr - 4")
    stray_sites = tls_sites_from_readelf(sections, symbols, stray)
    v = tls_verdict(stray_sites, stray_sites)
    check("__tls_get_addr outside the lookup is UNMET", v["verdict"], "UNMET")
    check("names the function", any("3GetEv" in p for p in v["problems"]), True)
    check("no TLS site at all is UNMET", tls_verdict([], [])["verdict"], "UNMET")
    check("tools disagreeing is UNMET", tls_verdict(re_sites, [])["verdict"], "UNMET")
    check("_ZTW wrapper counts", is_tls_relocation("R_X86_64_PLT32", "_ZTWN7rocksdb1xE"), True)

    # The bound: 50,000 calls at 1,000 Ir allow 1 Ir a call.
    base = {"calls": 50000, "inclusive_ir": 50_000_000}
    within = compare_entry(base, dict(base), {"calls": 50000, "inclusive_ir": 50_050_000})
    check("exactly 0.1% is within", within["verdict"], "WITHIN_BOUND")
    check("budget per call", within["budget_per_call"], 1.0)
    check("delta per call", within["delta_per_call"], 1.0)
    rises = compare_entry(base, dict(base), {"calls": 50000, "inclusive_ir": 50_050_001})
    check("one Ir over rises beyond", rises["verdict"], "RISES_BEYOND_BOUND")
    falls = compare_entry(base, dict(base), {"calls": 50000, "inclusive_ir": 49_900_000})
    check("a 0.2% fall is beyond, not a failure", falls["verdict"], "FALLS_BEYOND_BOUND")
    check("calls differ", compare_entry(base, dict(base), {"calls": 49999, "inclusive_ir": 1})["verdict"],
          "CALLS_DIFFER")
    rows = [{"phase": "readrandom", "entry": "Get", **within}]
    ok = {"lookup": True, "reader": True}
    tls_ok = {"verdict": "MET", "problems": []}
    check("all conditions met", bound_verdict(rows, {"head readrandom": {}}, ok, tls_ok)["verdict"], "MET")
    check("a fall is listed for attribution",
          bound_verdict([{"phase": "p", "entry": "e", **falls}], {}, ok, tls_ok)["attribute_per_function"], ["p e"])
    check("a fall alone keeps the bound met",
          bound_verdict([{"phase": "p", "entry": "e", **falls}], {}, ok, tls_ok)["verdict"], "MET")
    check("a rise fails", bound_verdict([{"phase": "p", "entry": "e", **rises}], {}, ok, tls_ok)["verdict"], "UNMET")
    ctl = compare_entry(base, {"calls": 50000, "inclusive_ir": 50_000_001}, dict(base))
    check("a control differing by one Ir fails", bound_verdict([{"phase": "p", "entry": "e", **ctl}], {}, ok,
                                                               tls_ok)["verdict"], "UNMET")
    check("a default cell calling a reader symbol fails",
          bound_verdict(rows, {"head readrandom": {"expanse_sync_map_reader_get": 1}}, ok, tls_ok)["verdict"], "UNMET")
    check("a matcher that found nothing in the opt cells fails",
          bound_verdict(rows, {}, {"lookup": False, "reader": True}, tls_ok)["verdict"], "UNMET")
    check("an unmet TLS scan fails", bound_verdict(rows, {}, ok, {"verdict": "UNMET", "problems": ["x"]})["verdict"],
          "UNMET")
    check("no rows fails", bound_verdict([], {}, ok, tls_ok)["verdict"], "UNMET")
    if "| `readrandom` | `Get` | 50,000 |" not in render_table(rows):
        fails.append(f"render_table lost the row: {render_table(rows)}")

    if fails:
        print("locate_ir_bound --self-test FAILED:")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("locate_ir_bound --self-test: all cases passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    r = sub.add_parser("run", help="build base twice and head, measure, and apply the bound")
    r.add_argument("--base-a", type=Path, required=True)
    r.add_argument("--base-b", type=Path, required=True)
    r.add_argument("--head", type=Path, required=True)
    r.add_argument("--out-dir", type=Path, required=True)
    r.add_argument("--cxx", default="c++")
    t = sub.add_parser("tls-scan", help="the R7 relocation scan on one object file")
    t.add_argument("--object", type=Path, required=True)
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.cmd == "run":
        return run(args)
    if args.cmd == "tls-scan":
        v = tls_scan(args.object)
        print(json.dumps(v, indent=2))
        return 0 if v["verdict"] == "MET" else 1
    ap.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
