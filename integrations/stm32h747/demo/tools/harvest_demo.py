#!/usr/bin/env python3
"""Turn the demo's VCP transcript into a per-step summary (table + JSON).

RESULT lines arrive once a second with cumulative per-step counters; the last
line of each step is that step's record. INFO step= lines mark step changes,
INFO rehash lines carry each measured doubling. Nothing is typed in (§8.2).

    harvest_demo.py transcript.txt [commit]
"""
import json, sys

path = sys.argv[1]; commit = sys.argv[2] if len(sys.argv) > 2 else "unknown"
steps, rehashes, checks, info = {}, [], [], {}
for line in open(path, errors="replace"):
    line = line.strip()
    if line.startswith("RESULT "):
        kv = dict(t.split("=", 1) for t in line[7:].split() if "=" in t)
        r = {k: (int(v) if v.lstrip("-").isdigit() else v) for k, v in kv.items()}
        steps[r["step"]] = r                                    # last line of the step wins
    elif line.startswith("INFO rehash"):
        rehashes.append(dict(t.split("=", 1) for t in line[5:].split() if "=" in t))
    elif line.startswith("CHECK"):
        checks.append(line)
    elif line.startswith("INFO step="):
        import re
        m = re.match(r"INFO step=(\d+) name=(.*?) sweep_hz=", line); info[int(m.group(1))] = m.group(2)   # the name carries a space
    elif line.startswith("[OK] lanes"):
        info["prefill"] = line
out = {"commit": commit, "steps": [], "rehashes": rehashes, "checks": checks, "info": info}
print(f"{'step':<9}{'hz':>3}{'mode':>5}{'grow':>5} | {'A blocked':>9}{'A stale':>8}{'A busy':>7}{'A wrong':>8}{'A nov%':>7} | {'B blocked':>9}{'B stale':>8}{'B wrong':>8}{'B nov%':>7}{'B mask us':>10} | {'A sweep':>8}{'B sweep':>8} | {'A B/rec':>8}{'B B/rec':>8}{'A ns':>6}{'B ns':>6}")
for s in sorted(steps):
    r = steps[s]
    at = r["a_served"] + r["a_blocked_ms"]; bt = r["b_served"] + r["b_blocked_ms"]
    row = {"step": s, "name": info.get(s, ""), "sweep_hz": r["sweep_hz"], "mode": r["mode"], "growth": r["growth"], "t_ms": r["t"],
           "a": {"records": r["a_records"], "blocked_ms": r["a_blocked_ms"], "stale_max_ms": r["a_stale_max_ms"], "busy": r["a_busy"], "wrong": r["a_nf"] + r["a_bad"],
                 "no_value_pct": 100 * r["a_no_value_ms"] / at if at else None, "lat_max_ns": r["a_lat_max_ns"], "sweep_us": r["a_sweep_us"], "sweep_net_us": r["a_sweep_net_us"],
                 "body_max_cyc": r["a_body_max_cyc"], "bytes": r["a_bytes"], "bytes_per_record": r["a_bytes"] / r["a_records"] if r["a_records"] else None, "lookup_ns": r["a_lookup_ns"], "drops": r["a_drops"]},
           "b": {"records": r["b_records"], "blocked_ms": r["b_blocked_ms"], "stale_max_ms": r["b_stale_max_ms"], "wrong": r["b_nf"] + r["b_bad"],
                 "no_value_pct": 100 * r["b_no_value_ms"] / bt if bt else None, "lat_max_ns": r["b_lat_max_ns"], "sweep_us": r["b_sweep_us"], "sweep_net_us": r["b_sweep_net_us"],
                 "body_max_cyc": r["b_body_max_cyc"], "mask_max_us": r["b_mask_max_us"], "bytes": r["b_bytes"], "bytes_per_record": r["b_bytes"] / r["b_records"] if r["b_records"] else None, "lookup_ns": r["b_lookup_ns"], "drops": r["b_drops"], "rehashes": r["b_rehashes"]}}
    out["steps"].append(row)
    a, b = row["a"], row["b"]
    print(f"{row['name']:<9}{r['sweep_hz']:>3}{r['mode']:>5}{r['growth']:>5} | {a['blocked_ms']:>9}{a['stale_max_ms']:>8}{a['busy']:>7}{a['wrong']:>8}{(a['no_value_pct'] or 0):>7.2f} | "
          f"{b['blocked_ms']:>9}{b['stale_max_ms']:>8}{b['wrong']:>8}{(b['no_value_pct'] or 0):>7.2f}{b['mask_max_us']:>10} | {a['sweep_us']/1000:>8.1f}{b['sweep_us']/1000:>8.1f} | "
          f"{(a['bytes_per_record'] or 0):>8.1f}{(b['bytes_per_record'] or 0):>8.1f}{a['lookup_ns']:>6}{b['lookup_ns']:>6}")
if rehashes: print("\nrehashes:", ", ".join(f"{r['moved']} moved -> {r['slots']} slots: {r['ms']} ms" for r in rehashes))
if checks: print("\nCHECK lines:", *checks, sep="\n  ")
json.dump(out, open(path.rsplit(".", 1)[0] + ".json", "w"), indent=1)
