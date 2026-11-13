#!/usr/bin/env python3
"""Per-operation tables of the #1680 Callgrind ranking (README.md, "Diagnostic:
Callgrind ranking of the #1680 shapes").

Reads the committed `results/callgrind_1680/` outputs and prints:
- per bench arm (`bench/`, `crates/expanse/benches/instructions.rs`): total
  Ir, conditional and indirect mispredicts per operation, then each
  function's self cost per operation, summed over the per-file rows that
  inlining splits it into;
- per `phase_probe` shape and arm (`phase_probe/`): Ir, branches and
  simulated mispredicts per `JudyLGet` / `JudyLIns` call, N against S.

Needs `callgrind_annotate` for the per-function rows; the totals are read
from the files' `summary:` / `totals:` line. The simulated mispredicts are
Callgrind's `--branch-sim=yes` model, not the hardware's predictor.

Usage: callgrind_1680_summary.py [results/callgrind_1680]
"""
import collections
import re
import subprocess
import sys
from pathlib import Path

SUITE = Path(__file__).resolve().parent.parent
DIR = Path(sys.argv[1]) if len(sys.argv) > 1 else SUITE / "results" / "callgrind_1680"

# Operations per arm, from the arm definitions in instructions.rs: the sm
# arms probe their draws again and then as many fresh draws (2 x draws); the
# insert arms make one `ins_slot` per key.
BENCH_OPS = {
    "map_get_slot_ptr.sm_10k": 20_000, "map_get_slot_ptr.sm_1m": 2_000_000,
    "map_get.sm_10k": 20_000, "map_get.sm_1m": 2_000_000, "map_get.random": 50_000,
    "map_ins_slot.gen_4m": 4_194_304, "map_ins_slot.random": 50_000,
}
# Calls per phase_probe shape under --toggle-collect: sm_* collects every
# JudyLGet, the build's lookups (draws) and the two get phases (2 x draws);
# judy_4m collects its 4,194,304 JudyLIns.
PROBE_CALLS = {"sm_default": 30_000, "sm_big": 3_000_000, "judy_4m": 4_194_304}
PROBE_FN = {"sm_default": "JudyLGet", "sm_big": "JudyLGet", "judy_4m": "JudyLIns"}


def totals(path):
    """Event name -> total, from the `events:` and `summary:`/`totals:` lines."""
    events, tot = None, None
    for line in path.read_text().splitlines():
        if line.startswith("events:"):
            events = line.split()[1:]
        elif line.startswith(("summary:", "totals:")):
            tot = [int(x) for x in line.split()[1:]]
    if events is None or tot is None:
        sys.exit(f"STOP: no events/totals line in {path}")
    return dict(zip(events, tot + [0] * (len(events) - len(tot))))


def per_function(path):
    """Function -> [Ir, Bcm, Bim] self cost, summed over per-file rows."""
    out = subprocess.run(["callgrind_annotate", "--inclusive=no", "--show=Ir,Bcm,Bim",
                          "--threshold=100", str(path)], capture_output=True, text=True, check=True).stdout
    agg = collections.defaultdict(lambda: [0, 0, 0])
    row = re.compile(r"^\s*([\d,]+) \([^)]*\)\s+([\d,.]+)(?: \([^)]*\))?\s+([\d,.]+)(?: \([^)]*\))?\s+(.*)$")
    for line in out.splitlines():
        m = row.match(line)
        if not m or m.group(4).startswith("PROGRAM TOTALS"):
            continue
        name = m.group(4).strip()
        fn = name.split(":", 1)[1] if ":" in name else name
        fn = re.sub(r"\s*\[.*\]$", "", fn).replace("expanse_trie::", "")
        for i, v in enumerate(m.group(1, 2, 3)):
            agg[fn][i] += 0 if v == "." else int(v.replace(",", ""))
    if not agg:
        sys.exit(f"STOP: callgrind_annotate parsed no function rows from {path}")
    return agg


def main():
    print("## bench arms (instructions.rs), per operation")
    for arm, ops in BENCH_OPS.items():
        f = DIR / "bench" / f"callgrind.{arm}.out"
        t = totals(f)
        print(f"\n### {arm}: {t['Ir'] / ops:.1f} Ir/op, Bcm {t['Bcm'] / ops:.3f}/op, "
              f"Bim {t['Bim'] / ops:.3f}/op (ops {ops:,})")
        for fn, (ir, bcm, bim) in sorted(per_function(f).items(), key=lambda kv: -kv[1][0])[:8]:
            if ir == 0:
                continue
            print(f"- {ir / t['Ir'] * 100:5.1f}%  {ir / ops:7.1f} Ir/op  Bcm {bcm / ops:.3f}  "
                  f"Bim {bim / ops:.3f}  `{fn[:100]}`")
    print("\n## phase_probe, per collected call, N against S")
    print("| shape | arm | Ir | Bc | Bcm | Bi | Bim |")
    print("|---|---|---|---|---|---|---|")
    for shape, calls in PROBE_CALLS.items():
        t = {arm: totals(DIR / "phase_probe" / f"cg.{arm}.{shape}.out") for arm in ("S", "N")}
        for arm in ("S", "N"):
            v = t[arm]
            print(f"| `{shape}` ({PROBE_FN[shape]}) | {arm} | {v['Ir'] / calls:.1f} | {v['Bc'] / calls:.2f} | "
                  f"{v['Bcm'] / calls:.3f} | {v['Bi'] / calls:.2f} | {v['Bim'] / calls:.3f} |")
        print(f"| `{shape}` | N/S | {t['N']['Ir'] / t['S']['Ir']:.3f} | {t['N']['Bc'] / t['S']['Bc']:.2f} | "
              f"{t['N']['Bcm'] / t['S']['Bcm']:.2f} | {t['N']['Bi'] / t['S']['Bi']:.2f} | "
              f"{t['N']['Bim'] / t['S']['Bim']:.2f} |")


if __name__ == "__main__":
    main()
