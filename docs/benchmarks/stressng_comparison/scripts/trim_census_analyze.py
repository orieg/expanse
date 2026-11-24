#!/usr/bin/env python3
"""Reads the #1680 trim census (README.md, "Attribution: where main's heap
trims come from") and writes one artifact per platform.

    trim_census_analyze.py <census dir> <platform label> <provenance.json> <out.json>
    trim_census_analyze.py --self-test
    trim_census_analyze.py --drive <loads.json> <run.sh> <run.sh args...>

--drive runs trim_census_run.sh once per W in $WS (default "1 4 8"), every
arm given, and takes a load snapshot (scripts/bench_provenance.py, found
through $BENCH_PROVENANCE_DIR) before the first W and after each, so the
artifact records the load the census ran under. The counts do not depend on
load; the snapshots are there because the suite's artifacts carry them.

<census dir> is trim_census_run.sh's output: <arm>/W<w>/<rep>/trim_<pid>.json
per worker and, on the strace platform, st.<pid> per process. Workers are the
processes with a trim_<pid>.json. Counts are summed over a process tree's
workers and are exact; there is no interval because nothing here is sampled.

The strace parser counts a brk call by its result against the process's
previous break. `strace -ff` writes one file per process, so no call should be
split; a line with "<unfinished ...>" or "resumed>" fails the parse rather than
being dropped (a whole-line regex silently drops them, the error behind the
first published version of the earlier census).
"""
import glob
import json
import os
import re
import sys

PAGE = 4096
PHASES = ("init", "insert", "get", "delete", "teardown", "freearray")
SLAB_PAGE_USABLE = None  # from provenance: the usable size of a 4,096 B, 64 B-aligned block
BRANCH_U_USABLE = 4168  # usable size of a 4,160 B, 64 B-aligned block (glibc 2.35, both platforms; measured with usable.c)
BRK = re.compile(r"^brk\((?:NULL|0x[0-9a-f]+)\)\s*=\s*(0x[0-9a-f]+)$")
OTHER = re.compile(r"^(mmap|munmap|madvise|mremap)\(")


def parse_strace(lines):
    """Per-process strace -ff lines -> dict of counts. Raises on a split call."""
    prev = None
    out = {"brk_shrink": 0, "pages_shrunk": 0, "brk_grow": 0, "pages_grown": 0,
           "mmap": 0, "munmap": 0, "madvise": 0, "mremap": 0}
    for ln in lines:
        ln = ln.rstrip("\n")
        if "<unfinished ...>" in ln or "resumed>" in ln:
            raise ValueError(f"split call in a per-process file: {ln!r}")
        m = BRK.match(ln)
        if m:
            cur = int(m.group(1), 16)
            if prev is not None:
                if cur < prev:
                    out["brk_shrink"] += 1
                    out["pages_shrunk"] += (prev - cur) // PAGE
                elif cur > prev:
                    out["brk_grow"] += 1
                    out["pages_grown"] += (cur - prev) // PAGE
            prev = cur
            continue
        m = OTHER.match(ln)
        if m:
            out[m.group(1)] += 1
    return out


def merge_sizes(acc, rows):
    for size, count, pages in rows:
        c = acc.setdefault(size, [0, 0])
        c[0] += count
        c[1] += pages


def read_rep(d):
    workers = sorted(glob.glob(os.path.join(d, "trim_*.json")))
    rep = {"workers": len(workers), "ops": {}, "phases": {p: {} for p in PHASES},
           "trim_sizes": {}, "trim_sizes_by_phase": {p: {} for p in PHASES},
           "grow_sizes": {}, "top_after_trim": {}, "trim_callers": {},
           "delete_frees": {}, "teardown_frees": {},
           "size_table_overflow": 0, "strace": None, "instrument_check": None}
    lib = open(os.path.join(d, "lib.txt")).read().split("=>")[1].split("(")[0].strip()
    rep["lib"] = lib
    st_tot = None
    per_worker_check = []
    for wf in workers:
        j = json.load(open(wf))
        for k, v in j["ops"].items():
            rep["ops"][k] = rep["ops"].get(k, 0) + v
        for p in PHASES:
            ph = j["phases"][p]
            for k in ("frees", "allocs", "trims", "pages_trimmed", "grows", "pages_grown"):
                rep["phases"][p][k] = rep["phases"][p].get(k, 0) + ph[k]
            merge_sizes(rep["trim_sizes"], ph["trim_by_free_usable_size"])
            merge_sizes(rep["trim_sizes_by_phase"][p], ph["trim_by_free_usable_size"])
            merge_sizes(rep["grow_sizes"], ph["grow_by_request_size"])
            if p in ("delete", "teardown"):
                merge_sizes(rep[p + "_frees"], ph["frees_by_usable_size"])
            merge_sizes(rep["top_after_trim"], ph["top_bytes_after_trim"])
        for obj, n in j["trim_callers"]:
            rep["trim_callers"][obj] = rep["trim_callers"].get(obj, 0) + n
        rep["size_table_overflow"] += j["size_table_overflow"]
        sf = os.path.join(d, f"st.{j['pid']}")
        if os.path.exists(sf):
            s = parse_strace(open(sf))
            st_tot = st_tot or {k: 0 for k in s}
            for k, v in s.items():
                st_tot[k] += v
            t1 = sum(j["phases"][p]["trims"] for p in PHASES)
            p1 = sum(j["phases"][p]["pages_trimmed"] for p in PHASES)
            per_worker_check.append({"pid": j["pid"], "i1_trims": t1, "i1_pages": p1,
                                     "i2_shrinks": s["brk_shrink"], "i2_pages": s["pages_shrunk"]})
    rep["strace"] = st_tot
    if per_worker_check:
        rep["instrument_check"] = {
            "workers": len(per_worker_check),
            "max_abs_trim_diff": max(abs(c["i1_trims"] - c["i2_shrinks"]) for c in per_worker_check),
            "max_abs_page_diff": max(abs(c["i1_pages"] - c["i2_pages"]) for c in per_worker_check),
            "per_worker": per_worker_check}
    # JSON keys must be strings; keep sizes sortable.
    for k in ("trim_sizes", "grow_sizes", "top_after_trim", "delete_frees", "teardown_frees"):
        rep[k] = {str(s): v for s, v in sorted(rep[k].items())}
    rep["trim_sizes_by_phase"] = {p: {str(s): v for s, v in sorted(t.items())}
                                  for p, t in rep["trim_sizes_by_phase"].items() if t}
    return rep


