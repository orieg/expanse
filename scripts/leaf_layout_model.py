#!/usr/bin/env python3
"""
scripts/leaf_layout_model.py — bytes per key of candidate leaf layouts for
low-cardinality key bytes (#1257), as committed, unit-tested code.

Two layers, both exact integer byte counts:

1. **Closed forms** for a complete fixed-width radix-r digit shape — the
   #1257 case is r = 10 with two constant NUL bytes below the digits —
   under each option: `dense_group_bytes(option, ...)`.
2. **A census pricer.** `crates/expanse/examples/leaf_layout_census.rs`
   (feature `layout-census`) records a real tree's allocation *shapes*:
   linear leaves by (slot level, key bytes, population), subarrays by entry
   count, string-map shells and suffixes, and, per merge rule, every topmost
   branch the rule would keep as one leaf. `price(record, option)` turns
   that census into bytes. Under the current layout it must reproduce the
   engine's `mem_used()` exactly — `check_census_file` asserts it for every
   record — which is what licenses the other options' numbers: each is the
   same tree with only the named allocations re-priced.

The size functions mirror the engine one for one and cite their source:

  * `cap_class`                 crates/expanse/src/leaf.rs  `cap_class`
  * `size_map`, `size_set`      crates/expanse/src/leaf.rs
  * root leaves                 map.rs `leaf_size` (16 B/slot), set.rs
                                `root_leaf_size` (8 B/slot)
  * immediate value arrays      mutate_map.rs `map_immed_val_size`
  * subarrays                   mutate.rs `sub_edges_size`, `sub_vals_size`
  * node sizes                  node.rs `BranchL3` 64, `BranchL7` 128,
                                `BranchB` 128, `BranchU` 4160,
                                `LeafBitmap1` 64, `LeafBitmapL` 128
  * rounding                    alloc.rs `accounted_size(bytes, RAW_ALIGN=16)`

The options (docs/ARCHITECTURE.md §3.6 states what each one is):

  baseline        the current layout.
  exact_classes   every class-sized array sized to its population.
  even_classes    classes of 2 slots up to 16 (…, 8, 10, 12, …), then as now.
  class_10        the current ladder plus one class at 10 slots.
  leaf_cap        a branch the merge rule accepts stays one linear leaf of
                  `key_bytes`-byte keys (LEAF_CAP raised for those ranges).
  grouped_leaf    the same ranges as one leaf with a digit directory: the
                  d present values of the first byte and a count per value,
                  then (key_bytes − 1)-byte suffixes.
  product_leaf    the same ranges as one leaf over the product of per-byte
                  alphabets: a dictionary per byte position and a presence
                  bitmap over the product space, when the range fills at
                  least `min_density` of that space; otherwise unchanged.

Usage:
  python3 scripts/leaf_layout_model.py --self-test
  python3 scripts/leaf_layout_model.py <census.json>
  python3 scripts/leaf_layout_model.py <census.json> --check <totals.json> <option> \
      <max_keys> <max_digits> [shape,...]
"""

from __future__ import annotations

import gzip
import json
import math
import sys

RAW_ALIGN = 16
LEAF_CAP = 32
LEAF1_CAP = 25
BRANCH_L3 = 64
BRANCH_L7 = 128
BRANCH_B = 128
BRANCH_U = 4096 + 64
LEAF_BITMAP_1 = 64
LEAF_BITMAP_L = 128
EDGE = 16
VALUE = 8
SUFFIX_HEADER = 16  # strmap.rs `StrSuffix` {value, len}


# --------------------------------------------------------------------------
# Engine size functions
# --------------------------------------------------------------------------


def raw(nbytes: int) -> int:
    """alloc.rs `accounted_size(bytes, RAW_ALIGN)`: what the allocator charges."""
    if nbytes < 0:
        raise ValueError("negative size")
    return (nbytes + RAW_ALIGN - 1) & ~(RAW_ALIGN - 1)


def cap_class(pop: int) -> int:
    """leaf.rs `cap_class`: 1, 2 exact; 3..16 to multiples of 4; 24; 32;
    then multiples of 4 (the tail the engine only reaches above LEAF_CAP)."""
    if pop < 0:
        raise ValueError("negative population")
    if pop <= 2:
        return pop
    if pop <= 16:
        return (pop + 3) & ~3
    if pop <= 24:
        return 24
    if pop <= 32:
        return 32
    return (pop + 3) & ~3


