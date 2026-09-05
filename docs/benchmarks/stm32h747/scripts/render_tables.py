#!/usr/bin/env python3
"""Render the STM32H747 suite's README tables from the committed artifact.

Every number in ``docs/benchmarks/stm32h747/README.md`` derives from
``results/results.json`` (AGENTS.md §8.2). This script prints those tables and
the derived figures the prose quotes (ratios, ranges, BUSY rates), so a
re-harvest updates the README by pasting, not by retyping. It also prints the
paired-capture table for a ``results/pairing_<sha>/`` directory when one is
given with ``--pair CONTROL.json TREATMENT.json``.

    python3 docs/benchmarks/stm32h747/scripts/render_tables.py
    python3 docs/benchmarks/stm32h747/scripts/render_tables.py --pair \
        results/pairing_fce563c2/control_a_22908c15.json \
        results/pairing_fce563c2/treatment_a_fce563c2.json
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

SUITE_DIR = Path(__file__).resolve().parent.parent
DATA = SUITE_DIR / "results" / "results.json"

FIXTURE_LABEL = {
    "ingest": "ingest, 2,000 sequential inserts (per insert)",
    "can_dispatch": "CAN dispatch, 500 gets (per get)",
    "evict_bulk_loop": "BLE evict 600 of 2,000, per-key `first`/`remove` loop (per evicted)",
    "evict_bulk_range": "BLE evict 600 of 2,000, `remove_range` (per evicted)",
    "evict_steady_loop": "BLE evict 25 of 2,000, per-key loop (per evicted)",
    "evict_steady_range": "BLE evict 25 of 2,000, `remove_range` (per evicted)",
}
FIXTURE_ORDER = list(FIXTURE_LABEL)
TWIN_LABEL = {
    "sorted_array": "sorted array, `bsearch` + `memmove`",
    "open_hash": "open-addressing hash, FNV-1a, ≤ 50% load",
    "tsearch": "newlib `tsearch` (unbalanced BST)",
}
M7 = 400_000_000
M4 = 200_000_000


def n0(x: float) -> str:
    return f"{x:,.0f}"


def n1(x: float) -> str:
    return f"{x:,.1f}"


def pct(x: float) -> str:
    return f"{100 * x:.1f}%"


def load(path: Path) -> dict:
    return json.load(open(path))


def cells(d: dict) -> dict:
    return {(f["core"], f["sysclk"], f["dcache"], f["name"], f["impl"]): f for f in d["fixtures"]}


def cyc(c: dict, core: str, clk: int, dc: int, name: str, impl: str = "expanse") -> float:
    return c[(core, clk, dc, name, impl)]["min"]


def ns(c: dict, core: str, clk: int, dc: int, name: str, impl: str = "expanse") -> float:
    return c[(core, clk, dc, name, impl)]["min_ns"]


def twin_cell(c: dict, core: str, clk: int, dc: int, name: str, impl: str):
    """The twin's cell for an Expanse fixture: the hash has a scan where the others have range/loop."""
    if (core, clk, dc, name, impl) in c:
        return c[(core, clk, dc, name, impl)]
    if impl == "open_hash" and name.endswith("_range"):
        alt = name.replace("_range", "_scan")
        if (core, clk, dc, alt, impl) in c:
            return c[(core, clk, dc, alt, impl)]
    return None


def ratio_word(twin: float, exp: float) -> str:
    return f"{exp / twin:.1f}× faster" if twin < exp else f"{twin / exp:.1f}× slower"


def table(rows: list[list[str]], header: list[str], align: str | None = None) -> str:
    align = align or ("|---" + "|---:" * (len(header) - 1) + "|")
    out = ["| " + " | ".join(header) + " |", align]
    out += ["| " + " | ".join(r) + " |" for r in rows]
    return "\n".join(out)


