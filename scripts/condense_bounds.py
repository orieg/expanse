#!/usr/bin/env python3
"""
scripts/condense_bounds.py — the byte model behind subtree condensation on
remove (docs/benchmarks/remove_retention/METHODOLOGY.md), as committed,
unit-tested code (AGENTS.md §8.8 commit 1).

Expanse's remove paths step a branch down its own ladder and free it when it
empties, but never rebuild a branch subtree into a packed leaf; only the root
condenses (`condense_to_root_leaf`, crates/expanse/src/map.rs and set.rs). The
design reference for condensing is Judy's "decascade (coalesce)" on delete with
at most one index of hysteresis: *Judy IV Shop Manual* (Alan Silverstein, 2002,
judy.sourceforge.net/doc/shop_interm.pdf), glossary entry "Cascade" and §5.
Expanse carries none of Judy's code (AGENTS.md §3); only the published idea.

What this module answers, each as a function with pinned reference values:

  * `child_bytes`, `branch_subtree_bytes`, `packed_leaf_bytes` — what a subtree
    costs held as a branch over children, against the same keys rebuilt as one
    packed leaf at the branch's level. The accounting is `mem_used()`: every
    allocation rounded up to its alignment (`alloc::accounted_size`).
  * `condense_saves` — the byte-aware rule: condense only when the packed leaf
    is strictly smaller than the subtree it replaces.
  * `worst_leaf_over_branch` — the largest packed-leaf / branch ratio over the
    linear-branch shapes, i.e. how badly a key-count-only condense can lose:
    1.25 for a set, 1.2778 for a map (1.20 and 1.2632 with the parent edge
    counted on both sides).
  * `H1_THRESHOLD`, `WIDE_THRESHOLD`, `evaluation_points`, `hysteresis` — the
    two pre-registered hysteresis arms and the populations at which each
    evaluates.
  * `thrash_bound`, `isolated_cost` — the G-thrash ceiling
    `baseline + (C_split + C_condense) / (2 (H + 1))` and the per-event costs
    it reads from a pair of isolating Callgrind arms.
  * `predicted_retention` — the drained-vs-fresh bytes ratio R of a uniform
    random 64-bit tree, from Poisson occupancy (the law `density_poisson.py`
    uses; Feller vol. 1, ch. VI §5) and binomial thinning of the survivors.

Every constant mirrors the engine. `test_engine_sync` reads each one, the node
sizes and the function bodies the model re-implements from the Rust source and
fails when the engine moves (with one negative control per kind of check);
`test_pins` fails when the model moves:

  LEAF_CAP, LEAF1_CAP, RAW_ALIGN, BRANCH_L3_CAP, BRANCH_L7_CAP,
  BITMAP_TO_UNCOMPRESSED_THRESHOLD, IMMED_PAYLOAD_BYTES   crates/expanse/src/types.rs
  cap_class, size_set, size_map                           crates/expanse/src/leaf.rs
  sub_edges_size (cap_class(n) * 16)                      crates/expanse/src/mutate.rs
  map_immed_max (7 // kb), map_immed_val_size             crates/expanse/src/mutate.rs, mutate_map.rs
  ImmedType::max_count (15 // kb)                         crates/expanse/src/types.rs
  BranchL3 64, BranchL7 128, BranchB 128, BranchU 4160    crates/expanse/src/node.rs (const asserts)
  B -> L7 at digits <= 6, L7 -> L3 at num < 3             crates/expanse/src/mutate_map.rs (remove)
  a map leaf above map_immed_max stays a leaf down to 1   crates/expanse/src/mutate_map.rs (remove)

Usage:
  python3 scripts/condense_bounds.py            # run the pinned tests, then print the tables
  python3 scripts/condense_bounds.py --self-test
"""

from __future__ import annotations

import functools
import math
import re
import sys
from pathlib import Path

EDGE = 16
RAW_ALIGN = 16
LEAF_CAP = 32
LEAF1_CAP = 25
BRANCH_L3_CAP = 3
BRANCH_L7_CAP = 7
BRANCHB_TO_L7_DOWN = BRANCH_L7_CAP - 1
BITMAP_TO_UNCOMPRESSED_THRESHOLD = 192
IMMED_PAYLOAD_BYTES = 15
MAP_IMMED_PAYLOAD_BYTES = 7
CACHE_LINE = 64
L3_BYTES = CACHE_LINE
L7_BYTES = 2 * CACHE_LINE
B_BYTES = 2 * CACHE_LINE
U_BYTES = 4096 + CACHE_LINE
LEAFB1_SET = 64  # LeafBitmap1: a 256-bit bitmap, one cache line
EXPANSES_64 = 1 << 16  # 2-byte-prefix expanses at 64 bits
FLAVORS = ("set", "map")

# The two pre-registered hysteresis arms (maintainer decision 1). Derived from
# LEAF_CAP, never literals (AGENTS.md §2.1 invariant 6).
H1_THRESHOLD = LEAF_CAP - 1  # Judy: at most one index of hysteresis
WIDE_THRESHOLD = LEAF_CAP - 8  # the 24-slot class


def cap_class(pop: int) -> int:
    """Slot class of a linear leaf of `pop` keys (`leaf::cap_class`)."""
    if pop < 0:
        raise ValueError("pop must be non-negative")
    if pop <= 2:
        return pop
    if pop <= 16:
        return (pop + 3) & ~3
    if pop <= 24:
        return 24
    if pop <= 32:
        return 32
    return (pop + 3) & ~3


def rnd(n: int, align: int = RAW_ALIGN) -> int:
    """`alloc::accounted_size`: the request rounded up to its alignment."""
    if n < 0 or align <= 0 or align & (align - 1):
        raise ValueError("n >= 0 and a power-of-two alignment")
    return (n + align - 1) & ~(align - 1)


