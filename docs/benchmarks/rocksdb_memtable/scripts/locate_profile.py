#!/usr/bin/env python3
"""Where one reader's time goes inside the #802 locked region, sampled on the reference host.

The narrowed-mutex arm of #802 locks only `expanse_map_prev_at_or_before`
instead of all of `ExpanseMemTableRep::FindLeafBlockForSeek`. Its prediction
needs two inputs this measures, frozen before the arm is pre-registered
(AGENTS.md section 8.21 item 4):

- `locked_fraction`: the share of a `Get` spent inside the current locked
  region, `FindLeafBlockForSeek`'s inclusive time less its
  `pthread_mutex_lock` and `pthread_mutex_unlock` calls;
- `trie_fraction`: the share spent in `expanse_map_prev_at_or_before`, the only
  call the narrowed arm still locks.

Both come from `perf report --children` over LBR call graphs, restricted to the
single reader thread. Inclusive time is what attributes the key comparisons
inside the lock to `FindLeafBlockForSeek` rather than to `Get`'s in-block scan,
which calls the same comparator. The cell is idle, R = 1: no writer and no
second reader, so the lock is uncontended and every sampled cycle belongs to
the read path. METHODOLOGY section 5.11 found this cell reads at the wall-clock
arm's rate in the counters harness, 4.06 vs 4.08-4.10 Mops/s; the paced R = 7
regime mismatch recorded there does not reach it.

It runs the harness's counters mode through `scripts/bench_counters.py`'s
handshake, pin and affinity check, one `perf record -p` attach per round after
the fixture is built. It refuses, rather than publish:

- a round whose harness did not run on the launch pin;
- a report missing any of `Get`, `FindLeafBlockForSeek`,
  `expanse_map_prev_at_or_before`, or a mutex call;
- inclusive times that cannot nest (trie above the locked region, locked
  region above the read);
- a round with fewer than `MIN_SAMPLES` reader samples;
- a host where the requested call-graph mode cannot record. There is no silent
  fallback between `lbr` and `dwarf`, and the artifact records which ran.

    locate_profile.py [--rounds N] [--call-graph lbr|dwarf] [--out-dir DIR]
    locate_profile.py --self-test
"""
from __future__ import annotations

import argparse
import inspect
import json
import math
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import bench_counters as bc  # noqa: E402
import bench_provenance as prov  # noqa: E402
from bca_bootstrap import bca_bootstrap_ci_with_method  # noqa: E402

CELL = "rocksdb_conc_idle_r1"
READER_COMM = "reader-0"
GET = "ExpanseMemTableRep::Get("
FIND = "ExpanseMemTableRep::FindLeafBlockForSeek("
TRIE = "expanse_map_prev_at_or_before"
LOCKS = ("pthread_mutex_lock", "pthread_mutex_unlock")
SHARES = ("locked_fraction", "trie_fraction", "trie_share_of_locked", "get_share_of_thread")
# A share p estimated from n independent samples has standard error
# sqrt(p(1 - p) / n), at most 0.5 / sqrt(n). 1,000 samples bounds that at 1.6
# percentage points, which is what resolves a trie share of a few percent from
# zero. A 2 s round at perf's default 4 kHz on one thread yields about 8,000.
MIN_SAMPLES = 1000


