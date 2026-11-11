#!/usr/bin/env python3
"""Reading of the microcode-uop attribution (README.md, "Diagnostic: taken
branches and microcode on the delete phase").

Copies `diag_ms_attribution.py`'s output to
`results/diagnostic_ms_attribution_<host>.json` and prints, per shape, D's
functions and instructions holding the most samples and the rep-string
reading. One process per (shape, arm): a census, no interval.

Usage: diag_ms_attribution_analyze.py <host_label> <attribution_prefix>
"""
import json
import sys

from analyze import RESULTS

CONFIRMED, REFUTED = 0.5, 0.1


def main():
    host, prefix = sys.argv[1], sys.argv[2]
    art = json.loads(open(f"{prefix}.json").read())
    dest = RESULTS / f"diagnostic_ms_attribution_{host}.json"
    dest.write_text(json.dumps(art, indent=1) + "\n")
    print(f"-> {dest}")
    for c in art["cells"]:
        print(f"\n{c['workload_id']} arm {c['arm']}: {c['event']}/{c['modifiers']}, "
              f"{c['samples']:.0f} samples, rep-string share {c['rep_string_share']:.3f}")
        for f in c["functions"][:6]:
            print(f"  fn {f['pct']:5.1f}%  {f['symbol']} [{f['dso']}]")
        for x in c["instructions"][:8]:
            print(f"  insn {100 * x['share']:5.1f}%  {x['insn']}  in {x['symbol']} [{x['dso']}] @{x['addr']}")
        if c["arm"] == "D":
            r = c["rep_string_share"]
            verdict = "CONFIRMED" if r >= CONFIRMED else "REFUTED" if r <= REFUTED else "PARTIAL"
            print(f"  rep-string hypothesis: {verdict}")


if __name__ == "__main__":
    main()