def render(d: dict) -> str:
    c = cells(d)
    out: list[str] = []
    P = out.append

    # --- T1: Expanse across clocks and cache states ------------------------
    P("### T1 — Expanse on the M7 across clocks and cache states\n")
    rows = []
    for f in FIXTURE_ORDER:
        r = [FIXTURE_LABEL[f]]
        for clk in (64_000_000, 160_000_000, 400_000_000):
            for dc in (0, 1):
                v = cyc(c, "m7", clk, dc, f)
                cell = n0(v)
                if clk == M7 and dc == 1:
                    cell += f" ({n0(ns(c, 'm7', clk, dc, f))} ns)"
                r.append(cell)
        rows.append(r)
    P(table(rows, ["fixture (`embedded_memtable.rs` shape, via the C ABI)", "64 MHz off", "64 MHz on", "160 MHz off", "160 MHz on", "400 MHz off", "400 MHz on (ns)"]))
    P("")
    P("derived: cache-off / cache-on ratio per fixture")
    for f in FIXTURE_ORDER:
        r21 = cyc(c, "m7", M7, 0, f) / cyc(c, "m7", M7, 1, f)
        r11 = cyc(c, "m7", 64_000_000, 0, f) / cyc(c, "m7", 64_000_000, 1, f)
        P(f"  {f:20} 2:1 core:bus (400 MHz) {r21:.2f}×   1:1 (64 MHz) {r11:.2f}×")
    for shape in ("bulk", "steady"):
        for dc in (0, 1):
            lp = cyc(c, "m7", M7, dc, f"evict_{shape}_loop") / cyc(c, "m7", M7, dc, f"evict_{shape}_range")
            P(f"  loop / remove_range, {shape:6} dc{dc}: {lp:.2f}×")
    P(f"  400 MHz cache on: point lookup {n0(ns(c,'m7',M7,1,'can_dispatch'))} ns, sequential insert {n0(ns(c,'m7',M7,1,'ingest'))} ns")
    P("")

    # --- T2: alternatives at 400 MHz, cache on ----------------------------
    P("### T2 — Expanse against the three twins (M7, 400 MHz, D-cache on)\n")
    order = [
        ("ingest", "ingest, 2,000 sequential inserts"),
        ("can_dispatch", "CAN dispatch, 500 gets"),
        ("evict_bulk_range", "evict 600 of 2,000, batched (`remove_range`; scan for the hash)"),
        ("evict_steady_range", "evict 25 of 2,000, batched"),
        ("evict_bulk_loop", "evict 600 of 2,000, per-key loop"),
        ("evict_steady_loop", "evict 25 of 2,000, per-key loop"),
    ]
    rows = []
    for f, label in order:
        e = c[("m7", M7, 1, f, "expanse")]
        r = [label, f"{n0(e['min'])} / {n0(e['min_ns'])} ns"]
        for impl in ("sorted_array", "open_hash", "tsearch"):
            t = twin_cell(c, "m7", M7, 1, f, impl)
            r.append("n/a" if t is None else f"{n0(t['min'])} / {n0(t['min_ns'])} ns ({ratio_word(t['min'], e['min'])})")
        rows.append(r)
    P(table(rows, ["fixture", "Expanse (C ABI)"] + [TWIN_LABEL[i] for i in ("sorted_array", "open_hash", "tsearch")]))
    P("")

    # --- T3: bytes per key ---------------------------------------------------
    P("### T3 — bytes per key (M7 block; the M4 block is byte-identical: %s)\n" % (
        "yes" if [b for b in d["bytes"] if b["core"] == "m7"] and all(
            any(m4["impl"] == m7["impl"] and m4["shape"] == m7["shape"] and m4["heap_bytes"] == m7["heap_bytes"] and m4["req_bytes"] == m7["req_bytes"]
                for m4 in d["bytes"] if m4["core"] == "m4") for m7 in d["bytes"] if m7["core"] == "m7") else "NO"))
    rows = []
    for impl in ("expanse", "sorted_array", "open_hash", "tsearch"):
        r = ["Expanse (C ABI)" if impl == "expanse" else TWIN_LABEL[impl]]
        for shape in ("ingest", "ble_index"):
            b = next(x for x in d["bytes"] if x["core"] == "m7" and x["impl"] == impl and x["shape"] == shape)
            r.append(f"{b['heap_bytes_per_key']:.1f} ({b['req_bytes_per_key']:.1f})")
        rows.append(r)
    P(table(rows, ["bytes per key, 2,000 keys (newlib heap in use via `mallinfo`, allocator overhead included; requested bytes in parentheses)", "sequential keys, one map", "BLE index: hash keys, dual index (ordered) or one table (hash)"]))
    P("")

    # --- T4/T6: ISR arms -------------------------------------------------------
    for core, clk in (("m7", M7), ("m4", M4)):
        P(f"### ISR arms — {core.upper()} ({clk // 10**6} MHz{', D-cache on' if core == 'm7' else ', no cache'})\n")
        sync = sorted((e for e in d["isr"] if e["core"] == core and e["name"] == "isr_sync32"), key=lambda e: e["period"])
        crit = {e["period"]: e for e in d["isr"] if e["core"] == core and e["name"] == "isr_critical_section"}
        rows = []
        for s in sync:
            k = crit[s["period"]]
            if s["period"] == 0:
                duty = f"full duty (~{s['mutations_per_s'] / 1000:.0f}k/s)"
            else:
                duty = f"{s['mutations_per_s'] / 1000:.0f}k/s" if s["mutations_per_s"] >= 10_000 else f"{s['mutations_per_s'] / 1000:.0f}k/s"
            rows.append([
                duty,
                pct(s["busy_rate"]),
                f"{s['lat_max']} / {s['lat_mean']:.0f} ({n0(s['lat_max_ns'])} ns)",
                f"{n0(k['lat_max'])} / {n0(k['lat_mean'])} ({n0(k['lat_max_ns'])} ns)",
                f"{n0(s['writer_cycles_per_mutation'])} / {n0(k['writer_cycles_per_mutation'])}",
            ])
        P(table(rows, ["writer duty (mutations/s)", "`sync32` single-attempt BUSY", "`sync32` ISR entry latency max / mean (max in ns)", "critical-section ISR entry latency max / mean (max in ns)", "writer cycles per mutation `sync32` / critical section"]))
        lm = [s["lat_max"] for s in sync]
        km = [crit[s["period"]]["lat_max"] for s in sync]
        lns = [s["lat_max_ns"] for s in sync]
        kns = [crit[s["period"]]["lat_max_ns"] for s in sync]
        P("")
        P(f"derived: sync32 ceiling {min(lm)}–{max(lm)} cycles ({n0(min(lns))}–{n0(max(lns))} ns) vs critical section {n0(min(km))}–{n0(max(km))} ({n0(min(kns))}–{n0(max(kns))} ns); bound {min(km)/max(lm):.0f}–{max(km)/min(lm):.0f}×")
        full_s, full_k = sync[0], crit[0]
        P(f"derived: full duty, critical section writer cycles/mutation vs sync32: {100*(1-full_k['writer_cycles_per_mutation']/full_s['writer_cycles_per_mutation']):.0f}% cheaper; BUSY by duty: " + " / ".join(pct(s["busy_rate"]) for s in sync))
        P(f"derived: bad (corrupted) total {sum(s['bad'] for s in sync)}, refused {sum(s['refused'] for s in sync)}, arena_full {sum(s['arena_full'] for s in sync)}")
        P("")

    # --- T5: M4 vs M7 ---------------------------------------------------------
    P("### T5 — Cortex-M4 against the M7\n")
    rows = []
    for f in ("ingest", "can_dispatch", "evict_bulk_range", "evict_steady_range"):
        m7on, m7off, m4 = cyc(c, "m7", M7, 1, f), cyc(c, "m7", M7, 0, f), cyc(c, "m4", M4, 0, f)
        rows.append([FIXTURE_LABEL[f], f"{n0(m7on)} ({n0(ns(c,'m7',M7,1,f))} ns)", n0(m7off), f"{n0(m4)} ({n0(ns(c,'m4',M4,0,f))} ns)", f"{m4/m7on:.1f}×", f"{ns(c,'m4',M4,0,f)/ns(c,'m7',M7,1,f):.1f}×"])
    P(table(rows, ["fixture (Expanse, C ABI)", "M7 400 MHz, cache on (ns)", "M7 400 MHz, cache off", "M4 200 MHz (ns)", "M4 / M7 cache-on, cycles", "M4 / M7 cache-on, time"]))
    P("")
    rs = [cyc(c, "m4", M4, 0, f) / cyc(c, "m7", M7, 1, f) for f in FIXTURE_ORDER]
    ro = [cyc(c, "m4", M4, 0, f) / cyc(c, "m7", M7, 0, f) for f in FIXTURE_ORDER]
    P(f"derived: Expanse M4 / M7 cache-on over the six fixtures {min(rs):.1f}–{max(rs):.1f}×; M4 / M7 cache-off {min(ro):.1f}–{max(ro):.1f}×")
    tw = []
    for impl in ("sorted_array", "open_hash"):
        for f in FIXTURE_ORDER:
            a, b = twin_cell(c, "m4", M4, 0, f, impl), twin_cell(c, "m7", M7, 1, f, impl)
            if a and b:
                tw.append((impl, f, a["min"] / b["min"]))
    P(f"derived: sorted array + hash M4 / M7 cache-on {min(t[2] for t in tw):.1f}–{max(t[2] for t in tw):.1f}×  (" + ", ".join(f"{i}/{f} {r:.1f}" for i, f, r in tw) + ")")
    e = lambda f, impl="expanse": cyc(c, "m4", M4, 0, f, impl)
    P(f"derived M4 leads: hash ingest {e('ingest')/e('ingest','open_hash'):.1f}× ({n0(e('ingest','open_hash'))} vs {n0(e('ingest'))}); hash lookup {e('can_dispatch')/e('can_dispatch','open_hash'):.1f}× ({n0(e('can_dispatch','open_hash'))} vs {n0(e('can_dispatch'))}); sorted lookup {e('can_dispatch')/e('can_dispatch','sorted_array'):.1f}× ({n0(e('can_dispatch','sorted_array'))}); sorted bulk range {e('evict_bulk_range','sorted_array')/e('evict_bulk_range'):.1f}× slower ({n0(e('evict_bulk_range','sorted_array'))}); sorted ingest {e('ingest')/e('ingest','sorted_array'):.1f}× ({n0(e('ingest','sorted_array'))}); tsearch steady range {n0(e('evict_steady_range','tsearch'))} vs {n0(e('evict_steady_range'))}")
    P("")

    # --- M7 scorecard figures ----------------------------------------------------
    g = lambda f, impl="expanse": cyc(c, "m7", M7, 1, f, impl)
    h = lambda f: twin_cell(c, "m7", M7, 1, f, "open_hash")["min"]
    P("derived M7 scorecard (400 MHz, cache on):")
    P(f"  hash: ingest {g('ingest')/h('ingest'):.1f}× / lookup {g('can_dispatch')/h('can_dispatch'):.1f}×; sorted: ingest {g('ingest')/g('ingest','sorted_array'):.1f}× / lookup {g('can_dispatch')/g('can_dispatch','sorted_array'):.1f}×")
    P(f"  bulk expiry: hash scan {g('evict_bulk_range')/h('evict_bulk_range'):.1f}× faster than Expanse remove_range; steady expiry: Expanse {h('evict_steady_range')/g('evict_steady_range'):.1f}× over the scan, {g('evict_steady_range','sorted_array')/g('evict_steady_range'):.1f}× over the sorted array")
    P(f"  tsearch: ingest {n0(g('ingest','tsearch'))} cyc ({g('ingest','tsearch')/g('ingest'):.1f}× slower), lookup {n0(g('can_dispatch','tsearch'))} ({g('can_dispatch','tsearch')/g('can_dispatch'):.1f}× slower), inserts vs its lookups {g('ingest','tsearch')/g('can_dispatch','tsearch'):.0f}×")
    P("")

    # --- T6: DWT decomposition ------------------------------------------------------
    if d.get("dwt"):
        P("### T6 — where the cycles of one operation go (DWT profiling counters, per op, bracket subtracted)\n")
        rows = []
        for e in sorted(d["dwt"], key=lambda e: (e["fixture"], e["core"], -e["sysclk"], -e["dcache"], e["impl"])):
            if e["fixture"] == "nop" or (e["core"] == "m7" and e["sysclk"] != M7):
                continue
            n = e["net_per_op"]
            rows.append([f"`{e['fixture']}`", f"`{e['core']}`", "off" if e["dcache"] == 0 else "on", e["impl"], n1(n["cycles"]), n1(n["instr"]), n1(n["cpi"]), n1(n["lsu"]), n1(n["fold"]), f"{e['cpi_max']} / {e['lsu_max']}", "**wrap-suspect**" if e["wrap_risk"] else "clean"])
        P(table(rows, ["fixture", "core", "D-cache", "impl", "cycles", "instructions (derived)", "CPI stalls", "LSU stalls", "folded", "per-op max CPI / LSU", "counter integrity"], "|---|---|---|---|---:|---:|---:|---:|---:|---:|---|"))
        nop = [e for e in d["dwt"] if e["fixture"] == "nop"]
        P("")
        P("derived: bracket cost per op (subtracted above): " + ", ".join(f"{e['core']} {e['sysclk']//10**6} MHz dc{e['dcache']} {e['raw_per_op']['cycles']:.1f} cyc" for e in sorted(nop, key=lambda e: (e['core'], e['sysclk'], e['dcache']))))
        P("")

    # --- T7: dual core -----------------------------------------------------------
    P("### T7 — two cores on one map\n")
    rows = []
    for e in d["dual"]:
        heap = "non-cacheable" if e["heap"] == "noncacheable" else "cacheable"
        mode = "`sync32` single-attempt" if e["mode"] == "optimistic" else "HSEM-locked twin"
        if e["period"] == 0:
            duty = "full duty"
        else:
            duty = f"{e['mutations_per_s']/1000:.0f}k/s" if e["mutations_per_s"] >= 10_000 else f"{e['mutations_per_s']/1000:.0f}k/s"
        m4ns = n0(e["m4_read_ns_max"]) if "m4_read_ns_max" in e else "?"
        wait = "—" if e["mode"] == "optimistic" else f"{n0(e['m4_wait_mean'])} / {n0(e['m4_wait_max'])}"
        rows.append([heap, mode, duty, n0(e["m4_reads"]), f"{n0(e['m4_ok'])} / {n0(e['m4_nf'])}", pct(e["busy_rate"]), f"{e['m4_bad']} / {e['writer_bad']}", f"{n0(e['m4_cyc_mean'])} / {n0(e['m4_cyc_max'])} ({m4ns} ns)", wait, n0(e["writer_cycles_per_mutation"])])
    P(table(rows, ["M7 heap", "reads", "writer duty", "M4 reads", "OK / not found", "BUSY", "corrupted (M4 / writer)", "M4 read cycles mean / max (max in ns)", "lock wait mean / max", "writer cycles per mutation"], "|---|---|---|---:|---:|---:|---:|---:|---:|---:|"))
    P("")
    nc = [e for e in d["dual"] if e["heap"] == "noncacheable" and e["mode"] == "optimistic"]
    hs = [e for e in d["dual"] if e["heap"] == "noncacheable" and e["mode"] == "hsem"]
    ca = [e for e in d["dual"] if e["heap"] == "cacheable"]
    m4clk = d["dual"][0]["m4_sysclk"]
    P("derived: non-cacheable BUSY by duty " + " / ".join(pct(e["busy_rate"]) for e in nc) + f"; low-duty read mean {n0(nc[-1]['m4_cyc_mean'])} cyc ({nc[-1]['m4_cyc_mean']/m4clk*1e6:.1f} µs); BUSY answer worst {n0(max(e['m4_cyc_max'] for e in nc))} cyc")
    P(f"derived: HSEM lock wait max full duty {n0(hs[0]['m4_wait_max'])} cyc ({hs[0]['m4_wait_max']/m4clk*1e6:.0f} µs), at ≤10k/s {n0(max(e['m4_wait_max'] for e in hs[2:]))} ({max(e['m4_wait_max'] for e in hs[2:])/m4clk*1e6:.1f} µs); HSEM read mean {n0(min(e['m4_cyc_mean'] for e in hs))}–{n0(max(e['m4_cyc_mean'] for e in hs))} cyc; HSEM BUSY " + " / ".join(pct(e["busy_rate"]) for e in hs))
    P("derived: cacheable BUSY " + " / ".join(pct(e["busy_rate"]) for e in ca) + f", corrupted {sum(e['m4_bad']+e['writer_bad'] for e in ca)}, ok {sum(e['m4_ok'] for e in ca)}")
    P(f"derived: same-core M7 ISR BUSY at 40k/s vs cross-core non-cacheable at 40k/s: see ISR table; cross-core/ISR ratio {nc[1]['busy_rate'] / next(e['busy_rate'] for e in d['isr'] if e['core']=='m7' and e['name']=='isr_sync32' and e['period']==10000):.1f}×")
    return "\n".join(out)