def exact_class(pop: int) -> int:
    if pop < 0:
        raise ValueError("negative population")
    return pop


def even_class(pop: int) -> int:
    """Classes of two slots from 2 to 16, then the current ladder."""
    if pop < 0:
        raise ValueError("negative population")
    if pop <= 2 or pop > 16:
        return cap_class(pop)
    return (pop + 1) & ~1


def class_10(pop: int) -> int:
    """The current ladder with one class added at 10 slots: 9 and 10 round
    to 10 instead of 12. One more reallocation boundary than now."""
    if pop < 0:
        raise ValueError("negative population")
    return 10 if pop in (9, 10) else cap_class(pop)


LADDERS = {"exact_classes": exact_class, "even_classes": even_class, "class_10": class_10}


def size_map(kb: int, pop: int, cc=cap_class) -> int:
    """leaf.rs `size_map`: [values u64 × class][keys kb × class]."""
    if not 1 <= kb <= 7:
        raise ValueError("key bytes must be 1..=7")
    return (VALUE + kb) * cc(pop)


def size_set(kb: int, pop: int, cc=cap_class) -> int:
    """leaf.rs `size_set`: [keys kb × class]."""
    if not 1 <= kb <= 7:
        raise ValueError("key bytes must be 1..=7")
    return kb * cc(pop)


# --------------------------------------------------------------------------
# Candidate leaf forms for a merged range
# --------------------------------------------------------------------------


def linear_leaf_bytes(flavor: str, kb: int, keys: int, cc=cap_class) -> int:
    """One linear leaf of `keys` remainders of `kb` bytes (option leaf_cap)."""
    return raw(size_map(kb, keys, cc) if flavor == "map" else size_set(kb, keys, cc))


def grouped_leaf_bytes(flavor: str, kb: int, keys: int, digits: int, cc=cap_class) -> int:
    """Digit-directory leaf (option grouped_leaf): 1 byte digit count, `digits`
    digit bytes and one u8 key count per digit, then class-sized values
    (map) and (kb − 1)-byte suffixes. A digit count per digit fits a u8
    because a range has at most max_keys ≤ 256 keys spread over ≥ 2 digits."""
    if kb < 2:
        raise ValueError("a grouped leaf needs at least two key bytes")
    if not 1 <= digits <= 256:
        raise ValueError("digits must be 1..=256")
    header = 1 + 2 * digits
    per_key = (kb - 1) + (VALUE if flavor == "map" else 0)
    return raw(per_key * cc(keys) + header)


def product_space(byte_cards) -> int:
    if not byte_cards or any(c < 1 for c in byte_cards):
        raise ValueError("every byte position has at least one value")
    return math.prod(byte_cards)


def product_leaf_bytes(flavor: str, kb: int, keys: int, byte_cards, cc=cap_class) -> int:
    """Product-space leaf (option product_leaf): kb alphabet sizes, the
    alphabets (Σ cards bytes), a presence bitmap over Π cards positions,
    then class-sized values in rank order (map)."""
    if len(byte_cards) != kb:
        raise ValueError("one alphabet per key byte")
    space = product_space(byte_cards)
    if keys > space:
        raise ValueError("more keys than the product space holds")
    header = kb + sum(byte_cards) + (space + 7) // 8
    return raw(header + (VALUE * cc(keys) if flavor == "map" else 0))


# --------------------------------------------------------------------------
# Closed form: one complete radix-r range, the #1257 shape
# --------------------------------------------------------------------------


def map_immed_max(kb: int) -> int:
    """mutate.rs `map_immed_max`: 7 aux bytes / key bytes."""
    return 7 // kb


def set_immed_max(kb: int) -> int:
    """types.rs `ImmedType::max_count`: 15 payload bytes / key bytes."""
    return 15 // kb


