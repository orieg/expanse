#!/usr/bin/env python3
"""
scripts/check_bench_shapes.py

Asserts that all timed benchmark and example harnesses declare a machine-readable
workload shape (# Workload shape table) in their module doc comments, and generates
the canonical harness audit tables in docs/BENCHMARKING.md.

Usage:
    python3 scripts/check_bench_shapes.py             # Check all harnesses and docs/BENCHMARKING.md
    python3 scripts/check_bench_shapes.py --check     # Same as above (exits non-zero on mismatch/error)
    python3 scripts/check_bench_shapes.py --write     # Updates docs/BENCHMARKING.md in place
    python3 scripts/check_bench_shapes.py --self-test # Runs unit self-tests (fail-then-pass)
"""

from __future__ import annotations

import argparse
import json
import glob
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

REQUIRED_FIELDS = [
    "workload_id",
    "group",
    "population",
    "insertion_order",
    "probes_and_reuse",
    "hit_rate",
    "miss_gen_method",
    "value_dereference",
    "measured_region",
    "arm_symmetry",
    "statistics",
    "verdict",
]

# Optional. A harness whose declared `workload_id` is not the id it writes into
# its artifact lists the others here, one per line or comma-separated.
#
# The gate below joins the two. `check_docs_hygiene.py` verifies that a
# published figure carries a `(workload: <id>)` tag and this file verifies that
# a shape is declared; nothing checked that a tag names a shape that exists, and
# `masstree_latency.rs` declared `masstree_latency` while writing
# `masstree_map_64bit` into all 72 cells of its committed baseline -- which is
# the id `docs/benchmarks/masstree_comparison/README.md` publishes twice. The
# tag was syntactically satisfied and resolved to nothing.
OPTIONAL_FIELDS = ["emits"]

GROUP_TITLES = {
    1: "Group 1: C-API Benches & Examples (`crates/expanse-capi/`)",
    2: "Group 2: Core Point/Batch Lookup & Micro-Benches (`crates/expanse/benches/`)",
    3: "Group 3: Hashbrown Comparison Suite (`crates/expanse/benches/`)",
    4: "Group 4: Workloads & Domain Suites (`crates/expanse/benches/`)",
    5: "Group 5: Standalone Examples & Profile Drivers (`crates/expanse/examples/`)",
    6: "Group 6: WebAssembly Instruments (`crates/expanse-wasm-fuel/`, `crates/expanse-wasm/tests/`)",
    7: "Group 7: Comparative FFI Suites (`crates/expanse-hot-bench/src/bin/`)",
    8: "Group 8: RocksDB MemTable Integration (`integrations/rocksdb/benches/`)",
}

# `insertion_order` is a controlled vocabulary, not prose (#726). The cell opens
# with one of these tokens and may then say anything; the token is what makes
# the regime checkable, and a competitor arm registered on the wrong one is the
# defect the field exists to prevent — the Masstree pre-registration was informed
# by a shuffled Step 0 build and evaluated on sorted cells, and one of its ten
# unpredicted losses traces to that (`masstree_comparison/METHODOLOGY.md` §10.2).
#
#   both       measured in both orders; every result row records which
#   sorted     the whole population is sorted before the build
#   shuffled   Fisher-Yates from the suite PRNG before the build
#   generator  the generator's own draw order, neither sorted nor shuffled
#   n/a        no population is built
#
# Whether a given arm needs `both` is a review question (docs/BENCHMARKING.md):
# a competitor whose cost or footprint is order-sensitive does, and the guide
# says so. What this list prevents is leaving the regime unstated.
INSERTION_ORDERS = ("both", "sorted", "shuffled", "generator", "n/a")

# Harnesses that live outside the benches/examples globs and carry no `fn main`:
# the fuel module's exports are the arms, and the Node harness is JavaScript.
# Listed explicitly so they are timed harnesses by declaration, not by regex.
EXTRA_TIMED_HARNESSES = [
    ("crates", "expanse-wasm-fuel", "src", "lib.rs"),
    ("crates", "expanse-wasm", "tests", "bench.js"),
]

# Binaries in `crates/expanse-hot-bench/src/bin/` that have an entry point but
# time nothing: census probes and validation gates whose every assertion is on
# a deterministic invariant. `is_timed_harness` keys on the entry point, so they
# are named here rather than inferred — the same "by declaration, not by regex"
# rule EXTRA_TIMED_HARNESSES follows in the other direction. A binary that
# starts timing something must come off this list; nothing detects that for you,
# which is why the list is short and each entry says what it does instead.
UNTIMED_HARNESSES = {
    # symmetry check on the latency pillars' probe bodies, run before they were written
    "hot_probe_symmetry.rs",
    # the HOT FFI validation gate: population counts, round-trip errors, byte accounting
    "hot_validate.rs",
    # reconciles `mem_used()` against bytes held from the C allocator
    "instrument_bridge.rs",
    # keyspace-width sweep behind METHODOLOGY §9.6; a density census, not a timing
    "keyspace_density_probe.rs",
}

BEGIN_MARKER = "<!-- BEGIN HARNESS AUDIT TABLE (generated by scripts/check_bench_shapes.py --write) -->"
END_MARKER = "<!-- END HARNESS AUDIT TABLE -->"


def get_repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def is_timed_harness(source: str) -> bool:
    """Classifies whether a source file is a timed harness or a helper module.

    Covers both entry-point spellings: Rust's `fn main` / criterion macros and
    C++'s `int main(`. The C++ form is needed because the RocksDB integration's
    benches are `.cc` and were outside every glob until #802 -- so their shape
    tables held by convention only, and `bench_memtable.cc` in fact had none.
    """
    return bool(re.search(
        r"\b(criterion_group!|criterion_main!|main!|fn\s+main\b|int\s+main\s*\()", source))


