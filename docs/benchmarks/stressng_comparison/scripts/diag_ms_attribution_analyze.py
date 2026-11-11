#!/usr/bin/env python3
"""Reading of the microcode-uop attribution (README.md, "Diagnostic: taken
branches and microcode on the delete phase").

Copies `diag_ms_attribution.py`'s output to
`results/diagnostic_ms_attribution_<host>.json` and prints, per shape, D's
functions and instructions holding the most samples and the rep-string
reading. One process per (shape, arm): a census, no interval.

With a raw directory (the recorder's `<shape>_<arm>.report.txt` and
`.annotate.txt`, copied verbatim), each cell's function and instruction lists
are parsed again from that text with the current parser, and the cell says so.

Usage: diag_ms_attribution_analyze.py <host_label> <attribution_prefix> [raw_dir]
"""
import json
import subprocess
import sys
from pathlib import Path

from analyze import RESULTS
from ms_annotate import parse

CONFIRMED, REFUTED = 0.5, 0.1


def main():
    host, prefix = sys.argv[1], sys.argv[2]
    art = json.loads(open(f"{prefix}.json").read())
    if len(sys.argv) > 3:
        raw = Path(sys.argv[3])
        head = subprocess.run(["git", "rev-parse", "--short=9", "HEAD"], capture_output=True,
                              text=True, check=True).stdout.strip()
        for c in art["cells"]:
            base = raw / f"{c['workload_id'][6:]}_{c['arm']}"
            functions, top, total, rep = parse((base.parent / (base.name + ".report.txt")).read_text(),
                                               (base.parent / (base.name + ".annotate.txt")).read_text())
            if abs(total - c["samples"]) > 0.5:
                sys.exit(f"STOP: reparsed {total} samples for {base.name}, the run recorded {c['samples']}")
            c.update(functions=functions, instructions=top, rep_string_share=rep,
                     reparsed={"from": f"{base.name}.report.txt, {base.name}.annotate.txt",
                               "parser_commit": head})
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