def totals(rep):
    trims = sum(v.get("trims", 0) for v in rep["phases"].values())
    pages = sum(v.get("pages_trimmed", 0) for v in rep["phases"].values())
    grown = sum(v.get("pages_grown", 0) for v in rep["phases"].values())
    return trims, pages, grown


def strip_volatile(rep):
    """A repeat comparison ignores pids, which differ by construction."""
    r = json.loads(json.dumps(rep))
    if r.get("instrument_check"):
        for c in r["instrument_check"]["per_worker"]:
            c.pop("pid")
        r["instrument_check"]["per_worker"].sort(key=lambda c: json.dumps(c, sort_keys=True))
    return r


def verdicts(cells, has_strace):
    """P1-P5 as pre-registered; each is read on both repeats at every W."""
    out = {}
    ws = sorted({w for (_, w) in cells})
    def every(pred):
        return all(pred(cells[(a, w)][rep], w) for w in ws for a in ("N",) for rep in ("a", "b"))

    if has_strace:
        out["P1"] = all(cells[(a, w)][rep]["strace"] is not None and
                        all(cells[(a, w)][rep]["strace"][k] == 0 for k in ("munmap", "madvise", "mremap"))
                        for a in ("N", "S") for w in ws for rep in ("a", "b"))
    else:
        out["P1"] = "not read on this platform (no strace)"

    def p2(_, w):
        ok = True
        for rep in ("a", "b"):
            tn, pn, _ = totals(cells[("N", w)][rep])
            ts, ps, _ = totals(cells[("S", w)][rep])
            ok &= pn > ps and tn >= 10 * max(ts, 1)
        return ok
    out["P2"] = all(p2(None, w) for w in ws)

    def p3(r, _):
        trims, pages, _ = totals(r)
        inband = sum(v[0] for s, v in r["top_after_trim"].items() if 131072 <= int(s) <= 135232)
        return trims > 0 and inband >= 0.95 * trims and pages / trims <= 3
    out["P3"] = every(p3)

    def p4(r, _):
        _, pages, _ = totals(r)
        td = r["phases"]["teardown"].get("pages_trimmed", 0)
        if not r["trim_sizes"]:
            return False
        modal = max(r["trim_sizes"].items(), key=lambda kv: kv[1][0])[0]
        return td >= 0.5 * pages and int(modal) == SLAB_PAGE_USABLE
    out["P4"] = every(p4) if SLAB_PAGE_USABLE else "undecided: slab-page usable size not set"

    def p5(arm, want):
        for w in ws:
            for rep in ("a", "b"):
                c = cells[(arm, w)][rep]["trim_callers"]
                tot = sum(c.values())
                if tot and sum(n for o, n in c.items() if o.startswith(want)) < 0.99 * tot:
                    return False
        return True
    # Both libraries are loaded under the name libJudy.so.1, which dladdr
    # reports; lib.txt proves per repeat which file that name resolved to.
    out["P5"] = p5("N", "libJudy.so.1") and p5("S", "libJudy.so.1")
    if any(a == "P" for (a, _) in cells):
        def p6(w):
            ok = True
            for rep in ("a", "b"):
                n, pp = cells[("N", w)][rep], cells[("P", w)][rep]
                ok &= pp["phases"]["delete"].get("trims", 0) <= 0.10 * n["phases"]["delete"].get("trims", 0)
                ok &= str(BRANCH_U_USABLE) not in pp["delete_frees"]
                ok &= totals(pp)[1] < totals(n)[1]
            return ok
        out["P6"] = all(p6(w) for w in ws)
    out["P5_library_files"] = {f"{a}/W{w}/{rep}": cells[(a, w)][rep]["lib"]
                               for (a, w) in cells for rep in ("a", "b")}
    return out