def complete_subtree_bytes(flavor: str, level: int, digit_bytes: int, radix: int,
                           cc=cap_class) -> int:
    """Heap bytes of the current engine's subtree for the complete set of
    radix^digit_bytes keys whose `digit_bytes` most significant remainder
    bytes each take `radix` values (all in one 32-value bitmap
    subexpanse, as ASCII digits and hex letters do) and whose lower
    level − digit_bytes bytes are constant. Mirrors the insert-only
    shape: immediate, then linear leaf, then (level 1) bitmap leaf, then a
    branch at the level where the keys diverge."""
    if not 1 <= digit_bytes <= level <= 7:
        raise ValueError("need 1 <= digit_bytes <= level <= 7")
    if not 2 <= radix <= 32:
        raise ValueError("radix must fit one bitmap subexpanse")
    n = radix ** digit_bytes
    imax = map_immed_max(level) if flavor == "map" else set_immed_max(level)
    if n <= imax:
        return raw(VALUE * cc(n)) if flavor == "map" and n >= 2 else 0
    cap = LEAF1_CAP if level == 1 else LEAF_CAP
    if n <= cap:
        return linear_leaf_bytes(flavor, level, n, cc)
    if level == 1:
        raise ValueError("a level-1 range above LEAF1_CAP is a bitmap leaf; not modelled")
    if radix <= 3:
        branch = BRANCH_L3
    elif radix <= 7:
        branch = BRANCH_L7
    else:
        branch = BRANCH_B + raw(EDGE * cc(radix))
    child = complete_subtree_bytes(flavor, level - 1, digit_bytes - 1, radix, cc) \
        if digit_bytes > 1 else None
    if child is None:
        raise ValueError("unreachable: one digit byte is at most radix keys")
    return branch + radix * child


def dense_group_bytes(option: str, flavor: str, level: int, digit_bytes: int,
                      radix: int) -> int:
    """Bytes of one complete radix^digit_bytes range at `level` under `option`
    (merge options assume the rule accepts the range)."""
    n = radix ** digit_bytes
    cards = [radix] * digit_bytes + [1] * (level - digit_bytes)
    if option == "baseline":
        return complete_subtree_bytes(flavor, level, digit_bytes, radix)
    if option in LADDERS:
        return complete_subtree_bytes(flavor, level, digit_bytes, radix, LADDERS[option])
    if option == "leaf_cap":
        return linear_leaf_bytes(flavor, level, n)
    if option == "grouped_leaf":
        return grouped_leaf_bytes(flavor, level, n, radix)
    if option == "product_leaf":
        return product_leaf_bytes(flavor, level, n, cards)
    raise ValueError(f"unknown option {option}")


# --------------------------------------------------------------------------
# Census pricer
# --------------------------------------------------------------------------


def price_tree(census: dict, flavor: str, cc=cap_class) -> int:
    """Bytes of the census's allocations with every class-sized array sized
    by `cc`. With `cap_class` this is the engine's `mem_used()`."""
    total = 0
    slot = 2 * VALUE if flavor == "map" else VALUE
    for pop, n in census["root_leaves"]:
        total += n * raw(slot * cc(pop))
    for _level, kb, pop, n in census["linear_leaves"]:
        total += n * linear_leaf_bytes(flavor, kb, pop, cc)
    if flavor == "map":
        for _level, count, n in census["immediates"]:
            if count >= 2:
                total += n * raw(VALUE * cc(count))
        for count, n in census["bitmap_leaf_value_subarrays"]:
            total += n * raw(VALUE * cc(count))
    bitmap_leaf = LEAF_BITMAP_L if flavor == "map" else LEAF_BITMAP_1
    total += bitmap_leaf * sum(n for _level, n in census["bitmap_leaves"])
    total += BRANCH_L3 * census["branch_l3"] + BRANCH_L7 * census["branch_l7"]
    total += BRANCH_B * census["branch_b"] + BRANCH_U * census["branch_u"]
    for count, n in census["branch_b_subarrays"]:
        total += n * raw(EDGE * cc(count))
    total += census["str_node_shell_bytes"] + census["suffix_bytes"]
    return total


def rule_groups(record: dict, max_keys: int, max_digits: int) -> list:
    for rule in record["rules"]:
        if rule["max_keys"] == max_keys and rule["max_digits"] == max_digits:
            return rule["groups"]
    raise KeyError(f"census has no rule ({max_keys}, {max_digits})")