def immed_max(kb: int, flavor: str) -> int:
    """Keys an immediate edge holds: 15 // kb (set), 7 // kb (map)."""
    if not 1 <= kb <= 7 or flavor not in FLAVORS:
        raise ValueError("kb in 1..=7, flavor set or map")
    return (IMMED_PAYLOAD_BYTES if flavor == "set" else MAP_IMMED_PAYLOAD_BYTES) // kb


def leaf_bytes(p: int, kb: int, flavor: str) -> int:
    """Accounted bytes of a linear leaf of `p` keys of `kb` bytes (no edge)."""
    if p < 1 or not 1 <= kb <= 7 or flavor not in FLAVORS:
        raise ValueError("p >= 1, kb in 1..=7, flavor set or map")
    per = kb if flavor == "set" else 8 + kb
    return rnd(per * cap_class(p))


def child_bytes(p: int, kb: int, flavor: str) -> int:
    """Heap bytes below one edge holding `p` keys of `kb` bytes, built fresh
    in the smallest form (the edge itself excluded)."""
    if not 1 <= kb <= 7 or p < 1 or flavor not in FLAVORS:
        raise ValueError("kb in 1..=7, p >= 1, flavor set or map")
    if p <= immed_max(kb, flavor):
        if flavor == "map" and p >= 2:
            return rnd(8 * cap_class(p))  # the immediate's value array
        return 0
    if kb == 1 and p > LEAF1_CAP:
        if flavor == "set":
            return LEAFB1_SET
        raise ValueError("map LeafB1 value subarrays are not modelled")
    if kb >= 2 and p > LEAF_CAP:
        raise ValueError("p exceeds LEAF_CAP: the child would itself be a branch")
    return leaf_bytes(p, kb, flavor)


def minimal_form(k: int) -> str:
    """The branch form a fresh build gives `k` children."""
    if not 1 <= k <= 256:
        raise ValueError("k in 1..=256")
    if k <= BRANCH_L3_CAP:
        return "L3"
    if k <= BRANCH_L7_CAP:
        return "L7"
    return "B" if k < BITMAP_TO_UNCOMPRESSED_THRESHOLD else "U"


def drained_form(k: int) -> str:
    """The form a `BranchB` has once removals leave it `k` children: it steps
    to L7 below 7 digits and to L3 below 3, and is never condensed."""
    if not 1 <= k < BITMAP_TO_UNCOMPRESSED_THRESHOLD:
        raise ValueError("k in 1..192")
    if k < BRANCH_L3_CAP:
        return "L3"
    if k <= BRANCHB_TO_L7_DOWN:
        return "L7"
    return "B"


def branch_node_bytes(groups: list[int], form: str) -> int:
    """Bytes of one branch node; `groups` is the child count in each of the 8
    32-digit groups (a BranchB packs one class-sized edge subarray per group)."""
    if len(groups) != 8 or any(g < 0 or g > 32 for g in groups):
        raise ValueError("8 groups of 0..=32 children")
    k = sum(groups)
    if form == "L3":
        if not 1 <= k <= BRANCH_L3_CAP:
            raise ValueError("L3 holds 1..=3 children")
        return L3_BYTES
    if form == "L7":
        if not 1 <= k <= BRANCH_L7_CAP:
            raise ValueError("L7 holds 1..=7 children")
        return L7_BYTES
    if form == "B":
        return B_BYTES + sum(rnd(cap_class(g) * EDGE) for g in groups if g)
    if form == "U":
        return U_BYTES
    raise ValueError(f"unknown form {form!r}")


def branch_subtree_bytes(child_pops, child_digits, kb_child, flavor, form=None, child_fn=None) -> int:
    """Heap bytes of a branch over its children (edge above it excluded)."""
    if len(child_pops) != len(child_digits) or not child_pops:
        raise ValueError("one digit per child, at least one child")
    if len(set(child_digits)) != len(child_digits) or any(not 0 <= d < 256 for d in child_digits):
        raise ValueError("distinct digits in 0..=255")
    groups = [0] * 8
    for d in child_digits:
        groups[d >> 5] += 1
    form = form or minimal_form(len(child_pops))
    child_fn = child_fn or child_bytes
    return branch_node_bytes(groups, form) + sum(child_fn(p, kb_child, flavor) for p in child_pops)


def packed_leaf_bytes(p: int, kb: int, flavor: str) -> int:
    """The same `p` keys rebuilt as one leaf (or immediate) at the branch's
    level, whose keys are `kb` bytes (edge excluded). A leaf is not narrowed
    below its parent: for a branch reached through a skip edge, a fresh build
    of the same keys is a leaf at the parent's child level, which is wider
    (21 keys under a 5-byte prefix below a level-7 parent build a 144-byte
    Leaf6, not an 80-byte Leaf3; engine probe on 463ff2d0). Price that case
    with `kb = parent_level - 1`."""
    return child_bytes(p, kb, flavor)


def condense_saves(packed: int, subtree: int) -> bool:
    """The byte-aware rule (maintainer decision 2): strictly smaller only."""
    if packed < 0 or subtree < 0:
        raise ValueError("byte counts are non-negative")
    return packed < subtree


