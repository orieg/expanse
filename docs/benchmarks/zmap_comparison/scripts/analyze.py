#!/usr/bin/env python3
"""Apply PREREG.md's void rules and decision rules to the raw JSONL files.

usage: analyze.py <results_dir> <repo_root>
"""
import json, os, sys
from collections import defaultdict

res, repo = sys.argv[1], sys.argv[2]
sys.path.insert(0, os.path.join(repo, "scripts"))
from bca_bootstrap import bca_bootstrap_ci_with_method, bca_bootstrap_ratio_ci_with_method  # noqa: E402

EXPECT = {"e1": 524288, "e2": 1048576}
TOL = {"e1": 52, "e2": 104}  # PREREG amendment A1: 0.01% of expected
VOID_FOREIGN = 2.5


def load(path):
    return [json.loads(l) for l in open(path) if l.strip()]


def recv_cpu(rec):
    t = rec["thread_cpu_ns"]
    tids = sorted(int(k) for k in t if int(k) != rec["zpid"])
    return t[str(tids[0])]  # first thread zmap creates (src/zmap.c:233)


def ci(xs):
    m, lo, hi, meth = bca_bootstrap_ci_with_method(xs)
    return m, lo, hi, meth


def ratio(num, den):
    return bca_bootstrap_ratio_ci_with_method(num, den)


def verdict(intervals):
    if all(hi < 1.0 for _, lo, hi, _ in intervals):
        return "EXPANSE_FASTER/SMALLER"
    if all(lo > 1.0 for _, lo, hi, _ in intervals):
        return "EXPANSE_SLOWER/LARGER"
    return "NO_DIFFERENCE_DETECTED"


def fmt(t, scale=1.0, nd=1):
    m, lo, hi, meth = t
    return f"{m*scale:.{nd}f} [{lo*scale:.{nd}f}, {hi*scale:.{nd}f}] ({meth})"


def e2e():
    print("## End to end")
    runs = sorted(d for d in os.listdir(res) if d.startswith("e2e_run"))
    share_iv = defaultdict(list); ratio_iv = defaultdict(list)
    for d in runs:
        recs = load(os.path.join(res, d, "raw.jsonl"))
        bad_rounds = set(); reasons = defaultdict(int)
        for r in recs:
            own = (r["user_s"] + r["sys_s"]) / max(r["wall_s"], 1e-9)
            foreign = r["host_busy_core_eq"] - own
            r["foreign"] = foreign
            why = None
            if r["rc"] != 0: why = "rc"
            elif r.get("pcap_drop") or r.get("pcap_ifdrop"): why = "pcap_drop"
            elif r.get("rows") < EXPECT[r["workload"]] - TOL[r["workload"]]: why = "rows"
            elif foreign > VOID_FOREIGN: why = "foreign_load"
            if why:
                bad_rounds.add(r["round"]); reasons[why] += 1
        keep = [r for r in recs if r["round"] not in bad_rounds]
        nr = len({r["round"] for r in keep})
        print(f"\n### {d}: {nr} admissible rounds of {len({r['round'] for r in recs})}; void cells {dict(reasons)}")
        fl = sorted(r["foreign"] for r in recs)
        print(f"foreign load core-eq: median {fl[len(fl)//2]:.2f}, max {fl[-1]:.2f}")
        for wl in ("e1", "e2"):
            by = defaultdict(lambda: defaultdict(list))
            for r in keep:
                if r["workload"] != wl: continue
                n = r["validation_passed"]  # A1: the run's own response count
                by[r["arm"]]["recv_ns_per_resp"].append(recv_cpu(r) / n)
                by[r["arm"]]["proc_cpu_s"].append(r["user_s"] + r["sys_s"])
                by[r["arm"]]["maxrss_mb"].append(r["maxrss_kb"] / 1024)
                by[r["arm"]]["wall_s"].append(r["wall_s"])
            print(f"\n#### {wl} (responses {EXPECT[wl]:,})")
            print("| metric | NONE | STOCK | EXPANSE | EXPANSE/STOCK |")
            print("|---|---|---|---|---|")
            for met in ("recv_ns_per_resp", "proc_cpu_s", "maxrss_mb", "wall_s"):
                cells = [fmt(ci(by[a][met]), nd=3 if met != "recv_ns_per_resp" else 1) for a in ("NONE", "STOCK", "EXPANSE")]
                rt = ratio(by["EXPANSE"][met], by["STOCK"][met]); ratio_iv[(wl, met)].append(rt)
                print(f"| {met} | " + " | ".join(cells) + f" | {fmt(rt, nd=4)} |")
            s, n_ = by["STOCK"]["recv_ns_per_resp"], by["NONE"]["recv_ns_per_resp"]
            # share = 1 - NONE/STOCK; interval from the ratio NONE/STOCK
            m, lo, hi, meth = ratio(n_, s)
            sh = (1 - m, 1 - hi, 1 - lo, meth); share_iv[wl].append(sh)
            print(f"\ndedup share of recv-thread CPU (STOCK): {fmt(sh, 100, 1)} %")
            m, lo, hi, meth = ratio(by["NONE"]["recv_ns_per_resp"], by["EXPANSE"]["recv_ns_per_resp"])
            print(f"dedup share of recv-thread CPU (EXPANSE): {fmt((1-m, 1-hi, 1-lo, meth), 100, 1)} %")
    print("\n### E2E verdicts")
    for wl, ivs in share_iv.items():
        if all(hi < 0.10 for _, lo, hi, _ in ivs): v = "OFF_HOT_PATH"
        elif all(lo > 0.10 for _, lo, hi, _ in ivs): v = "ON_HOT_PATH"
        else: v = "INTERMEDIATE"
        print(f"H1 {wl}: {v}")
    for (wl, met), ivs in sorted(ratio_iv.items()):
        print(f"H2 {wl} {met}: {verdict(ivs)}")