def discover_harnesses(repo_root: Path) -> Tuple[List[Path], List[Path]]:
    """Discovers Rust source files under benches and examples directories across all crates.
    Returns (timed_harnesses, helper_modules)."""
    capi_benches = glob.glob(str(repo_root / "crates" / "expanse-capi" / "benches" / "*.rs"))
    capi_examples = glob.glob(str(repo_root / "crates" / "expanse-capi" / "examples" / "*.rs"))
    core_benches = glob.glob(str(repo_root / "crates" / "expanse" / "benches" / "**" / "*.rs"), recursive=True)
    core_examples = glob.glob(str(repo_root / "crates" / "expanse" / "examples" / "*.rs"))
    # The comparative FFI suites (HOT, Masstree) live in a crate of their own and
    # were outside every glob above, so their shape tables held by convention
    # only — which is the wrong place for the gate to stop, since these are the
    # harnesses that carry a competitor arm (#726).
    ffi_bins = glob.glob(str(repo_root / "crates" / "expanse-hot-bench" / "src" / "bin" / "*.rs"))
    # The RocksDB integration's benches are C++ and sat outside every glob above,
    # so their declarations held by convention only -- the same place #726 found
    # the FFI bins, and `bench_memtable.cc` had no table at all (#802).
    rocksdb_benches = glob.glob(str(repo_root / "integrations" / "rocksdb" / "benches" / "*.cc"))

    all_files = sorted(
        [
            Path(p)
            for p in (capi_benches + capi_examples + core_benches + core_examples
                      + ffi_bins + rocksdb_benches)
            if not p.endswith(".disabled")
        ]
    )

    timed = []
    helpers = []
    for parts in EXTRA_TIMED_HARNESSES:
        extra = repo_root.joinpath(*parts)
        if extra.exists():
            timed.append(extra)
    for path in all_files:
        try:
            content = path.read_text(encoding="utf-8")
        except Exception as e:
            print(f"Error reading {path}: {e}", file=sys.stderr)
            continue
        if path.name in UNTIMED_HARNESSES and path.parent.name == "bin":
            helpers.append(path)
        elif is_timed_harness(content):
            timed.append(path)
        else:
            helpers.append(path)

    return timed, helpers


def parse_workload_shape(source: str, filename: str) -> Tuple[Optional[Dict[str, str]], List[str]]:
    """Parses the //! # Workload shape table from the module doc comments.
    Returns (shape_dict, list_of_errors)."""
    errors = []
    
    # Extract leading module doc comment lines: Rust `//!` lines, or for a
    # JavaScript harness the leading `/** ... */` block (` * ` prefixed lines).
    doc_lines = []
    if filename.endswith(".js"):
        block = re.search(r"/\*\*(.*?)\*/", source, re.DOTALL)
        if block:
            for line in block.group(1).splitlines():
                trimmed = line.strip()
                if trimmed.startswith("*"):
                    trimmed = trimmed[1:]
                doc_lines.append(trimmed.strip())
    elif filename.endswith((".cc", ".cpp", ".hpp")):
        # C++ has no `//!` inner-doc form, so the leading `//` block IS the
        # module doc. Collect it whole and let the section regex below find the
        # table inside it; a licence header above the table is harmless.
        for line in source.splitlines():
            trimmed = line.strip()
            if trimmed.startswith("//"):
                doc_lines.append(trimmed[2:].strip())
            elif trimmed == "" or trimmed.startswith("#include"):
                continue
            else:
                break
    else:
        for line in source.splitlines():
            trimmed = line.strip()
            if trimmed.startswith("//!"):
                doc_lines.append(trimmed[3:].strip())
            elif trimmed.startswith("//") or trimmed == "":
                continue
            else:
                # First non-comment, non-empty line marks end of module doc
                break

    doc_text = "\n".join(doc_lines)
    # Find the section starting at '# Workload shape' up to the next markdown header or end
    section_match = re.search(r"#+\s+Workload shape\b(.*?)(?=\n#+ |\Z)", doc_text, re.DOTALL | re.IGNORECASE)
    if not section_match:
        errors.append(f"{filename}: Missing '# Workload shape' markdown table in module doc comments (//!)")
        return None, errors

    section_text = section_match.group(1)
    shape_match = re.search(r"((?:\|[^\n]+\n?)+)", section_text)
    if not shape_match:
        errors.append(f"{filename}: Missing markdown table (| Property | Value |) under '# Workload shape'")
        return None, errors

    table_block = shape_match.group(1)
    props: Dict[str, str] = {}
    
    for row in table_block.strip().splitlines():
        cells = [c.strip() for c in row.split("|")]
        # cells: ['', prop, val, '']
        if len(cells) >= 4:
            k = cells[1].strip("` ").strip()
            v = cells[2].strip()
            if k and k != "---" and k.lower() != "property":
                if k in ("workload_id", "group"):
                    v = v.strip("` ").strip()
                props[k] = v

    # Validate required fields
    for field in REQUIRED_FIELDS:
        if field not in props:
            errors.append(f"{filename}: Missing required property '{field}' in '# Workload shape' table")
        elif not props[field]:
            errors.append(f"{filename}: Empty value for property '{field}' in '# Workload shape' table")

    if "group" in props:
        try:
            grp = int(props["group"])
            if grp not in GROUP_TITLES:
                errors.append(f"{filename}: Invalid group '{props['group']}', expected integer 1..{max(GROUP_TITLES)}")
        except ValueError:
            errors.append(f"{filename}: Group '{props['group']}' is not a valid integer")

    if props.get("insertion_order"):
        token = props["insertion_order"].split()[0].strip("`*_,;:").lower()
        if token not in INSERTION_ORDERS:
            errors.append(
                f"{filename}: insertion_order must open with one of "
                f"{', '.join(INSERTION_ORDERS)} — got '{token}'"
            )

    return props, errors