def render_pair(cp: Path, tp: Path) -> str:
    c, t = load(cp), load(tp)
    ci, ti = cells(c), cells(t)
    out = [f"### Paired capture — control `{c['info']['commit']}` vs treatment `{t['info']['commit']}`\n"]
    rows = []
    for core, clk, dc, name in sorted({k[:4] for k in ci if k[4] == "expanse"}):
        a, b = ci[(core, clk, dc, name, "expanse")]["min"], ti[(core, clk, dc, name, "expanse")]["min"]
        moves = [abs(ti[k]["min"] / ci[k]["min"] - 1) for k in ci if k[:4] == (core, clk, dc, name) and k[4] != "expanse" and k in ti]
        floor = max(moves) if moves else float("nan")
        delta = b / a - 1
        verdict = "inside noise" if abs(delta) <= floor else ("outside noise — attributed" if delta < 0 else "outside noise — **moved up**")
        rows.append([f"`{core}`", str(clk // 10**6), "off" if dc == 0 else "on", f"`{name}`", n0(a), f"**{n0(b)}**" if abs(delta) > floor else n0(b), f"**{100*delta:+.1f}%**" if abs(delta) > floor else f"{100*delta:+.1f}%", f"{100*floor:.1f}%", verdict])
    out.append(table(rows, ["core", "MHz", "D-cache", "fixture", f"`{c['info']['commit']}`", f"`{t['info']['commit']}`", "Δ", "noise floor", "verdict"], "|---|---:|---|---|---:|---:|---:|---:|---|"))
    out.append("")
    if c.get("dwt") and t.get("dwt"):
        out.append("")
        out.append("DWT decomposition of the Expanse cells, per op, bracket subtracted (control → treatment):")
        rows = []
        key = lambda e: (e["core"], e["fixture"], e["impl"], e["sysclk"], e["dcache"])
        ti_d = {key(e): e for e in t["dwt"]}
        for e in sorted(c["dwt"], key=lambda e: (e["fixture"], e["core"], -e["sysclk"], -e["dcache"])):
            if e["impl"] != "expanse" or key(e) not in ti_d:
                continue
            f = ti_d[key(e)]
            cn, tn = e["net_per_op"], f["net_per_op"]
            cell = lambda k: f"{cn[k]:.1f} → {tn[k]:.1f} ({tn[k]-cn[k]:+.1f})"
            rows.append([f"`{e['fixture']}`", f"`{e['core']}`", str(e["sysclk"] // 10**6), "off" if e["dcache"] == 0 else "on", cell("cycles"), cell("instr"), cell("cpi"), cell("lsu"), cell("fold"), "**wrap-suspect**" if (e["wrap_risk"] or f["wrap_risk"]) else "clean"])
        out.append(table(rows, ["fixture", "core", "MHz", "D-cache", "cycles", "instructions (derived)", "CPI stalls", "LSU stalls", "folded", "counter integrity"], "|---|---|---:|---|---:|---:|---:|---:|---:|---|"))
    for core in ("m7", "m4"):
        cs = {e["period"]: e for e in c["isr"] if e["core"] == core and e["name"] == "isr_sync32"}
        ts = {e["period"]: e for e in t["isr"] if e["core"] == core and e["name"] == "isr_sync32"}
        ck = {e["period"]: e for e in c["isr"] if e["core"] == core and e["name"] == "isr_critical_section"}
        tk = {e["period"]: e for e in t["isr"] if e["core"] == core and e["name"] == "isr_critical_section"}
        s, u, k, v = cs[0], ts[0], ck[0], tk[0]
        out.append(f"ISR full duty, {core}: sync32 writer cycles/mutation {n0(s['writer_cycles_per_mutation'])} → {n0(u['writer_cycles_per_mutation'])} ({100*(u['writer_cycles_per_mutation']/s['writer_cycles_per_mutation']-1):+.1f}%), BUSY {pct(s['busy_rate'])} → {pct(u['busy_rate'])}; critical-section twin writer {n0(k['writer_cycles_per_mutation'])} → {n0(v['writer_cycles_per_mutation'])} ({100*(v['writer_cycles_per_mutation']/k['writer_cycles_per_mutation']-1):+.1f}%)")
    return "\n".join(out)


def main() -> int:
    args = sys.argv[1:]
    if args[:1] == ["--pair"]:
        print(render_pair(Path(args[1]), Path(args[2])))
        return 0
    print(render(load(DATA)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