# --------------------------------------------------------------------------
# perf report, `-t ,` output
# --------------------------------------------------------------------------
def parse_report(text: str) -> list[dict]:
    """Rows of `perf report --children --sort sym -t ,`, with or without `-n`.

    `-t ,` separates the fields with commas. perf rewrites a comma inside a
    field to a dot -- `Get(rocksdb::LookupKey const&. void*. ...)` on the
    reference host -- so a symbol carries none. The split still stops after the
    leading numeric fields, so a perf that did not rewrite would not break the
    symbol either.
    """
    rows = []
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = line.split(",", 3)
        if len(parts) >= 4 and parts[2].strip().isdigit():
            children, self_pct, samples, sym = parts[0], parts[1], int(parts[2].strip()), parts[3]
        else:
            parts = line.split(",", 2)
            if len(parts) < 3:
                continue
            children, self_pct, samples, sym = parts[0], parts[1], None, parts[2]
        try:
            c = float(children.strip().rstrip("%"))
            s = float(self_pct.strip().rstrip("%"))
        except ValueError:
            continue
        sym = re.sub(r"^\s*\[[.k]\]\s*", "", sym).strip()
        rows.append({"children": c, "self": s, "samples": samples, "symbol": sym})
    return rows


def children_of(rows: list[dict], needle: str, nested_variants: bool = False) -> float | None:
    """Inclusive percentage of the symbol matching `needle`, or None when absent.

    A `.cold` split of a function is called from the function itself, so its
    inclusive time is already inside the parent's; it is excluded rather than
    counted twice.

    Rows with an identical symbol name are one function whose samples perf
    split across two histogram entries, and their inclusive time is summed. On
    the reference host `expanse_map_prev_at_or_before` appeared once at 11.79%
    and again at 0.01%, and its Rust callees likewise (run 34790214222, round
    0); another run showed no split. Should one sample reach both entries, the
    sum over-counts by at most the smaller row. Rows whose full names differ --
    a compiler clone, an overload -- are different functions, and more than one
    of those is refused as ambiguous.

    A PLT stub (`@plt`) is excluded too. It is a jump into the function, not a
    call, so its inclusive time is not the call's: on the reference host
    `expanse_map_prev_at_or_before@plt` carried 0.52% against the function's
    11.24%, and `pthread_mutex_lock@plt` 2.69% against 2.60% (run 34789727679).
    Dropping it loses the trampoline's own few samples, which then count in
    whatever called it.

    Otherwise exactly one distinct name must match. Two names would be two real
    symbols, and picking either would hide an attribution this tool cannot
    make, so it refuses instead.

    `nested_variants` is for glibc's mutex entry points, taken after identical
    names are summed. `___pthread_mutex_unlock`
    calls `__pthread_mutex_unlock_usercnt`, so both match `pthread_mutex_unlock`
    and the outer row's inclusive time already contains the inner's. The
    outermost -- the largest -- is the call, and summing would double it.
    """
    hits = [r for r in rows if needle in r["symbol"]
            and ".cold" not in r["symbol"] and "@plt" not in r["symbol"]]
    if not hits:
        return None
    by_name: dict[str, float] = {}
    for r in hits:
        by_name[r["symbol"]] = by_name.get(r["symbol"], 0.0) + r["children"]
    if nested_variants:
        return max(by_name.values())
    if len(by_name) > 1:
        raise RuntimeError(f"{len(by_name)} distinct symbols match {needle!r} "
                           f"({[name[:60] for name in by_name]}); the attribution is ambiguous")
    return next(iter(by_name.values()))


def shares(rows: list[dict]) -> dict:
    """The four shares from one round's report, or a RuntimeError naming why not."""
    get, find, trie = children_of(rows, GET), children_of(rows, FIND), children_of(rows, TRIE)
    locks = [children_of(rows, name, nested_variants=True) for name in LOCKS]
    missing = [n for n, v in ((GET, get), (FIND, find), (TRIE, trie), *zip(LOCKS, locks)) if v is None]
    if missing:
        raise RuntimeError(f"the report has no row for {missing}; the round cannot be attributed")
    samples = sum(r["samples"] for r in rows if r["samples"] is not None)
    lock = sum(locks)
    locked = find - lock
    if not (0.0 < get <= 100.0 + 1e-9):
        raise RuntimeError(f"Get's inclusive share is {get}%, outside (0, 100]")
    if not (0.0 <= lock <= find <= get + 1e-9):
        raise RuntimeError(f"inclusive times do not nest: lock {lock}% <= locate {find}% <= Get {get}% fails")
    if not (0.0 <= trie <= locked + 1e-9):
        raise RuntimeError(f"the trie call ({trie}%) exceeds the locked region ({locked}%)")
    if locked <= 0.0:
        raise RuntimeError("the locked region has no time left after its mutex calls")
    return {
        "samples": samples,
        "get_pct": get, "locate_pct": find, "lock_pct": lock, "trie_pct": trie, "locked_pct": locked,
        "get_share_of_thread": get / 100.0,
        "locked_fraction": locked / get,
        "trie_fraction": trie / get,
        "trie_share_of_locked": trie / locked,
    }