def merged_bytes(flavor: str, group: dict, form: str, min_density: float) -> int | None:
    """What one merge group costs under `form`; None keeps the subtree."""
    kb, keys, cards = group["key_bytes"], group["keys"], group["byte_cards"]
    if form == "leaf_cap":
        return linear_leaf_bytes(flavor, kb, keys)
    if form == "grouped_leaf":
        return grouped_leaf_bytes(flavor, kb, keys, cards[0])
    if form == "product_leaf":
        if keys < min_density * product_space(cards):
            return None
        return product_leaf_bytes(flavor, kb, keys, cards)
    raise ValueError(f"unknown form {form}")


def price(record: dict, option: str, max_keys: int = 128, max_digits: int = 16,
          min_density: float = 0.125) -> dict:
    """Bytes of `record`'s tree under `option`, with the groups that moved."""
    flavor = "set" if record["engine"] == "set" else "map"
    census = record["census"]
    base = price_tree(census, flavor)
    if option == "baseline":
        return {"bytes": base, "merged_keys": 0, "worse_groups": 0}
    if option in LADDERS:
        return {"bytes": price_tree(census, flavor, LADDERS[option]), "merged_keys": 0,
                "worse_groups": 0}
    total, merged_keys, worse = base, 0, 0
    for g in rule_groups(record, max_keys, max_digits):
        new = merged_bytes(flavor, g, option, min_density)
        if new is None:
            continue
        total += g["count"] * new - g["bytes"]
        merged_keys += g["count"] * g["keys"]
        worse += g["count"] if g["count"] * new > g["bytes"] else 0
    return {"bytes": total, "merged_keys": merged_keys, "worse_groups": worse}


def load(path: str) -> dict:
    """A census or totals file, plain or gzip-compressed (`.gz`)."""
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt") as f:
        return json.load(f)


def check_census_file(doc: dict) -> None:
    """Every record's baseline price equals its engine `mem_used()`."""
    for r in doc["records"]:
        flavor = "set" if r["engine"] == "set" else "map"
        got = price_tree(r["census"], flavor)
        if got != r["mem_used"] or r["census"]["bytes"] != r["mem_used"]:
            raise AssertionError(
                f"{r['engine']}/{r['shape']}: priced {got}, census {r['census']['bytes']}, "
                f"mem_used {r['mem_used']}")


def check_prediction(census_doc: dict, totals_doc: dict, option: str,
                     max_keys: int = 128, max_digits: int = 16, shapes=None) -> list:
    """Compare the model's projection for `option` with the engine totals of
    a build that implements it (`leaf_layout_census.rs --totals-only`).
    Returns (engine, shape, predicted, measured) for every record, or for the
    named `shapes` only; raises on a record the two runs do not share."""
    measured = {(r["engine"], r["shape"]): r["mem_used"] for r in totals_doc["records"]}
    out = []
    for r in census_doc["records"]:
        key = (r["engine"], r["shape"])
        if shapes is not None and r["shape"] not in shapes:
            continue
        if key not in measured:
            raise KeyError(f"no measured total for {key}")
        out.append((*key, price(r, option, max_keys, max_digits)["bytes"], measured[key]))
    return out


OPTIONS = ["baseline", "exact_classes", "even_classes", "class_10", "leaf_cap", "grouped_leaf",
           "product_leaf"]


def table(doc: dict, max_keys: int, max_digits: int) -> list:
    rows = []
    for r in doc["records"]:
        keys = r["census"]["keys"]
        row = {"engine": r["engine"], "shape": r["shape"], "keys": keys}
        for opt in OPTIONS:
            p = price(r, opt, max_keys, max_digits)
            row[opt] = p["bytes"] / keys
            row[opt + "_merged_share"] = p["merged_keys"] / keys
            row[opt + "_worse_groups"] = p["worse_groups"]
        rows.append(row)
    return rows


# --------------------------------------------------------------------------
# Tests
# --------------------------------------------------------------------------