EMITTED_ID_RE = re.compile(
    r"""workload_id\\?["']\s*:\s*\\?["']([A-Za-z0-9_]+)"""
    r"""|workload_id\s*:\s*["']([A-Za-z0-9_]+)["']"""
)


def emitted_workload_ids(source: str) -> set:
    """Ids the harness writes into its own JSON artifact.

    Textual, because the alternative is running the harness, and the two forms
    that occur are an escaped literal inside a Rust format string and a struct
    field initialiser.
    """
    return {m.group(1) or m.group(2) for m in EMITTED_ID_RE.finditer(source)}


def check_emitted_ids(relpath: str, source: str, shape: Dict[str, str]) -> List[str]:
    """Every id a harness emits must be one it declares."""
    declared = {shape.get("workload_id", "")}
    for chunk in shape.get("emits", "").replace(",", " ").split():
        declared.add(chunk.strip("`"))
    declared.discard("")
    errors = []
    for wid in sorted(emitted_workload_ids(source) - declared):
        errors.append(
            f"{relpath}: writes workload_id '{wid}' into its output but declares "
            f"'{shape.get('workload_id')}'; a published (workload: `{wid}`) tag "
            "would resolve to no shape table. Set `workload_id` to it, or list it "
            "in an `emits` row."
        )
    return errors


def generate_audit_tables(harness_data: List[Tuple[str, Dict[str, str]]]) -> str:
    """Generates the markdown audit tables, one per group in GROUP_TITLES."""
    # Organize by group
    grouped: Dict[int, List[Tuple[str, Dict[str, str]]]] = {g: [] for g in GROUP_TITLES}
    for relpath, shape in harness_data:
        grp = int(shape["group"])
        grouped[grp].append((relpath, shape))

    lines = []
    last = max(GROUP_TITLES)
    for grp in sorted(GROUP_TITLES):
        title = GROUP_TITLES[grp]
        lines.append(f"### {title}\n")
        lines.append("| File | Population ($N$) | Insertion Order | Probes & Reuse | Hit Rate | Miss Gen Method | Value Dereference | Measured Region | Arm Symmetry | Statistics | Verdict & Notes |")
        lines.append("|---|---|---|---|---|---|---|---|---|---|---|")
        for relpath, shape in grouped[grp]:
            file_link = f"[`{relpath}`](../{relpath})"
            pop = shape.get("population", "")
            order = shape.get("insertion_order", "")
            probes = shape.get("probes_and_reuse", "")
            hit = shape.get("hit_rate", "")
            miss = shape.get("miss_gen_method", "")
            deref = shape.get("value_dereference", "")
            region = shape.get("measured_region", "")
            arm = shape.get("arm_symmetry", "")
            stats = shape.get("statistics", "")
            verdict = shape.get("verdict", "")
            lines.append(f"| {file_link} | {pop} | {order} | {probes} | {hit} | {miss} | {deref} | {region} | {arm} | {stats} | {verdict} |")
        if grp < last:
            lines.append("\n---\n")

    return "\n".join(lines)


# Fields that discriminate one workload from another when a number is read out
# of context. Population and probe cardinality are what made the 366x mismatch
# in #453 invisible; hit rate and dereference are what made two "lookup" arms
# non-comparable. Emitted beside every published figure so a reader does not
# have to open the harness (#487 item A).
DISCRIMINATING_FIELDS = [
    "population",
    "probes_and_reuse",
    "hit_rate",
    "value_dereference",
]


def collect_shapes(repo_root: Path) -> Dict[str, Dict[str, str]]:
    """`{workload_id -> shape}` for every timed harness that declares one.

    Each entry carries `source` (repo-relative path) and `stem` (the file stem)
    on top of the declared fields. `stem` is the join key reports use:
    iai-callgrind names a bench `<target>::<group>::<bench>`, and the target is
    the harness file stem, so `search_instructions` resolves to
    `benches/search_instructions.rs` and thence to `domain_search_instructions`.

    Parse failures are skipped rather than raised — `--check` is the gate that
    fails on them, and a report should not die because a declaration regressed.
    """
    timed_paths, _ = discover_harnesses(repo_root)
    shapes: Dict[str, Dict[str, str]] = {}
    for path in timed_paths:
        relpath = str(path.relative_to(repo_root))
        shape, errs = parse_workload_shape(path.read_text(encoding="utf-8"), relpath)
        if shape is None or errs:
            continue
        wid = shape.get("workload_id")
        if not wid:
            continue
        entry = dict(shape)
        entry["source"] = relpath
        entry["stem"] = path.stem
        shapes[wid] = entry
    return shapes


def shapes_by_stem(repo_root: Path) -> Dict[str, Dict[str, str]]:
    """`{file stem -> shape}` for stems that identify exactly one harness.

    Stems shared by more than one harness are **excluded**, not arbitrarily
    resolved: `crates/expanse/benches/smoke_instructions.rs` and
    `crates/expanse-capi/benches/smoke_instructions.rs` both produce the bench
    target `smoke_instructions`, so a report row naming it could be either.
    Attaching one of the two shapes would state a population and hit rate the
    number may not have, which is the failure this whole mechanism exists to
    prevent. Ambiguous stems come back from `ambiguous_stems` instead, for the
    caller to report as ambiguous rather than as declared or missing.
    """
    grouped = ambiguous_stems(repo_root)
    return {
        v["stem"]: v
        for v in collect_shapes(repo_root).values()
        if v["stem"] not in grouped
    }