# --------------------------------------------------------------------------
# the measurement
# --------------------------------------------------------------------------
def call_graph_preflight(event: str, mode: str) -> str:
    """`perf --version` once `perf record --call-graph <mode>` is known to record here."""
    with tempfile.TemporaryDirectory() as td:
        data = Path(td) / "probe.data"
        proc = subprocess.run(["perf", "record", "-e", event, "--call-graph", mode, "-o", str(data),
                               "--", "true"], capture_output=True, text=True)
        if proc.returncode != 0 or not data.is_file():
            raise bc.Preflight(
                f"`perf record -e {event} --call-graph {mode}` cannot record on this host "
                f"(rc {proc.returncode}): {proc.stderr.strip()[:400]}. No profile was taken. "
                f"Pass --call-graph {'dwarf' if mode == 'lbr' else 'lbr'} to try the other mode "
                f"deliberately; this tool does not switch modes by itself.")
    return subprocess.run(["perf", "--version"], capture_output=True, text=True).stdout.strip()


def one_profiled_round(cell, child, round_idx: int, event: str, mode: str, pin: list[str],
                       stderr_path: Path, out_dir: Path) -> dict:
    ready = bc.read_until(child.stdout, lambda o: o.get("event") == "threads_ready",
                          f"`threads_ready` for round {round_idx}", cell, stderr_path)
    if ready.get("round") != round_idx:
        raise bc.Preflight(f"expected threads_ready for round {round_idx}, got {ready}")
    pid = int(ready["pid"])
    data = out_dir / f"profile_{CELL}_round{round_idx}.data"
    perf = subprocess.Popen(["perf", "record", "-e", event, "--call-graph", mode, "-p", str(pid),
                             "-o", str(data)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    time.sleep(bc.ATTACH_SETTLE_S)
    if perf.poll() is not None:
        _o, err = perf.communicate()
        raise bc.Preflight(f"perf record -p {pid} exited {perf.returncode} before round {round_idx} "
                           f"was released: {err.strip()[:400]}")
    child.stdin.write("\n")
    child.stdin.flush()
    row = bc.read_until(child.stdout, lambda o: o.get("role") == "counters",
                        f"the `counters` row for round {round_idx}", cell, stderr_path)
    problem = bc.affinity_problem(row, pin)
    if problem:
        bc.stop_perf(perf, "perf record", cell)
        raise bc.Preflight(f"round {round_idx}: {problem}; the round is void")
    rc, err = bc.stop_perf(perf, "perf record", cell)
    if not data.is_file():
        raise bc.Preflight(f"perf record wrote no {data.name} (rc {rc}): {err.strip()[:400]}")
    rep = subprocess.run(["perf", "report", "-i", str(data), "--children", "--sort", "sym", "-g", "none",
                          "--stdio", "-t", ",", "-n", "--comms", READER_COMM, "--percentage", "relative",
                          "--percent-limit", "0"], capture_output=True, text=True)
    data.unlink(missing_ok=True)
    if rep.returncode != 0 or not rep.stdout.strip():
        raise bc.Preflight(f"perf report exited {rep.returncode} or was empty: {rep.stderr.strip()[:400]}")
    report_path = out_dir / f"profile_{CELL}_round{round_idx}.txt"
    report_path.write_text(rep.stdout)
    try:
        got = shares(parse_report(rep.stdout))
    except RuntimeError as exc:
        raise bc.Preflight(f"round {round_idx}: {exc} (report kept at {report_path.name})")
    if got["samples"] < MIN_SAMPLES:
        raise bc.Preflight(f"round {round_idx}: {got['samples']} reader samples, fewer than {MIN_SAMPLES}")
    return {"round": round_idx, "pid": pid, "report": report_path.name, "harness_row": row, **got}


def interval(values: list[float]) -> dict:
    if len(values) < 3:
        return {"point": None, "ci_lower": None, "ci_upper": None, "ci_method": None, "n": len(values),
                "why_no_interval": "fewer than 3 rounds; BCa needs n >= 3"}
    point, lo, hi, method = bca_bootstrap_ci_with_method(values)
    return {"point": point, "ci_lower": lo, "ci_upper": hi, "ci_method": method, "n": len(values)}


def run(rounds: int, mode: str, out_dir: Path) -> Path:
    cell = bc.BY_NAME[CELL]
    pmu, why, pin, available, _unavailable = bc.preflight(["cycles"])
    if "cycles" not in available:
        raise bc.Preflight("the `cycles` event is not available for the selected PMU")
    event = bc.qualify("cycles", pmu)
    perf_version = call_graph_preflight(event, mode)
    p = prov.new_provenance("rocksdb_locate_profile", 802, "share of a Get", repo_root=REPO_ROOT,
                            pre_registration="docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section 5")
    p["commit"] = os.environ.get("EXPANSE_BENCH_COMMIT", p.get("commit"))
    p["host"] = prov.host_facts(pin[-1] if pin else None)
    p["estimators"] = prov.estimators(
        ratio="share of a Get's inclusive sampled cycles, per round; BCa 95% over rounds",
        columns="perf report --children --sort sym, restricted to the reader thread",
        raw="rounds_raw",
    )
    p.update({"pmu": pmu, "pmu_reason": why, "pin": pin,
              "pin_source": ("pmu" if os.environ.get("EXPANSE_BENCH_PIN_APPLIED") in bc.UNPINNED
                             else "EXPANSE_BENCH_PIN_APPLIED"),
              "event": event, "call_graph": mode, "perf_version": perf_version, "min_samples": MIN_SAMPLES})
    out_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".stderr", delete=False) as fh:
        stderr_path = Path(fh.name)
    prov.add_load(p, "before rounds")
    child = bc.start_harness(cell, pin, dict(os.environ), rounds, stderr_path)
    raw = []
    try:
        for i in range(rounds):
            raw.append(one_profiled_round(cell, child, i, event, mode, pin, stderr_path, out_dir))
        bc.finish_harness(child, cell, stderr_path)
    finally:
        if child.poll() is None:
            child.kill()
        stderr_path.unlink(missing_ok=True)
    prov.add_load(p, "after rounds")
    art = prov.attach({
        "schema": "expanse.profile.v1",
        "suite": "rocksdb_locate_profile",
        "profile_cell": CELL,
        "binary": cell.binary, "args": cell.args,
        "rounds": rounds,
        "rounds_raw": raw,
        "shares": {name: interval([r[name] for r in raw]) for name in SHARES},
    }, p)
    path = out_dir / f"profile_{CELL}.json"
    path.write_text(json.dumps(art, indent=2) + "\n")
    return path


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------
# Verbatim rows of `perf report --children --sort sym -g none --stdio -t ,`
# (perf 5.15, Linux container on the development host): the header and row
# layout the parser reads. Its symbols did not resolve there.
VERBATIM_ROWS = """\
# Children,    Self,Symbol                            
 99.88% , 0.00%  ,[.] 0x0000aaaaddc50730
 99.82% , 0.00%  ,[.] __libc_start_main
 47.19% , 47.19% ,[.] 0x0000000000000888
"""

# Verbatim rows from the reference host (run 34789727679, round 0, perf 6.8):
# the harness's, libexpanse's and glibc's symbols as that host names them,
# including the PLT stubs that made round 0 ambiguous before stubs were
# excluded. Subset of the report: the header lines and the rows this tool reads.
HOST_ROWS = """\
# Samples: 8K of event 'cpu_core/cycles/'
# Children,    Self,     Samples,Symbol
 99.68% , 9.88%  , 795        ,[.] rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&. void*. bool (*)(void*. char const*))
 62.64% , 39.05% , 3129       ,[.] (anonymous namespace)::BenchCmp::operator()(char const*. char const*) const
 54.80% , 9.57%  , 763        ,[.] rocksdb::ExpanseMemTableRep::FindLeafBlockForSeek(rocksdb::Slice const&. char const*) const
 11.24% , 0.44%  , 35         ,[.] expanse_map_prev_at_or_before
 10.61% , 6.76%  , 542        ,[.] _RINvNtCs6fUdrY3seH1_12expanse_trie3nav4prevKb1_EB4_
 2.69%  , 0.09%  , 7          ,[.] pthread_mutex_lock@plt
 2.60%  , 2.60%  , 210        ,[.] pthread_mutex_lock
 2.18%  , 0.05%  , 4          ,[.] pthread_mutex_unlock@plt
 2.13%  , 2.13%  , 172        ,[.] pthread_mutex_unlock
 0.52%  , 0.08%  , 6          ,[.] expanse_map_prev_at_or_before@plt
"""

# Verbatim rows from run 34790214222, round 0: `expanse_map_prev_at_or_before`
# and its Rust callees each appear twice under one name, once at 0.01%.
HOST_ROWS_SPLIT = """\
# Samples: 8K of event 'cpu_core/cycles/'
# Children,    Self,     Samples,Symbol
 99.57% , 10.21% , 817        ,[.] rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&. void*. bool (*)(void*. char const*))
 55.15% , 10.26% , 820        ,[.] rocksdb::ExpanseMemTableRep::FindLeafBlockForSeek(rocksdb::Slice const&. char const*) const
 11.79% , 0.37%  , 29         ,[.] expanse_map_prev_at_or_before
 11.42% , 0.32%  , 25         ,[.] _RNvMsd_NtCs6fUdrY3seH1_12expanse_trie3mapNtB5_10ExpanseMap17prev_at_or_before
 11.11% , 7.42%  , 591        ,[.] _RINvNtCs6fUdrY3seH1_12expanse_trie3nav4prevKb1_EB4_
 2.44%  , 0.10%  , 8          ,[.] pthread_mutex_lock@plt
 2.34%  , 2.34%  , 190        ,[.] pthread_mutex_lock
 2.02%  , 0.06%  , 5          ,[.] pthread_mutex_unlock@plt
 1.96%  , 1.96%  , 157        ,[.] pthread_mutex_unlock
 0.45%  , 0.09%  , 7          ,[.] expanse_map_prev_at_or_before@plt
 0.01%  , 0.00%  , 1          ,[.] _RNvMsd_NtCs6fUdrY3seH1_12expanse_trie3mapNtB5_10ExpanseMap17prev_at_or_before
 0.01%  , 0.00%  , 0          ,[.] expanse_map_prev_at_or_before
 0.01%  , 0.01%  , 4          ,[.] _RINvNtCs6fUdrY3seH1_12expanse_trie3nav4prevKb1_EB4_
"""

# Synthetic rows in the `-n` layout (children, self, samples, symbol), with the
# real symbols the harness carries. The numbers are chosen to check the
# arithmetic, not measured.
SYNTHETIC = """\
# Children,    Self,Samples,Symbol
 99.10% , 0.40%  ,32,[.] std::thread::_State_impl<std::thread::_Invoker<std::tuple<RunCell((anonymous namespace)::CellArgs const&, int)::{lambda()#1}> > >::_M_run()
 80.00% , 20.00% ,1600,[.] rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&, void*, bool (*)(void*, char const*))
  1.00% , 1.00%  ,80,[.] rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&, void*, bool (*)(void*, char const*)) (.cold)
 30.00% , 12.00% ,960,[.] rocksdb::ExpanseMemTableRep::FindLeafBlockForSeek(rocksdb::Slice const&, char const*) const
 10.00% , 10.00% ,800,[.] expanse_map_prev_at_or_before
  3.00% , 3.00%  ,240,[.] ___pthread_mutex_lock
  1.00% , 0.40%  ,32,[.] ___pthread_mutex_unlock
  0.60% , 0.60%  ,48,[.] __pthread_mutex_unlock_usercnt
  8.00% , 8.00%  ,640,[.] expanse_rocksdb::CompareInternalKeys(rocksdb::Slice const&, rocksdb::Slice const&)
  4.00% , 4.00%  ,320,[k] 0xffffffff94a2b712
"""


def self_test() -> int:
    fails: list[str] = []

    def check(name, got, want, tol=None):
        ok = (abs(got - want) <= tol) if tol is not None and got is not None else got == want
        if not ok:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    rows = parse_report(VERBATIM_ROWS)
    check("verbatim rows parsed", len(rows), 3)
    check("verbatim children", rows[0]["children"], 99.88)
    check("verbatim symbol", rows[1]["symbol"], "__libc_start_main")
    check("verbatim layout has no samples column", rows[0]["samples"], None)

    # The reference host's own rows: commas rewritten to dots, padded sample
    # counts, PLT stubs beside the functions they jump to.
    host = parse_report(HOST_ROWS)
    try:
        hs = shares(host)
    except RuntimeError as exc:
        fails.append(f"the reference host's rows were refused: {exc}")
    else:
        check("host Get", hs["get_pct"], 99.68)
        check("host locate", hs["locate_pct"], 54.80)
        check("host mutex calls, stubs excluded", hs["lock_pct"], 2.60 + 2.13, tol=1e-9)
        check("host locked region", hs["locked_pct"], 54.80 - 4.73, tol=1e-9)
        check("host trie, stub excluded", hs["trie_pct"], 11.24)
        check("host locked_fraction", hs["locked_fraction"], 50.07 / 99.68, tol=1e-9)
        check("host trie_fraction", hs["trie_fraction"], 11.24 / 99.68, tol=1e-9)
    if not any("LookupKey const&. void*." in r["symbol"] for r in host):
        fails.append("the host fixture no longer carries perf's comma-to-dot rewrite")

    # The same function split across two entries under one name is summed.
    split = parse_report(HOST_ROWS_SPLIT)
    try:
        ss = shares(split)
    except RuntimeError as exc:
        fails.append(f"a function split across two identically named rows was refused: {exc}")
    else:
        check("split trie rows summed", ss["trie_pct"], 11.79 + 0.01, tol=1e-9)
        check("split host locate", ss["locate_pct"], 55.15)
        check("split host mutex calls, stubs excluded", ss["lock_pct"], 2.34 + 1.96, tol=1e-9)

    rows = parse_report(SYNTHETIC)
    check("synthetic rows parsed", len(rows), 10)
    get_row = next(r for r in rows if r["symbol"].startswith("rocksdb::ExpanseMemTableRep::Get(") and ".cold" not in r["symbol"])
    check("a C++ signature keeps its commas", get_row["symbol"],
          "rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&, void*, bool (*)(void*, char const*))")
    check("samples column", get_row["samples"], 1600)
    s = shares(rows)
    check("Get's cold split is excluded, leaving one Get row", s["get_pct"], 80.0)
    check("lock and unlock are both subtracted, nested unlock variants once", s["lock_pct"], 4.0)
    check("locked region", s["locked_pct"], 26.0, tol=1e-9)
    check("locked_fraction", s["locked_fraction"], 26.0 / 80.0, tol=1e-12)
    check("trie_fraction", s["trie_fraction"], 10.0 / 80.0, tol=1e-12)
    check("trie_share_of_locked", s["trie_share_of_locked"], 10.0 / 26.0, tol=1e-12)
    check("samples summed over rows", s["samples"], 4752)
    clone = SYNTHETIC.replace(
        "  8.00% , 8.00%  ,640,[.] expanse_rocksdb::CompareInternalKeys",
        "  2.00% , 2.00%  ,160,[.] rocksdb::ExpanseMemTableRep::Get(rocksdb::LookupKey const&, void*, bool (*)(void*, char const*)) [clone .constprop.0]\n"
        "  8.00% , 8.00%  ,640,[.] expanse_rocksdb::CompareInternalKeys")

    def refuses(name, text, needle):
        try:
            shares(parse_report(text))
        except RuntimeError as exc:
            if needle not in str(exc):
                fails.append(f"{name}: refused, but without {needle!r}: {exc}")
        else:
            fails.append(f"{name}: did not refuse")

    refuses("two distinct Get symbols", clone, "ambiguous")
    refuses("no trie row", "\n".join(ln for ln in SYNTHETIC.splitlines() if TRIE not in ln), "no row for")
    refuses("no mutex row", "\n".join(ln for ln in SYNTHETIC.splitlines() if "mutex_unlock" not in ln), "no row for")
    refuses("trie above the locked region", SYNTHETIC.replace(" 10.00% , 10.00% ,800,[.] expanse_map", " 29.00% , 29.00% ,800,[.] expanse_map"),
            "exceeds the locked region")
    refuses("locate above Get", SYNTHETIC.replace(" 30.00% , 12.00% ,960", " 90.00% , 12.00% ,960"), "do not nest")

    check("MIN_SAMPLES bounds the standard error of any share at 1.6 points",
          0.5 / math.sqrt(MIN_SAMPLES) <= 0.016, True)

    # The decisions at their call sites.
    src_round = inspect.getsource(one_profiled_round)
    for needle, why in (("bc.affinity_problem(row, pin)", "verifies the harness's affinity"),
                        ('"--comms", READER_COMM', "restricts the report to the reader thread"),
                        ('"--children"', "reads inclusive time"),
                        ('"--call-graph", mode', "records with the requested call-graph mode"),
                        ("got[\"samples\"] < MIN_SAMPLES", "refuses a thin round")):
        if needle not in src_round:
            fails.append(f"one_profiled_round no longer {why} (`{needle}` missing)")
    src_main = inspect.getsource(main)
    if src_main.find("bench_pin.apply(") < 0 or src_main.find("bench_pin.apply(") > src_main.find("run("):
        fails.append("main does not apply the benchmark pin before running")
    if "call_graph_preflight(event, mode)" not in inspect.getsource(run):
        fails.append("run does not probe the call-graph mode before the rounds")

    if fails:
        print("locate_profile.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("locate_profile.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--call-graph", choices=("lbr", "dwarf"), default="lbr")
    ap.add_argument("--out-dir", type=Path, default=Path("."))
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    import bench_pin  # noqa: PLC0415

    bench_pin.apply("locate_profile.py")
    try:
        path = run(args.rounds, args.call_graph, args.out_dir)
    except bc.Preflight as exc:
        print(f"::error::locate_profile.py: {exc}", file=sys.stderr)
        return 1
    art = json.loads(path.read_text())
    for name in SHARES:
        v = art["shares"][name]
        if v["point"] is not None:
            print(f"  {name:22s} {v['point']:.4f} [{v['ci_lower']:.4f}, {v['ci_upper']:.4f}] ({v['ci_method']})")
    print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
