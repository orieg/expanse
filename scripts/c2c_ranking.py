#!/usr/bin/env python3
"""The contended cache lines of a `writer_scaling_diagnostic` run, ranked (#930).

Reads the `c2c` block `docs/benchmarks/concurrency/scripts/writer_scaling.py`
writes into a diagnostic artifact and ranks the cache lines `perf c2c report`
listed by their share of HITM load samples. Every share is computed here from
the artifact (AGENTS.md section 8.2); nothing is typed.

What the artifact carries, and therefore what this can say:

- `summary`: the report's trace-event totals. `Load Local HITM` plus
  `Load Remote HITM` is the denominator of every share. The report prints its
  own `Hitm %` per line, and `parse` refuses a run whose shares disagree with
  that column, because then the denominator is not the one `perf` used.
- `hot_cache_lines`: the report's "Shared Data Cache Line Table" (one row per
  line) followed by as much of the "Shared Cache Line Distribution Pareto" as
  fits under the harness's row cap. The Pareto blocks carry the per-offset
  split of a line's HITM and stores and the (truncated) symbols that touched
  each offset. Only the blocks that fit are carried; a block that the cap cut
  is marked partial and never classified.
- `symbol_profile`: `perf report --sort symbol,dso` over the same recording,
  untruncated, top rows only. It is over all samples, not HITM samples, and it
  is used only to resolve a truncated symbol by prefix (AGENTS.md section
  8.20.5 step 5).

What it cannot carry: the binary, the struct layout (`sync::layout_report()`),
or the address any structure was allocated at. So a line is placed by its
address *layout* (section 8.20.5 step 4), never by field name:

- `kernel`: an address at or above the canonical kernel half.
- `mapping head`: within the first page of a 64 MiB-aligned mapping. The
  alignment is read from the address; which allocator structure sits there is
  not resolved and is not claimed.
- `block`: a sub-page cluster of lines whose relative offsets recur across
  several clusters. One recurring block is one per-object layout instantiated
  several times; lines are named by their offset from the block's lowest line.
- `cluster`: a sub-page cluster that does not recur.
- `scattered`: a line with no neighbour within `CLUSTER_GAP`.

Sharing, from the Pareto detail of a line (section 8.20.5 step 4):
- `true sharing`: every offset that took HITM loads was also stored to in the
  sample;
- `false sharing`: no offset that took HITM loads was stored to in the sample,
  while the line was;
- `mixed`: some of each, with the split given;
- `not classifiable`: no complete per-offset detail for the line.
Stores are sampled, so "not stored to in the sample" is a statement about the
sample, and the evidence column gives the split rather than the label alone.

HITM on lines the report did not list is reported as `unexplained`: it is
never assigned to a listed line or group by subtraction (section 8.20.4).

This is observational (section 8.20.3): it names where contention sits, not
what removing it would buy.

Usage:
    python3 scripts/c2c_ranking.py ARTIFACT [ARTIFACT2]  # per-line and per-group ranking
    python3 scripts/c2c_ranking.py --self-test
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RESULTS = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results"

# The canonical x86-64 kernel half starts here; user space is below it.
KERNEL_FLOOR = 0xFFFF_8000_0000_0000
# A mapping head: an address within the first page of a 64 MiB-aligned region.
MAPPING_ALIGN = 0x400_0000
PAGE = 0x1000
LINE = 64
# Two listed lines closer than this belong to one sub-page cluster. It sits
# between a block's widest internal gap and the narrowest gap between two
# instances of a block in the committed runs; the self-test's check that the
# recurring block recurs once per recorded round in both committed artifacts
# fails if it does not.
CLUSTER_GAP = 0x800
# Rounding tolerance when comparing a share with the report's own two-decimal
# `Hitm %` column, in percentage points.
PCT_TOLERANCE = 0.006
# A complete Pareto block's per-offset percentages sum to 100 within the
# rounding of its two-decimal rows; further off than this many percentage
# points, the block was misread.
DETAIL_SUM_TOLERANCE = 0.75

TABLE_HEAD = "Shared Data Cache Line Table"
PARETO_HEAD = "Shared Cache Line Distribution Pareto"
TABLE_COLUMNS = ("Index", "Address", "LclHitm", "RmtHitm", "records", "Stores")
PARETO_COLUMNS = ("RmtHitm", "LclHitm", "L1 Hit", "Offset", "Code address", "Symbol")

TABLE_ROW = re.compile(
    r"^\s*(\d+)\s+(0x[0-9a-f]+)\s+(\d+)\s+(\d+)\s+([\d.]+)%"
    + r"\s+(\d+)" * 18 + r"\s*$"
)
PARETO_BLOCK = re.compile(r"^\s*(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(0x[0-9a-f]+)\s*$")
PARETO_ROW = re.compile(
    r"^\s*([\d.]+)%\s+([\d.]+)%\s+([\d.]+)%\s+([\d.]+)%\s+([\d.]+)%"
    r"\s+(0x[0-9a-f]+)\s+(\d+)\s+(\d+)\s+(0x[0-9a-f]+)"
    r"\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+\[(.)\]\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s*$"
)
PROFILE_ROW = re.compile(r"^\s*([\d.]+)%\s+\[(.)\]\s+(\S+)\s+(\S+)")


class C2CFormatError(ValueError):
    """The artifact's c2c text is not in the shape this tool reads (section 8.1)."""


@dataclass
class OffsetDetail:
    offset: int
    hitm: float = 0.0
    stores: float = 0.0
    symbols: dict[str, float] = field(default_factory=dict)


@dataclass
class Line:
    index: int
    addr: int
    lcl_hitm: int
    rmt_hitm: int
    records: int
    loads: int
    stores: int
    printed_pct: float
    share: float = 0.0
    layout: str = ""
    group: str = ""
    detail: dict[int, OffsetDetail] | None = None
    detail_complete: bool = False
    # Store refs in the Pareto block header (L1 Hit + L1 Miss + N/A). The line
    # table's `Stores` column also counts stores the header does not split, so
    # per-offset store shares are shares of this.
    detail_stores: int = 0

    @property
    def hitm(self) -> int:
        return self.lcl_hitm + self.rmt_hitm


@dataclass
class Group:
    key: str
    layout: str
    lines: list[Line]
    instances: int
    lcl_hitm: int = 0
    rmt_hitm: int = 0
    share: float = 0.0
    sharing: str = "not classifiable"
    evidence: str = "no complete per-offset detail carried"

    @property
    def hitm(self) -> int:
        return self.lcl_hitm + self.rmt_hitm