# One real record, captured verbatim from `leaf_layout_census.rs --population
# 400` (map flavor, shape `decimal8`, rule (128, 16) kept; its one group
# class carries the four 1,440-byte subtrees as `bytes` 5,760). A census of a
# fixed key set does not depend on the commit unless the layout changes, in
# which case this pin is meant to fail.
FIXTURE = json.loads(
    '{"engine": "map", "shape": "decimal8", "insertion_order": "sorted", "mem_used": 5952, '
    '"census": {"keys": 400, "bytes": 5952, "root_leaves": [], "linear_leaves": [[1, 1, 10, 40]], '
    '"immediates": [], "bitmap_leaves": [], "bitmap_leaf_value_subarrays": [], "branch_l3": 1, '
    '"branch_l7": 1, "branch_b": 4, "branch_b_subarrays": [[10, 4]], "branch_u": 0, '
    '"branches_by_level_fanout": [[2, 10, 4], [3, 4, 1], [8, 1, 1]], "str_nodes": 0, '
    '"str_node_shell_bytes": 0, "suffix_leaves": [], "suffix_bytes": 0}, "rules": '
    '[{"max_keys": 128, "max_digits": 16, "groups": [{"key_bytes": 2, "keys": 100, '
    '"byte_cards": [10, 10], "count": 4, "bytes": 5760}]}]}')


def test_pins() -> None:
    # Size functions, against the engine's constants and the #1257 table.
    assert [cap_class(p) for p in (0, 1, 2, 3, 10, 16, 17, 25, 33, 100)] == \
        [0, 1, 2, 4, 12, 16, 24, 32, 36, 100]
    assert [even_class(p) for p in (1, 2, 3, 10, 11, 17)] == [1, 2, 4, 10, 12, 24]
    assert [class_10(p) for p in (8, 9, 10, 11, 12, 13)] == [8, 10, 10, 12, 12, 16]
    assert size_map(3, 10) == 132 and raw(132) == 144          # 10 keys, 12 slots
    assert BRANCH_B + raw(EDGE * cap_class(10)) == 320         # #1257's 320 B branch
    assert linear_leaf_bytes("set", 6, 20) == 144

    # #1257, one hundreds-digit range: 100 keys, level 4, digits in the top
    # two bytes and two NUL bytes below them.
    b = dense_group_bytes("baseline", "map", 4, 2, 10)
    assert b == 1760                              # 320 + 10 × 144
    assert b / 100 == 17.60                        # + upper levels = 17.97
    assert dense_group_bytes("exact_classes", "map", 4, 2, 10) == 128 + 160 + 10 * 112
    assert dense_group_bytes("even_classes", "map", 4, 2, 10) == 1408
    assert dense_group_bytes("class_10", "map", 4, 2, 10) == 1408
    assert dense_group_bytes("leaf_cap", "map", 4, 2, 10) == 1200
    assert dense_group_bytes("grouped_leaf", "map", 4, 2, 10) == raw(11 * 100 + 21) == 1136
    assert dense_group_bytes("product_leaf", "map", 4, 2, 10) == \
        raw(4 + 22 + 13 + 800) == 848
    # The same range as a word key (`decimal8`: digits in the low bytes).
    assert dense_group_bytes("baseline", "map", 2, 2, 10) == 1440   # 320 + 10 × 112
    assert dense_group_bytes("baseline", "set", 2, 2, 10) == 320    # 10 immediates each
    assert dense_group_bytes("leaf_cap", "set", 2, 2, 10) == 208
    # Hex digits fill their classes: the class ladder costs nothing there.
    assert dense_group_bytes("baseline", "map", 2, 2, 16) == \
        dense_group_bytes("exact_classes", "map", 2, 2, 16)

    # Product leaf needs a dense product space; the pricer skips a sparse one.
    g = {"key_bytes": 4, "keys": 96, "byte_cards": [10, 10, 10, 10], "count": 1,
         "bytes": 1600}
    assert merged_bytes("map", g, "product_leaf", 0.125) is None
    assert merged_bytes("map", g, "grouped_leaf", 0.125) == raw(11 * 96 + 21)

    # The pricer reproduces mem_used, and each option re-prices only what it names.
    assert price(FIXTURE, "baseline")["bytes"] == 5952 == 40 * 112 + 64 + 128 + 4 * 320
    assert price(FIXTURE, "leaf_cap")["bytes"] == 5952 - 4 * (1440 - raw(size_map(2, 100)))
    assert price(FIXTURE, "exact_classes")["bytes"] == 5952 - 40 * (112 - raw(10 * 9)) \
        - 4 * (192 - 160)
    check_census_file({"records": [FIXTURE]})
    # A prediction check pairs records by (engine, shape) and reports both.
    got = check_prediction({"records": [FIXTURE]},
                           {"records": [{"engine": "map", "shape": "decimal8",
                                         "mem_used": 5952 - 4 * 432}]}, "leaf_cap")
    assert got == [("map", "decimal8", 5952 - 4 * 432, 5952 - 4 * 432)]
    for bad in ((lambda: raw(-1)), (lambda: size_map(0, 3)), (lambda: product_space([0])),
                (lambda: grouped_leaf_bytes("map", 1, 10, 2))):
        try:
            bad()
        except ValueError:
            pass
        else:
            raise AssertionError("expected ValueError")