def micro():
    print("\n## Microbenchmark")
    runs = sorted(d for d in os.listdir(res) if d.startswith("micro_run"))
    ivs = defaultdict(list)
    for d in runs:
        recs = load(os.path.join(res, d, "raw.jsonl"))
        bad = {r["round"] for r in recs if r["rc"] != 0 or not r["ok"] or r["host_busy_core_eq"] - 1.0 > VOID_FOREIGN}
        keep = [r for r in recs if r["round"] not in bad]
        print(f"\n### {d}: {len({r['round'] for r in keep})} admissible rounds; void rounds {sorted(bad)}")
        print("| workload | STOCK ns/resp | EXPANSE ns/resp | ratio | STOCK RSS growth MB | EXPANSE RSS growth MB | ratio |")
        print("|---|---|---|---|---|---|---|")
        for wl in ("m1", "m2", "m3"):
            g = lambda a, k: [r[k] for r in keep if r["workload"] == wl and r["arm"] == a]
            rss = lambda a: [(r["maxrss_kb_after"] - r["maxrss_kb_before"]) / 1024 for r in keep if r["workload"] == wl and r["arm"] == a]
            s, e = g("STOCK", "ns_per_response"), g("EXPANSE", "ns_per_response")
            rt = ratio(e, s); ivs[(wl, "ns")].append(rt)
            rr = ratio(rss("EXPANSE"), rss("STOCK")); ivs[(wl, "rss")].append(rr)
            print(f"| {wl} | {fmt(ci(s))} | {fmt(ci(e))} | {fmt(rt, nd=3)} | {fmt(ci(rss('STOCK')))} | {fmt(ci(rss('EXPANSE')))} | {fmt(rr, nd=3)} |")
    print("\n### Micro verdicts")
    for k, v in sorted(ivs.items()):
        print(f"H3 {k[0]} {k[1]}: {verdict(v)}")