def ambiguous_stems(repo_root: Path) -> Dict[str, List[str]]:
    """`{file stem -> [workload ids]}` for stems shared by several harnesses."""
    seen: Dict[str, List[str]] = {}
    for wid, sh in collect_shapes(repo_root).items():
        seen.setdefault(sh["stem"], []).append(wid)
    return {k: sorted(v) for k, v in seen.items() if len(v) > 1}


def summarize_shape(shape: Dict[str, str]) -> str:
    """One-line rendering of the discriminating fields, for report inlining."""
    parts = [f"{f.replace('_', ' ')} {shape[f]}" for f in DISCRIMINATING_FIELDS if shape.get(f)]
    return " · ".join(parts)


# ---------------------------------------------------------------------------
# Arm inventory: the deterministic Callgrind arms, the ops count each one is
# divided by, and every other timed harness named with the reason its arms are
# not statically enumerable. A plan that predicts a per-arm delta reads its arm
# names and ops counts from here instead of transcribing them from memory.
# ---------------------------------------------------------------------------

ARM_BEGIN_MARKER = "<!-- BEGIN ARM INVENTORY TABLE (generated by scripts/check_bench_shapes.py --write) -->"
ARM_END_MARKER = "<!-- END ARM INVENTORY TABLE -->"

LIB_BENCH_RE = re.compile(r"#\[library_benchmark[^\]]*\]")
BENCH_ARM_RE = re.compile(r"#\[bench::([A-Za-z0-9_]+)\s*\(")
BENCH_FN_RE = re.compile(r"^(?:pub\s+)?fn\s+([a-z0-9_]+)\s*\(", re.M)
GROUP_DECL_RE = re.compile(
    r"library_benchmark_group!\s*\(\s*name\s*=\s*([A-Za-z0-9_]+)\s*;\s*benchmarks\s*=\s*([^)]*)\)",
    re.S,
)

# A harness whose arm ids only exist at run time is named here with the reason,
# by declaration and never by regex (§8.12.1's rule for `UNTIMED_HARNESSES`).
# The classifier below covers the two structural cases; this dict is for the
# harnesses neither case describes.
NOT_ENUMERABLE_OVERRIDES: Dict[str, str] = {
    "crates/expanse-wasm-fuel/src/lib.rs": (
        "wasm fuel instrument: each arm is a `#[no_mangle]` export that "
        "`scripts/wasm_fuel.py` instantiates under wasmtime and reads fuel "
        "back from, so the arm list is that script's, not an iai attribute's"
    ),
    "crates/expanse-wasm/tests/bench.js": (
        "Node wall-clock harness: rows are JS objects built at run time, and "
        "the deterministic wasm instrument is the fuel crate above"
    ),
}


def parse_iai_arms(source: str) -> Dict[str, List[str]]:
    """`{benchmark fn -> [arm, ...]}` for one iai-callgrind harness.

    An arm is a `#[bench::NAME(...)]` attribute above the function it belongs
    to; a `#[library_benchmark]` with no `#[bench::…]` is one unnamed arm and
    is recorded as such rather than skipped.
    """
    out: Dict[str, List[str]] = {}
    for m in LIB_BENCH_RE.finditer(source):
        tail = source[m.end():]
        fn_match = BENCH_FN_RE.search(tail)
        if not fn_match:
            continue
        head = tail[: fn_match.start()]
        arms = BENCH_ARM_RE.findall(head)
        out[fn_match.group(1)] = arms
    return out


def parse_iai_groups(source: str) -> Dict[str, str]:
    """`{benchmark fn -> library_benchmark_group! name}`."""
    out: Dict[str, str] = {}
    for group, body in GROUP_DECL_RE.findall(source):
        for fn in (n.strip() for n in body.split(",")):
            if fn:
                out[fn] = group
    return out


def not_enumerable_reason(relpath: str, source: str) -> str:
    """Why a timed harness contributes no arm rows, or '' when it should."""
    if relpath in NOT_ENUMERABLE_OVERRIDES:
        return NOT_ENUMERABLE_OVERRIDES[relpath]
    if "criterion_group!" in source or "criterion_main!" in source:
        return "criterion: arm ids are built at run time"
    if "/expanse-hot-bench/src/bin/" in relpath:
        return "FFI suite binary: arms are selected by CLI arguments"
    if "fn main" in source or "int main" in source:
        return "standalone driver: no declared arms"
    return ""