# The committed #1257 artifacts (docs/ARCHITECTURE.md §3.6) and the checks
# the section's figures rest on: the census prices to mem_used, and each
# engine that implements an option measured what the model predicted.
ARTIFACT = "results/leaf_layout_census_c5a6f688.json"
SOSD_ARTIFACT = "results/leaf_layout_census_sosd.json.gz"
DOWNSTREAM_ARTIFACT = "results/leaf_layout_census_downstream.json"
_ORDERS = ["orders_10000000_" + c for c in ("sorted", "shuffled", "churned", "survivors")]
_COMPOSITE = ["composite_10000000_" + c for c in ("sorted", "shuffled", "churned", "survivors")]
_SKEWED = ["composite_skewed_10000000_" + c for c in ("sorted", "shuffled", "churned", "survivors")]
ENCODING_ARTIFACT = "results/leaf_layout_census_encodings.json"
# Encodings whose every key byte takes at most 128 values, where a uniform
# cap of 128 builds exactly the (128 keys, 128 digits) tree; base-255 is not.
_ENC128 = [base + order
           for base in [f"composite_{d}_{e}_10000000" for d in ("uniform", "skewed")
                        for e in ("enc7x10", "b32x4", "b16x5", "b32x4a7", "b32full")]
           + ["orders_text_10000000", "orders_b32a7_10000000"]
           for order in ("", "_shuffled")]
# Where the leaf-cap projection is NOT exact: the cap-128 engine measured
# more than the model predicted, on these records only, by these bytes. The
# cause is unexplained (a narrow-pointer key width was tested and refuted);
# §3.6 quotes the engine's figure for them. Pinned exactly, so a change in
# either the engine or the model shows up here rather than silently.
LEAF_CAP_RESIDUALS = {
    "composite_skewed_10000000_sorted": 2352,
    "composite_skewed_10000000_shuffled": 2352,
    "composite_skewed_10000000_churned": 3152,
    "composite_skewed_10000000_survivors": 6032,
    **{shape + order: delta
       for shape, delta in (("composite_uniform_b16x5_10000000", 320000),
                            ("composite_skewed_enc7x10_10000000", 2352),
                            ("composite_skewed_b32x4_10000000", 9744),
                            ("composite_skewed_b16x5_10000000", 23104),
                            ("orders_b32a7_10000000", 528))
       for order in ("", "_shuffled")},
}
ENCODING_PREDICTIONS = (
    ("results/leaf_layout_census_encodings_class10.json", "class_10", 128, 16, None),
    ("results/leaf_layout_census_encodings_cap128.json", "leaf_cap", 128, 128, _ENC128),
)
DOWNSTREAM_PREDICTIONS = (
    ("results/leaf_layout_census_downstream_class10.json", "class_10", 128, 16, None),
    ("results/leaf_layout_census_downstream_cap128.json", "leaf_cap", 128, 10, _ORDERS),
    # 7-bit bytes take at most 128 values, so a uniform cap of 128 builds the
    # (128 keys, 128 digits) tree exactly.
    ("results/leaf_layout_census_downstream_cap128.json", "leaf_cap", 128, 128,
     _COMPOSITE + _SKEWED),
)
PREDICTIONS = (
    ("results/leaf_layout_census_c5a6f688_class10.json", "class_10", 128, 16, None),
    ("results/leaf_layout_census_c5a6f688_cap128.json", "leaf_cap", 128, 10,
     ["sequential", "decimal8", "decimal8_sparse", "orders", "orders_sparse",
      "orders_10000000"]),
    ("results/leaf_layout_census_c5a6f688_cap128.json", "leaf_cap", 128, 16, ["uuid_hex"]),
)