@dataclass
class Analysis:
    total_records: int
    local_hitm: int
    remote_hitm: int
    lines: list[Line]
    groups: list[Group]
    unlisted_hitm: int
    profile: list[tuple[float, str, str]]
    blocks_carried: int
    blocks_partial: int

    @property
    def total_hitm(self) -> int:
        return self.local_hitm + self.remote_hitm

    @property
    def unlisted_share(self) -> float:
        return self.unlisted_hitm / self.total_hitm


# ---- parsing ------------------------------------------------------------------


def _summary_count(summary: str, label: str) -> int:
    m = re.search(re.escape(label) + r"\s*:\s*(\d+)", summary)
    if not m:
        raise C2CFormatError(f"c2c summary carries no `{label}`")
    return int(m.group(1))


def _section(rows: list[str], head: str) -> int | None:
    for i, row in enumerate(rows):
        if head in row:
            return i
    return None


def parse_table(rows: list[str]) -> list[Line]:
    start = _section(rows, TABLE_HEAD)
    if start is None:
        raise C2CFormatError(f"hot_cache_lines carries no {TABLE_HEAD!r}")
    end = _section(rows, PARETO_HEAD)
    if end is None:
        raise C2CFormatError(
            f"hot_cache_lines ends inside the {TABLE_HEAD!r}: the row cap cut the line table itself, "
            "so lines are missing from it and no share of the remainder can be read"
        )
    header = next((r for r in rows[start:end] if r.lstrip().startswith("# Index")), None)
    if header is None or not all(c in header for c in TABLE_COLUMNS):
        raise C2CFormatError(f"line table header is not the one this tool reads: {header!r}")
    lines = []
    for row in rows[start:end]:
        if not row.strip() or row.lstrip().startswith(("#", "=")) or TABLE_HEAD in row:
            continue
        m = TABLE_ROW.match(row)
        if not m:
            raise C2CFormatError(f"unreadable line-table row: {row!r}")
        g = m.groups()
        # index, address, node, PA cnt, Hitm %, Tot Hitm, LclHitm, RmtHitm,
        # records, loads, stores, ...
        lines.append(Line(
            index=int(g[0]), addr=int(g[1], 16), printed_pct=float(g[4]),
            lcl_hitm=int(g[6]), rmt_hitm=int(g[7]), records=int(g[8]),
            loads=int(g[9]), stores=int(g[10]),
        ))
        if int(g[5]) != int(g[6]) + int(g[7]):
            raise C2CFormatError(f"line {g[0]}: Tot Hitm is not LclHitm + RmtHitm")
    if not lines:
        raise C2CFormatError("the line table has no rows")
    return lines


def parse_pareto(rows: list[str], lines: list[Line]) -> tuple[int, int]:
    """Attaches per-offset detail to the lines it covers. Returns (carried, partial)."""
    start = _section(rows, PARETO_HEAD)
    if start is None:
        return 0, 0
    header = next((r for r in rows[start:] if r.lstrip().startswith("#   Num")), None)
    if header is None or not all(c in header for c in PARETO_COLUMNS):
        raise C2CFormatError(f"Pareto header is not the one this tool reads: {header!r}")
    by_index = {ln.index: ln for ln in lines}
    carried = partial = 0
    current: Line | None = None
    head: tuple[int, ...] | None = None
    i = start
    while i < len(rows):
        row = rows[i]
        mb = PARETO_BLOCK.match(row)
        mr = PARETO_ROW.match(row)
        if mb:
            num, rmt, lcl, l1hit, l1miss, na = (int(x) for x in mb.groups()[:6])
            addr = int(mb.group(7), 16)
            current = by_index.get(num)
            if current is None or current.addr != addr:
                raise C2CFormatError(f"Pareto block {num} at {addr:#x} matches no line-table row")
            if (lcl, rmt) != (current.lcl_hitm, current.rmt_hitm):
                raise C2CFormatError(f"Pareto block {num}: HITM counts differ from the line table")
            head = (rmt, lcl, l1hit, l1miss, na)
            current.detail = {}
            current.detail_stores = l1hit + l1miss + na
            carried += 1
        elif mr and current is not None and head is not None:
            g = mr.groups()
            rmt_p, lcl_p, l1hit_p, l1miss_p, na_p = (float(x) for x in g[:5])
            off = int(g[5], 16)
            rmt, lcl, l1hit, l1miss, na = head
            d = current.detail.setdefault(off, OffsetDetail(off))
            h = (rmt_p * rmt + lcl_p * lcl) / 100.0
            d.hitm += h
            d.stores += (l1hit_p * l1hit + l1miss_p * l1miss + na_p * na) / 100.0
            d.symbols[g[15]] = d.symbols.get(g[15], 0.0) + h
        elif not row.strip() and current is not None:
            # A blank line closes a block: everything the report printed for
            # this line is in hand.
            _check_block_sums(current, rows, i)
            current.detail_complete = True
            current, head = None, None
        i += 1
    if current is not None:
        partial += 1  # the row cap cut this block; it stays unclassified
    return carried, partial


def _check_block_sums(line: Line, rows: list[str], at: int) -> None:
    hitm = sum(d.hitm for d in line.detail.values())
    stores = sum(d.stores for d in line.detail.values())
    for what, got, want in (("HITM", hitm, line.hitm), ("stores", stores, line.detail_stores)):
        if want and abs(100.0 * got / want - 100.0) > DETAIL_SUM_TOLERANCE:
            raise C2CFormatError(
                f"Pareto block for line {line.index}: per-offset {what} sums to {got:.2f}, "
                f"the block header has {want} (block ends at hot_cache_lines row {at})"
            )


def parse_profile(rows: list[str]) -> list[tuple[float, str, str]]:
    out = []
    for row in rows:
        m = PROFILE_ROW.match(row)
        if m:
            out.append((float(m.group(1)), m.group(3), m.group(4)))
    return out


# ---- layout (AGENTS.md section 8.20.5 step 4) ------------------------------------


def base_layout(addr: int) -> str:
    if addr >= KERNEL_FLOOR:
        return "kernel"
    if addr % MAPPING_ALIGN < PAGE:
        return "mapping head"
    return "user"


def clusters(addrs: list[int]) -> list[list[int]]:
    """Sub-page clusters of distinct user-space line addresses."""
    out: list[list[int]] = []
    for a in sorted(set(addrs)):
        if out and a - out[-1][-1] <= CLUSTER_GAP and a - out[-1][0] < PAGE:
            out[-1].append(a)
        else:
            out.append([a])
    return out