def spread(p: int) -> tuple[list[int], list[int]]:
    """One key per child, children dealt round-robin over the 8 digit groups."""
    if not 1 <= p <= 256:
        raise ValueError("p in 1..=256")
    return [1] * p, [(i % 8) * 32 + i // 8 for i in range(p)]


@functools.lru_cache(maxsize=None)
def _partitions(total: int, parts: int, cap: int) -> tuple[tuple[int, ...], ...]:
    """Non-increasing tuples of `parts` positive ints <= cap summing to `total`."""
    if parts == 0:
        return ((),) if total == 0 else ()
    out = []
    for first in range(min(cap, total - parts + 1), 0, -1):
        out.extend((first,) + rest for rest in _partitions(total - first, parts - 1, first))
    return tuple(out)


def worst_leaf_over_branch(flavor: str, with_edge: bool = False) -> tuple[float, tuple]:
    """Largest packed-leaf / branch-subtree byte ratio over every linear-branch
    subtree a packed leaf of at most LEAF_CAP keys could replace: branch levels
    2..=7, 1..=7 children in the form a fresh build or a drain gives them, and
    every multiset of child populations (each child an immediate or a leaf, in
    its fresh form) summing to at most LEAF_CAP. Returns (ratio, (level, form,
    child pops, leaf bytes, branch bytes)), the first maximum in search order
    (level ascending, form, population descending). With `with_edge`, the
    16-byte edge above the subtree is added to both sides, which is how
    `mem_used()` per subtree reads once the parent's edge slot is charged."""
    if flavor not in FLAVORS:
        raise ValueError("flavor set or map")
    extra = EDGE if with_edge else 0
    best = (0.0, ())
    for level in range(2, 8):
        kb_child = level - 1
        # A map LeafB1 (kb 1 above LEAF1_CAP) is not modelled; keep map
        # children at kb 1 inside the linear-leaf range.
        cap = LEAF1_CAP if (kb_child == 1 and flavor == "map") else LEAF_CAP
        cb = {c: child_bytes(c, kb_child, flavor) for c in range(1, cap + 1)}
        for k in range(1, BRANCH_L7_CAP + 1):
            # A fresh build gives k children its minimal form; a drained
            # BranchB reaches L7 at k <= 6 and L3 at k <= 2.
            for form in sorted({minimal_form(k), drained_form(k)}):
                groups = [k] + [0] * 7  # digits 0..k-1, all in group 0
                node = branch_node_bytes(groups, form)
                for p in range(LEAF_CAP, k - 1, -1):
                    leaf = packed_leaf_bytes(p, level, flavor) + extra
                    for pops in _partitions(p, k, cap):
                        branch = node + extra + sum(cb[c] for c in pops)
                        if leaf / branch > best[0]:
                            best = (leaf / branch, (level, form, list(pops), leaf, branch))
    return best


def class_tops(upto: int) -> list[int]:
    """Populations p <= upto that fill their slot class exactly (p == cap_class(p)),
    descending: where a packed leaf would carry no empty slot."""
    if upto < 1:
        raise ValueError("upto >= 1")
    return [p for p in range(upto, 0, -1) if cap_class(p) == p]


def evaluation_points(threshold: int) -> list[int]:
    """Post-remove populations at which a subtree under `threshold` is
    evaluated for condensation: the threshold itself (the arm's first chance),
    then every class top below it (a declined subtree is retried whenever its
    packed leaf would get cheaper). Descending."""
    if not 1 <= threshold <= LEAF_CAP:
        raise ValueError("threshold in 1..=LEAF_CAP")
    return [threshold] + [p for p in class_tops(threshold) if p < threshold]


def band_width(threshold: int) -> int:
    """Removes needed after a cascade (at LEAF_CAP + 1) before the subtree is
    first evaluated, i.e. the hysteresis band; an insert/remove oscillation
    across it rebuilds at most twice per 2 * band_width operations."""
    if not 1 <= threshold <= LEAF_CAP:
        raise ValueError("threshold in 1..=LEAF_CAP")
    return LEAF_CAP + 1 - threshold


def rebuilds_per_op(threshold: int) -> float:
    """Worst-case structural rebuilds (one cascade plus one condense per cycle)
    per operation on an oscillation between LEAF_CAP + 1 and `threshold`."""
    return 2.0 / (2 * band_width(threshold))


def hysteresis(threshold: int) -> int:
    """H of an arm: how many indexes below LEAF_CAP its threshold sits
    (threshold = LEAF_CAP - H). The H1 arm is H = 1, Judy's at most one index
    of hysteresis; the wide arm is H = 8."""
    if not 1 <= threshold <= LEAF_CAP:
        raise ValueError("threshold in 1..=LEAF_CAP")
    return LEAF_CAP - threshold


def thrash_bound(baseline_per_op: float, c_split: float, c_condense: float, h: int) -> float:
    """G-thrash ceiling: the oscillation arm's per-operation cost may not exceed
    main's per-operation cost on the same input plus one cascade and one
    condense amortised over the 2(H + 1) operations of one full cycle
    (LEAF_CAP - H -> LEAF_CAP + 1 -> LEAF_CAP - H):

        baseline + (C_split + C_condense) / (2 (H + 1))

    C_split and C_condense are per-event instruction costs from the isolating
    arms (`isolated_cost`). Units are whatever the three inputs share
    (instructions per operation for Callgrind)."""
    if baseline_per_op < 0 or c_split < 0 or c_condense < 0:
        raise ValueError("costs are non-negative")
    if not 0 <= h < LEAF_CAP:
        raise ValueError("h in 0..LEAF_CAP")
    return baseline_per_op + (c_split + c_condense) / (2 * (h + 1))


def isolated_cost(ir_event: int, ir_control: int, events: int) -> float:
    """Per-event cost from a pair of isolating Callgrind arms: the event arm
    performs `events` structural events (one cascade, or one condense, per
    expanse) and the control arm performs the same number of the same
    operation on the same tree shape without the event. The difference,
    divided by the event count, is C_split or C_condense. A control that costs
    more than the event arm is a defect in the arm pair, not a zero."""
    if events <= 0 or ir_event < 0 or ir_control < 0:
        raise ValueError("events > 0, instruction counts >= 0")
    if ir_control > ir_event:
        raise ValueError("control arm exceeds event arm: the pair does not isolate the event")
    return (ir_event - ir_control) / events


# ---------------------------------------------------------------------------
# Drain model: uniform random 64-bit keys, N built, a uniform subset of M kept
# ---------------------------------------------------------------------------
def poisson_pmf(k: int, lam: float) -> float:
    if k < 0 or lam <= 0.0:
        raise ValueError("k >= 0, lam > 0")
    return math.exp(-lam + k * math.log(lam) - math.lgamma(k + 1))


def binom_pmf(k: int, n: int, p: float) -> float:
    if not 0 <= k <= n or not 0.0 <= p <= 1.0:
        raise ValueError("0 <= k <= n, p in [0, 1]")
    return math.comb(n, k) * p**k * (1.0 - p) ** (n - k)


def _branch_b_bytes_expected(p_child: float) -> tuple[float, float, float]:
    """For a level-6 branch whose 256 child digits are each present with
    probability p_child independently: (P(K >= 7), E[BranchB bytes; K >= 7],
    E[L7/L3 bytes; 1 <= K <= 6]). Exact over the 8 groups by a DP on K
    saturated at 7."""
    g = [binom_pmf(i, 32, p_child) for i in range(33)]
    # state k (0..7, 7 = ">= 7") -> (probability, E[sum of subarray bytes]).
    st = {0: (1.0, 0.0)}
    for _ in range(8):
        nxt: dict[int, tuple[float, float]] = {}
        for k, (pr, ex) in st.items():
            for i, pi in enumerate(g):
                if pi == 0.0:
                    continue
                kk = min(7, k + i)
                sub = rnd(cap_class(i) * EDGE) if i else 0
                p0, e0 = nxt.get(kk, (0.0, 0.0))
                nxt[kk] = (p0 + pr * pi, e0 + ex * pi + pr * pi * sub)
        st = nxt
    p7, e7 = st.get(7, (0.0, 0.0))
    small = sum(pr * (L3_BYTES if k < BRANCH_L3_CAP else L7_BYTES) for k, (pr, _) in st.items() if 1 <= k <= 6)
    return p7, B_BYTES * p7 + e7, small


@functools.lru_cache(maxsize=None)
def _drained_spread_bytes(y: int, flavor: str) -> int:
    """A cascaded level-6 expanse drained to `y` single-key children spread
    evenly, in the form the remove path leaves it (edge excluded)."""
    pops, digs = spread(min(y, 256))
    return branch_subtree_bytes(pops, digs, 5, flavor, drained_form(min(y, 191)))


def predicted_retention(n: int, m: int, flavor: str, expanses: int = EXPANSES_64,
                        threshold: int | None = None) -> dict:
    """Predicted mem_used of a uniform random 64-bit tree built to `n` keys and
    drained by a uniform random subset to `m`, against a fresh build of `m`.

    Model. Every 2-byte prefix is populated at the populations this is used
    for, so the top two levels are one root BranchU and 256 level-7 BranchU
    (fixed, identical in both trees). Below them each expanse holds
    X ~ Poisson(n / expanses) keys at build and Y ~ Binomial(X, m / n) after.
    Fresh: a Leaf6 (or an immediate) of Y keys; above LEAF_CAP, a BranchB of
    single-key children. Drained: an expanse that never cascaded (X <= 32) is a
    leaf of Y keys, as fresh; one that did is a branch whose 256 level-5
    digits are Poissonized, each holding c ~ Poisson(n / (256 expanses)) at
    build and Binomial(c, m / n) after. The branch keeps its BranchB form at
    K >= 7 surviving digits, L7 at 3..=6, L3 at 1..=2. Set children of up to
    3 five-byte keys are immediates (0 bytes). A map child built with 2 or more
    keys is a Leaf5 and stays one down to its last key, costing
    rnd(13 * cap_class(y)) for y survivors.

    Approximations, stated: the digit-level Poissonization ignores the
    conditioning on X > 32 (P(X <= 32) = 0.0069 at n = 3.2M); set children of
    4 or more keys are costed as immediates, and so is a drained set child
    that kept 3 of 4 or more keys, which the remove path leaves a Leaf5 (it
    converts a set leaf to an immediate one key below `ImmedType::max_count`);
    a fresh expanse above LEAF_CAP is costed with single-key children spread
    evenly.

    With `threshold`, also returns `r_condensed`: the same drain under a
    condensation arm of that threshold (see the comment at its computation)."""
    if flavor not in FLAVORS or not 0 < m <= n or expanses <= 0:
        raise ValueError("flavor set or map, 0 < m <= n, expanses > 0")
    q = m / n
    lam_n, lam_m = n / expanses, m / expanses
    kmax = int(lam_n + 12 * math.sqrt(lam_n) + 40)
    fixed = (1 + 256) * U_BYTES

    @functools.lru_cache(maxsize=None)
    def fresh_expanse(y: int) -> float:
        if y == 0:
            return 0.0
        if y <= LEAF_CAP:
            return child_bytes(y, 6, flavor)
        pops, digs = spread(min(y, 256))
        return branch_subtree_bytes(pops, digs, 5, flavor)

    fresh = sum(poisson_pmf(y, lam_m) * fresh_expanse(y) for y in range(kmax))
    leaf_part = 0.0
    p_casc = 0.0
    for x in range(kmax):
        px = poisson_pmf(x, lam_n)
        if x > LEAF_CAP:
            p_casc += px
            continue
        for y in range(x + 1):
            leaf_part += px * binom_pmf(y, x, q) * fresh_expanse(y)
    mu_n = lam_n / 256
    p_child = 1.0 - math.exp(-mu_n * q)
    p7, b_bytes, small_bytes = _branch_b_bytes_expected(p_child)
    child_part = 0.0
    if flavor == "map":
        for c in range(2, 40):
            pc = poisson_pmf(c, mu_n)
            for y in range(1, c + 1):
                child_part += pc * binom_pmf(y, c, q) * leaf_bytes(y, 5, "map")
        child_part *= 256
    drained = leaf_part + p_casc * (b_bytes + small_bytes + child_part)
    used_fresh = fixed + expanses * fresh
    used_drained = fixed + expanses * drained
    condensed = None
    if threshold is not None:
        # Condensation arm: a cascaded expanse whose survivors fall to
        # `threshold` or below passed through `threshold` one remove at a time,
        # was evaluated there, and (the byte rule accepting at level 6, which
        # `test_pins` checks for P <= 31) ends as the fresh form of its Y keys.
        # One left above keeps a branch: y single-key children spread evenly,
        # plus the map's retained Leaf5 bytes at the model's per-key rate.
        if not 1 <= threshold <= LEAF_CAP:
            raise ValueError("threshold in 1..=LEAF_CAP")
        per_key_child = child_part / lam_m
        casc = 0.0
        for x in range(LEAF_CAP + 1, kmax):
            px = poisson_pmf(x, lam_n)
            for y in range(x + 1):
                py = binom_pmf(y, x, q)
                if py < 1e-15:
                    continue
                if y <= threshold:
                    cost = fresh_expanse(y)
                else:
                    cost = _drained_spread_bytes(y, flavor) + per_key_child * y
                casc += px * py * cost
        condensed = fixed + expanses * (leaf_part + casc)
    return {
        "lambda_n": lam_n,
        "lambda_m": lam_m,
        "p_cascaded": p_casc,
        "p_branch_b": p7,
        "branch_b_bytes_per_expanse": b_bytes,
        "used_fresh": used_fresh,
        "used_drained": used_drained,
        "bpk_fresh": used_fresh / m,
        "bpk_drained": used_drained / m,
        "r": used_drained / used_fresh,
        "r_condensed": None if condensed is None else condensed / used_fresh,
    }


# ---------------------------------------------------------------------------
# Engine source sync: every constant and size above, read from the Rust source
# ---------------------------------------------------------------------------
REPO = Path(__file__).resolve().parent.parent
SRC = "crates/expanse/src"
SOURCE_FILES = ("types.rs", "node.rs", "leaf.rs", "mutate.rs", "mutate_map.rs", "alloc.rs")

# (file, `pub const` name, the Python value it must equal).
ENGINE_CONSTS = (
    ("types.rs", "CACHE_LINE", CACHE_LINE),
    ("types.rs", "RAW_ALIGN", RAW_ALIGN),
    ("types.rs", "LEAF_CAP", LEAF_CAP),
    ("types.rs", "LEAF1_CAP", LEAF1_CAP),
    ("types.rs", "BRANCH_L3_CAP", BRANCH_L3_CAP),
    ("types.rs", "BRANCH_L7_CAP", BRANCH_L7_CAP),
    ("types.rs", "BRANCHB_TO_L7_DOWN", BRANCHB_TO_L7_DOWN),
    ("types.rs", "BITMAP_TO_UNCOMPRESSED_THRESHOLD", BITMAP_TO_UNCOMPRESSED_THRESHOLD),
    ("types.rs", "IMMED_PAYLOAD_BYTES", IMMED_PAYLOAD_BYTES),
)

# node.rs const asserts: `assert!(size_of::<T>() == <expr>);`.
ENGINE_SIZES = (
    ("Edge", EDGE),
    ("BranchL3", L3_BYTES),
    ("BranchL7", L7_BYTES),
    ("BranchB", B_BYTES),
    ("BranchU", U_BYTES),
    ("LeafBitmap1", LEAFB1_SET),
)

# Function bodies the model re-implements, whitespace and comments removed.
ENGINE_BODIES = (
    ("leaf.rs", "cap_class",
     "ifpop<=2{pop}elseifpop<=16{(pop+3)&!3}elseifpop<=24{24}elseifpop<=32{32}else{(pop+3)&!3}"),
    ("leaf.rs", "size_set", "key_bytesasusize*cap_class(pop)"),
    ("leaf.rs", "size_map", "8*cap_class(pop)+key_bytesasusize*cap_class(pop)"),
    ("types.rs", "max_count", "(IMMED_PAYLOAD_BYTES/key_bytesasusize)asu8"),
    ("mutate.rs", "map_immed_max", "7/kbasusize"),
    ("mutate.rs", "sub_edges_size", "leaf::cap_class(n)*size_of::<Edge>()"),
    ("alloc.rs", "accounted_size", "(bytes+(align-1))&!(align-1)"),
)

# The remove-path demotions `drained_form` models: one site in each remove
# walk. Each file has two walks, the plain one (`remove` / `map_remove`) and
# the shared tree's copy (`remove_occ` / `map_remove_occ`, #1086), and both
# must carry the demotion.
ENGINE_DEMOTIONS = (
    ("mutate.rs", "digits <= crate::types::BRANCHB_TO_L7_DOWN", 2),
    ("mutate.rs", "num < BRANCH_L3_CAP", 2),
    ("mutate_map.rs", "digits <= crate::types::BRANCHB_TO_L7_DOWN", 2),
    ("mutate_map.rs", "num < BRANCH_L3_CAP", 2),
)


def _strip_comments(text: str) -> str:
    return re.sub(r"//[^\n]*", "", text)


def _arith(expr: str, env: dict[str, int]) -> int | None:
    """Evaluate `+ - *` over integer literals and names in `env`, nothing else."""
    tokens = re.findall(r"\d+|[A-Z_][A-Z0-9_]*|[+\-*()]|\S", expr)
    out = []
    for t in tokens:
        if t.isdigit() or t in "+-*()":
            out.append(t)
        elif t in env:
            out.append(str(env[t]))
        else:
            return None
    return int(eval(" ".join(out), {"__builtins__": {}}, {}))  # digits and operators only


def _const_value(text: str, name: str, env: dict[str, int]) -> int | None:
    m = re.search(rf"^pub const {name}: usize = ([^;]+);", text, re.M)
    return _arith(m.group(1), env) if m else None


def _fn_body(text: str, name: str) -> str | None:
    m = re.search(rf"\bfn {name}\b[^{{]*\{{", text)
    if not m:
        return None
    depth, i = 1, m.end()
    while depth and i < len(text):
        depth += {"{": 1, "}": -1}.get(text[i], 0)
        i += 1
    return re.sub(r"\s+", "", _strip_comments(text[m.end():i - 1]))


def engine_source_problems(texts: dict[str, str]) -> list[str]:
    """Every mismatch between this model and the engine source, as text.
    `texts` maps a file name under crates/expanse/src to its contents, so the
    self-test can hand in a mutated copy and watch the check turn red."""
    problems = []
    env: dict[str, int] = {}
    for fname, name, want in ENGINE_CONSTS:
        got = _const_value(_strip_comments(texts[fname]), name, env)
        if got is None:
            problems.append(f"{fname}: `pub const {name}: usize` not found or not plain arithmetic")
            continue
        env[name] = got
        if got != want:
            problems.append(f"{fname}: {name} = {got}, model has {want}")
    node = _strip_comments(texts["node.rs"])
    for ty, want in ENGINE_SIZES:
        m = re.search(rf"assert!\(size_of::<{ty}>\(\) == ([^;]+)\);", node)
        got = _arith(m.group(1), env) if m else None
        if got is None:
            problems.append(f"node.rs: no readable `size_of::<{ty}>()` const assert")
        elif got != want:
            problems.append(f"node.rs: size_of::<{ty}>() == {got}, model has {want}")
    for fname, fn, want in ENGINE_BODIES:
        got = _fn_body(texts[fname], fn)
        if got != want:
            problems.append(f"{fname}: `fn {fn}` body is {got!r}, model mirrors {want!r}")
    for fname, needle, want in ENGINE_DEMOTIONS:
        n = _strip_comments(texts[fname]).count(needle)
        if n != want:
            problems.append(
                f"{fname}: `{needle}` appears {n} times outside comments, model expects {want}"
            )
    return problems


def engine_texts(root: Path = REPO) -> dict[str, str]:
    base = root / SRC
    if not base.is_dir():
        raise FileNotFoundError(f"{base}: engine source not found; run from a repository clone")
    return {f: (base / f).read_text() for f in SOURCE_FILES}


def test_engine_sync() -> None:
    """The model's constants against the engine source, then one negative
    control per kind of check: each mutation must be reported (AGENTS.md
    section 5: a scanner is only as good as its mutation test)."""
    texts = engine_texts()
    problems = engine_source_problems(texts)
    assert not problems, "\n".join(problems)
    mutations = (
        ("types.rs", "pub const LEAF_CAP: usize = 32;", "pub const LEAF_CAP: usize = 48;", "LEAF_CAP"),
        ("types.rs", "BRANCHB_TO_L7_DOWN: usize = BRANCH_L7_CAP - 1;",
         "BRANCHB_TO_L7_DOWN: usize = BRANCH_L7_CAP - 2;", "BRANCHB_TO_L7_DOWN"),
        ("node.rs", "size_of::<BranchU>() == 4096 + CACHE_LINE",
         "size_of::<BranchU>() == 8192 + CACHE_LINE", "BranchU"),
        ("node.rs", "size_of::<BranchB>() == 2 * CACHE_LINE",
         "size_of::<BranchB>() == 3 * CACHE_LINE", "BranchB"),
        ("leaf.rs", "} else if pop <= 24 {\n        24", "} else if pop <= 24 {\n        28", "cap_class"),
        ("mutate.rs", "7 / kb as usize", "6 / kb as usize", "map_immed_max"),
        ("mutate.rs", "if digits <= crate::types::BRANCHB_TO_L7_DOWN {",
         "if digits < crate::types::BRANCHB_TO_L7_DOWN {",
         "digits <= crate::types::BRANCHB_TO_L7_DOWN"),
        # A demotion that survives only in a comment must not count.
        ("mutate_map.rs", "if !is_l3 && num < BRANCH_L3_CAP {",
         "if false { // num < BRANCH_L3_CAP", "num < BRANCH_L3_CAP"),
    )
    for fname, old, new, expect in mutations:
        assert old in texts[fname], f"negative control out of date: {old!r} not in {fname}"
        bad = dict(texts)
        bad[fname] = texts[fname].replace(old, new, 1)
        found = engine_source_problems(bad)
        assert any(expect in p for p in found), f"mutation {old!r} -> {new!r} not reported: {found}"


def test_pins() -> None:
    """Reference values; a drift on either side fails here."""
    assert [cap_class(p) for p in (0, 1, 2, 3, 16, 17, 24, 25, 32, 33)] == [0, 1, 2, 4, 16, 24, 24, 32, 32, 36]
    # The two arms, derived from LEAF_CAP (§2.1 invariant 6).
    assert (H1_THRESHOLD, WIDE_THRESHOLD) == (31, 24)
    assert cap_class(WIDE_THRESHOLD) == WIDE_THRESHOLD  # the wide arm sits on a class top
    assert cap_class(H1_THRESHOLD) == 32  # H1's first evaluation packs 31 keys into 32 slots
    # Under the strict reading "evaluate only when P == cap_class(P)", both arms
    # would first evaluate at 24 and every point after would coincide: the arms
    # are then indistinguishable. `evaluation_points` adds the threshold itself.
    assert class_tops(H1_THRESHOLD) == class_tops(WIDE_THRESHOLD) == [24, 16, 12, 8, 4, 2, 1]
    assert evaluation_points(H1_THRESHOLD) == [31, 24, 16, 12, 8, 4, 2, 1]
    assert evaluation_points(WIDE_THRESHOLD) == [24, 16, 12, 8, 4, 2, 1]
    assert (band_width(H1_THRESHOLD), band_width(WIDE_THRESHOLD)) == (2, 9)
    assert rebuilds_per_op(H1_THRESHOLD) == 0.5
    assert abs(rebuilds_per_op(WIDE_THRESHOLD) - 1 / 9) < 1e-15
    # Node sizes the model charges (node.rs const asserts, read by test_engine_sync).
    assert (EDGE, L3_BYTES, L7_BYTES, B_BYTES, U_BYTES) == (16, 64, 128, 128, 4160)
    # ARCHITECTURE §3.5: a Leaf6 at p = 32 is 6 * 32 = 192 B, 208 B with its edge.
    # remove_retention.rs `model_pins` reads 192 from the engine (mem_used delta).
    assert packed_leaf_bytes(32, 6, "set") == 192 and packed_leaf_bytes(32, 6, "set") + EDGE == 208
    assert packed_leaf_bytes(32, 6, "map") == 14 * 32 == 448
    # The 33rd key cascades: 33 single-key level-5 children, BranchB, groups
    # [5,4,4,4,4,4,4,4] -> subarrays 8 + 7 * 4 = 36 edges: 704 B, 720 B with
    # its edge; the same for a map (single-key immediates hold the value inline).
    pops, digs = spread(33)
    assert branch_subtree_bytes(pops, digs, 5, "set") == 128 + 36 * 16 == 704
    assert branch_subtree_bytes(pops, digs, 5, "map") == 704
    assert branch_subtree_bytes(pops, digs, 5, "set") + EDGE == 720
    # A drained BranchB (kept by hysteresis down to 7 digits) at P = 20:
    # groups [3,3,3,3,2,2,2,2] -> 4 * 4 + 4 * 2 = 24 edges.
    pops, digs = spread(20)
    assert branch_subtree_bytes(pops, digs, 5, "set", "B") == 128 + 24 * 16 == 512
    assert packed_leaf_bytes(20, 6, "set") == 6 * 24 == 144
    assert condense_saves(144, 512) and not condense_saves(512, 512)
    # Map: single-key immediates carry the value inline, so the branch costs
    # the same; the packed leaf carries 8 B of value per slot.
    assert branch_subtree_bytes(pops, digs, 5, "map", "B") == 512
    assert packed_leaf_bytes(20, 6, "map") == 14 * 24 == 336
    # Immediates: two six-byte keys fit one set edge (15 // 6), one map edge (7 // 6).
    assert packed_leaf_bytes(2, 6, "set") == 0 and immed_max(6, "map") == 1
    # The worst key-count-only condense, i.e. packed leaf over branch subtree.
    # Set: an L3 at level 3 over three full two-byte immediates (7 keys each):
    # 64 B, against 21 keys packed as a Leaf3, 3 * 24 = 72 -> 80 B.
    ratio, shape = worst_leaf_over_branch("set")
    assert ratio == 1.25 and shape == (3, "L3", [7, 7, 7], 80, 64), (ratio, shape)
    ratio, shape = worst_leaf_over_branch("set", with_edge=True)
    assert ratio == 1.2 and shape == (3, "L3", [7, 7, 7], 96, 80), (ratio, shape)
    # Map: an L3 at level 7 over a 16-key Leaf6 (14 * 16 = 224 B) and a
    # single-key immediate: 288 B, against 17 keys packed as a Leaf7,
    # 15 * 24 = 360 -> 368 B. 1.2778 without the edge, 1.2632 with it (the
    # "up to 1.26x" of the research pass). remove_retention.rs `model_pins`
    # builds this shape on the engine and reads 288 and 368.
    ratio, shape = worst_leaf_over_branch("map")
    assert round(ratio, 4) == 1.2778 and shape == (7, "L3", [16, 1], 368, 288), (ratio, shape)
    ratio, shape = worst_leaf_over_branch("map", with_edge=True)
    assert round(ratio, 4) == 1.2632 and shape == (7, "L3", [16, 1], 384, 304), (ratio, shape)
    # G-thrash: H is the arm's distance below LEAF_CAP; one cycle is 2(H + 1) ops.
    assert (hysteresis(H1_THRESHOLD), hysteresis(WIDE_THRESHOLD)) == (1, 8)
    assert all(band_width(t) == hysteresis(t) + 1 for t in (H1_THRESHOLD, WIDE_THRESHOLD))
    assert thrash_bound(1000.0, 3000.0, 1000.0, 1) == 1000.0 + 4000.0 / 4 == 2000.0
    assert thrash_bound(1000.0, 3000.0, 1000.0, 8) == 1000.0 + 4000.0 / 18
    assert thrash_bound(0.0, 0.0, 0.0, 0) == 0.0
    assert isolated_cost(1_500, 500, 10) == 100.0 and isolated_cost(7, 7, 1) == 0.0
    # Drain model at the Step 0a headline cell (3.2M -> 1M, 64-bit).
    s = predicted_retention(3_200_000, 1_000_000, "set")
    mp = predicted_retention(3_200_000, 1_000_000, "map")
    assert round(s["lambda_n"], 4) == 48.8281 and round(s["lambda_m"], 4) == 15.2588
    assert round(s["p_cascaded"], 4) == 0.9931, s["p_cascaded"]
    assert round(s["r"], 3) == 3.291, s["r"]
    assert round(mp["r"], 3) == 1.697, mp["r"]
    assert round(s["bpk_fresh"], 2) == 8.21, s["bpk_fresh"]
    assert round(mp["bpk_fresh"], 2) == 17.58, mp["bpk_fresh"]
    assert round(s["bpk_drained"], 2) == 27.02, s["bpk_drained"]
    # The byte rule accepts every drained level-6 shape of single-key children
    # at P <= H1_THRESHOLD, both flavours, so the arm predictions below may
    # assume a condense wherever an arm evaluates.
    for flavor in FLAVORS:
        for p in range(1, H1_THRESHOLD + 1):
            pops, digs = spread(p)
            sub = branch_subtree_bytes(pops, digs, 5, flavor, drained_form(p))
            assert condense_saves(packed_leaf_bytes(p, 6, flavor), sub), (flavor, p)
    # The two arms on the headline cell and on 3.2M -> 2M, where the wide arm
    # leaves every expanse that ends at 25..=32 keys as a branch.
    def rc(n, m, flavor, t):
        return round(predicted_retention(n, m, flavor, threshold=t)["r_condensed"], 3)
    assert rc(3_200_000, 1_000_000, "set", H1_THRESHOLD) == 1.000
    assert rc(3_200_000, 1_000_000, "map", H1_THRESHOLD) == 1.000
    assert rc(3_200_000, 1_000_000, "set", WIDE_THRESHOLD) == 1.048
    assert rc(3_200_000, 1_000_000, "map", WIDE_THRESHOLD) == 1.013
    assert rc(3_200_000, 2_000_000, "set", H1_THRESHOLD) == 1.068
    assert rc(3_200_000, 2_000_000, "map", H1_THRESHOLD) == 1.091
    assert rc(3_200_000, 2_000_000, "set", WIDE_THRESHOLD) == 1.511
    assert rc(3_200_000, 2_000_000, "map", WIDE_THRESHOLD) == 1.290
    # No retention without a cascade to retain: 1M -> 312.5k (lambda_n = 15.26).
    assert abs(predicted_retention(1_000_000, 312_500, "set")["r"] - 1.0) < 0.01
    # Input validation is a ValueError, never a silent number.
    for bad in (
        lambda: cap_class(-1),
        lambda: child_bytes(0, 6, "set"),
        lambda: child_bytes(5, 0, "set"),
        lambda: child_bytes(33, 6, "set"),
        lambda: child_bytes(30, 1, "map"),
        lambda: immed_max(3, "blob"),
        lambda: branch_node_bytes([1] * 8, "L3"),
        lambda: branch_subtree_bytes([1, 1], [4, 4], 5, "set"),
        lambda: evaluation_points(0),
        lambda: evaluation_points(LEAF_CAP + 1),
        lambda: predicted_retention(10, 20, "set"),
        lambda: predicted_retention(20, 10, "set", threshold=0),
        lambda: condense_saves(-1, 0),
        lambda: thrash_bound(-1.0, 0.0, 0.0, 1),
        lambda: thrash_bound(0.0, 0.0, 0.0, LEAF_CAP),
        lambda: isolated_cost(1, 2, 1),
        lambda: isolated_cost(2, 1, 0),
        lambda: hysteresis(0),
        lambda: worst_leaf_over_branch("blob"),
    ):
        try:
            bad()
        except ValueError:
            pass
        else:
            raise AssertionError("invalid input must raise")


def main(argv: list[str]) -> int:
    test_engine_sync()
    test_pins()
    if "--self-test" in argv:
        print("condense_bounds: self-test OK")
        return 0
    print(f"arms: H1 threshold {H1_THRESHOLD} (band {band_width(H1_THRESHOLD)}), "
          f"wide threshold {WIDE_THRESHOLD} (band {band_width(WIDE_THRESHOLD)})")
    print(f"evaluation points: H1 {evaluation_points(H1_THRESHOLD)}  wide {evaluation_points(WIDE_THRESHOLD)}")
    for flavor in FLAVORS:
        for with_edge in (False, True):
            ratio, shape = worst_leaf_over_branch(flavor, with_edge)
            label = "with edge" if with_edge else "no edge"
            print(f"worst packed-leaf / branch ({flavor}, {label}): {ratio:.4f} at "
                  f"(level, form, child pops, leaf B, branch B) = {shape}")
    print("\nsubtree held as a drained branch of single-key children vs packed (B/key)")
    print("level flavor   P  form  branch  leaf  ratio")
    for flavor in FLAVORS:
        for level in (6, 4, 3):
            for p, form in ((31, "B"), (24, "B"), (16, "B"), (8, "B"), (7, "L7"), (3, "L3")):
                pops, digs = spread(p)
                b = branch_subtree_bytes(pops, digs, level - 1, flavor, form)
                leaf = packed_leaf_bytes(p, level, flavor)
                ratio = f"{b / leaf:6.2f}" if leaf else "   imm"  # packed to an immediate: 0 B
                print(f"{level:5} {flavor:6} {p:3}  {form:4} {b / p:7.2f} {leaf / p:5.2f} {ratio}")
    print("\npredicted drained / fresh (uniform random 64-bit): main, H1 arm, wide arm")
    for n, m in ((3_200_000, 1_000_000), (4_000_000, 1_000_000), (3_200_000, 2_000_000), (3_200_000, 320_000),
                 (2_000_000, 1_000_000), (1_000_000, 312_500)):
        for flavor in FLAVORS:
            r = predicted_retention(n, m, flavor)
            h1 = predicted_retention(n, m, flavor, threshold=H1_THRESHOLD)["r_condensed"]
            wide = predicted_retention(n, m, flavor, threshold=WIDE_THRESHOLD)["r_condensed"]
            print(f"  {n:>9} -> {m:>9} {flavor}: R = {r['r']:.3f}  "
                  f"({r['bpk_drained']:.2f} vs {r['bpk_fresh']:.2f} B/key)  H1 {h1:.3f}  wide {wide:.3f}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