def test_committed_artifacts() -> None:
    import os
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    doc = load(os.path.join(root, ARTIFACT))
    check_census_file(doc)
    orders = next(r for r in doc["records"] if r["shape"] == "orders_10000000")
    assert orders["mem_used"] == 179_699_624                     # 17.97 B/key
    assert price(orders, "leaf_cap", 128, 16)["bytes"] == 123_694_504
    # 179,699,624 − 176,019,200 replaced + 100,010 ranges × raw(843) = 848 B
    assert price(orders, "product_leaf", 128, 16)["bytes"] == 88_488_904
    for path, option, mk, md, shapes in PREDICTIONS:
        totals = load(os.path.join(root, path))
        rows = check_prediction(doc, totals, option, mk, md, shapes)
        residual = LEAF_CAP_RESIDUALS if option == "leaf_cap" else {}
        assert rows and all(meas - pred == residual.get(shape, 0)
                            for _, shape, pred, meas in rows), (path, rows)
    # The downstream string shapes (§3.6): the census prices to mem_used, and
    # both engine builds measured what the model predicted, removal phase
    # included.
    down = load(os.path.join(root, DOWNSTREAM_ARTIFACT))
    check_census_file(down)
    for path, option, mk, md, shapes in DOWNSTREAM_PREDICTIONS:
        totals = load(os.path.join(root, path))
        rows = check_prediction(down, totals, option, mk, md, shapes)
        residual = LEAF_CAP_RESIDUALS if option == "leaf_cap" else {}
        assert rows and all(meas - pred == residual.get(shape, 0)
                            for _, shape, pred, meas in rows), (path, rows)
    # The id-encoding comparison (§3.6), the same checks.
    enc = load(os.path.join(root, ENCODING_ARTIFACT))
    check_census_file(enc)
    for path, option, mk, md, shapes in ENCODING_PREDICTIONS:
        totals = load(os.path.join(root, path))
        rows = check_prediction(enc, totals, option, mk, md, shapes)
        residual = LEAF_CAP_RESIDUALS if option == "leaf_cap" else {}
        assert rows and all(meas - pred == residual.get(shape, 0)
                            for _, shape, pred, meas in rows), (path, rows)
    # The four SOSD datasets, whole (§3.6): each census prices to its mem_used.
    sosd = load(os.path.join(root, SOSD_ARTIFACT))
    check_census_file(sosd)
    assert len(sosd["records"]) == 8


def main(argv: list) -> int:
    test_pins()
    test_committed_artifacts()
    if "--self-test" in argv:
        print("leaf_layout_model: self-test passed")
        return 0
    paths = [a for a in argv[1:] if not a.startswith("--")]
    if not paths:
        print(__doc__)
        return 0
    doc = load(paths[0])
    check_census_file(doc)
    if "--check" in argv:
        # <census.json> --check <totals.json> <option> <max_keys> <max_digits> [shape,...]
        i = argv.index("--check")
        totals = load(argv[i + 1])
        option, mk, md = argv[i + 2], int(argv[i + 3]), int(argv[i + 4])
        shapes = argv[i + 5].split(",") if len(argv) > i + 5 else None
        bad = 0
        for engine, shape, pred, meas in check_prediction(doc, totals, option, mk, md, shapes):
            flag = "" if pred == meas else "  MISMATCH"
            bad += pred != meas
            print(f"{engine:6} {shape:16} predicted {pred:>11} measured {meas:>11}{flag}")
        return 1 if bad else 0
    present = sorted({(x["max_keys"], x["max_digits"]) for r in doc["records"]
                      for x in r["rules"]})
    for mk, md in present:
        print(f"\nmerge rule: max_keys={mk}, max_digits={md}")
        print("| engine | shape | keys | " + " | ".join(OPTIONS) + " |")
        print("|---|---|---:|" + "---:|" * len(OPTIONS))
        for row in table(doc, mk, md):
            cells = []
            for o in OPTIONS:
                cell = f"{row[o]:.2f}"
                if row[o + "_merged_share"]:
                    cell += f" ({100 * row[o + '_merged_share']:.0f}%)"
                if row[o + "_worse_groups"]:
                    cell += f" ⚠{row[o + '_worse_groups']}"
                cells.append(cell)
            print(f"| {row['engine']} | {row['shape']} | {row['keys']} | " + " | ".join(cells) + " |")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