def key_order_gates():
    """Gates K1, K2, K3 and K5 of results/zmap_key_order_1681_preregistration.yaml.

    Printed only for a record that carries the MAIN arm (libexpanse at the
    comparison base); EXPANSE is the candidate. Void rules as above.
    """
    def runs(prefix):
        return sorted(d for d in os.listdir(res) if d.startswith(prefix))

    def bound(name, ivs, limit, strict=False):
        ok = all((hi < limit) if strict else (hi <= limit) for _, _, hi, _ in ivs)
        op = "<" if strict else "<="
        return f"{name}: {'MET' if ok else 'NOT MET'} (upper bound {op} {limit} required in each of {len(ivs)} cells)"

    micro_runs = [d for d in runs("micro_run") if any(r["arm"] == "MAIN" for r in load(os.path.join(res, d, "raw.jsonl")))]
    e2e_runs = [d for d in runs("e2e_run") if any(r["arm"] == "MAIN" for r in load(os.path.join(res, d, "raw.jsonl")))]
    if not micro_runs and not e2e_runs:
        return
    print("\n## Key-order gates (EXPANSE = candidate, MAIN = comparison base)")
    k1, k2, k3 = [], [], []
    for d in micro_runs:
        recs = load(os.path.join(res, d, "raw.jsonl"))
        bad = {r["round"] for r in recs if r["rc"] != 0 or not r["ok"] or r["host_busy_core_eq"] - 1.0 > VOID_FOREIGN}
        keep = [r for r in recs if r["round"] not in bad]
        print(f"\n### {d}: {len({r['round'] for r in keep})} admissible rounds; void rounds {sorted(bad)}")
        print("| workload | MAIN ns/resp | EXPANSE/MAIN ns | EXPANSE/STOCK ns | MAIN RSS growth MB | EXPANSE/MAIN RSS | EXPANSE/STOCK RSS |")
        print("|---|---|---|---|---|---|---|")
        for wl in ("m1", "m2", "m3"):
            g = lambda a: [r["ns_per_response"] for r in keep if r["workload"] == wl and r["arm"] == a]
            rss = lambda a: [(r["maxrss_kb_after"] - r["maxrss_kb_before"]) / 1024 for r in keep if r["workload"] == wl and r["arm"] == a]
            tm, ts = ratio(g("EXPANSE"), g("MAIN")), ratio(g("EXPANSE"), g("STOCK"))
            rm, rs = ratio(rss("EXPANSE"), rss("MAIN")), ratio(rss("EXPANSE"), rss("STOCK"))
            k1.append(rm); k2.append(tm); k3.append(ts)
            print(f"| {wl} | {fmt(ci(g('MAIN')))} | {fmt(tm, nd=3)} | {fmt(ts, nd=3)} | {fmt(ci(rss('MAIN')))} | {fmt(rm, nd=3)} | {fmt(rs, nd=3)} |")
    k5t, k5r = [], []
    for d in e2e_runs:
        recs = load(os.path.join(res, d, "raw.jsonl"))
        bad = set()
        for r in recs:
            own = (r["user_s"] + r["sys_s"]) / max(r["wall_s"], 1e-9)
            if (r["rc"] != 0 or r.get("pcap_drop") or r.get("pcap_ifdrop")
                    or r.get("rows") < EXPECT[r["workload"]] - TOL[r["workload"]]
                    or r["host_busy_core_eq"] - own > VOID_FOREIGN):
                bad.add(r["round"])
        keep = [r for r in recs if r["round"] not in bad]
        print(f"\n### {d}: {len({r['round'] for r in keep})} admissible rounds; void rounds {sorted(bad)}")
        print("| workload | EXPANSE/MAIN recv ns per response | EXPANSE/MAIN peak RSS | EXPANSE/MAIN process CPU | EXPANSE/MAIN wall |")
        print("|---|---|---|---|---|")
        for wl in ("e1", "e2"):
            col = lambda a, f: [f(r) for r in keep if r["workload"] == wl and r["arm"] == a]
            pair = lambda f: ratio(col("EXPANSE", f), col("MAIN", f))
            t = pair(lambda r: recv_cpu(r) / r["validation_passed"])
            m = pair(lambda r: r["maxrss_kb"] / 1024)
            c = pair(lambda r: r["user_s"] + r["sys_s"])
            w = pair(lambda r: r["wall_s"])
            k5t.append(t); k5r.append(m)
            print(f"| {wl} | {fmt(t, nd=4)} | {fmt(m, nd=4)} | {fmt(c, nd=4)} | {fmt(w, nd=4)} |")
    print("\n### Key-order gate outcomes (this host)")
    if k1:
        print(bound("K1 micro RSS growth EXPANSE/MAIN", k1, 0.95))
        print(bound("K2 micro ns per response EXPANSE/MAIN", k2, 1.02))
        print(bound("K3 micro ns per response EXPANSE/STOCK", k3, 1.0, strict=True))
    if k5t:
        print(bound("K5 e2e receive-thread ns per response EXPANSE/MAIN", k5t, 1.01))
        print(bound("K5 e2e peak RSS EXPANSE/MAIN", k5r, 1.005))


if any(d.startswith("e2e_run") for d in os.listdir(res)):
    e2e()
if any(d.startswith("micro_run") for d in os.listdir(res)):
    micro()
key_order_gates()