def _best_shift(offsets: set[int], template: set[int], span: int) -> tuple[int, int]:
    best = (len(offsets & template), 0)
    for k in range(-span // LINE, span // LINE + 1):
        s = k * LINE
        hit = len({o + s for o in offsets} & template)
        if hit > best[0] or (hit == best[0] and abs(s) < abs(best[1])):
            best = (hit, s)
    return best


def recurring_blocks(multi: list[list[int]]) -> tuple[list[tuple[str, dict[int, int]]], list[list[int]]]:
    """Finds recurring blocks among multi-line clusters.

    Returns ([(block label, {addr: offset from the block's lowest line})], the
    clusters that recur with nothing). A block needs at least two clusters that
    share at least two relative offsets after alignment.
    """
    remaining = [c for c in multi]
    blocks: list[tuple[str, dict[int, int]]] = []
    label = ord("A")
    while len(remaining) >= 2:
        span = max(c[-1] - c[0] for c in remaining)
        rel = [{a - c[0] for a in c} for c in remaining]
        shifts = [0] * len(remaining)
        template: set[int] = set()
        # Two passes: a template from the clusters as they fall, then again
        # from the clusters aligned to it, so one cluster carrying an extra
        # line below the others does not move the frame.
        for _ in range(2):
            counts: dict[int, int] = {}
            for r, sh in zip(rel, shifts):
                for o in r:
                    counts[o + sh] = counts.get(o + sh, 0) + 1
            template = {o for o, n in counts.items() if n >= 2}
            if len(template) < 2:
                break
            shifts = [_best_shift(r, template, span)[1] for r in rel]
        if len(template) < 2:
            break
        members, rest = [], []
        for c, r, sh in zip(remaining, rel, shifts):
            hit = len({o + sh for o in r} & template)
            (members if hit >= 2 else rest).append((c, sh))
        if len(members) < 2:
            break
        # Offsets are from the template's lowest line, so a line one instance
        # carries below it reads as a negative offset instead of renaming the
        # whole block.
        low = min(template)
        blocks.append((chr(label), {a: a - c[0] + sh - low for c, sh in members for a in c}))
        label += 1
        remaining = [c for c, _ in rest]
    return blocks, remaining


def _off(o: int) -> str:
    return f"+{o:#05x}" if o >= 0 else f"-{-o:#05x}"


def assign_groups(lines: list[Line]) -> list[Group]:
    for ln in lines:
        ln.layout = base_layout(ln.addr)
    user = [ln.addr for ln in lines if ln.layout == "user"]
    cl = clusters(user)
    multi = [c for c in cl if len(c) >= 2]
    blocks, lone_clusters = recurring_blocks(multi)
    where: dict[int, tuple[str, str, int | None]] = {}
    for label, offs in blocks:
        for a, o in offs.items():
            where[a] = ("block", f"block {label} {_off(o)}", None)
    for c in lone_clusters:
        for a in c:
            where[a] = ("cluster", "non-recurring sub-page clusters", None)
    for c in cl:
        if len(c) == 1:
            where[c[0]] = ("scattered", "scattered lines", None)
    for ln in lines:
        if ln.layout == "kernel":
            ln.group = "kernel lines"
        elif ln.layout == "mapping head":
            ln.group = f"mapping head +{ln.addr % MAPPING_ALIGN:#05x}"
        else:
            ln.layout, ln.group, _ = where[ln.addr]

    groups: dict[str, Group] = {}
    for ln in lines:
        g = groups.setdefault(ln.group, Group(ln.group, ln.layout, [], 0))
        g.lines.append(ln)
    for g in groups.values():
        g.lcl_hitm = sum(ln.lcl_hitm for ln in g.lines)
        g.rmt_hitm = sum(ln.rmt_hitm for ln in g.lines)
        if g.layout == "block":
            g.instances = len({ln.addr for ln in g.lines})
        elif g.layout == "mapping head":
            g.instances = len({ln.addr - ln.addr % MAPPING_ALIGN for ln in g.lines})
        else:
            g.instances = len({ln.addr & ~(PAGE - 1) for ln in g.lines})
        classify(g)
    return sorted(groups.values(), key=lambda g: (-g.hitm, g.key))


# ---- sharing ---------------------------------------------------------------------


def classify(g: Group) -> None:
    """True / false / mixed sharing from the complete per-offset detail of a group's lines."""
    complete = [ln for ln in g.lines if ln.detail_complete and ln.detail]
    if not complete:
        g.sharing, g.evidence = "not classifiable", "no complete per-offset detail carried"
        return
    agg: dict[int, OffsetDetail] = {}
    for ln in complete:
        for off, d in ln.detail.items():
            a = agg.setdefault(off, OffsetDetail(off))
            a.hitm += d.hitm
            a.stores += d.stores
            for s, h in d.symbols.items():
                a.symbols[s] = a.symbols.get(s, 0.0) + h
    hitm = sum(d.hitm for d in agg.values())
    stores = sum(d.stores for d in agg.values())
    on_stored = sum(d.hitm for d in agg.values() if d.hitm > 0 and d.stores > 0)
    on_unstored = sum(d.hitm for d in agg.values() if d.hitm > 0 and d.stores == 0)
    if hitm == 0:
        g.sharing = "not classifiable"
    elif on_unstored == 0:
        g.sharing = "true sharing"
    elif on_stored == 0 and stores > 0:
        g.sharing = "false sharing"
    else:
        g.sharing = "mixed"
    parts = []
    for off in sorted(agg):
        d = agg[off]
        if d.hitm == 0 and d.stores == 0:
            continue
        parts.append(f"`{off:#04x}` {pct(d.hitm / hitm if hitm else 0)} of HITM, "
                     f"{pct(d.stores / stores if stores else 0)} of stores")
    g.evidence = (f"{'; '.join(parts)} (detail for {len(complete)} of {len(g.lines)} lines; "
                  f"HITM on stored offsets {pct(on_stored / hitm if hitm else 0)})")


# ---- symbols (AGENTS.md section 8.20.5 step 5) -------------------------------------


def v0_path(sym: str) -> str:
    """The identifiers of a Rust v0 mangling, in order, joined by `::`.

    Not a demangler: it skips crate and impl disambiguators (`s..._`), back
    references (`B..._`) and const generic values (`K...`), and reads every
    length-prefixed identifier. Enough to name a function; generic arguments
    and impl paths are not reconstructed.
    """
    if not sym.startswith("_R"):
        return sym
    out, i, n = [], 2, len(sym)
    while i < n:
        c = sym[i]
        if c.isdigit():
            j = i
            while j < n and sym[j].isdigit():
                j += 1
            k = int(sym[i:j])
            if j < n and sym[j] == "_" and k and j + 1 + k <= n and not sym[j + 1].isalpha():
                j += 1  # the separator before an identifier starting with a digit or `_`
            ident = sym[j:j + k]
            if k and len(ident) == k and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", ident):
                out.append(ident)
                i = j + k
            else:
                i = j
        elif c in "sB":
            m = re.match(r"[sB][0-9A-Za-z]*_", sym[i:])
            i += m.end() if m else 1
        elif c == "K":
            i += 1
            if i < n and sym[i] == "B":
                continue
            m = re.match(r"[a-z]n?[0-9a-f]*_", sym[i:])
            i += m.end() if m else 0
        else:
            i += 1
    return "::".join(out) if out else sym


def resolve(truncated: str, profile: list[tuple[float, str, str]]) -> list[str]:
    """Every `symbol_profile` name the truncated report symbol is a prefix of."""
    return sorted({name for _, name, _ in profile if name.startswith(truncated)})


# ---- the analysis --------------------------------------------------------------------


def analyse(c2c: dict) -> Analysis:
    for key in ("summary", "hot_cache_lines", "symbol_profile", "total_records"):
        if key not in c2c:
            raise C2CFormatError(f"c2c block carries no `{key}`")
    summary = c2c["summary"]
    records = _summary_count(summary, "Total records")
    if records != c2c["total_records"]:
        raise C2CFormatError("c2c.total_records differs from the summary's `Total records`")
    local = _summary_count(summary, "Load Local HITM")
    remote = _summary_count(summary, "Load Remote HITM")
    if local + remote == 0:
        raise C2CFormatError("the recording holds no HITM load samples; there is nothing to rank")
    rows = c2c["hot_cache_lines"]
    lines = parse_table(rows)
    carried, partial = parse_pareto(rows, lines)
    total = local + remote
    for ln in lines:
        ln.share = ln.hitm / total
        if abs(100.0 * ln.share - ln.printed_pct) > PCT_TOLERANCE:
            raise C2CFormatError(
                f"line {ln.index}: computed share {100 * ln.share:.3f}% disagrees with the report's "
                f"{ln.printed_pct:.2f}%; the denominator is not the one perf used"
            )
    groups = assign_groups(lines)
    for g in groups:
        g.share = g.hitm / total
    listed = sum(ln.hitm for ln in lines)
    if listed > total:
        raise C2CFormatError("listed lines hold more HITM than the recording")
    return Analysis(records, local, remote, lines, groups, total - listed,
                    parse_profile(c2c["symbol_profile"]), carried, partial)


def ranks(a: Analysis) -> dict[str, int]:
    return {g.key: i + 1 for i, g in enumerate(a.groups)}


# ---- rendering -------------------------------------------------------------------------


def pct(x: float) -> str:
    return f"{100.0 * x:.2f}%"


def _share_cell(g: Group | None) -> str:
    if g is None:
        return "not listed"
    return f"{pct(g.share)} ({g.lcl_hitm} / {g.rmt_hitm})"


def _iv(m: float, lo: float, hi: float, method: str) -> str:
    return f"{pct(m)} [{pct(lo)}, {pct(hi)}] {method}"


def _layout_text(g: Group) -> str:
    return {
        "block": f"line of a recurring sub-page block, listed in {g.instances} of its instances",
        "mapping head": f"first page of a 64 MiB-aligned mapping, {g.instances} mappings",
        "cluster": f"sub-page clusters that do not recur, {g.instances} pages",
        "scattered": f"isolated lines, {g.instances} pages",
        "kernel": f"kernel addresses, {g.instances} pages",
    }[g.layout]


def render(runs: list[tuple[str, str, dict]], section: str = "11.9",
           run_label: str = "CI run") -> list[str]:
    """Markdown for the README section, from (artifact name, run URL, artifact) pairs.

    `run_label` names the second row of the recordings table. It is `CI run`
    for a dispatch and is set by the caller for a recording taken another way,
    so the row never says `CI run` of something no CI run produced.
    """
    if not runs:
        raise ValueError("render needs at least one run")
    analyses = [analyse(art["c2c"]) for _, _, art in runs]
    labels = [f"run {i + 1}" for i in range(len(runs))]
    out: list[str] = []

    # -- run facts
    out += [f"#### {section}.1 The recordings", "",
            "| | " + " | ".join(labels) + " |",
            "|---|" + "---|" * len(runs)]

    def row(name: str, cells: list[str]) -> None:
        out.append(f"| {name} | " + " | ".join(cells) + " |")

    row("artifact", [f"`results/{n}`" for n, _, _ in runs])
    row(run_label, [u for _, u, _ in runs])
    row("commit", [f"`{art['provenance']['commit']}`" for _, _, art in runs])
    row("pin", [f"`{art['provenance']['core_pin']}`" for _, _, art in runs])
    row("arm, W", [f"`{art['c2c']['arm']}`, {art['c2c']['writers']}" for _, _, art in runs])
    row("rounds recorded", [str(art["c2c"]["rounds"]) for _, _, art in runs])
    row("window", [art["c2c"]["window"] for _, _, art in runs])
    row("cell isolation", [art["c2c"]["cell_isolation"] for _, _, art in runs])
    row("total records", [str(a.total_records) for a in analyses])
    row("HITM load samples (local / remote)", [f"{a.local_hitm} / {a.remote_hitm}" for a in analyses])
    row("lines the report listed", [str(len(a.lines)) for a in analyses])
    row("HITM on listed lines", [pct(1 - a.unlisted_share) for a in analyses])
    row("HITM on lines not listed (unexplained)", [pct(a.unlisted_share) for a in analyses])
    row("lines with per-offset detail carried (complete / cut by the row cap)",
        [f"{sum(1 for ln in a.lines if ln.detail_complete)} / {a.blocks_partial}" for a in analyses])
    out.append("")

    # -- group ranking
    keys: list[str] = []
    for a in analyses:
        for g in a.groups:
            if g.key not in keys:
                keys.append(g.key)
    rk = [ranks(a) for a in analyses]
    bygroup = [{g.key: g for g in a.groups} for a in analyses]
    keys.sort(key=lambda k: (rk[0].get(k, 10**6), k))
    out += [f"#### {section}.2 Ranking by line group", "",
            "| line group | layout | " + " | ".join(f"{lb} rank" for lb in labels) + " | "
            + " | ".join(f"{lb} HITM share (local / remote samples)" for lb in labels)
            + " | same rank in every run | sharing | sharing evidence |",
            "|---|---|" + "--:|" * len(runs) + "--:|" * len(runs) + "---|---|---|"]
    for k in keys:
        gs = [b.get(k) for b in bygroup]
        first = next(g for g in gs if g is not None)
        rank_cells = [str(r[k]) if k in r else "—" for r in rk]
        same = "yes" if len({r.get(k) for r in rk}) == 1 else "no"
        shar = [g.sharing for g in gs if g is not None]
        sharing = shar[0] if len(set(shar)) == 1 else " / ".join(shar)
        ev = "; ".join(f"{lb}: {g.evidence}" for lb, g in zip(labels, gs)
                       if g is not None and g.sharing != "not classifiable")
        out.append(f"| `{k}` | {_layout_text(first)} | " + " | ".join(rank_cells) + " | "
                   + " | ".join(_share_cell(g) for g in gs) + f" | {same} | {sharing} | {ev or '—'} |")
    out.append("| [not listed by `perf c2c report`] | unexplained | "
               + " | ".join("—" for _ in runs) + " | "
               + " | ".join(f"{pct(a.unlisted_share)} ({a.unlisted_hitm})" for a in analyses)
               + " | — | — | — |")
    out.append("")

    # -- per-offset detail and symbols
    out += [f"#### {section}.3 Per-offset detail of the lines the artifact carries", "",
            "| run | line (report index) | line group | offset | share of the line's HITM | "
            "share of the line's stores | HITM at the offset by report symbol |",
            "|---|---|---|---|--:|--:|---|"]
    truncated: dict[str, list[str]] = {}
    profile = [row for a in analyses for row in a.profile]
    for lb, a in zip(labels, analyses):
        for ln in a.lines:
            if not (ln.detail_complete and ln.detail):
                continue
            h = sum(d.hitm for d in ln.detail.values())
            s = sum(d.stores for d in ln.detail.values())
            for off in sorted(ln.detail):
                d = ln.detail[off]
                syms = "; ".join(f"`{name}` {pct(v / d.hitm if d.hitm else 0)}"
                                 for name, v in sorted(d.symbols.items(), key=lambda kv: (-kv[1], kv[0])) if v > 0)
                for name in d.symbols:
                    truncated.setdefault(name, resolve(name, profile))
                out.append(f"| {lb} | {ln.index} | `{ln.group}` | `{off:#04x}` | "
                           f"{pct(d.hitm / h if h else 0)} | {pct(d.stores / s if s else 0)} | {syms or '—'} |")
    out.append("")
    out += [f"#### {section}.4 Report symbols resolved against `symbol_profile`", "",
            "| report symbol (truncated by `perf c2c report`) | `symbol_profile` names it prefixes | resolution |",
            "|---|---|---|"]
    for name in sorted(truncated):
        cands = truncated[name]
        if len(cands) == 1:
            res = "unique"
        elif cands:
            res = f"ambiguous, {len(cands)} candidates"
        else:
            res = "unresolved"
        names = "; ".join(f"`{v0_path(c)}`" for c in cands) or "—"
        out.append(f"| `{name}` | {names} | {res} |")
    out.append("")

    # -- frequency droop and the same cell's scaling factor (section 8.20.2, step 3)
    # The arm is the one the recording was taken on, read from the c2c block
    # and never a fixed name: the wrapper recordings are `str`, `bytes` and
    # `blob`, and a hardcoded `map` raised on them rather than printing a
    # number. The runs of one section are the runs of one arm, so a section
    # whose artifacts disagree is refused rather than rendered from the first.
    arms = {art["c2c"]["arm"] for _, _, art in runs}
    if len(arms) != 1:
        raise C2CFormatError(
            f"the runs of one section must be recordings of one arm; got {sorted(arms)}")
    arm = arms.pop()
    out += [f"#### {section}.5 1 − (cycles/ref-cycles at W = 8) ÷ (at W = 1), "
            "beside the same cell's C(8)", "",
            "| | 1 − (cycles/ref-cycles) ratio at W = 8, BCa 95% | harness verdict | rounds "
            f"| `{arm}` C(8), throughput pass | C(8) rounds |",
            "|---|--:|---|--:|--:|--:|"]
    for lb, (_, _, art) in zip(labels, runs):
        d = art["pmu"]["frequency_droop"]["by_writers"]["8"]
        cell = next((c for c in art["throughput"]
                     if c["arm"] == arm and c["writers"] == 8), None)
        if cell is None:
            raise C2CFormatError(
                f"the artifact carries no `{arm}` W = 8 throughput cell to set the droop against")
        c8 = (f"{cell['scaling_factor_c_n_mean']:.2f} [{cell['scaling_factor_c_n_ci_lower']:.2f}, "
              f"{cell['scaling_factor_c_n_ci_upper']:.2f}] {cell['scaling_factor_c_n_ci_method']}")
        out.append(f"| {lb} | {_iv(d['droop_mean'], d['droop_ci_lower'], d['droop_ci_upper'], d['droop_ci_method'])} "
                   f"| `{d['verdict']}` | {d['n_measured']} | {c8} | {len(cell['rounds_raw'])} |")
    out += ["", "The harness names this field `pmu.frequency_droop`, and it is a frequency only "
            "where the cells compared retire work the same way; where writers serialise on one "
            "mutex they need not, so what a large reading mixes is not established here (see this "
            "section's closing discussion)."]
    out.append("")

    # -- load snapshots (section 8.17)
    out += [f"#### {section}.6 Host load", "",
            "| | cell `foreign_busy_cpus`, min – max | peak `load1` over the snapshots | snapshot labels | "
            "a snapshot taken during or after the PMU and c2c passes |",
            "|---|--:|--:|---|---|"]
    for lb, (_, _, art) in zip(labels, runs):
        fb = [c["load"]["foreign_busy_cpus"] for c in art["throughput"]]
        loads = art["provenance"]["loads"]
        after = [s["label"] for s in loads if re.search(r"pmu|c2c", s["label"], re.IGNORECASE)]
        out.append(f"| {lb} | {min(fb):.2f} – {max(fb):.2f} | {max(s['load1'] for s in loads):.2f} | "
                   + ", ".join(f"`{s['label']}`" for s in loads) + f" | {', '.join(after) or 'none'} |")
    return out


def render_lines(a: Analysis) -> list[str]:
    out = [f"total HITM {a.total_hitm} (local {a.local_hitm}, remote {a.remote_hitm}); "
           f"unlisted {a.unlisted_hitm} ({pct(a.unlisted_share)}, unexplained)",
           "index  address              share   lcl  rmt  stores  group"]
    for ln in sorted(a.lines, key=lambda x: (-x.hitm, x.index)):
        out.append(f"{ln.index:5d}  {ln.addr:#018x}  {pct(ln.share):>7}  {ln.lcl_hitm:4d} {ln.rmt_hitm:4d} "
                   f"{ln.stores:6d}  {ln.group}")
    return out


# ---- self-test -------------------------------------------------------------------------

_HDR = ("#        ----------- Cacheline ----------      Tot  ------- Load Hitm -------    Total    Total    Total  "
        "--------- Stores --------  ----- Core Load Hit -----  - LLC Load Hit --  - RMT Load Hit --  --- Load Dram ----")
_COLS = ("# Index             Address  Node  PA cnt     Hitm    Total  LclHitm  RmtHitm  records    Loads   Stores    "
         "L1Hit   L1Miss      N/A       FB       L1       L2    LclHit  LclHitm    RmtHit  RmtHitm       Lcl       Rmt")
_PHDR = ("#   Num  RmtHitm  LclHitm   L1 Hit  L1 Miss      N/A              Offset  Node  PA cnt        Code address  "
         "rmt hitm  lcl hitm      load  records       cnt                          Symbol          Object                     Source:Line  Node")
_SYM_A = "_RNvMsk_NtCsfHnohCjqgYz_12expanse_trie4syncNtB5_14SyncExpanseMap14olc_insert_map"
_SYM_B = "_RINvNtCsfHnohCjqgYz_12expanse_trie10mutate_map24map_insert_with_path_occKb1_KB19_Kb0_EB4_"
_SYM_C = "_RINvNtCsfHnohCjqgYz_12expanse_trie6mutate15upgrade_l7_to_bKb1_EB4_"


def _trow(idx: int, addr: int, pct_s: str, lcl: int, rmt: int, loads: int, stores: int) -> str:
    return (f"  {idx:5d}  {addr:#18x}     0     10  {pct_s:>7}  {lcl + rmt:7d}  {lcl:7d}  {rmt:7d}  "
            f"{loads + stores:7d}  {loads:7d}  {stores:7d}  {stores:7d}        0        0       10       10        0"
            f"         0  {lcl:7d}         0  {rmt:7d}         0         0")


def _prow(rmt_p: str, lcl_p: str, st_p: str, off: int, sym: str) -> str:
    return (f"           {rmt_p:>6}%  {lcl_p:>6}%  {st_p:>6}%    0.00%    0.00%  {off:#18x}     0       1  "
            f"    0x5caca826481d         0       434       363      120         8  [.] {sym}  writer_scaling  {sym}   0")


def _fixture() -> dict:
    """A report in the committed format: two instances of a three-line block,
    a mapping head pair, a scattered line, a kernel line; total HITM 1000
    (local 990, remote 10), of which the table lists 950."""
    b1, b2 = 0x5CACAD2B1140, 0x5CACADAF6100
    rows = [
        "           Shared Data Cache Line Table          ",
        "=================================================",
        "#", _HDR, _COLS, "# .....", "#",
        # index, address, printed %, local, remote, loads, stores
        _trow(0, b1, "20.00%", 195, 5, 300, 100),
        _trow(1, b2, "18.00%", 180, 0, 280, 80),
        _trow(2, b1 + 0x500, "15.00%", 150, 0, 200, 40),
        _trow(3, b2 + 0x500, "14.00%", 140, 0, 190, 30),
        _trow(4, b1 + 0x40, "8.00%", 80, 0, 90, 20),
        _trow(5, b2 + 0x40, "7.00%", 70, 0, 85, 10),
        _trow(6, 0x7DA7D0000000, "4.00%", 40, 0, 50, 5),
        _trow(7, 0x7DA7D8000080, "3.00%", 30, 0, 40, 5),
        _trow(8, 0x5CACAD2A3F40, "5.00%", 45, 5, 60, 1),
        _trow(9, 0xFFFF8909C258B440, "1.00%", 10, 0, 20, 2),
        "",
        "=================================================",
        "      Shared Cache Line Distribution Pareto      ",
        "=================================================",
        "#", _PHDR, "# .....", "#",
        "  ----------------------------------------------------------------------",
        f"      0        5      195      100        0        0      {b1:#x}",
        "  ----------------------------------------------------------------------",
        # line 0: HITM on 0x28 (60 + 20 = 80%) and 0x30 (20%), both stored: true sharing
        _prow("100.00", "60.00", "30.00", 0x28, _SYM_B[:26]),
        _prow("0.00", "20.00", "10.00", 0x28, _SYM_A[:26]),
        _prow("0.00", "20.00", "60.00", 0x30, _SYM_B[:26]),
        "",
        "  ----------------------------------------------------------------------",
        f"      2        0      150       40        0        0      {b1 + 0x500:#x}",
        "  ----------------------------------------------------------------------",
        # line 2: HITM only on 0x10, stores only on 0x18: false sharing
        _prow("0.00", "100.00", "0.00", 0x10, _SYM_A[:26]),
        _prow("0.00", "0.00", "100.00", 0x18, _SYM_A[:26]),
        "",
        "  ----------------------------------------------------------------------",
        f"      1        0      180       80        0        0      {b2:#x}",
        "  ----------------------------------------------------------------------",
        # cut by the row cap: no terminating blank line
        _prow("0.00", "90.00", "50.00", 0x28, _SYM_B[:26]),
    ]
    summary = ("  Total records                     :      50000\n"
               "  Load Local HITM                   :        990\n"
               "  Load Remote HITM                  :         10\n")
    profile = [
        f"    29.95%  [.] {_SYM_B}   writer_scaling    -      -",
        f"    17.64%  [.] {_SYM_A}   writer_scaling    -      -",
        f"     1.09%  [.] {_SYM_C}   writer_scaling    -      -",
        "     7.93%  [.] 0x00000000001a094d   libc.so.6         -      -",
    ]
    return {"arm": "map", "writers": 8, "rounds": 2, "total_records": 50000,
            "window": "w", "cell_isolation": "c", "summary": summary,
            "hot_cache_lines": rows, "symbol_profile": profile}


def _self_test() -> int:
    failures: list[str] = []

    def expect(cond: bool, msg: str) -> None:
        if not cond:
            failures.append(msg)

    def raises(fn, needle: str, msg: str) -> None:
        try:
            fn()
        except C2CFormatError as exc:
            expect(needle in str(exc), f"{msg}: wrong refusal: {exc}")
        else:
            failures.append(f"{msg}: accepted")

    # 1. Share arithmetic, read through `analyse` (the call site tables.py uses).
    a = analyse(_fixture())
    expect(a.total_hitm == 1000 and a.local_hitm == 990 and a.remote_hitm == 10, "summary totals")
    expect(a.lines[0].share == 0.2, f"line 0 share {a.lines[0].share} != 0.2 (200 of 1000)")
    expect(a.unlisted_hitm == 50 and abs(a.unlisted_share - 0.05) < 1e-12,
           f"unlisted HITM {a.unlisted_hitm} != 50 (1000 - 950 listed)")
    g = {x.key: x for x in a.groups}
    expect(abs(sum(x.share for x in a.groups) + a.unlisted_share - 1.0) < 1e-12,
           "group shares plus the unlisted residual must be the whole recording")
    # The residual is never inside a group.
    expect(all("not listed" not in x.key for x in a.groups), "the unlisted residual was assigned to a group")

    # A share that disagrees with the report's own column is refused: a
    # denominator of local HITM alone would print 20.20% for line 0.
    bad = _fixture()
    bad["summary"] = bad["summary"].replace("Load Remote HITM                  :         10",
                                            "Load Remote HITM                  :          0")
    raises(lambda: analyse(bad), "denominator", "a denominator without remote HITM")
    bad = _fixture()
    bad["hot_cache_lines"][7] = bad["hot_cache_lines"][7].replace("20.00%", "21.00%")
    raises(lambda: analyse(bad), "disagrees", "a printed share the arithmetic does not reproduce")

    # 2. Layout.
    expect(set(g) == {"block A +0x000", "block A +0x040", "block A +0x500", "mapping head +0x000",
                      "mapping head +0x080", "scattered lines", "kernel lines"},
           f"groups: {sorted(g)}")
    expect(g["block A +0x000"].instances == 2 and g["block A +0x000"].hitm == 380,
           f"block +0x000: {g['block A +0x000'].instances} instances, {g['block A +0x000'].hitm} HITM")
    expect(abs(g["block A +0x000"].share - 0.38) < 1e-12, "block +0x000 share 380/1000")
    expect(a.groups[0].key == "block A +0x000", f"rank 1 is {a.groups[0].key}")
    expect(g["kernel lines"].layout == "kernel", "kernel layout")
    expect(g["scattered lines"].hitm == 50, "scattered HITM")
    # Two instances closer than a page but further than CLUSTER_GAP stay apart.
    # (the two nearest instances in the second committed run, offsets +0x000,
    # +0x040, +0x500, +0x540, gap 0xd00 between them)
    cl = clusters([0x1000_03C0, 0x1000_0400, 0x1000_08C0, 0x1000_0900, 0x1000_1600, 0x1000_1640,
                   0x1000_1B00, 0x1000_1B40])
    expect(len(cl) == 2, f"cluster split at a gap wider than CLUSTER_GAP: {[[hex(x) for x in c] for c in cl]}")
    blocks, rest = recurring_blocks(cl)
    expect(len(blocks) == 1 and not rest, "two clusters with a shared offset set are one block")
    expect(sorted(set(blocks[0][1].values())) == [0, 0x40, 0x500, 0x540],
           f"block offsets {sorted(set(blocks[0][1].values()))}")
    # A cluster whose lowest line is missing is aligned, not mis-named.
    blocks, _ = recurring_blocks([[0x2000_0000, 0x2000_0040, 0x2000_0500],
                                  [0x3000_0000, 0x3000_0040, 0x3000_0500],
                                  [0x4000_0040, 0x4000_0500]])
    expect(blocks and blocks[0][1][0x4000_0040] == 0x40, "a shifted instance aligns to the template")
    # An extra line below one instance is a negative offset, not a new frame.
    blocks, _ = recurring_blocks([[0x2000_0000, 0x2000_0040, 0x2000_0500],
                                  [0x3000_0000, 0x3000_0040, 0x3000_0500],
                                  [0x4000_0000, 0x4000_0200, 0x4000_0240, 0x4000_0700]])
    expect(blocks and blocks[0][1][0x2000_0000] == 0 and blocks[0][1][0x4000_0000] == -0x200,
           f"an extra low line renamed the block: {blocks and {hex(k): v for k, v in blocks[0][1].items()}}")

    # 3. Sharing, read through the groups `analyse` built.
    expect(g["block A +0x000"].sharing == "true sharing", f"line 0: {g['block A +0x000'].sharing}")
    expect("`0x28` 80.50% of HITM, 40.00% of stores" in g["block A +0x000"].evidence,
           f"line 0 evidence: {g['block A +0x000'].evidence}")
    expect("detail for 1 of 2 lines" in g["block A +0x000"].evidence, "the cut block is not counted as detail")
    expect(g["block A +0x500"].sharing == "false sharing", f"line 2: {g['block A +0x500'].sharing}")
    expect(g["block A +0x040"].sharing == "not classifiable", "a line with no detail is not classified")
    expect(a.blocks_carried == 3 and a.blocks_partial == 1, f"blocks {a.blocks_carried}/{a.blocks_partial}")
    line1 = next(x for x in a.lines if x.index == 1)
    expect(line1.detail is not None and not line1.detail_complete, "the cut block is partial")
    mixed = Group("m", "block", [Line(0, 0, 10, 0, 0, 0, 10, 0.0)], 1)
    mixed.lines[0].detail = {0: OffsetDetail(0, hitm=6, stores=10), 8: OffsetDetail(8, hitm=4, stores=0)}
    mixed.lines[0].detail_complete = True
    classify(mixed)
    expect(mixed.sharing == "mixed" and "HITM on stored offsets 60.00%" in mixed.evidence,
           f"mixed: {mixed.sharing} / {mixed.evidence}")
    bad = _fixture()
    bad["hot_cache_lines"][28] = bad["hot_cache_lines"][28].replace("60.00%", "90.00%", 1)
    raises(lambda: analyse(bad), "per-offset HITM sums", "a Pareto block whose offsets do not sum to the line")

    # 4. Symbols.
    expect(v0_path(_SYM_A) == "expanse_trie::sync::SyncExpanseMap::olc_insert_map", v0_path(_SYM_A))
    expect(v0_path(_SYM_B) == "expanse_trie::mutate_map::map_insert_with_path_occ", v0_path(_SYM_B))
    expect(v0_path(_SYM_C) == "expanse_trie::mutate::upgrade_l7_to_b", v0_path(_SYM_C))
    expect(resolve(_SYM_A[:26], a.profile) == [_SYM_A], "a unique prefix resolves")
    expect(len(resolve(_SYM_B[:26], a.profile)) == 2, "a shared prefix is ambiguous, never guessed")
    expect(resolve("_RNvXnothing", a.profile) == [], "an absent prefix is unresolved")

    # 5. The rendered section says what the arithmetic said.
    art = {"c2c": _fixture(), "provenance": {"commit": "abc", "core_pin": "0,2", "loads": [
        {"label": "start", "load1": 1.5}, {"label": "arm:map:writers", "load1": 2.5}]},
        "pmu": {"frequency_droop": {"by_writers": {"8": {
            "droop_mean": 0.0766, "droop_ci_lower": 0.0759, "droop_ci_upper": 0.0772,
            "droop_ci_method": "bca", "verdict": "SINGLE_RUN_PASS", "n_measured": 8}}}},
        "throughput": [{"arm": "map", "writers": 8, "scaling_factor_c_n_mean": 2.23,
                        "scaling_factor_c_n_ci_lower": 2.21, "scaling_factor_c_n_ci_upper": 2.25,
                        "scaling_factor_c_n_ci_method": "bca", "rounds_raw": [1] * 8,
                        "load": {"foreign_busy_cpus": 0.01}}]}
    md = render([("x.json", "https://example.invalid/run/1", art),
                 ("y.json", "https://example.invalid/run/2", art)])
    text = "\n".join(md)
    expect("| `block A +0x000` | line of a recurring sub-page block, listed in 2 of its instances | 1 | 1 | 38.00% (375 / 5) | "
           "38.00% (375 / 5) | yes | true sharing |" in text, "ranking row for block +0x000")
    expect("| [not listed by `perf c2c report`] | unexplained | — | — | 5.00% (50) | 5.00% (50) |" in text,
           "the unexplained row")
    expect("| run 1 | 0 | `block A +0x000` | `0x28` | 80.50% | 40.00% |" in text, "per-offset row")
    expect("7.66% [7.59%, 7.72%] bca" in text, "droop interval")
    expect("| run 1 | 0.01 – 0.01 | 2.50 |" in text and "| none |" in text, "load row")
    expect("ambiguous, 2 candidates" in text and "| unique |" in text, "symbol resolution rows")

    # THE DEFECT THIS PINS: the droop table looked up a `map` W = 8 throughput
    # cell by a fixed name. On the `str`, `bytes` and `blob` wrapper recordings
    # there is no such cell, so `render` raised `StopIteration` — and a lookup
    # that found some other arm's cell would have printed a wrong number
    # instead. The assertion is on the rendered text, not on a helper: revert
    # the arm to a literal and this goes red.
    wrapper = json.loads(json.dumps(art))
    wrapper["c2c"]["arm"] = "blob"
    wrapper["throughput"][0]["arm"] = "blob"
    wtext = "\n".join(render([("w.json", "d", wrapper)], section="16.3", run_label="recording"))
    expect("| `blob` C(8), throughput pass |" in wtext, "the droop header names the recorded arm")
    expect("#### 16.3.5 1 − (cycles/ref-cycles at W = 8)" in wtext,
           "the section number reaches the ratio heading")
    # The heading and column name the quantity computed, not a frequency reading:
    # `1 - (cycles/ref-cycles at W) / (at W = 1)` reads ~81% on the str and bytes
    # wrapper arms, which as a bare "frequency droop" asserts a core clock that
    # did not happen. Revert either label and this goes red.
    expect("Frequency droop" not in wtext, "the heading still asserts a frequency reading")
    expect("| 1 − (cycles/ref-cycles) ratio at W = 8, BCa 95% | harness verdict |" in wtext,
           "the column names the computed ratio")
    expect("it is a frequency only where the cells compared retire work the same way" in wtext,
           "the table carries its qualifying sentence")
    expect("| recording | d |" in wtext, "run_label names the row a CI dispatch did not produce")
    expect("2.23 [2.21, 2.25] bca" in wtext, "the recorded arm's own C(8) cell is the one read")
    # An artifact with no throughput cell for the arm it recorded is refused,
    # never rendered from a neighbouring arm's cell.
    mismatched = json.loads(json.dumps(art))
    mismatched["c2c"]["arm"] = "bytes"
    raises(lambda: render([("m.json", "d", mismatched)]),
           "no `bytes` W = 8 throughput cell",
           "a recording whose arm has no throughput cell")
    # Two recordings of different arms are not one section.
    other = json.loads(json.dumps(art))
    other["c2c"]["arm"] = "blob"
    other["throughput"][0]["arm"] = "blob"
    raises(lambda: render([("a.json", "d", wrapper), ("b.json", "d", json.loads(json.dumps(art)))]),
           "recordings of one arm",
           "two runs of different arms rendered as one section")

    # 6. The committed artifacts are usable for the question (section 8.20.7):
    # the line table is whole, the top line's Pareto block is complete, and the
    # recurring block recurs once per recorded round.
    for name in ("diagnostic_writer_scaling_ac8f1c6d.json", "diagnostic_writer_scaling_ac8f1c6d_run2.json"):
        path = RESULTS / name
        if not path.is_file():
            failures.append(f"committed artifact missing: {path.relative_to(REPO_ROOT)}")
            continue
        art = json.loads(path.read_text())
        ra = analyse(art["c2c"])
        top = max(ra.lines, key=lambda x: (x.hitm, -x.index))
        expect(top.detail_complete, f"{name}: the top line's per-offset detail is not complete")
        blocks = {x.instances for x in ra.groups if x.layout == "block"}
        expect(art["c2c"]["rounds"] in blocks,
               f"{name}: no block recurs once per recorded round ({art['c2c']['rounds']}); instances {blocks}")
        expect(ra.groups[0].sharing != "not classifiable", f"{name}: the top group is not classified")
        expect(math.isclose(sum(x.share for x in ra.groups) + ra.unlisted_share, 1.0),
               f"{name}: shares do not cover the recording")
        expect(len(render([(name, "u", art)])) > 20, f"{name}: renders nothing")

    if failures:
        for f in failures:
            print(f"FAIL: {f}", file=sys.stderr)
        return 1
    print("c2c_ranking self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("artifacts", nargs="*", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return _self_test()
    if not args.artifacts:
        ap.error("name at least one diagnostic artifact, or pass --self-test")
    runs = []
    for p in args.artifacts:
        art = json.loads(p.read_text())
        if "c2c" not in art:
            print(f"{p}: no c2c block (AGENTS.md section 8.1)", file=sys.stderr)
            return 1
        print(f"== {p.name}")
        print("\n".join(render_lines(analyse(art["c2c"]))))
        runs.append((p.name, "(run URL not recorded in the artifact)", art))
    print()
    print("\n".join(render(runs)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