def load_perf_report(repo_root: Path):
    """The ops-count maps and name normalisation, from their one owner.

    Imported rather than reimplemented: a second copy of the resolution order
    would drift from the one the report actually divides by.
    """
    import importlib.util

    path = repo_root / "scripts" / "perf_report.py"
    spec = importlib.util.spec_from_file_location("perf_report_for_arms", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def registered_ops_key(pr, bench_name: str, is_smoke: bool) -> Optional[str]:
    """The ops-map key `get_bench_n` would resolve, or None if it falls through.

    Mirrors `get_bench_n`'s lookup order over the same maps; falling through is
    what the report turns into `N = 1`, which reads as "one operation per
    invocation" whether or not anyone meant it.
    """
    clean_name = pr.normalize_bench_name(bench_name)
    base_name = bench_name.split("/")[0]
    if is_smoke and clean_name in pr.SMOKE_BENCH_N_MAP:
        return clean_name
    for key in (bench_name, clean_name, base_name):
        if key in pr.BENCH_N_MAP:
            return key
    return None


def collect_arm_inventory(
    repo_root: Path, timed_paths: List[Path]
) -> Tuple[List[Dict[str, str]], List[Dict[str, str]], List[str]]:
    """`(arm rows, non-enumerable rows, errors)` across every timed harness."""
    pr = load_perf_report(repo_root)
    arm_rows: List[Dict[str, str]] = []
    other_rows: List[Dict[str, str]] = []
    errors: List[str] = []
    used_keys: set = set()

    for path in timed_paths:
        relpath = str(path.relative_to(repo_root))
        source = path.read_text(encoding="utf-8")
        arms = parse_iai_arms(source)
        if not arms:
            reason = not_enumerable_reason(relpath, source)
            if not reason:
                errors.append(
                    f"{relpath}: declares no iai arms and matches no "
                    "not-enumerable case — add an entry to "
                    "NOT_ENUMERABLE_OVERRIDES stating why its arms cannot be "
                    "listed, so the absence is declared and not silent"
                )
            else:
                other_rows.append({"harness": relpath, "reason": reason})
            continue

        is_smoke = path.name == "smoke_instructions.rs"
        groups = parse_iai_groups(source)
        for fn, fn_arms in sorted(arms.items()):
            for arm in fn_arms or [""]:
                bench_name = f"{fn}/{arm}" if arm else fn
                key = registered_ops_key(pr, bench_name, is_smoke)
                if key is None:
                    errors.append(
                        f"{relpath}: arm '{bench_name}' has no ops entry, so "
                        "`perf_report.py` reports its raw count as `Ins / Op` "
                        "(N = 1). Register it in BENCH_N_MAP (or "
                        "SMOKE_BENCH_N_MAP), with an explicit `1` when one "
                        "invocation really is one operation"
                    )
                    continue
                used_keys.add(key)
                n = pr.get_bench_n(bench_name, is_smoke=is_smoke)
                arm_rows.append(
                    {
                        "harness": relpath,
                        "group": groups.get(fn, ""),
                        "fn": fn,
                        "arm": arm or "(single)",
                        "n": f"{n:,}",
                        "ops_key": key,
                    }
                )

    declared = set(pr.BENCH_N_MAP) | set(pr.SMOKE_BENCH_N_MAP)
    for key in sorted(declared - used_keys):
        errors.append(
            f"scripts/perf_report.py: ops entry '{key}' matches no arm in any "
            "harness — a renamed or deleted arm leaves its count behind, and "
            "the next arm to take that name inherits it silently"
        )
    return arm_rows, other_rows, errors


def generate_arm_table(
    arm_rows: List[Dict[str, str]], other_rows: List[Dict[str, str]]
) -> str:
    """The generated arm inventory block."""
    lines: List[str] = []
    lines.append(
        "Every deterministic Callgrind arm, and the ops count `perf_report.py` "
        "divides it by. Generated by `python3 scripts/check_bench_shapes.py "
        "--write`; do not hand-edit."
    )
    lines.append("")
    lines.append(
        "**`N` is keys, not engine calls.** It is the arm's operation count, "
        "which is what `Ins / Op` divides by. How often an operation reaches a "
        "given engine function is a separate question — for inserts, "
        "[ALGORITHMS.md](ALGORITHMS.md) §3.2."
    )
    lines.append("")
    by_harness: Dict[str, List[Dict[str, str]]] = {}
    for row in arm_rows:
        by_harness.setdefault(row["harness"], []).append(row)
    for harness in sorted(by_harness):
        lines.append(f"#### [`{harness}`](../{harness})")
        lines.append("")
        lines.append("| Group | Benchmark | Arms | `N` per arm |")
        lines.append("|---|---|---|---:|")
        seen: Dict[Tuple[str, str], List[Tuple[str, str]]] = {}
        for row in by_harness[harness]:
            seen.setdefault((row["group"], row["fn"]), []).append((row["arm"], row["n"]))
        for (group, fn), arms in sorted(seen.items()):
            counts = {n for _, n in arms}
            names = ", ".join(f"`{a}`" for a, _ in arms)
            n_cell = arms[0][1] if len(counts) == 1 else ", ".join(
                f"`{a}` {n}" for a, n in arms
            )
            lines.append(f"| `{group}` | `{fn}` | {names} | {n_cell} |")
        lines.append("")

    if other_rows:
        lines.append("#### Timed harnesses with no statically listed arms")
        lines.append("")
        lines.append("| Harness | Why its arms are not listed |")
        lines.append("|---|---|")
        for row in sorted(other_rows, key=lambda r: r["harness"]):
            lines.append(f"| [`{row['harness']}`](../{row['harness']}) | {row['reason']} |")
        lines.append("")
    return "\n".join(lines).strip()


def splice_block(
    doc_path: Path, begin: str, end: str, body: str, write: bool
) -> List[str]:
    """Replaces the text between two markers, or reports the mismatch."""
    content = doc_path.read_text(encoding="utf-8")
    pattern = re.escape(begin) + r"(.*?)" + re.escape(end)
    match = re.search(pattern, content, re.DOTALL)
    if not match:
        return [f"{doc_path}: missing the `{begin}` / `{end}` block"]
    if match.group(1).strip() == body.strip():
        return []
    if write:
        doc_path.write_text(
            content[: match.start()] + begin + "\n\n" + body.strip() + "\n\n" + end + content[match.end():],
            encoding="utf-8",
        )
        return []
    return [
        f"{doc_path}: the arm inventory block is out of date — run "
        "`python3 scripts/check_bench_shapes.py --write`"
    ]


def check_and_generate(repo_root: Path, write: bool = False) -> int:
    timed_paths, helper_paths = discover_harnesses(repo_root)
    print(f"check_bench_shapes.py: {len(timed_paths)} timed harnesses, {len(helper_paths)} helper modules found")

    all_errors = []
    seen_workload_ids: Dict[str, str] = {}
    harness_data: List[Tuple[str, Dict[str, str]]] = []

    for path in timed_paths:
        relpath = str(path.relative_to(repo_root))
        source = path.read_text(encoding="utf-8")
        shape, errs = parse_workload_shape(source, relpath)
        if errs:
            all_errors.extend(errs)
        if shape:
            wid = shape.get("workload_id")
            if wid:
                if wid in seen_workload_ids:
                    all_errors.append(f"Duplicate workload_id '{wid}' in {relpath} (previously seen in {seen_workload_ids[wid]})")
                else:
                    seen_workload_ids[wid] = relpath
            all_errors.extend(check_emitted_ids(relpath, source, shape))
            harness_data.append((relpath, shape))

    if all_errors:
        print("\n".join(f"ERROR: {e}" for e in all_errors), file=sys.stderr)
        return 1

    generated_tables = generate_audit_tables(harness_data)

    arm_rows, other_rows, arm_errors = collect_arm_inventory(repo_root, timed_paths)
    if arm_errors:
        print("\n".join(f"ERROR: {e}" for e in arm_errors), file=sys.stderr)
        return 1
    generated_arm_table = generate_arm_table(arm_rows, other_rows)

    benchmarking_doc = repo_root / "docs" / "BENCHMARKING.md"
    if not benchmarking_doc.exists():
        print(f"ERROR: {benchmarking_doc} does not exist", file=sys.stderr)
        return 1

    doc_content = benchmarking_doc.read_text(encoding="utf-8")
    
    pattern = re.escape(BEGIN_MARKER) + r"(.*?)" + re.escape(END_MARKER)
    match = re.search(pattern, doc_content, re.DOTALL)
    if not match:
        print(f"ERROR: Demarcation markers not found in {benchmarking_doc}.\nExpected '{BEGIN_MARKER}' and '{END_MARKER}'", file=sys.stderr)
        return 1

    existing_section = match.group(1).strip()
    desired_section = generated_tables.strip()

    if write:
        new_content = doc_content[:match.start()] + BEGIN_MARKER + "\n\n" + desired_section + "\n\n" + END_MARKER + doc_content[match.end():]
        benchmarking_doc.write_text(new_content, encoding="utf-8")
        arm_errors = splice_block(
            benchmarking_doc, ARM_BEGIN_MARKER, ARM_END_MARKER, generated_arm_table, write=True
        )
        if arm_errors:
            print("\n".join(f"ERROR: {e}" for e in arm_errors), file=sys.stderr)
            return 1
        print(f"check_bench_shapes.py: Updated {benchmarking_doc}")
        return 0
    else:
        if existing_section != desired_section:
            print(f"ERROR: {benchmarking_doc} audit table is out of date. Run 'python3 scripts/check_bench_shapes.py --write' to sync.", file=sys.stderr)
            return 1
        arm_errors = splice_block(
            benchmarking_doc, ARM_BEGIN_MARKER, ARM_END_MARKER, generated_arm_table, write=False
        )
        if arm_errors:
            print("\n".join(f"ERROR: {e}" for e in arm_errors), file=sys.stderr)
            return 1
        print(
            f"check_bench_shapes.py: {len(arm_rows)} Callgrind arms across "
            f"{len({r['harness'] for r in arm_rows})} harnesses, "
            f"{len(other_rows)} harnesses with no listed arms; ops entries all reachable."
        )
        # Derived, never stamped (AGENTS.md §8.2): the count said 35 while 51
        # harnesses were being checked, and extending the globs to the FFI
        # suites made the gap visible rather than creating it.
        print(f"check_bench_shapes.py: all {len(harness_data)} harness shapes valid "
              "and docs/BENCHMARKING.md is in sync.")
        return 0


def run_self_tests() -> int:
    """Fail-then-pass unit self-tests for classifier, parser, required fields, and table generation."""
    print("Running check_bench_shapes.py unit self-tests...")

    # 1. Classifier test
    sample_timed = "fn main() { println!(\"hello\"); }"
    sample_bench = "criterion_group!(benches, bench_fn); criterion_main!(benches);"
    sample_helper = "pub fn helper_fn() -> u64 { 42 }"
    assert is_timed_harness(sample_timed) is True, "sample_timed should classify as timed"
    assert is_timed_harness(sample_bench) is True, "sample_bench should classify as timed"
    assert is_timed_harness(sample_helper) is False, "sample_helper should classify as helper"

    # 2. Valid shape parsing
    valid_doc = """//! Headline description.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `test_workload` |
//! | `group` | 1 |
//! | `population` | 10k |
//! | `insertion_order` | generator draw order (ascending) |
//! | `probes_and_reuse` | 10k, reuse 1.0 |
//! | `hit_rate` | 100% |
//! | `miss_gen_method` | None |
//! | `value_dereference` | `sink ^= *slot` |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Symmetric |
//! | `statistics` | Exact |
//! | `verdict` | **PASS** |

fn main() {}
"""
    shape, errs = parse_workload_shape(valid_doc, "test.rs")
    assert not errs, f"Expected no errors for valid doc, got: {errs}"
    assert shape is not None and shape["workload_id"] == "test_workload"
    assert shape["group"] == "1"

    # 3. Fail-then-pass test for every required field
    for field in REQUIRED_FIELDS:
        # Create invalid doc omitting this field
        lines = []
        for l in valid_doc.splitlines():
            if f"`{field}`" not in l:
                lines.append(l)
        bad_doc = "\n".join(lines)
        bad_shape, bad_errs = parse_workload_shape(bad_doc, "bad.rs")
        assert any(f"Missing required property '{field}'" in e for e in bad_errs), f"Expected missing '{field}' error, got {bad_errs}"

    # 3b. insertion_order is a controlled vocabulary, not prose (#726). A cell
    # that describes the regime in words the gate does not know leaves the
    # regime unstated, which is the defect the field exists to prevent.
    for bad in ("ascending-ish", "random", "as-drawn", "unspecified"):
        doc = valid_doc.replace(
            "| `insertion_order` | generator draw order (ascending) |",
            f"| `insertion_order` | {bad} |")
        _, errs = parse_workload_shape(doc, "bad_order.rs")
        assert any("insertion_order must open with one of" in e for e in errs), (bad, errs)
    for good in ("both", "sorted", "shuffled", "generator", "n/a"):
        doc = valid_doc.replace(
            "| `insertion_order` | generator draw order (ascending) |",
            f"| `insertion_order` | {good} — and then any prose at all |")
        _, errs = parse_workload_shape(doc, "good_order.rs")
        assert not errs, (good, errs)

    # 4. Invalid group test
    bad_grp_doc = valid_doc.replace("| `group` | 1 |", "| `group` | 99 |")
    _, grp_errs = parse_workload_shape(bad_grp_doc, "bad_grp.rs")
    assert any("Invalid group '99'" in e for e in grp_errs), f"Expected invalid group error, got {grp_errs}"

    # 5. Missing workload shape header test
    no_header_doc = "//! Just a doc without table\nfn main() {}"
    _, no_hdr_errs = parse_workload_shape(no_header_doc, "no_hdr.rs")
    assert any("Missing '# Workload shape'" in e for e in no_hdr_errs), f"Expected missing header error, got {no_hdr_errs}"

    # 6. Table generation test
    test_data = [
        ("crates/expanse/benches/test.rs", {
            "workload_id": "test_workload",
            "group": "1",
            "population": "10k",
            "probes_and_reuse": "10k",
            "hit_rate": "100%",
            "miss_gen_method": "None",
            "value_dereference": "deref",
            "measured_region": "Clean",
            "arm_symmetry": "Symmetric",
            "statistics": "Exact",
            "verdict": "**PASS**",
        })
    ]
    tables = generate_audit_tables(test_data)
    assert "### Group 1: C-API Benches & Examples (`crates/expanse-capi/`)" in tables
    assert "[`crates/expanse/benches/test.rs`](../crates/expanse/benches/test.rs)" in tables

    # 7. Emit mode: shapes are joinable by file stem and summarise to one line,
    #    so reports can print a number's shape beside it (#487).
    shapes = collect_shapes(get_repo_root())
    assert shapes, "no workload shapes collected"
    by_stem = shapes_by_stem(get_repo_root())
    assert "search_instructions" in by_stem, sorted(by_stem)[:5]
    assert by_stem["search_instructions"]["workload_id"] == "domain_search_instructions"
    # Every collected entry carries the join key and its source.
    for wid, sh in shapes.items():
        assert sh.get("stem"), f"{wid} has no stem"
        # `.cc` since #802 brought the RocksDB integration's C++ benches into
        # scope; the join key is the file stem, which is language-agnostic.
        assert sh.get("source", "").endswith((".rs", ".js", ".cc")), f"{wid} has no source path"
    # The C++ `//` doc form parses, and the two RocksDB benches are present with
    # the group-8 heading. Pinned because the extractor has a per-language branch
    # and a `.cc` file whose table stopped being read would simply vanish from
    # the audit rather than fail (#802).
    assert "rocksdb_memtable_single_threaded" in shapes, sorted(shapes)[:8]
    assert "rocksdb_memtable_concurrent_read_scaling" in shapes, sorted(shapes)[:8]
    assert shapes["rocksdb_memtable_single_threaded"]["source"].endswith("bench_memtable.cc")
    assert int(shapes["rocksdb_memtable_concurrent_read_scaling"]["group"]) == 8
    # A `//`-commented table must actually be parsed, not merely discovered: the
    # hit-rate cell of the single-threaded bench is distinctive, so an extractor
    # that returned an empty table would fail here rather than pass vacuously.
    assert "8.98%" in shapes["rocksdb_memtable_single_threaded"]["hit_rate"], \
        shapes["rocksdb_memtable_single_threaded"]["hit_rate"][:80]
    assert is_timed_harness("int main(int argc, char** argv) { return 0; }"), \
        "C++ entry point must classify as a timed harness"
    assert not is_timed_harness("static void helper() {}"), \
        "a C++ helper with no entry point must not classify as timed"

    # A stem shared by two harnesses is excluded from the join, never resolved
    # to one of them: core and capi both ship benches/smoke_instructions.rs.
    ambiguous = ambiguous_stems(get_repo_root())
    assert "smoke_instructions" in ambiguous, ambiguous
    assert ambiguous["smoke_instructions"] == ["capi_smoke_instructions", "core_smoke_instructions"]
    assert "smoke_instructions" not in by_stem, "ambiguous stem must not resolve to one harness"
    assert len(by_stem) == len(shapes) - sum(len(v) for v in ambiguous.values())
    summary = summarize_shape(shapes["capi_bench_vs_libjudy"])
    assert "population" in summary and "hit rate" in summary, summary
    # fail-then-pass: a shape missing every discriminating field summarises to
    # nothing rather than to a fabricated description.
    assert summarize_shape({"workload_id": "x"}) == ""

    # 8. Arm inventory: parsing, ops registration, and the not-enumerable
    # classifier. The motivating defects are transcription errors in plans —
    # an arm name that does not exist, and an arm nobody registered whose
    # `Ins / Op` column silently became its raw count.
    iai_src = """
#[library_benchmark]
#[bench::sequential(args = ("sequential",), setup = keys)]
#[bench::random(args = ("random",), setup = keys)]
fn map_insert(ks: Vec<u64>) -> u64 { 0 }

#[library_benchmark]
fn lone_arm() -> u64 { 0 }

library_benchmark_group!(
    name = cost;
    benchmarks = map_insert, lone_arm
);
"""
    arms = parse_iai_arms(iai_src)
    assert arms == {"map_insert": ["sequential", "random"], "lone_arm": []}, arms
    assert parse_iai_groups(iai_src) == {"map_insert": "cost", "lone_arm": "cost"}

    class _StubReport:
        BENCH_N_MAP = {"map_insert": 50_000, "map_insert/random": 1_007}
        SMOKE_BENCH_N_MAP = {"map_insert": 10_000}

        @staticmethod
        def normalize_bench_name(name: str) -> str:
            base = name.split("/")[0]
            for suffix in ("_expanse_dl", "_expanse", "_stock"):
                if base.endswith(suffix):
                    return base[: -len(suffix)]
            return base

    stub = _StubReport()
    # A per-distribution entry wins over the arm's, as in `get_bench_n`.
    assert registered_ops_key(stub, "map_insert/random", False) == "map_insert/random"
    assert registered_ops_key(stub, "map_insert/sequential", False) == "map_insert"
    # The suffixed twins of one family resolve through the normalised name.
    assert registered_ops_key(stub, "map_insert_stock/random", False) == "map_insert"
    # The smoke map is consulted first only for a smoke harness.
    assert registered_ops_key(stub, "map_insert/random", True) == "map_insert"
    # fail-then-pass: an unregistered arm resolves to nothing, which is what
    # `get_bench_n` turns into N = 1 — the silent fallback this gate exists for.
    assert registered_ops_key(stub, "set_insert/random", False) is None

    assert not_enumerable_reason("x.rs", "criterion_group!(benches, f);")
    assert not_enumerable_reason("crates/expanse-hot-bench/src/bin/x.rs", "fn other() {}")
    # fail-then-pass: a harness matching no case reports no reason, which
    # `collect_arm_inventory` turns into an error rather than a silent skip.
    assert not_enumerable_reason("x.rs", "pub fn helper() {}") == ""

    table = generate_arm_table(
        [{"harness": "a.rs", "group": "cost", "fn": "map_insert", "arm": "random", "n": "50,000", "ops_key": "map_insert"}],
        [{"harness": "b.rs", "reason": "criterion: arm ids are built at run time"}],
    )
    assert "`map_insert`" in table and "`random`" in table and "50,000" in table, table
    assert "b.rs" in table and "criterion" in table, table

    print("ALL 8 SELF-TESTS PASSED.")
    return 0


def _self_test_emitted_id_join() -> List[str]:
    """Pin the defect the join exists for, verbatim.

    `masstree_latency.rs` declared `masstree_latency` and wrote
    `masstree_map_64bit` into every cell of its committed baseline, and
    `docs/benchmarks/masstree_comparison/README.md` publishes figures tagged
    with the emitted id. A gate that passes while ignoring that case is
    measuring the wrong invariant (AGENTS.md §8.12.3).
    """
    failures = []
    src = (
        'let _ = "{{\\"workload_id\\":\\"masstree_map_64bit\\",\\"arm\\":\\"map\\"}}";'
    )
    if emitted_workload_ids(src) != {"masstree_map_64bit"}:
        failures.append("emitted_workload_ids missed the escaped-JSON form")

    undeclared = {"workload_id": "masstree_latency"}
    if not check_emitted_ids("x.rs", src, undeclared):
        failures.append("the join accepted an emitted id that no shape declares")

    declared_via_emits = {"workload_id": "masstree_latency", "emits": "`masstree_map_64bit`"}
    if check_emitted_ids("x.rs", src, declared_via_emits):
        failures.append("an `emits` row did not satisfy the join")

    renamed = {"workload_id": "masstree_map_64bit"}
    if check_emitted_ids("x.rs", src, renamed):
        failures.append("renaming the primary did not satisfy the join")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description="Check benchmark workload shape declarations and sync audit table.")
    parser.add_argument("--check", action="store_true", help="Check declarations and audit table sync (default)")
    parser.add_argument("--write", action="store_true", help="Write generated audit table to docs/BENCHMARKING.md")
    parser.add_argument("--self-test", action="store_true", help="Run unit self-tests")
    parser.add_argument("--json", action="store_true", help="Emit {workload_id: shape} as JSON for report joining")
    args = parser.parse_args()

    if args.self_test:
        return run_self_tests()

    repo_root = get_repo_root()
    if args.json:
        json.dump(collect_shapes(repo_root), sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
        return 0

    return check_and_generate(repo_root, write=args.write)


if __name__ == "__main__":
    sys.exit(main())
