#!/usr/bin/env python3
"""Single-threaded Callgrind rankings of the coarse-mutex wrappers' mutations (#929).

#929 step 2 ranks costs before any multi-writer design for `SyncExpanseStrMap`,
`SyncExpanseBytesMap` and `SyncExpanseBlobMap`. This reads the committed
`callgrind_annotate` output of their `sync_*_{insert,remove,churn}` arms in
`crates/expanse/benches/instructions.rs`, and of the plain-map arms beside them,
from `docs/benchmarks/concurrency/results/callgrind_wrapper_mutations/`, and
derives every figure the README quotes from those files (AGENTS.md section 8.2).

What is counted. `Ir` is instructions retired inside the benchmark function,
the region iai-callgrind collects; setup is outside it. Callgrind attributes an
instruction to the symbol whose machine code holds it, so a callee the compiler
inlined is counted under its caller. Nothing here is a time, a cycle or a cache
cost, and nothing here says why a symbol costs what it does (section 8.9).

Reconciliation. Every arm is read three ways and they must agree exactly or the
tool refuses the arm (section 8.1):
  - `Collected :` in callgrind's own log, the arm's Ir as iai-callgrind reports it;
  - the `PROGRAM TOTALS` row of the exclusive annotate listing;
  - the sum of the exclusive listing's per-function rows (listed at threshold
    100, so no function is dropped).
Every category sum and every share below is over those rows, so the categories
of an arm sum to its Ir.

Categories are assigned **by symbol name, a code-review assignment** (AGENTS.md
section 2.3, "partition sum identity signposting"). A symbol is read as its
module path plus, for an impl method, its method name, with generic arguments
dropped, and the first rule in `RULES` whose prefix matches wins. The sum
identity proves no Ir is dropped or double-counted; it cannot prove a symbol is
in the right category, and inlining puts a callee's instructions under its
caller's category.

Wrapper addition. For an arm whose plain counterpart runs the same keys, key
order, probe order, values and operation sequence (`PAIRS`, checked against the
bench source by `like_for_like`), the addition is the `sync_*` arm's Ir minus
the plain arm's, per category and per symbol. Where no such counterpart exists
the addition is not computed, and the table says why.

Usage:
    python3 scripts/callgrind_wrapper_ranking.py            # the README block
    python3 scripts/callgrind_wrapper_ranking.py --self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
ARTIFACTS = REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / "callgrind_wrapper_mutations"
BENCH_SRC = REPO_ROOT / "crates" / "expanse" / "benches" / "instructions.rs"
ISSUE = "[#929](https://github.com/orieg/expanse/issues/929)"

# (wrapper, operation, sync arm, plain counterpart or None, bench id)
ARMS: list[tuple[str, str, str, str | None, str]] = [
    ("SyncExpanseStrMap", "insert", "sync_strmap_insert", "strmap_insert", "routes"),
    ("SyncExpanseStrMap", "remove", "sync_strmap_remove", None, "routes"),
    ("SyncExpanseStrMap", "churn", "sync_strmap_churn", "strmap_churn", "routes"),
    ("SyncExpanseBytesMap", "insert", "sync_bytesmap_insert", "bytesmap_insert", "routes"),
    ("SyncExpanseBytesMap", "remove", "sync_bytesmap_remove", None, "routes"),
    ("SyncExpanseBytesMap", "churn", "sync_bytesmap_churn", "bytesmap_churn", "routes"),
    ("SyncExpanseBlobMap", "insert", "sync_blobmap_insert", None, "random"),
    ("SyncExpanseBlobMap", "remove", "sync_blobmap_remove", None, "random"),
    ("SyncExpanseBlobMap", "churn", "sync_blobmap_churn", None, "random"),
]
PAIRS = [(s, p) for _, _, s, p, _ in ARMS if p]
PLAIN_IDS = {p: bid for _, _, _, p, bid in ARMS if p}

# Categories, first match wins, over the qualified path `qualify` returns.
WRITER = "`Shared::write`, writer mutex (`sync` symbols)"
BRACKET = "version bracket"
EPOCH = "epoch pin / retire / collector"
ALLOC = "allocation"
ENGINE = "engine mutation (other `expanse_trie` symbols)"
OTHER = "other"
CATEGORIES = (WRITER, BRACKET, EPOCH, ALLOC, ENGINE, OTHER)

LIBC_ALLOC = {
    "malloc", "free", "calloc", "realloc", "cfree", "memalign", "aligned_alloc",
    "posix_memalign", "_int_malloc", "_int_free", "_int_free_chunk",
    "_int_free_merge_chunk", "_int_free_create_chunk", "_int_free_maybe_consolidate",
    "_int_realloc", "_int_memalign", "_mid_memalign", "malloc_consolidate",
    "unlink_chunk.isra.0", "unlink_chunk", "tcache_init", "tcache_init.part.0",
    "sysmalloc", "__default_morecore", "sbrk", "__brk", "__libc_malloc", "__libc_free",
    "__libc_calloc", "__libc_realloc", "alloc_perturb", "_mid_memalign.isra.0", "systrim.constprop.0",
    "__glibc_morecore", "munmap_chunk", "sysmalloc_mmap_fallback.constprop.0", "sysmalloc_mmap.isra.0",
    # The system calls glibc's allocator makes; nothing else in these arms maps memory.
    "mmap", "munmap", "brk", "__set_vma_name",
}

RULES: list[tuple[str, str]] = [
    (WRITER, "expanse_trie::sync::"),
    (WRITER, "std::sys::sync::mutex::"),
    (WRITER, "std::sync::poison::"),
    (WRITER, "std::sync::mutex::"),
    (BRACKET, "expanse_trie::occ::SeqVersion"),
    (BRACKET, "expanse_trie::occ::version_"),
    (BRACKET, "expanse_trie::occ::tree_begin_if"),
    (BRACKET, "expanse_trie::occ::tree_end_if"),
    (BRACKET, "expanse_trie::occ::Cover"),
    (BRACKET, "expanse_trie::occ::NodeLock"),
    (BRACKET, "expanse_trie::occ::LockSet"),
    (BRACKET, "expanse_trie::occ::node_sample"),
    (BRACKET, "expanse_trie::occ::node_validate"),
    (BRACKET, "expanse_trie::alloc::NodeAlloc::bracket_"),
    (BRACKET, "expanse_trie::alloc::bracket_stack::"),
    (EPOCH, "expanse_trie::occ::"),
    (ALLOC, "expanse_trie::alloc::"),
    (ALLOC, "alloc::alloc::"),
    (ALLOC, "alloc::raw_vec::"),
    (ALLOC, "__rust_alloc"),
    (ALLOC, "__rust_dealloc"),
    (ALLOC, "__rust_realloc"),
    (ALLOC, "__rustc::__rust_"),
    (ALLOC, "__rdl_"),
    (ENGINE, "expanse_trie::"),
]


class ProfileError(ValueError):
    """An artifact is not in the shape this tool reads, or does not reconcile."""


@dataclass
class Row:
    ir: int
    file: str
    fn: str
    obj: str

    @property
    def base(self) -> str:
        """The symbol with callgrind's recursion-copy suffix (`'2`) dropped."""
        return re.sub(r"'\d+$", "", self.fn)


@dataclass
class Profile:
    arm: str
    events: list[str]
    totals_ir: int
    rows: list[Row] = field(default_factory=list)
    log_ir: int | None = None

    def by_symbol(self) -> dict[str, int]:
        out: dict[str, int] = {}
        for r in self.rows:
            out[r.base] = out.get(r.base, 0) + r.ir
        return out

    def by_category(self) -> dict[str, int]:
        """Ir per category. A label outside `CATEGORIES` is kept, not dropped,
        so `reconcile` can refuse it rather than lose its Ir."""
        out = dict.fromkeys(CATEGORIES, 0)
        for sym, ir in self.by_symbol().items():
            cat = category(sym)
            out[cat] = out.get(cat, 0) + ir
        return out


# --------------------------------------------------------------------------
# parsing
# --------------------------------------------------------------------------

_COL = re.compile(r"\s*(?:(\.)|([\d,]+)(?:\s+\(\s*[\d.]+%\))?)(?=\s)")
_NAME = re.compile(r"^(?P<file>[^:]*):(?P<fn>.*?)(?: \[(?P<obj>[^\]]*)\])?$")


def _int(s: str) -> int:
    return int(s.replace(",", ""))


def _columns(line: str, n: int) -> tuple[list[int], str]:
    vals, pos = [], 0
    for _ in range(n):
        m = _COL.match(line, pos)
        if not m:
            raise ProfileError(f"cannot read {n} event columns from: {line!r}")
        vals.append(0 if m.group(1) else _int(m.group(2)))
        pos = m.end()
    return vals, line[pos:].strip()


def parse_annotate(text: str, arm: str) -> Profile:
    """Reads a function-level `callgrind_annotate --inclusive=no` listing."""
    lines = text.splitlines()
    events = None
    threshold = None
    for l in lines:
        if l.startswith("Events shown:"):
            events = l.split(":", 1)[1].split()
        elif l.startswith("Thresholds:"):
            threshold = l.split(":", 1)[1].strip()
    if not events or events[0] != "Ir":
        raise ProfileError(f"{arm}: no `Events shown:` header starting with Ir")
    if threshold != "100":
        raise ProfileError(f"{arm}: listed at threshold {threshold!r}; rows reconcile only at 100")
    totals = [l for l in lines if l.rstrip().endswith("PROGRAM TOTALS")]
    if len(totals) != 1:
        raise ProfileError(f"{arm}: expected one PROGRAM TOTALS row, found {len(totals)}")
    vals, _ = _columns(totals[0], len(events))
    prof = Profile(arm, events, vals[0])
    try:
        head = next(i for i, l in enumerate(lines) if l.rstrip().endswith("file:function"))
    except StopIteration as exc:
        raise ProfileError(f"{arm}: no `file:function` header") from exc
    i = head + 2  # skip the dashed rule under the header
    while i < len(lines) and lines[i].strip():
        vals, name = _columns(lines[i], len(events))
        m = _NAME.match(name)
        if not m:
            raise ProfileError(f"{arm}: cannot read a symbol from {name!r}")
        prof.rows.append(Row(vals[0], m.group("file"), m.group("fn"), m.group("obj") or ""))
        i += 1
    if not prof.rows:
        raise ProfileError(f"{arm}: no function rows")
    return prof


def parse_log_ir(text: str, arm: str) -> int:
    """The Ir callgrind reports in its own log (`Collected :`, first event)."""
    ms = re.findall(r"^==\d+== Events\s*:\s*(.+)$", text, re.M)
    cs = re.findall(r"^==\d+== Collected\s*:\s*(.+)$", text, re.M)
    if len(ms) != 1 or len(cs) != 1:
        raise ProfileError(f"{arm}: log carries {len(ms)} Events and {len(cs)} Collected lines, want one each")
    names, counts = ms[0].split(), cs[0].split()
    # Callgrind omits trailing zero events from `Collected` (the committed
    # `strmap_churn` log has 7 counts for 9 events: DLmr and DLmw are 0), so
    # fewer counts than names is valid; more, or none, is not.
    if not names or names[0] != "Ir" or not 1 <= len(counts) <= len(names):
        raise ProfileError(f"{arm}: log Events/Collected lines do not pair up")
    return int(counts[0])


def reconcile(prof: Profile) -> None:
    """Refuses an arm whose three Ir readings disagree (section 8.1)."""
    row_sum = sum(r.ir for r in prof.rows)
    if row_sum != prof.totals_ir:
        raise ProfileError(f"{prof.arm}: rows sum to {row_sum:,} Ir, PROGRAM TOTALS says {prof.totals_ir:,}")
    if prof.log_ir is None:
        raise ProfileError(f"{prof.arm}: no log Ir to reconcile against")
    if prof.log_ir != prof.totals_ir:
        raise ProfileError(f"{prof.arm}: log says {prof.log_ir:,} Ir, annotate says {prof.totals_ir:,}")
    # `by_category` puts each symbol's Ir under exactly one label, so once every
    # label is a rendered category the categories sum to the rows, which the
    # check above tied to the arm's Ir. A label outside `CATEGORIES` is the one
    # way Ir could leave the category tables.
    unknown = sorted(set(prof.by_category()) - set(CATEGORIES))
    if unknown:
        raise ProfileError(f"{prof.arm}: symbols assigned to categories no table renders: {unknown}")


# --------------------------------------------------------------------------
# categorisation
# --------------------------------------------------------------------------

def qualify(sym: str) -> str:
    """The name `RULES` prefixes are matched against.

    An impl item becomes its type path plus its method, generics and trait
    dropped: `<expanse_trie::occ::Collector>::retire` ->
    `expanse_trie::occ::Collector::retire`, and
    `<expanse_trie::alloc::NodeAlloc as core::ops::drop::Drop>::drop` ->
    `expanse_trie::alloc::NodeAlloc::drop`. Any other symbol is returned as is:
    a prefix match never reaches its generic arguments
    (`expanse_trie::strmap::dispose::<expanse_trie::occ::Collector>` matches
    `expanse_trie::` and not `expanse_trie::occ::`).
    """
    if sym.startswith("<"):
        depth = 0
        for i, ch in enumerate(sym):
            if ch == "<":
                depth += 1
            elif ch == ">":
                depth -= 1
                if depth == 0:
                    break
        else:
            return sym
        inner, rest = sym[1:i], sym[i + 1:]
        ty = re.match(r"[A-Za-z0-9_:]*", inner).group(0).rstrip(":")
        meth = re.match(r"::([A-Za-z0-9_]+)", rest)
        return f"{ty}::{meth.group(1)}" if meth else ty
    return sym


def category(sym: str) -> str:
    if sym in LIBC_ALLOC:
        return ALLOC
    q = qualify(sym)
    for cat, prefix in RULES:
        if q.startswith(prefix):
            return cat
    return OTHER


# --------------------------------------------------------------------------
# like-for-like check against the bench source (section 8.3)
# --------------------------------------------------------------------------

def _fn_body(src: str, name: str) -> str:
    m = re.search(rf"\nfn {re.escape(name)}\(", src)
    if not m:
        raise ProfileError(f"instructions.rs: no fn {name}")
    end = src.find("\n}\n", m.end())
    return src[m.start():end + 3]


def _bench_attr(src: str, name: str) -> tuple[str, str, str]:
    m = re.search(rf"#\[bench::(\w+)\(args = \(([^)]*)\), setup = (\w+)\)\]\s*fn {re.escape(name)}\(", src)
    if not m:
        raise ProfileError(f"instructions.rs: no #[bench::...(args, setup)] on fn {name}")
    return m.group(1), m.group(2), m.group(3)


def _setup_facts(src: str, setup: str) -> dict[str, str | None]:
    """Key generator, build-loop insert, and probe shuffle seed of a setup fn."""
    body = _fn_body(src, setup)
    gen = re.search(r"let ks = (\w+)\(dist\)", body)
    if gen is None:
        # The setup is itself the key generator (the insert arms).
        return {"keys": setup, "build_insert": None, "seed": None}
    ins = re.search(r"map\.insert\(([^;]*)\);", body)
    seed = re.search(r"XorShift\((0x[0-9A-Fa-f_]+)\)", body)
    if seed is None:
        helper = re.search(r"\((?:map|m), (\w+)\(ks\)\)", body)
        if helper:
            seed = re.search(r"XorShift\((0x[0-9A-Fa-f_]+)\)", _fn_body(src, helper.group(1)))
    return {
        "keys": gen.group(1),
        "build_insert": re.sub(r"\s+", "", ins.group(1)) if ins else None,
        "seed": seed.group(1) if seed else None,
    }


def _normal_loop(body: str) -> str:
    """The timed body with the wrapper's type names and `mut` bindings folded away."""
    b = body.split("{", 1)[1]
    b = re.sub(r"SyncExpanse(\w+)", r"Expanse\1", b)
    b = re.sub(r"\bmut\s+", "", b)
    b = re.sub(r"//[^\n]*", "", b)
    return re.sub(r"\s+", "", b)


def like_for_like(src: str, sync_arm: str, plain_arm: str) -> list[str]:
    """Findings that make a pair not like-for-like; empty when it is."""
    out = []
    sid, sargs, ssetup = _bench_attr(src, sync_arm)
    pid, pargs, psetup = _bench_attr(src, plain_arm)
    if (sid, sargs) != (pid, pargs):
        out.append(f"bench id/args differ: {sid}{sargs} vs {pid}{pargs}")
    sf, pf = _setup_facts(src, ssetup), _setup_facts(src, psetup)
    for k in ("keys", "build_insert", "seed"):
        if sf[k] != pf[k]:
            out.append(f"setup {k} differs: {ssetup} {sf[k]!r} vs {psetup} {pf[k]!r}")
    if _normal_loop(_fn_body(src, sync_arm)) != _normal_loop(_fn_body(src, plain_arm)):
        out.append("the timed bodies differ beyond the wrapper's type name and `mut` bindings")
    return out


# --------------------------------------------------------------------------
# loading the committed artifacts
# --------------------------------------------------------------------------

def load_arm(arm: str, bench_id: str, root: Path = ARTIFACTS) -> Profile:
    stem = f"{arm}.{bench_id}"
    excl, log = root / f"{stem}.exclusive.txt", root / f"{stem}.callgrind.log"
    for p in (excl, log, root / f"{stem}.inclusive.txt"):
        if not p.is_file():
            raise ProfileError(f"missing artifact {p.relative_to(REPO_ROOT) if p.is_relative_to(REPO_ROOT) else p}")
    prof = parse_annotate(excl.read_text(), arm)
    prof.log_ir = parse_log_ir(log.read_text(), arm)
    reconcile(prof)
    return prof


def load_all(root: Path = ARTIFACTS) -> dict[str, Profile]:
    out = {}
    for _, _, s, p, bid in ARMS:
        out[s] = load_arm(s, bid, root)
        if p:
            out[p] = load_arm(p, bid, root)
    return out


# --------------------------------------------------------------------------
# rendering
# --------------------------------------------------------------------------

TOP = 10
TOP_DELTA = 8


def pair_key(sym: str, sync_arm: str, plain_arm: str) -> str:
    """The name a `sync_*` arm's symbol is paired under in its plain twin's listing.

    Every symbol pairs by its own name except the benchmark function itself,
    which iai-callgrind names after the arm
    (`instructions::sync_strmap_insert::__iai_callgrind_wrapper_mod::sync_strmap_insert`)
    and which is paired with the plain arm's.
    """
    return sym.replace(sync_arm, plain_arm) if sym.startswith("instructions::") else sym


def paired_by_symbol(profiles: dict[str, Profile], sync_arm: str, plain_arm: str) -> tuple[dict[str, int], dict[str, int]]:
    """Both arms' per-symbol Ir under the plain arm's names."""
    a: dict[str, int] = {}
    for sym, ir in profiles[sync_arm].by_symbol().items():
        k = pair_key(sym, sync_arm, plain_arm)
        a[k] = a.get(k, 0) + ir
    return a, profiles[plain_arm].by_symbol()


def one_sided_engine(a: dict[str, int], b: dict[str, int]) -> tuple[list[tuple[str, int]], list[tuple[str, int]]]:
    """Engine-category symbols with Ir in only one of two paired listings."""
    only_a = sorted(((s, v) for s, v in a.items() if v and not b.get(s) and category(s) == ENGINE),
                    key=lambda kv: (-kv[1], kv[0]))
    only_b = sorted(((s, v) for s, v in b.items() if v and not a.get(s) and category(s) == ENGINE),
                    key=lambda kv: (-kv[1], kv[0]))
    return only_a, only_b


def n(x: int) -> str:
    return f"{x:,}"


def signed(x: int) -> str:
    return f"+{x:,}" if x > 0 else f"{x:,}"


def pct(num: int, den: int) -> str:
    return f"{100.0 * num / den:.2f}%" if den else "—"


def render(profiles: dict[str, Profile], manifest: dict, src: str) -> list[str]:
    prov = manifest["provenance"]
    out: list[str] = []
    add = out.append

    add("#### 14.1 The recordings")
    add("")
    add("| wrapper | operation | arm | Ir, callgrind log | Ir, annotate `PROGRAM TOTALS` | Ir, sum of listed functions | functions listed | plain counterpart | counterpart Ir |")
    add("|---|---|---|--:|--:|--:|--:|---|--:|")
    for w, op, s, p, bid in ARMS:
        pr = profiles[s]
        cp = f"`{p}`" if p else "none in `instructions.rs`"
        cir = n(profiles[p].totals_ir) if p else "—"
        add(f"| `{w}` | {op} | `{s}` ({bid}) | {n(pr.log_ir)} | {n(pr.totals_ir)} | "
            f"{n(sum(r.ir for r in pr.rows))} | {len(pr.rows)} | {cp} | {cir} |")
    add("")
    add(f"Commit `{prov['commit'][:8]}`; {prov['rustc']}; {prov['valgrind']}; "
        f"iai-callgrind-runner {prov['iai_callgrind_runner']}; {prov['platform']}.")
    add("")

    add("#### 14.2 Like-for-like check of each plain counterpart")
    add("")
    add("| `sync_*` arm | plain arm | same bench id and args, key generator, build insert, probe shuffle seed, timed body | findings | engine symbols with Ir only in the `sync_*` arm: count, Ir, largest | engine symbols with Ir only in the plain arm: count, Ir, largest |")
    add("|---|---|---|---|---|---|")
    for s, p in PAIRS:
        f = like_for_like(src, s, p)
        only_s, only_p = one_sided_engine(*paired_by_symbol(profiles, s, p))

        def side(rows: list[tuple[str, int]]) -> str:
            if not rows:
                return "0"
            return f"{len(rows)}, {n(sum(v for _, v in rows))}, `{rows[0][0]}`"

        add(f"| `{s}` | `{p}` | {'yes' if not f else 'no'} | {'; '.join(f) if f else '—'} | "
            f"{side(only_s)} | {side(only_p)} |")
    add("")

    add("#### 14.3 Exclusive Ir by category, per arm")
    add("")
    add("| arm | " + " | ".join(CATEGORIES) + " | sum = arm Ir |")
    add("|---|" + "--:|" * len(CATEGORIES) + "---|")
    for arm in [a for _, _, s, p, _ in ARMS for a in (s, p) if a]:
        pr = profiles[arm]
        cats = pr.by_category()
        cells = [f"{n(cats[c])} ({pct(cats[c], pr.totals_ir)})" for c in CATEGORIES]
        add(f"| `{arm}` | " + " | ".join(cells) + f" | {'yes' if sum(cats.values()) == pr.totals_ir else 'no'} |")
    add("")

    add(f"#### 14.4 Top {TOP} functions by exclusive Ir, per `sync_*` arm")
    add("")
    add("| arm | rank | function | category | exclusive Ir | share of the arm | plain counterpart's Ir for the same symbol |")
    add("|---|--:|---|---|--:|--:|--:|")
    for _, _, s, p, _ in ARMS:
        pr = profiles[s]
        plain = profiles[p].by_symbol() if p else None
        ranked = sorted(pr.by_symbol().items(), key=lambda kv: (-kv[1], kv[0]))[:TOP]
        for i, (sym, ir) in enumerate(ranked, 1):
            pv = "—" if plain is None else n(plain.get(pair_key(sym, s, p), 0))
            add(f"| `{s}` | {i} | `{sym}` | {category(sym)} | {n(ir)} | {pct(ir, pr.totals_ir)} | {pv} |")
    add("")

    add("#### 14.5 The wrapper's addition: `sync_*` arm minus its plain counterpart")
    add("")
    add("| arm pair | category | `sync_*` Ir | plain Ir | addition, Ir | share of the arm pair's net addition |")
    add("|---|---|--:|--:|--:|--:|")
    for s, p in PAIRS:
        if like_for_like(src, s, p):
            add(f"| `{s}` − `{p}` | not computed: not like-for-like (14.2) | — | — | — | — |")
            continue
        sc, pc = profiles[s].by_category(), profiles[p].by_category()
        net = profiles[s].totals_ir - profiles[p].totals_ir
        for c in CATEGORIES:
            add(f"| `{s}` − `{p}` | {c} | {n(sc[c])} | {n(pc[c])} | {signed(sc[c] - pc[c])} | {pct(sc[c] - pc[c], net)} |")
        add(f"| `{s}` − `{p}` | **total** | {n(profiles[s].totals_ir)} | {n(profiles[p].totals_ir)} | "
            f"{signed(net)} | {pct(net, net)} |")
    for _, _, s, p, _ in ARMS:
        if not p:
            add(f"| `{s}` | not computed: no plain arm with this operation and population in `instructions.rs` | — | — | — | — |")
    add("")

    add(f"#### 14.6 Largest per-symbol differences, `sync_*` arm minus plain counterpart (top {TOP_DELTA} each way)")
    add("")
    add("| arm pair | direction | function | category | `sync_*` Ir | plain Ir | difference, Ir |")
    add("|---|---|---|---|--:|--:|--:|")
    for s, p in PAIRS:
        if like_for_like(src, s, p):
            continue
        a, b = paired_by_symbol(profiles, s, p)
        d = {k: a.get(k, 0) - b.get(k, 0) for k in set(a) | set(b)}
        up = sorted((kv for kv in d.items() if kv[1] > 0), key=lambda kv: (-kv[1], kv[0]))[:TOP_DELTA]
        down = sorted((kv for kv in d.items() if kv[1] < 0), key=lambda kv: (kv[1], kv[0]))[:TOP_DELTA]
        for label, rows in (("more in `sync_*`", up), ("less in `sync_*`", down)):
            for sym, dv in rows:
                add(f"| `{s}` − `{p}` | {label} | `{sym}` | {category(sym)} | {n(a.get(sym, 0))} | "
                    f"{n(b.get(sym, 0))} | {signed(dv)} |")
    return out


def render_committed() -> list[str]:
    manifest = json.loads((ARTIFACTS / "manifest.json").read_text())
    return render(load_all(), manifest, BENCH_SRC.read_text())


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------

_HDR = """--------------------------------------------------------------------------------
Profile data file '/t/x.out' (creator: callgrind-3.22.0)
--------------------------------------------------------------------------------
I1 cache: 32768 B, 64 B, 8-way associative
Events recorded:  Ir Dr Dw I1mr D1mr D1mw ILmr DLmr DLmw
Events shown:     Ir Dr Dw I1mr D1mr D1mw ILmr DLmr DLmw
Event sort order: Ir
Thresholds:       100
Include dirs:
Auto-annotation:  off

--------------------------------------------------------------------------------
Ir                  Dr                 Dw                 I1mr           D1mr             D1mw            ILmr         DLmr        DLmw
--------------------------------------------------------------------------------
{total} (100.0%) 5 (100.0%) 5 (100.0%) 3 (100.0%) 1 (100.0%) 1 (100.0%) 1 (100.0%) 0 (100.0%) 1 (100.0%)  PROGRAM TOTALS

--------------------------------------------------------------------------------
Ir                  Dr                 Dw                 I1mr         D1mr            D1mw            ILmr         DLmr       DLmw             file:function
--------------------------------------------------------------------------------
"""


def _row(ir: int | None, name: str) -> str:
    irs = "." if ir is None else f"{ir:,} (1.00%)"
    return f"{irs:>20} 1 (1.00%) 1 (1.00%) 0 .  1 ( 0.00%) 0 0 . .  {name}\n"


# Fixture: a `sync_*` arm and its plain twin, in the listing's format.
_SYNC_ROWS = [
    (6_000, "???:<expanse_trie::strmap::ExpanseStrMap>::insert [/t/bin]"),
    (1_500, "???:<expanse_trie::sync::SyncExpanseStrMap>::insert [/t/bin]"),
    (700, "???:<expanse_trie::occ::Collector>::try_advance [/t/bin]"),
    (400, "./malloc/./malloc/malloc.c:_int_malloc [/usr/lib/x86_64-linux-gnu/libc.so.6]"),
    (300, "???:<expanse_trie::alloc::NodeAlloc>::alloc_bytes_dispatch::<true> [/t/bin]"),
    (250, "???:<expanse_trie::occ::SeqVersion>::begin [/t/bin]"),
    (200, "???:expanse_trie::strmap::dispose::<expanse_trie::occ::Collector> [/t/bin]"),
    (150, "???:instructions::sync_strmap_insert::__iai_callgrind_wrapper_mod::sync_strmap_insert [/t/bin]"),
    (100, "???:<expanse_trie::strmap::ExpanseStrMap>::insert'2 [/t/bin]"),
    (None, "???:main [/t/bin]"),
]
_PLAIN_ROWS = [
    (6_300, "???:<expanse_trie::strmap::ExpanseStrMap>::insert [/t/bin]"),
    (500, "./malloc/./malloc/malloc.c:_int_malloc [/usr/lib/x86_64-linux-gnu/libc.so.6]"),
    (120, "???:instructions::strmap_insert::__iai_callgrind_wrapper_mod::strmap_insert [/t/bin]"),
    (None, "???:main [/t/bin]"),
]


def _listing(rows: list[tuple[int | None, str]], total: int | None = None) -> str:
    t = sum(r or 0 for r, _ in rows) if total is None else total
    return _HDR.replace("{total}", f"{t:,}") + "".join(_row(r, nm) for r, nm in rows) + "\n"


def _log(ir: int) -> str:
    return (f"==7== Events    : Ir Dr Dw I1mr D1mr D1mw ILmr DLmr DLmw\n"
            f"==7== Collected : {ir} 5 5 3 1 1 1 0 1\n")


_SRC_OK = """
fn str_keys(_dist: &str) -> Vec<Vec<u8>> { x }

fn built_strmap(dist: &str) -> (ExpanseStrMap, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let mut map = ExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    let mut probes = ks;
    let mut rng = XorShift(0x9E37_79B9);
    (map, probes)
}

fn shuffled_bytes(ks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut rng = XorShift(0x9E37_79B9);
    probes
}

fn built_sync_strmap(dist: &str) -> (SyncExpanseStrMap, Vec<Vec<u8>>) {
    let ks = str_keys(dist);
    let map = SyncExpanseStrMap::new();
    for (i, k) in ks.iter().enumerate() {
        map.insert(tk(k), i as u64);
    }
    (map, shuffled_bytes(ks))
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_strmap)]
fn strmap_churn(built: (ExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (mut map, probes) = built;
    for k in &probes {
        sink ^= map.insert(black_box(tk(k)), black_box(7)).unwrap_or(0);
    }
    black_box(sink)
}

#[library_benchmark]
#[bench::routes(args = ("routes",), setup = built_sync_strmap)]
fn sync_strmap_churn(built: (SyncExpanseStrMap, Vec<Vec<u8>>)) -> u64 {
    let (map, probes) = built;
    for k in &probes {
        sink ^= map.insert(black_box(tk(k)), black_box(7)).unwrap_or(0);
    }
    black_box(sink)
}
"""


def _self_test() -> int:
    failures: list[str] = []

    def expect(cond: bool, msg: str) -> None:
        if not cond:
            failures.append(msg)

    def raises(fn, needle: str, msg: str) -> None:
        try:
            fn()
        except ProfileError as exc:
            if needle not in str(exc):
                failures.append(f"{msg}: raised, but without {needle!r}: {exc}")
            return
        failures.append(f"{msg}: did not raise")

    # 1. Parsing and reconciliation on the fixture.
    sync = parse_annotate(_listing(_SYNC_ROWS), "sync_fixture")
    sync.log_ir = parse_log_ir(_log(9_600), "sync_fixture")
    expect(sync.totals_ir == 9_600, f"fixture totals {sync.totals_ir} != 9600")
    expect(len(sync.rows) == 10, f"fixture rows {len(sync.rows)} != 10")
    expect(sync.rows[-1].ir == 0, "a `.` row must read as 0 Ir")
    reconcile(sync)
    plain = parse_annotate(_listing(_PLAIN_ROWS), "plain_fixture")
    plain.log_ir = 6_920
    reconcile(plain)

    # 2. The recursion copy folds into its symbol.
    sym = sync.by_symbol()
    expect(sym["<expanse_trie::strmap::ExpanseStrMap>::insert"] == 6_100,
           f"recursion copy not folded: {sym.get('<expanse_trie::strmap::ExpanseStrMap>::insert')}")

    # 3. Categories, pinned per symbol: the generic argument `occ::Collector` in
    #    an engine symbol must not pull it into the epoch category.
    want = {
        "<expanse_trie::strmap::ExpanseStrMap>::insert": ENGINE,
        "<expanse_trie::sync::SyncExpanseStrMap>::insert": WRITER,
        "<expanse_trie::occ::Collector>::try_advance": EPOCH,
        "_int_malloc": ALLOC,
        "<expanse_trie::alloc::NodeAlloc>::alloc_bytes_dispatch::<true>": ALLOC,
        "<expanse_trie::occ::SeqVersion>::begin": BRACKET,
        "expanse_trie::strmap::dispose::<expanse_trie::occ::Collector>": ENGINE,
        "instructions::sync_strmap_insert::__iai_callgrind_wrapper_mod::sync_strmap_insert": OTHER,
        "<expanse_trie::alloc::NodeAlloc>::bracket_enter": BRACKET,
        "<expanse_trie::alloc::NodeAlloc as core::ops::drop::Drop>::drop": ALLOC,
        "<std::sys::sync::mutex::futex::Mutex>::lock_contended": WRITER,
        "__memcpy_avx_unaligned_erms": OTHER,
        "main": OTHER,
        # Names from the committed listings that a prefix rule alone missed.
        "__rustc::__rust_alloc_zeroed": ALLOC,
        "mmap": ALLOC,
        "<alloc::raw_vec::RawVec<expanse_trie::occ::Garbage>>::grow_one": ALLOC,
        "<expanse_trie::sync::ShardedTreePop>::flush_and_set": WRITER,
        "<std::hash::random::DefaultHasher as core::hash::Hasher>::write": OTHER,
        "expanse_trie::occ::claim_writer_slot": EPOCH,
    }
    for s, c in want.items():
        expect(category(s) == c, f"category({s!r}) = {category(s)!r}, want {c!r}")
    cats = sync.by_category()
    expect(cats == {WRITER: 1_500, BRACKET: 250, EPOCH: 700, ALLOC: 700, ENGINE: 6_300, OTHER: 150},
           f"fixture category sums {cats}")
    pc = plain.by_category()
    expect({c: cats[c] - pc[c] for c in CATEGORIES}
           == {WRITER: 1_500, BRACKET: 250, EPOCH: 700, ALLOC: 200, ENGINE: 0, OTHER: 30},
           f"fixture wrapper addition {({c: cats[c] - pc[c] for c in CATEGORIES})}")

    # 3b. Pairing: the benchmark function pairs with its twin's; an engine
    #     symbol present on one side only is reported, never folded away.
    profs = {"sync_strmap_insert": sync, "strmap_insert": plain}
    a, b = paired_by_symbol(profs, "sync_strmap_insert", "strmap_insert")
    bench_sym = "instructions::strmap_insert::__iai_callgrind_wrapper_mod::strmap_insert"
    expect(a.get(bench_sym) == 150 and b.get(bench_sym) == 120,
           f"benchmark function not paired: sync {a.get(bench_sym)}, plain {b.get(bench_sym)}")
    expect(not any("sync_strmap_insert" in k for k in a), "a sync benchmark symbol survived pairing")
    only_s, only_p = one_sided_engine(a, b)
    expect(only_s == [("expanse_trie::strmap::dispose::<expanse_trie::occ::Collector>", 200)],
           f"one-sided engine symbols, sync side: {only_s}")
    expect(only_p == [], f"one-sided engine symbols, plain side: {only_p}")
    expect(pair_key("<expanse_trie::sync::SyncExpanseStrMap>::insert", "sync_strmap_insert", "strmap_insert")
           == "<expanse_trie::sync::SyncExpanseStrMap>::insert", "a non-benchmark symbol was renamed")

    # 4. Every disagreement is refused, not averaged (section 8.1).
    bad = parse_annotate(_listing(_SYNC_ROWS, total=9_601), "bad_total")
    bad.log_ir = 9_601
    raises(lambda: reconcile(bad), "rows sum to 9,600", "a row sum short of PROGRAM TOTALS")
    off = parse_annotate(_listing(_SYNC_ROWS), "bad_log")
    off.log_ir = 9_599
    raises(lambda: reconcile(off), "log says 9,599", "a log Ir that disagrees with the listing")
    # A rule naming a category no table renders would drop that Ir from every
    # category table; reconcile must refuse it.
    RULES.insert(0, ("hashing", "std::hash::"))
    try:
        ok = parse_annotate(_listing(_SYNC_ROWS + [(9, "???:<std::hash::random::DefaultHasher>::write [/t/bin]")]), "unk")
        ok.log_ir = 9_609
        raises(lambda: reconcile(ok), "no table renders", "a symbol in a category outside CATEGORIES")
    finally:
        RULES.pop(0)
    raises(lambda: parse_annotate(_listing(_SYNC_ROWS).replace("Thresholds:       100", "Thresholds:       99"), "t"),
           "threshold '99'", "a listing cut below threshold 100")
    raises(lambda: parse_annotate(_listing(_SYNC_ROWS).replace("  ???:main", " garbage"), "g"),
           "cannot read", "an unreadable row")
    raises(lambda: parse_log_ir("==7== Events : Ir\n", "l"), "0 Collected", "a log without Collected")
    expect(parse_log_ir("==7== Events    : Ir Dr Dw I1mr\n==7== Collected : 125138622 26464263\n", "z")
           == 125_138_622, "a Collected line with trailing zero events dropped must still read")
    raises(lambda: parse_log_ir("==7== Events    : Ir Dr\n==7== Collected : 1 2 3\n", "x"),
           "do not pair up", "a Collected line with more counts than events")

    # 5. The like-for-like check passes a matching pair and names each break.
    expect(like_for_like(_SRC_OK, "sync_strmap_churn", "strmap_churn") == [],
           f"matching fixture pair reported: {like_for_like(_SRC_OK, 'sync_strmap_churn', 'strmap_churn')}")
    seed = _SRC_OK.replace("fn shuffled_bytes(ks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {\n    let mut rng = XorShift(0x9E37_79B9);",
                           "fn shuffled_bytes(ks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {\n    let mut rng = XorShift(0x1234);")
    expect(any("seed" in f for f in like_for_like(seed, "sync_strmap_churn", "strmap_churn")),
           "a different probe shuffle seed was not reported")
    body = _SRC_OK.replace("black_box(7)).unwrap_or(0);\n    }\n    black_box(sink)\n}\n\n#[library_benchmark]\n#[bench::routes(args = (\"routes\",), setup = built_sync_strmap)]",
                           "black_box(8)).unwrap_or(0);\n    }\n    black_box(sink)\n}\n\n#[library_benchmark]\n#[bench::routes(args = (\"routes\",), setup = built_sync_strmap)]")
    expect(any("timed bodies" in f for f in like_for_like(body, "sync_strmap_churn", "strmap_churn")),
           "a different timed body was not reported")
    keys = _SRC_OK.replace("fn built_sync_strmap(dist: &str) -> (SyncExpanseStrMap, Vec<Vec<u8>>) {\n    let ks = str_keys(dist);",
                           "fn built_sync_strmap(dist: &str) -> (SyncExpanseStrMap, Vec<Vec<u8>>) {\n    let ks = short_keys(dist);")
    expect(any("keys" in f for f in like_for_like(keys, "sync_strmap_churn", "strmap_churn")),
           "a different key generator was not reported")

    # 6. The real bench source: every declared pair is like-for-like today.
    src = BENCH_SRC.read_text()
    for s, p in PAIRS:
        f = like_for_like(src, s, p)
        expect(f == [], f"instructions.rs: {s} vs {p} not like-for-like: {f}")

    # 7. The committed artifacts reconcile and render, and the rendered pairs
    #    reproduce the fixture's arithmetic on real data.
    if (ARTIFACTS / "manifest.json").is_file():
        try:
            profs = load_all()
            lines = render(profs, json.loads((ARTIFACTS / "manifest.json").read_text()), src)
            expect(any(l.startswith("#### 14.5") for l in lines), "render lost section 14.5")
            for s, p in PAIRS:
                a, b = profs[s].by_category(), profs[p].by_category()
                expect(sum(a[c] - b[c] for c in CATEGORIES) == profs[s].totals_ir - profs[p].totals_ir,
                       f"{s} - {p}: category additions do not sum to the arm difference")
            with tempfile.TemporaryDirectory() as d:
                # A committed listing with one row dropped must be refused.
                stem = f"{ARMS[0][2]}.{ARMS[0][4]}"
                for suffix in (".exclusive.txt", ".inclusive.txt", ".callgrind.log"):
                    (Path(d) / f"{stem}{suffix}").write_text((ARTIFACTS / f"{stem}{suffix}").read_text())
                ex = Path(d) / f"{stem}.exclusive.txt"
                ls = ex.read_text().splitlines(keepends=True)
                head = next(i for i, l in enumerate(ls) if l.rstrip().endswith("file:function"))
                del ls[head + 2]
                ex.write_text("".join(ls))
                raises(lambda: load_arm(ARMS[0][2], ARMS[0][4], Path(d)), "rows sum to",
                       "a committed listing with its top row deleted")
        except ProfileError as exc:
            failures.append(f"committed artifacts: {exc}")
    else:
        failures.append(f"no manifest.json under {ARTIFACTS.relative_to(REPO_ROOT)}")

    if failures:
        for f in failures:
            print(f"FAIL: {f}")
        print(f"callgrind_wrapper_ranking.py --self-test: {len(failures)} failure(s)")
        return 1
    print("callgrind_wrapper_ranking.py --self-test: ok")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return _self_test()
    print("\n".join(render_committed()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