def _bench_provenance():
    sys.path.insert(0, os.environ.get("BENCH_PROVENANCE_DIR", os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "scripts")))
    import importlib
    return importlib.import_module("bench_provenance")


def drive(argv):
    import subprocess
    bp = _bench_provenance()
    out, run = argv[2], argv[3:]
    prov = {}
    bp.add_load(prov, "start")
    for w in os.environ.get("WS", "1 4 8").split():
        subprocess.run(["bash", *run], env=dict(os.environ, WS=w), check=True)
        bp.add_load(prov, f"after W{w}")
    with open(out, "w") as fh:
        json.dump({"host": bp.host_facts(), "loads": prov["loads"]}, fh, indent=1)


def main(argv):
    global SLAB_PAGE_USABLE
    root, plat, prov_path, out_path = argv[1:5]
    prov = json.load(open(prov_path))
    loads_path = os.path.join(root, "loads.json")
    if os.path.exists(loads_path):
        lj = json.load(open(loads_path))
        if isinstance(prov.get("host"), str):
            prov["host_description"] = prov["host"]
        prov["host"], prov["loads"] = lj["host"], lj["loads"]
    prov["estimators"] = {
        "ratio": "none in this artifact: every figure is an exact count; the README's ratios "
                 "are quotients of two of these counts",
        "columns": "counts summed over a process tree's workers, per repeat",
        "raw": "every cell carries both repeats in full; nothing is sampled, so there are "
               "no rounds and no interval"}
    SLAB_PAGE_USABLE = prov.get("slab_page_usable_size")
    cells = {}
    for arm in ("N", "S", "P"):
        for wd in sorted(glob.glob(os.path.join(root, arm, "W*"))):
            w = int(os.path.basename(wd)[1:])
            cells[(arm, w)] = {rep: read_rep(os.path.join(wd, rep)) for rep in ("a", "b")}
            for rep, r in cells[(arm, w)].items():
                # Every worker of the cell must have finished all 3·W bogo ops
                # (stress-ng's -t can stop a worker early); a short repeat is void.
                r["complete"] = r["ops"].get("teardown", 0) == 3 * w
                if not r["complete"]:
                    sys.exit(f"VOID: {arm} W{w} {rep}: {r['ops'].get('teardown', 0)} bogo ops, expected {3 * w}")
    det = {f"{a}/W{w}": strip_volatile(c["a"]) == strip_volatile(c["b"]) for (a, w), c in cells.items()}
    has_strace = all(c[r]["strace"] is not None for c in cells.values() for r in ("a", "b"))
    art = {"provenance": dict(prov, platform=plat, workload="sng_judy_4m",
                              statistics="exact counts summed over a process tree's workers; no interval"),
           "deterministic_repeats": det,
           "verdicts": verdicts(cells, has_strace),
           "cells": {f"{a}/W{w}": c for (a, w), c in sorted(cells.items())}}
    json.dump(art, open(out_path, "w"), indent=1, sort_keys=True)
    print(f"platform {plat}; repeats identical: {det}")
    print("| arm | W | workers | trims | pages given back | pages grown | pages/trim | teardown share of pages |")
    print("|---|---|---|---|---|---|---|---|")
    for (a, w), c in sorted(cells.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        r = c["a"]
        t, p, g = totals(r)
        td = r["phases"]["teardown"].get("pages_trimmed", 0)
        print(f"| {a} | {w} | {r['workers']} | {t:,} | {p:,} | {g:,} | "
              f"{(p / t if t else 0):.2f} | {(td / p if p else 0):.3f} |")
    print("verdicts:", art["verdicts"])


def self_test():
    ok = parse_strace(["brk(NULL) = 0x1000000", "brk(0x1021000) = 0x1021000",
                       "brk(0x1010000) = 0x1010000", "munmap(0x7f, 4096) = 0"])
    assert ok["brk_grow"] == 1 and ok["pages_grown"] == 33, ok
    assert ok["brk_shrink"] == 1 and ok["pages_shrunk"] == 17, ok
    assert ok["munmap"] == 1, ok
    for bad in ("brk(0x1010000 <unfinished ...>", "<... brk resumed>) = 0x1010000"):
        try:
            parse_strace(["brk(NULL) = 0x1000000", bad])
        except ValueError:
            continue
        raise AssertionError(f"split line accepted: {bad}")
    print("self-test passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    elif sys.argv[1:2] == ["--drive"]:
        drive(sys.argv)
    else:
        main(sys.argv)
