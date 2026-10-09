#!/usr/bin/env python3
"""
scripts/check_abi_parity.py — Automated C ABI Symbol Parity Linter for Expanse.

Verifies 100% symbol and feature parity of the modern libexpanse C API
(`include/expanse.h`) across all target language bindings:
  1. Java Panama FFM (`bindings/java/src/main/java/io/github/orieg/expanse/internal/ExpanseNative.java`)
  2. .NET C# P/Invoke (`bindings/dotnet/src/Expanse.NET/Native/NativeMethods.cs`)
  3. Python PyO3 (`crates/expanse-py/src/`)
  4. Node.js N-API (`crates/expanse-node/src/`)
  5. Go purego (`bindings/go/`)

Java, .NET and Go call the C ABI, so their coverage is the symbol name itself.
Python and Node bind the Rust API, so theirs is a per-symbol
`// abi-parity: <symbol>` marker at the implementing site (see the comment
above PYTHON_FEATURE_MAPPING).

Enforces that the exported C ABI symbol count satisfies the pinned floor
(baseline: MIN_C_SYMBOLS = 100). The floor constant is verified against the base
ref (e.g. `origin/main`); any decrease in the constant or reduction in declared
symbols requires an explicit `allow-symbol-shrink: <reason>` directive in the PR
body. The zero margin (exactly 100 symbols vs floor of 100) is deliberate: the
first legitimate deprecation trips the floor and requires an explicit rationale.

Usage:
  python3 scripts/check_abi_parity.py [--check] [--verbose] [--json] [--markdown] [--base origin/main]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Tuple


def get_repo_root() -> Path:
    """Returns the repository root directory."""
    return Path(__file__).resolve().parent.parent


@dataclass
class CSymbol:
    name: str
    return_type: str
    signature: str
    category: str
    line_number: int
    # Declared inside a `#if !EXPANSE_WIDE_SURFACE` block: present only in a
    # 32-bit libexpanse. The bindings target 64-bit hosts, so these are
    # reported but excluded from binding coverage (#578).
    narrow_only: bool = False


@dataclass
class ParityReport:
    total_c_symbols: int
    java_covered: Set[str] = field(default_factory=set)
    java_missing: Set[str] = field(default_factory=set)
    java_deferred: Set[str] = field(default_factory=set)
    dotnet_covered: Set[str] = field(default_factory=set)
    dotnet_missing: Set[str] = field(default_factory=set)
    dotnet_deferred: Set[str] = field(default_factory=set)
    python_covered: Set[str] = field(default_factory=set)
    python_missing: Set[str] = field(default_factory=set)
    python_deferred: Set[str] = field(default_factory=set)
    node_covered: Set[str] = field(default_factory=set)
    node_missing: Set[str] = field(default_factory=set)
    node_deferred: Set[str] = field(default_factory=set)
    # Marker/mapping inconsistencies (a marker naming an unknown symbol, or
    # sitting in a file its mapping does not name). Always fatal.
    python_errors: List[str] = field(default_factory=list)
    node_errors: List[str] = field(default_factory=list)
    go_covered: Set[str] = field(default_factory=set)
    go_missing: Set[str] = field(default_factory=set)
    go_deferred: Set[str] = field(default_factory=set)
    narrow_only: List[str] = field(default_factory=list)
    category_breakdown: Dict[str, Dict[str, int]] = field(default_factory=dict)


# Python and Node do not call the C ABI: they bind the Rust API directly, so
# there is no symbol name to find in their sources. Coverage is therefore
# declared at the implementing site, by a line comment naming the symbol:
#
#     // abi-parity: expanse_map_first
#     pub fn first(&self) -> Option<(u64, u64)> { ... }
#
# Each mapping below says which binding file must carry that marker. A symbol
# is covered only when its own name appears in a marker in that file; a word
# elsewhere in the file never counts. The mappings previously listed generic
# name tokens ("new", "get", "prev", ...) and accepted any of them anywhere in
# the file, so a new symbol mapped to ["prev"] read as covered by an unrelated
# `prev` method while nothing bound it. `self_test` plants exactly that case.
#
# Whether a marker sits on the function that really provides the capability is
# a review judgement; the check guarantees only that the claim is explicit,
# per symbol, and in the named file.

# Python PyO3: C ABI symbol -> file under crates/expanse-py/src carrying its marker.
PYTHON_FEATURE_MAPPING: Dict[str, str] = {
    # Identity
    "expanse_version": "lib.rs",

    # Set (20 functions)
    "expanse_set_new": "set.rs",
    "expanse_set_free": "set.rs",
    "expanse_set_insert": "set.rs",
    "expanse_set_remove": "set.rs",
    "expanse_set_contains": "set.rs",
    "expanse_set_len": "set.rs",
    "expanse_set_mem_used": "set.rs",
    "expanse_set_mem_held": "set.rs",
    "expanse_set_shrink_to_fit": "set.rs",
    "expanse_set_clear": "set.rs",
    "expanse_set_first": "set.rs",
    "expanse_set_last": "set.rs",
    "expanse_set_next_at_or_after": "set.rs",
    "expanse_set_next_after": "set.rs",
    "expanse_set_prev_at_or_before": "set.rs",
    "expanse_set_prev_before": "set.rs",
    "expanse_set_count_below": "set.rs",
    "expanse_set_count_range": "set.rs",
    "expanse_set_by_count": "set.rs",
    "expanse_set_contains_batch": "set.rs",
    "expanse_set_validate": "set.rs",

    # Map (25 functions)
    "expanse_map_new": "map.rs",
    "expanse_map_free": "map.rs",
    "expanse_map_insert": "map.rs",
    "expanse_map_get": "map.rs",
    "expanse_map_contains": "map.rs",
    "expanse_map_get_batch": "map.rs",
    "expanse_map_remove": "map.rs",
    "expanse_map_len": "map.rs",
    "expanse_map_mem_used": "map.rs",
    "expanse_map_mem_held": "map.rs",
    "expanse_map_shrink_to_fit": "map.rs",
    "expanse_map_clear": "map.rs",
    "expanse_map_validate": "map.rs",
    "expanse_map_validate_explain": "map.rs",
    "expanse_map_slot": "map.rs",
    "expanse_map_ins_slot": "map.rs",
    "expanse_map_first": "map.rs",
    "expanse_map_last": "map.rs",
    "expanse_map_next_at_or_after": "map.rs",
    "expanse_map_next_after": "map.rs",
    "expanse_map_prev_at_or_before": "map.rs",
    "expanse_map_prev_before": "map.rs",
    "expanse_map_count_below": "map.rs",
    "expanse_map_count_range": "map.rs",
    "expanse_map_by_count": "map.rs",

    # BytesMap (12 functions)
    "expanse_bytesmap_new": "bytesmap.rs",
    "expanse_bytesmap_free": "bytesmap.rs",
    "expanse_bytesmap_insert": "bytesmap.rs",
    "expanse_bytesmap_get": "bytesmap.rs",
    "expanse_bytesmap_contains": "bytesmap.rs",
    "expanse_bytesmap_remove": "bytesmap.rs",
    "expanse_bytesmap_slot": "bytesmap.rs",
    "expanse_bytesmap_ins_slot": "bytesmap.rs",
    "expanse_bytesmap_len": "bytesmap.rs",
    "expanse_bytesmap_mem_used": "bytesmap.rs",
    "expanse_bytesmap_clear": "bytesmap.rs",
    "expanse_bytesmap_for_each": "bytesmap.rs",

    # StrMap (19 functions)
    "expanse_strmap_new": "strmap.rs",
    "expanse_strmap_free": "strmap.rs",
    "expanse_strmap_insert": "strmap.rs",
    "expanse_strmap_get": "strmap.rs",
    "expanse_strmap_contains": "strmap.rs",
    "expanse_strmap_remove": "strmap.rs",
    "expanse_strmap_slot": "strmap.rs",
    "expanse_strmap_ins_slot": "strmap.rs",
    "expanse_strmap_len": "strmap.rs",
    "expanse_strmap_mem_used": "strmap.rs",
    "expanse_strmap_mem_held": "strmap.rs",
    "expanse_strmap_shrink_to_fit": "strmap.rs",
    "expanse_strmap_clear": "strmap.rs",
    "expanse_strmap_first": "strmap.rs",
    "expanse_strmap_last": "strmap.rs",
    "expanse_strmap_next_at_or_after": "strmap.rs",
    "expanse_strmap_next_after": "strmap.rs",
    "expanse_strmap_prev_at_or_before": "strmap.rs",
    "expanse_strmap_prev_before": "strmap.rs",

    # StrMap truncation-aware navigation (6 functions)
    "expanse_strmap_first_ex": "strmap.rs",
    "expanse_strmap_last_ex": "strmap.rs",
    "expanse_strmap_next_at_or_after_ex": "strmap.rs",
    "expanse_strmap_next_after_ex": "strmap.rs",
    "expanse_strmap_prev_at_or_before_ex": "strmap.rs",
    "expanse_strmap_prev_before_ex": "strmap.rs",

    # SyncSet (11 functions)
    "expanse_sync_set_new": "sync.rs",
    "expanse_sync_set_free": "sync.rs",
    "expanse_sync_set_insert": "sync.rs",
    "expanse_sync_set_remove": "sync.rs",
    "expanse_sync_set_contains": "sync.rs",
    "expanse_sync_set_len": "sync.rs",
    "expanse_sync_set_mem_held": "sync.rs",
    "expanse_sync_set_shrink_to_fit": "sync.rs",
    "expanse_sync_set_reader_new": "sync.rs",
    "expanse_sync_set_reader_free": "sync.rs",
    "expanse_sync_set_reader_contains": "sync.rs",

    # SyncMap (18 functions)
    "expanse_sync_map_new": "sync.rs",
    "expanse_sync_map_free": "sync.rs",
    "expanse_sync_map_insert": "sync.rs",
    "expanse_sync_map_get": "sync.rs",
    "expanse_sync_map_remove": "sync.rs",
    "expanse_sync_map_len": "sync.rs",
    "expanse_sync_map_mem_used": "sync.rs",
    "expanse_sync_map_mem_held": "sync.rs",
    "expanse_sync_map_shrink_to_fit": "sync.rs",
    "expanse_sync_map_reader_new": "sync.rs",
    "expanse_sync_map_reader_free": "sync.rs",
    "expanse_sync_map_reader_get": "sync.rs",
    "expanse_sync_map_reader_first": "sync.rs",
    "expanse_sync_map_reader_last": "sync.rs",
    "expanse_sync_map_reader_next_at_or_after": "sync.rs",
    "expanse_sync_map_reader_next_after": "sync.rs",
    "expanse_sync_map_reader_prev_at_or_before": "sync.rs",
    "expanse_sync_map_reader_prev_before": "sync.rs",

    # BlobMap (12 functions)
    "expanse_blob_map_new": "blobmap.rs",
    "expanse_blob_map_new_with_capacity": "blobmap.rs",
    "expanse_blob_map_set_reclaim_at_cap": "blobmap.rs",
    "expanse_blob_map_insert_ex": "blobmap.rs",
    "expanse_blob_map_arena_stats": "blobmap.rs",
    "expanse_blob_map_free": "blobmap.rs",
    "expanse_blob_map_insert": "blobmap.rs",
    "expanse_blob_map_remove": "blobmap.rs",
    "expanse_blob_map_get": "blobmap.rs",
    "expanse_blob_map_get_into": "blobmap.rs",
    "expanse_blob_map_scan_filtered": "blobmap.rs",
    "expanse_blob_map_compact": "blobmap.rs",
    "expanse_blob_map_len": "blobmap.rs",
    "expanse_blob_map_mem_used": "blobmap.rs",
    "expanse_blob_map_clear": "blobmap.rs",
    "expanse_blob_map_contains_key": "blobmap.rs",
    "expanse_blob_map_contains": "blobmap.rs",
}

# Node.js N-API: C ABI symbol -> file under crates/expanse-node/src carrying its marker.
NODE_FEATURE_MAPPING: Dict[str, str] = {
    # Identity
    "expanse_version": "lib.rs",

    # Set (20 functions)
    "expanse_set_new": "set.rs",
    "expanse_set_free": "set.rs",
    "expanse_set_insert": "set.rs",
    "expanse_set_remove": "set.rs",
    "expanse_set_contains": "set.rs",
    "expanse_set_len": "set.rs",
    "expanse_set_mem_used": "set.rs",
    "expanse_set_mem_held": "set.rs",
    "expanse_set_shrink_to_fit": "set.rs",
    "expanse_set_clear": "set.rs",
    "expanse_set_first": "set.rs",
    "expanse_set_last": "set.rs",
    "expanse_set_next_at_or_after": "set.rs",
    "expanse_set_next_after": "set.rs",
    "expanse_set_prev_at_or_before": "set.rs",
    "expanse_set_prev_before": "set.rs",
    "expanse_set_count_below": "set.rs",
    "expanse_set_count_range": "set.rs",
    "expanse_set_by_count": "set.rs",
    "expanse_set_contains_batch": "set.rs",
    "expanse_set_validate": "set.rs",

    # Map (25 functions)
    "expanse_map_new": "map.rs",
    "expanse_map_free": "map.rs",
    "expanse_map_insert": "map.rs",
    "expanse_map_get": "map.rs",
    "expanse_map_contains": "map.rs",
    "expanse_map_get_batch": "map.rs",
    "expanse_map_remove": "map.rs",
    "expanse_map_len": "map.rs",
    "expanse_map_mem_used": "map.rs",
    "expanse_map_mem_held": "map.rs",
    "expanse_map_shrink_to_fit": "map.rs",
    "expanse_map_clear": "map.rs",
    "expanse_map_validate": "map.rs",
    "expanse_map_validate_explain": "map.rs",
    "expanse_map_slot": "map.rs",
    "expanse_map_ins_slot": "map.rs",
    "expanse_map_first": "map.rs",
    "expanse_map_last": "map.rs",
    "expanse_map_next_at_or_after": "map.rs",
    "expanse_map_next_after": "map.rs",
    "expanse_map_prev_at_or_before": "map.rs",
    "expanse_map_prev_before": "map.rs",
    "expanse_map_count_below": "map.rs",
    "expanse_map_count_range": "map.rs",
    "expanse_map_by_count": "map.rs",

    # BytesMap (12 functions)
    "expanse_bytesmap_new": "bytesmap.rs",
    "expanse_bytesmap_free": "bytesmap.rs",
    "expanse_bytesmap_insert": "bytesmap.rs",
    "expanse_bytesmap_get": "bytesmap.rs",
    "expanse_bytesmap_contains": "bytesmap.rs",
    "expanse_bytesmap_remove": "bytesmap.rs",
    "expanse_bytesmap_slot": "bytesmap.rs",
    "expanse_bytesmap_ins_slot": "bytesmap.rs",
    "expanse_bytesmap_len": "bytesmap.rs",
    "expanse_bytesmap_mem_used": "bytesmap.rs",
    "expanse_bytesmap_clear": "bytesmap.rs",
    "expanse_bytesmap_for_each": "bytesmap.rs",

    # StrMap (19 functions)
    "expanse_strmap_new": "strmap.rs",
    "expanse_strmap_free": "strmap.rs",
    "expanse_strmap_insert": "strmap.rs",
    "expanse_strmap_get": "strmap.rs",
    "expanse_strmap_contains": "strmap.rs",
    "expanse_strmap_remove": "strmap.rs",
    "expanse_strmap_slot": "strmap.rs",
    "expanse_strmap_ins_slot": "strmap.rs",
    "expanse_strmap_len": "strmap.rs",
    "expanse_strmap_mem_used": "strmap.rs",
    "expanse_strmap_mem_held": "strmap.rs",
    "expanse_strmap_shrink_to_fit": "strmap.rs",
    "expanse_strmap_clear": "strmap.rs",
    "expanse_strmap_first": "strmap.rs",
    "expanse_strmap_last": "strmap.rs",
    "expanse_strmap_next_at_or_after": "strmap.rs",
    "expanse_strmap_next_after": "strmap.rs",
    "expanse_strmap_prev_at_or_before": "strmap.rs",
    "expanse_strmap_prev_before": "strmap.rs",

    # StrMap truncation-aware navigation (6 functions)
    "expanse_strmap_first_ex": "strmap.rs",
    "expanse_strmap_last_ex": "strmap.rs",
    "expanse_strmap_next_at_or_after_ex": "strmap.rs",
    "expanse_strmap_next_after_ex": "strmap.rs",
    "expanse_strmap_prev_at_or_before_ex": "strmap.rs",
    "expanse_strmap_prev_before_ex": "strmap.rs",

    # SyncSet (11 functions)
    "expanse_sync_set_new": "sync.rs",
    "expanse_sync_set_free": "sync.rs",
    "expanse_sync_set_insert": "sync.rs",
    "expanse_sync_set_remove": "sync.rs",
    "expanse_sync_set_contains": "sync.rs",
    "expanse_sync_set_len": "sync.rs",
    "expanse_sync_set_mem_held": "sync.rs",
    "expanse_sync_set_shrink_to_fit": "sync.rs",
    "expanse_sync_set_reader_new": "sync.rs",
    "expanse_sync_set_reader_free": "sync.rs",
    "expanse_sync_set_reader_contains": "sync.rs",

    # SyncMap (18 functions)
    "expanse_sync_map_new": "sync.rs",
    "expanse_sync_map_free": "sync.rs",
    "expanse_sync_map_insert": "sync.rs",
    "expanse_sync_map_get": "sync.rs",
    "expanse_sync_map_remove": "sync.rs",
    "expanse_sync_map_len": "sync.rs",
    "expanse_sync_map_mem_used": "sync.rs",
    "expanse_sync_map_mem_held": "sync.rs",
    "expanse_sync_map_shrink_to_fit": "sync.rs",
    "expanse_sync_map_reader_new": "sync.rs",
    "expanse_sync_map_reader_free": "sync.rs",
    "expanse_sync_map_reader_get": "sync.rs",
    "expanse_sync_map_reader_first": "sync.rs",
    "expanse_sync_map_reader_last": "sync.rs",
    "expanse_sync_map_reader_next_at_or_after": "sync.rs",
    "expanse_sync_map_reader_next_after": "sync.rs",
    "expanse_sync_map_reader_prev_at_or_before": "sync.rs",
    "expanse_sync_map_reader_prev_before": "sync.rs",

    # BlobMap (12 functions)
    "expanse_blob_map_new": "blobmap.rs",
    "expanse_blob_map_new_with_capacity": "blobmap.rs",
    "expanse_blob_map_set_reclaim_at_cap": "blobmap.rs",
    "expanse_blob_map_insert_ex": "blobmap.rs",
    "expanse_blob_map_arena_stats": "blobmap.rs",
    "expanse_blob_map_free": "blobmap.rs",
    "expanse_blob_map_insert": "blobmap.rs",
    "expanse_blob_map_remove": "blobmap.rs",
    "expanse_blob_map_get": "blobmap.rs",
    "expanse_blob_map_get_into": "blobmap.rs",
    "expanse_blob_map_scan_filtered": "blobmap.rs",
    "expanse_blob_map_compact": "blobmap.rs",
    "expanse_blob_map_len": "blobmap.rs",
    "expanse_blob_map_mem_used": "blobmap.rs",
    "expanse_blob_map_clear": "blobmap.rs",
    "expanse_blob_map_contains_key": "blobmap.rs",
    "expanse_blob_map_contains": "blobmap.rs",
}


# Stated deferrals for C ABI symbols intentionally pending binding support.
# Every entry MUST specify:
#   - issue: The issue or PR tracking the binding implementation.
#   - reason: A non-empty rationale explaining why the symbol is deferred.
#   - ecosystems: Set of ecosystem identifiers where deferral applies
#                 ('java', 'dotnet', 'python', 'node', 'go').
STATED_DEFERRALS: Dict[str, Dict[str, Any]] = {
    "expanse_ordered_bytesmap_new": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_free": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_insert": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_get": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_contains": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_remove": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_slot": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_ins_slot": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_len": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_mem_used": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_mem_held": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_shrink_to_fit": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_clear": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_first": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_last": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_next_at_or_after": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_next_after": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_prev_at_or_before": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
    "expanse_ordered_bytesmap_prev_before": {
        "issue": "#1397",
        "reason": "the C ABI landed in v0.11.0 (#808); the language bindings are tracked in #1397",
        "ecosystems": {"python", "node", "go"},
    },
}

def validate_stated_deferrals(
    deferrals: Dict[str, Dict[str, Any]], wide_symbols: List[CSymbol]
) -> List[str]:
    """Validates the integrity of stated deferral entries.

    Returns a list of error messages (empty if all entries are valid).
    """
    errors: List[str] = []
    all_header_symbols = {s.name for s in wide_symbols}
    valid_ecos = {"java", "dotnet", "python", "node", "go"}
    for sym_name, entry in sorted(deferrals.items()):
        if sym_name not in all_header_symbols:
            errors.append(
                f"STATED_DEFERRALS names {sym_name}, which is not a wide-surface symbol of include/expanse.h"
            )
        issue = entry.get("issue")
        if not issue or not (isinstance(issue, str) and (issue.startswith("#") or issue.startswith("http"))):
            errors.append(f"STATED_DEFERRALS[{sym_name}] must cite an issue or PR (found: {issue!r})")
        reason = entry.get("reason")
        if not reason or not (isinstance(reason, str) and len(reason.strip()) > 0):
            errors.append(f"STATED_DEFERRALS[{sym_name}] must carry a non-empty rationale")
        ecos = entry.get("ecosystems")
        if not ecos or not isinstance(ecos, (set, list, tuple)) or len(ecos) == 0:
            errors.append(f"STATED_DEFERRALS[{sym_name}] must declare at least one ecosystem")
        elif not set(ecos).issubset(valid_ecos):
            errors.append(f"STATED_DEFERRALS[{sym_name}] has invalid ecosystems: {set(ecos) - valid_ecos}")
    return errors


def stale_deferrals(
    deferred_by_ecosystem: Dict[str, Set[str]],
    deferrals: Optional[Dict[str, Dict[str, Any]]] = None,
) -> List[str]:
    """Deferral entries that excuse nothing: the ecosystem already binds the symbol.

    `split_deferred` consults an entry only for a symbol that is missing, so
    an entry left behind after its binding lands is never read. It would then
    excuse that symbol silently if the binding later lost it. An entry is live
    only while every ecosystem it names is actually missing the symbol.
    """
    if deferrals is None:
        deferrals = STATED_DEFERRALS
    errors: List[str] = []
    for sym_name, entry in sorted(deferrals.items()):
        for eco in sorted(entry.get("ecosystems", ())):
            if sym_name not in deferred_by_ecosystem.get(eco, set()):
                errors.append(
                    f"STATED_DEFERRALS[{sym_name}] lists {eco}, which already binds it; remove the entry"
                )
    return errors


def issue_state(issue: str) -> str:
    """`OPEN`, `CLOSED` or `UNKNOWN` for a `#N` reference, read through `gh`."""
    m = re.fullmatch(r"#(\d+)", issue or "")
    if not m:
        return "UNKNOWN"
    try:
        res = subprocess.run(
            ["gh", "issue", "view", m.group(1), "--json", "state", "--jq", ".state"],
            capture_output=True, text=True, check=False,
        )
    except OSError:
        return "UNKNOWN"
    state = res.stdout.strip().upper() if res.returncode == 0 else ""
    return state if state in ("OPEN", "CLOSED") else "UNKNOWN"


def deferral_issue_problems(
    deferrals: Dict[str, Dict[str, Any]], state_of=issue_state
) -> Tuple[List[str], List[str]]:
    """Checks that every deferral's tracking issue is still open.

    A deferral is an exemption granted against a promise to do the work, and
    the cited issue is that promise. Once it is closed nothing tracks the
    missing binding, so the exemption no longer stands.

    Returns `(errors, unverified)`: an error per closed issue, naming the
    symbols it was excusing, and the issues whose state could not be read (no
    `gh`, no token, offline). The caller reports the second list and does not
    fail on it (AGENTS.md section 8.11.5).
    """
    by_issue: Dict[str, List[str]] = {}
    for sym_name, entry in sorted(deferrals.items()):
        by_issue.setdefault(str(entry.get("issue")), []).append(sym_name)
    errors: List[str] = []
    unverified: List[str] = []
    for issue, symbols in sorted(by_issue.items()):
        state = state_of(issue)
        if state == "CLOSED":
            errors.append(
                f"STATED_DEFERRALS cites {issue}, which is closed, for {len(symbols)} symbol(s) "
                f"({symbols[0]}{', ...' if len(symbols) > 1 else ''}): bind them or cite an open issue"
            )
        elif state != "OPEN":
            unverified.append(issue)
    return errors, unverified


def split_deferred(
    raw_missing: Set[str],
    ecosystem: str,
    deferrals: Optional[Dict[str, Dict[str, Any]]] = None,
) -> Tuple[Set[str], Set[str]]:
    """Splits raw missing symbols into (deferred, truly_missing) for the given ecosystem.

    A symbol is deferred iff it is present in `deferrals` and lists `ecosystem`
    in its declared target ecosystems.
    """
    if deferrals is None:
        deferrals = STATED_DEFERRALS
    deferred = {
        s
        for s in raw_missing
        if s in deferrals and ecosystem in deferrals[s].get("ecosystems", ())
    }
    missing = raw_missing - deferred
    return deferred, missing


# `#if !EXPANSE_WIDE_SURFACE` / `#if EXPANSE_WIDE_SURFACE == 0` open the
# 32-bit-only surface block in expanse.h.
_NARROW_IF_RE = re.compile(r"^#if\s*(?:!\s*EXPANSE_WIDE_SURFACE|EXPANSE_WIDE_SURFACE\s*==\s*0)\b")


def parse_c_header(header_path: Path) -> List[CSymbol]:
    """Parses C function declarations from expanse.h."""
    text = header_path.read_text(encoding="utf-8")
    lines = text.splitlines()

    symbols: List[CSymbol] = []
    current_category = "General"

    # Regex for C function declarations like:
    # bool expanse_set_insert(expanse_set_t *set, uint64_t key);
    # const char *expanse_version(void);
    # uint64_t *expanse_map_slot(expanse_map_t *map, uint64_t key);
    # size_t expanse_blob_map_scan_filtered(...);
    
    # We will iterate line by line or collapse multi-line signatures
    sig_buffer = ""
    start_line = 0
    # Preprocessor nesting: a stack of booleans, True for the `#if` block
    # that opens the 32-bit-only surface. Any enclosing True marks a
    # declaration narrow-only.
    if_stack: List[bool] = []

    for idx, raw_line in enumerate(lines, start=1):
        line = raw_line.strip()

        if line.startswith("#if"):
            if_stack.append(bool(_NARROW_IF_RE.match(line)))
            continue
        if line.startswith("#endif"):
            if if_stack:
                if_stack.pop()
            continue
        if line.startswith("#else") or line.startswith("#elif"):
            if if_stack:
                if_stack[-1] = False
            continue

        # Check for category comments
        if line.startswith("/* ----") or line.startswith("/* ---"):
            cat_match = re.search(r"----\s*([A-Za-z0-9_:\s]+?)\s*---", line)
            if cat_match:
                current_category = cat_match.group(1).strip()
            continue

        if not sig_buffer and not line.startswith("/*") and not line.startswith("*") and not line.startswith("//") and not line.startswith("#"):
            if "expanse_" in line:
                sig_buffer = line
                start_line = idx
        elif sig_buffer:
            sig_buffer += " " + line

        if sig_buffer and ";" in sig_buffer:
            # Completed a statement
            sig = sig_buffer[: sig_buffer.index(";") + 1].strip()
            sig_buffer = ""

            clean_sig = re.sub(r'EXPANSE_DEPRECATED\([^)]*\)\s*', '', sig)
            clean_sig = re.sub(r'\s*EXPANSE_NONNULL\([^)]*\)', '', clean_sig)

            # Check if this is a function declaration:
            # (return_type) (expanse_...) (args)
            match = re.match(
                r"^((?:const\s+)?[\w\s\*]+?)\s*\b(expanse_[a-z0-9_]+)\s*\((.*)\)\s*;$",
                clean_sig,
            )
            if match:
                ret_type = match.group(1).strip()
                func_name = match.group(2).strip()
                symbols.append(
                    CSymbol(
                        name=func_name,
                        return_type=ret_type,
                        signature=sig,
                        category=current_category,
                        line_number=start_line,
                        narrow_only=any(if_stack),
                    )
                )

    return symbols


def parse_java_panama(java_path: Path) -> Set[str]:
    """Parses downcall C symbol names in ExpanseNative.java."""
    text = java_path.read_text(encoding="utf-8")
    symbols: Set[str] = set()

    # Matches: downcall("expanse_set_insert", ...)
    matches = re.findall(r'downcall\(\s*"([a-z0-9_]+)"', text)
    symbols.update(matches)

    # Also check MH_ fields
    field_matches = re.findall(r'MH_(expanse_[a-z0-9_]+)', text)
    symbols.update(field_matches)

    return symbols


def parse_dotnet_pinvoke(cs_path: Path) -> Set[str]:
    """Parses P/Invoke EntryPoint symbols in NativeMethods.cs."""
    text = cs_path.read_text(encoding="utf-8")
    symbols: Set[str] = set()

    # Matches: EntryPoint = "expanse_set_insert"
    matches = re.findall(r'EntryPoint\s*=\s*"([a-z0-9_]+)"', text)
    symbols.update(matches)

    # Matches: public static extern ... expanse_set_insert(...)
    method_matches = re.findall(r'public\s+static\s+extern\s+.*?\s+(expanse_[a-z0-9_]+)\s*\(', text)
    symbols.update(method_matches)

    return symbols


# `// abi-parity: expanse_a, expanse_b`. A `///` doc comment is not a marker:
# PyO3 and napi render doc comments into the Python docstring / TypeScript
# declaration, which is no place for a parity claim.
_MARKER_RE = re.compile(r"(?<!/)//[ \t]*abi-parity:[ \t]*([^\n]*)")
_MARKER_SYMBOL_RE = re.compile(r"^expanse_[a-z0-9_]+$")


def parse_parity_markers(text: str) -> Tuple[Set[str], List[str]]:
    """Returns the symbols named by `// abi-parity:` markers, and any malformed entries.

    Entries are compared as whole names, so a marker for `expanse_strmap_first_ex`
    never covers `expanse_strmap_first`.
    """
    names: Set[str] = set()
    malformed: List[str] = []
    for m in _MARKER_RE.finditer(text):
        for part in m.group(1).split(","):
            part = part.strip()
            if _MARKER_SYMBOL_RE.match(part):
                names.add(part)
            else:
                malformed.append(part)
    return names, malformed


def verify_marker_bindings(
    binding_dir: Path,
    c_symbols: List[CSymbol],
    mapping: Dict[str, str],
    label: str,
) -> Tuple[Set[str], Set[str], List[str]]:
    """Coverage of a Rust-API binding (Python, Node) by per-symbol markers.

    A symbol is covered iff `mapping` names a file for it and that file carries
    an `abi-parity` marker naming the symbol exactly. Returns (covered,
    missing, errors); errors are inconsistencies that are fatal whatever the
    coverage: a malformed marker, a marker naming something that is not a
    wide-surface symbol, a marker in a file other than the one its mapping
    names, and a mapping entry for a symbol the header does not declare.
    """
    covered: Set[str] = set()
    missing: Set[str] = set()
    errors: List[str] = []

    markers: Dict[str, Set[str]] = {}
    for rs_file in sorted(binding_dir.glob("*.rs")):
        names, malformed = parse_parity_markers(rs_file.read_text(encoding="utf-8"))
        markers[rs_file.name] = names
        for bad in malformed:
            errors.append(f"{label}: {rs_file.name}: malformed abi-parity marker entry {bad!r}")

    declared = {s.name for s in c_symbols}
    for sym in c_symbols:
        rs_filename = mapping.get(sym.name)
        if rs_filename is not None and sym.name in markers.get(rs_filename, set()):
            covered.add(sym.name)
        else:
            missing.add(sym.name)

    for rs_filename, names in sorted(markers.items()):
        for name in sorted(names):
            if name not in declared:
                errors.append(
                    f"{label}: {rs_filename}: abi-parity marker names {name}, "
                    "which is not a wide-surface symbol of include/expanse.h"
                )
            elif name not in mapping:
                errors.append(f"{label}: {rs_filename}: abi-parity marker for {name}, which has no mapping entry")
            elif mapping[name] != rs_filename:
                errors.append(
                    f"{label}: {rs_filename}: abi-parity marker for {name}, "
                    f"but its mapping names {mapping[name]}"
                )
    for name in sorted(set(mapping) - declared):
        errors.append(f"{label}: mapping entry {name} is not a wide-surface symbol of include/expanse.h")

    return covered, missing, errors


def verify_python_bindings(
    py_dir: Path, c_symbols: List[CSymbol], mapping: Optional[Dict[str, str]] = None
) -> Tuple[Set[str], Set[str], List[str]]:
    """Verifies that all C functionality is bound in the Python PyO3 modules."""
    return verify_marker_bindings(py_dir, c_symbols, PYTHON_FEATURE_MAPPING if mapping is None else mapping, "python")


def verify_node_bindings(
    node_dir: Path, c_symbols: List[CSymbol], mapping: Optional[Dict[str, str]] = None
) -> Tuple[Set[str], Set[str], List[str]]:
    """Verifies that all C functionality is bound in the Node.js N-API modules."""
    return verify_marker_bindings(node_dir, c_symbols, NODE_FEATURE_MAPPING if mapping is None else mapping, "node")


def parse_go_purego(go_path: Path) -> Set[str]:
    """Parses C symbol names bound via purego in native_purego.go."""
    if not go_path.exists():
        return set()
    text = go_path.read_text(encoding="utf-8")
    symbols: Set[str] = set()
    matches = re.findall(r'"(expanse_[a-z0-9_]+)"', text)
    symbols.update(matches)
    return symbols


def check_no_dangling_capi_include_references(root: Path) -> List[str]:
    """Checks for dangling references to the deleted crates/expanse-capi/include path (#563)."""
    errors: List[str] = []
    duplicate_header_dir = root / "crates" / "expanse-capi" / "include"
    if duplicate_header_dir.exists():
        errors.append(
            f"Duplicate header directory found: {duplicate_header_dir}. "
            "Canonical public headers must live exclusively in include/ (#563)."
        )

    try:
        # Match both path separators. A PowerShell step spells the same path
        # with backslashes, and a forward-slash-only pattern reports clean over
        # it — which is how a Windows release step and the NuGet package
        # definition both kept referencing the deleted directory (#749).
        # AGENTS.md section 8.11: a gate's pattern is part of the gate.
        res = subprocess.run(
            ["git", "grep", "-n", "-E", r"expanse-capi[/\\]include",
             "--", ".", ":!scripts/check_*.py"],
            cwd=str(root),
            capture_output=True,
            text=True,
        )
        if res.returncode == 0 and res.stdout.strip():
            lines = res.stdout.strip().splitlines()
            errors.append(
                f"Dangling references to deleted 'expanse-capi/include' found ({len(lines)} site(s)):\n"
                + "\n".join(f"  {line}" for line in lines)
                + "\nCanonical public headers must live exclusively in include/ (#563)."
            )
        elif res.returncode > 1 or (res.returncode != 0 and res.returncode != 1):
            errors.append(
                f"dangling-reference check could not run: git grep exited {res.returncode}: {res.stderr.strip()}"
            )
        # res.returncode == 1 -> clean (no matches found)
    except FileNotFoundError:
        errors.append("dangling-reference check could not run: 'git' command not found on PATH")

    return errors


def build_parity_report(root: Path) -> Tuple[List[CSymbol], ParityReport]:
    """Builds the full cross-ecosystem ABI parity report."""
    dangling_errors = check_no_dangling_capi_include_references(root)
    if dangling_errors:
        raise RuntimeError("\n".join(dangling_errors))

    header_path = root / "include" / "expanse.h"

    java_path = (
        root
        / "bindings"
        / "java"
        / "src"
        / "main"
        / "java"
        / "io"
        / "github"
        / "orieg"
        / "expanse"
        / "internal"
        / "ExpanseNative.java"
    )
    dotnet_path = root / "bindings" / "dotnet" / "src" / "Expanse.NET" / "Native" / "NativeMethods.cs"
    py_dir = root / "crates" / "expanse-py" / "src"
    node_dir = root / "crates" / "expanse-node" / "src"
    go_path = root / "bindings" / "go" / "native_purego.go"

    all_symbols = parse_c_header(header_path)
    narrow_only = sorted(s.name for s in all_symbols if s.narrow_only)
    # Bindings run on 64-bit hosts, where the narrow block does not exist:
    # coverage is measured over the wide-or-shared surface only.
    c_symbols = [s for s in all_symbols if not s.narrow_only]
    c_symbol_names = {s.name for s in c_symbols}

    java_symbols = parse_java_panama(java_path)
    dotnet_symbols = parse_dotnet_pinvoke(dotnet_path)
    py_covered, py_raw_missing, py_errors = verify_python_bindings(py_dir, c_symbols)
    node_covered, node_raw_missing, node_errors = verify_node_bindings(node_dir, c_symbols)
    go_symbols = parse_go_purego(go_path)

    java_raw_missing = c_symbol_names - java_symbols
    java_deferred, java_missing = split_deferred(java_raw_missing, "java")

    dotnet_raw_missing = c_symbol_names - dotnet_symbols
    dotnet_deferred, dotnet_missing = split_deferred(dotnet_raw_missing, "dotnet")

    py_deferred, py_missing = split_deferred(py_raw_missing, "python")

    node_deferred, node_missing = split_deferred(node_raw_missing, "node")

    go_raw_missing = c_symbol_names - go_symbols
    go_deferred, go_missing = split_deferred(go_raw_missing, "go")

    report = ParityReport(
        total_c_symbols=len(c_symbols),
        java_covered=c_symbol_names.intersection(java_symbols),
        java_missing=java_missing,
        java_deferred=java_deferred,
        dotnet_covered=c_symbol_names.intersection(dotnet_symbols),
        dotnet_missing=dotnet_missing,
        dotnet_deferred=dotnet_deferred,
        python_covered=py_covered,
        python_missing=py_missing,
        python_deferred=py_deferred,
        node_covered=node_covered,
        node_missing=node_missing,
        node_deferred=node_deferred,
        python_errors=py_errors,
        node_errors=node_errors,
        go_covered=c_symbol_names.intersection(go_symbols),
        go_missing=go_missing,
        go_deferred=go_deferred,
        narrow_only=narrow_only,
    )

    # Category breakdown
    categories: Dict[str, List[CSymbol]] = {}
    for s in c_symbols:
        categories.setdefault(s.category, []).append(s)

    for cat_name, sym_list in categories.items():
        cat_names = {s.name for s in sym_list}
        report.category_breakdown[cat_name] = {
            "total": len(sym_list),
            "java": len(cat_names.intersection(java_symbols)),
            "dotnet": len(cat_names.intersection(dotnet_symbols)),
            "python": len(cat_names.intersection(py_covered)),
            "node": len(cat_names.intersection(node_covered)),
            "go": len(cat_names.intersection(go_symbols)),
        }

    return c_symbols, report


def print_text_report(c_symbols: List[CSymbol], report: ParityReport, verbose: bool = False) -> None:
    """Prints a clean human-readable CLI report."""
    print("================================================================================")
    print("           libexpanse C ABI Multi-Ecosystem Symbol Parity Report                ")
    print("================================================================================")
    print("Canonical C ABI Header: include/expanse.h")
    print(f"Total Declared C Functions: {report.total_c_symbols}")
    if report.narrow_only:
        print(
            f"32-bit-only surface (`!EXPANSE_WIDE_SURFACE`, not bindable from 64-bit hosts, "
            f"excluded from coverage): {len(report.narrow_only)} — {', '.join(report.narrow_only)}"
        )
    print()

    print("--------------------------------------------------------------------------------")
    print(f"{'Ecosystem / Binding Layer':<35} | {'Wrapped':<10} | {'Coverage':<10} | {'Status'}")
    print("--------------------------------------------------------------------------------")

    def format_row(name: str, covered: int, total: int, missing: Set[str], deferred: Set[str]) -> str:
        pct = (covered / total * 100.0) if total > 0 else 100.0
        if not missing:
            if deferred:
                status = f"✓ PASS (100% active, {len(deferred)} deferred)"
            else:
                status = "✓ PASS (100%)"
        else:
            status = f"✗ FAIL ({len(missing)} missing)"
        return f"{name:<35} | {covered:>3}/{total:<5} | {pct:>8.1f}% | {status}"

    print(format_row("Java 22+ (Panama FFM downcalls)", len(report.java_covered), report.total_c_symbols, report.java_missing, report.java_deferred))
    print(format_row(".NET C# (P/Invoke NativeMethods)", len(report.dotnet_covered), report.total_c_symbols, report.dotnet_missing, report.dotnet_deferred))
    print(format_row("Python (PyO3 native classes)", len(report.python_covered), report.total_c_symbols, report.python_missing, report.python_deferred))
    print(format_row("Node.js (N-API native bindings)", len(report.node_covered), report.total_c_symbols, report.node_missing, report.node_deferred))
    print(format_row("Go 1.22+ (purego / cgo)", len(report.go_covered), report.total_c_symbols, report.go_missing, report.go_deferred))
    print("--------------------------------------------------------------------------------\n")

    print("--- Breakdown by Container / Functional Category ---")
    print(f"{'Category':<32} | {'Total':<6} | {'Java':<6} | {'.NET':<6} | {'Python':<6} | {'Node':<6} | {'Go':<6}")
    print("--------------------------------------------------------------------------------")
    for cat_name, counts in report.category_breakdown.items():
        print(f"{cat_name:<32} | {counts['total']:<6} | {counts['java']:<6} | {counts['dotnet']:<6} | {counts['python']:<6} | {counts['node']:<6} | {counts['go']:<6}")
    print("--------------------------------------------------------------------------------\n")

    if verbose or report.java_missing or report.dotnet_missing or report.python_missing or report.node_missing or report.go_missing:
        print("--- Detailed Per-Symbol Coverage Matrix ---")
        header = f"{'Symbol Name':<44} | {'Java':<8} | {'.NET':<8} | {'Python':<8} | {'Node':<8} | {'Go':<8}"
        print(header)
        print("-" * len(header))
        for s in c_symbols:
            def mark(covered: Set[str], deferred: Set[str]) -> str:
                if s.name in covered:
                    return "✓"
                if s.name in deferred:
                    return "DEFERRED"
                return "MISSING"
            j = mark(report.java_covered, report.java_deferred)
            d = mark(report.dotnet_covered, report.dotnet_deferred)
            p = mark(report.python_covered, report.python_deferred)
            n = mark(report.node_covered, report.node_deferred)
            g = mark(report.go_covered, report.go_deferred)
            print(f"{s.name:<44} | {j:<8} | {d:<8} | {p:<8} | {n:<8} | {g:<8}")
        print("--------------------------------------------------------------------------------\n")

    if report.java_missing or report.dotnet_missing or report.python_missing or report.node_missing or report.go_missing:
        print("::error::ABI Parity check failed! Missing symbols detected:")
        if report.java_missing:
            print(f"  Java missing: {sorted(report.java_missing)}")
        if report.dotnet_missing:
            print(f"  .NET missing: {sorted(report.dotnet_missing)}")
        if report.python_missing:
            print(f"  Python missing: {sorted(report.python_missing)}")
        if report.node_missing:
            print(f"  Node.js missing: {sorted(report.node_missing)}")
        if report.go_missing:
            print(f"  Go missing: {sorted(report.go_missing)}")
    else:
        total_deferred = len(report.java_deferred | report.dotnet_deferred | report.python_deferred | report.node_deferred | report.go_deferred)
        if total_deferred > 0:
            print(f"✓ All {report.total_c_symbols - total_deferred} active libexpanse C ABI symbols are 100% covered across Java, .NET, Python, Node.js, and Go ({total_deferred} stated deferrals)!")
        else:
            print(f"✓ All {report.total_c_symbols} libexpanse C ABI symbols are 100% covered across Java, .NET, Python, Node.js, and Go!")


def format_markdown_table(c_symbols: List[CSymbol], report: ParityReport) -> str:
    """Generates GitHub markdown table for docs/COMPAT.md."""
    lines = [
        "| Container / API Family | C Functions | Java 22+ Panama | .NET P/Invoke | Python (PyO3) | Node.js (N-API) | Go (purego) | Feature Parity |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for cat_name, counts in report.category_breakdown.items():
        total = counts["total"]
        j = f"{counts['java']}/{total}"
        d = f"{counts['dotnet']}/{total}"
        p = f"{counts['python']}/{total}"
        n = f"{counts['node']}/{total}"
        g = f"{counts['go']}/{total}"
        status = "100% Full Parity" if counts["java"] == total and counts["dotnet"] == total and counts["python"] == total and counts["node"] == total and counts["go"] == total else "Partial"
        lines.append(f"| `{cat_name}` | {total} | {j} | {d} | {p} | {n} | {g} | {status} |")

    if report.narrow_only:
        lines.append(
            f"| 32-bit-only surface (`!EXPANSE_WIDE_SURFACE`) | {len(report.narrow_only)} | — | — | — | — | — | "
            f"not bindable from 64-bit hosts; excluded from coverage: {', '.join(f'`{n}`' for n in report.narrow_only)} |"
        )
    lines.append(f"| **Total C ABI Symbols** | **{report.total_c_symbols}** | **{len(report.java_covered)}/{report.total_c_symbols}** | **{len(report.dotnet_covered)}/{report.total_c_symbols}** | **{len(report.python_covered)}/{report.total_c_symbols}** | **{len(report.node_covered)}/{report.total_c_symbols}** | **{len(report.go_covered)}/{report.total_c_symbols}** | **100% Complete** |")
    return "\n".join(lines)


MIN_C_SYMBOLS = 100


def parse_allow_symbol_shrink(pr_body: str) -> Optional[str]:
    """Extracts symbol-shrink override reason from a PR body."""
    if not pr_body:
        return None

    pattern = re.compile(
        r"^[ \t]*(?:<!--[ \t]*)?allow-symbol-shrink:[ \t]*([^\n]+)",
        re.IGNORECASE | re.MULTILINE,
    )

    for match in pattern.finditer(pr_body):
        reason = match.group(1).strip()
        reason = re.sub(r"(?:--!?>|`)+\s*$", "", reason).strip()
        if not reason:
            continue
        lower = reason.lower()
        if lower.startswith("<reason>") or lower.startswith("<rationale>"):
            continue
        if lower in ("todo", "tbd", "none", "n/a", "null"):
            continue
        return reason

    return None


def get_base_floor_constant(
    base_ref: str,
    script_rel_path: str = "scripts/check_abi_parity.py",
    var_name: str = "MIN_C_SYMBOLS",
    root: Optional[Path] = None,
) -> Tuple[Optional[int], str]:
    """Reads the floor constant from script_rel_path in base_ref using `git show`.

    Returns (floor_int, "") on success.
    Returns (None, error_message) on any resolution failure, shallow clone failure,
    missing file, or if the constant is not defined/found in the base ref.
    Fails loud — never returns (None, "") to avoid failing open.
    """
    cwd = str(root) if root else None

    # First check if base_ref exists locally. If not, try to fetch it shallowly.
    check_ref = subprocess.run(
        ["git", "rev-parse", "--verify", base_ref],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if check_ref.returncode != 0:
        remote = "origin"
        branch = base_ref
        if base_ref.startswith("origin/"):
            branch = base_ref[len("origin/"):]
        fetch_res = subprocess.run(
            ["git", "fetch", remote, f"{branch}:{base_ref}"],
            cwd=cwd,
            capture_output=True,
            text=True,
        )
        recheck = subprocess.run(
            ["git", "rev-parse", "--verify", base_ref],
            cwd=cwd,
            capture_output=True,
            text=True,
        )
        if recheck.returncode != 0:
            err_details = (
                fetch_res.stderr.strip()
                or check_ref.stderr.strip()
                or f"fatal: ref '{base_ref}' does not exist"
            )
            return None, f"Base ref '{base_ref}' could not be resolved or fetched:\n{err_details}"

    # Read the script content from base_ref
    show_res = subprocess.run(
        ["git", "show", f"{base_ref}:{script_rel_path}"],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if show_res.returncode != 0:
        return (
            None,
            f"Failed to read '{script_rel_path}' from base ref '{base_ref}':\n{show_res.stderr.strip()}",
        )

    # Parse constant
    pattern = re.compile(rf"^[ \t]*{var_name}[ \t]*=[ \t]*(\d+)", re.MULTILINE)
    match = pattern.search(show_res.stdout)
    if not match:
        return (
            None,
            f"Floor constant '{var_name}' not found in '{script_rel_path}' on base ref '{base_ref}'",
        )

    try:
        return int(match.group(1)), ""
    except ValueError as e:
        return None, f"Failed to parse integer floor from '{match.group(1)}': {e}"


def self_test() -> int:
    """Runs internal self-tests for symbol parity, fail-loud git errors, and floor checks."""
    # 1. Override parser tests
    assert parse_allow_symbol_shrink("allow-symbol-shrink: deprecated v1 symbols") == "deprecated v1 symbols"
    assert parse_allow_symbol_shrink("<!-- allow-symbol-shrink: removed legacy sync helpers -->") == "removed legacy sync helpers"
    # `--!>` also closes an HTML comment; left on the reason it let a placeholder through.
    assert parse_allow_symbol_shrink("<!-- allow-symbol-shrink: removed legacy sync helpers --!>") == "removed legacy sync helpers"
    assert parse_allow_symbol_shrink("<!-- allow-symbol-shrink: TODO --!>") is None
    assert parse_allow_symbol_shrink("  allow-symbol-shrink: indented reason") == "indented reason"
    assert parse_allow_symbol_shrink("allow-symbol-shrink: <reason>") is None
    assert parse_allow_symbol_shrink("allow-symbol-shrink: TODO") is None
    assert parse_allow_symbol_shrink("allow-symbol-shrink:") is None
    assert parse_allow_symbol_shrink("mentioning allow-symbol-shrink: mid-sentence") is None

    # 2. Mock C header parse
    mock_header = """
    /* --- Set --- */
    bool expanse_set_insert(expanse_set_t *set, uint64_t key);
    bool expanse_set_remove(expanse_set_t *set, uint64_t key);
    """
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".h", delete=False) as tf:
        tf.write(mock_header)
        tf_name = tf.name
    try:
        symbols = parse_c_header(Path(tf_name))
        assert len(symbols) == 2
        assert symbols[0].name == "expanse_set_insert"
        assert symbols[1].name == "expanse_set_remove"
    finally:
        os.remove(tf_name)

    # 2b. Narrow-surface block: symbols inside `#if !EXPANSE_WIDE_SURFACE`
    # are tagged narrow_only (through nesting) and nothing else is.
    mock_narrow = """
    /* --- Map --- */
    bool expanse_map_get(const expanse_map_t *map, expanse_word_t key, expanse_word_t *out);
    #if !EXPANSE_WIDE_SURFACE
    typedef void (*expanse_map_remove_range_fn)(expanse_word_t key, expanse_word_t value, void *ctx);
    size_t expanse_map_remove_range(expanse_map_t *map, expanse_word_t lo, expanse_word_t hi,
                                    expanse_map_remove_range_fn cb, void *ctx);
    #ifdef SOMETHING_NESTED
    bool expanse_map_nested_probe(expanse_map_t *map);
    #endif
    #endif /* !EXPANSE_WIDE_SURFACE */
    #if EXPANSE_WIDE_SURFACE
    uint64_t expanse_map_count_below(const expanse_map_t *map, uint64_t key);
    #endif
    #if EXPANSE_WIDE_SURFACE == 0
    bool expanse_map_narrow_two(expanse_map_t *map);
    #else
    bool expanse_map_wide_two(expanse_map_t *map);
    #endif
    """
    with tempfile.NamedTemporaryFile("w", suffix=".h", delete=False) as tf:
        tf.write(mock_narrow)
        tf_name = tf.name
    try:
        symbols = parse_c_header(Path(tf_name))
        tags = {sym.name: sym.narrow_only for sym in symbols}
        assert tags == {
            "expanse_map_get": False,
            "expanse_map_remove_range": True,
            "expanse_map_nested_probe": True,
            "expanse_map_count_below": False,
            "expanse_map_narrow_two": True,
            "expanse_map_wide_two": False,
        }, tags
        # The callback typedef is not a function declaration.
        assert "expanse_map_remove_range_fn" not in tags
    finally:
        os.remove(tf_name)

    # 3. Base floor extraction self-tests (Task 2)
    # Valid ref against HEAD
    base_fl, base_err = get_base_floor_constant("HEAD", "scripts/check_abi_parity.py", "MIN_C_SYMBOLS")
    assert base_err == "", base_err
    assert base_fl == 100, base_fl

    # Unresolvable base ref must fail loud with non-empty error string
    bad_fl, bad_err = get_base_floor_constant("origin/nonexistent-branch-12345-never-exists")
    assert bad_fl is None
    assert bad_err != ""

    # Missing constant in file must fail loud with non-empty error string
    missing_fl, missing_err = get_base_floor_constant("HEAD", "scripts/check_abi_parity.py", "NONEXISTENT_CONSTANT_NAME")
    assert missing_fl is None
    assert missing_err != ""

    # 4. Anti-drift dangling reference check (#563)
    root = get_repo_root()
    dangling = check_no_dangling_capi_include_references(root)
    assert not dangling, f"Unexpected dangling references found in repo: {dangling}"

    # Fails closed if run outside git repository
    with tempfile.TemporaryDirectory() as td:
        errs = check_no_dangling_capi_include_references(Path(td))
        assert errs, "non-git directory must fail closed"
        assert any("dangling-reference check could not run" in e for e in errs)

    # 5. Python / Node coverage markers.
    #
    # The planted false match first: a binding file that carries the generic
    # words `prev` and `prev_before` for another reason (a writer-excluding
    # ordered read) and binds nothing for the reader-handle symbol. Under the
    # old any-keyword-anywhere rule, a mapping of ["prev_before", "prev"]
    # reported it covered.
    planted = CSymbol("expanse_sync_map_reader_prev_before", "bool", "", "t", 1)
    bound = CSymbol("expanse_sync_map_get", "bool", "", "t", 2)
    symbols_under_test = [planted, bound]
    mapping = {planted.name: "sync.rs", bound.name: "sync.rs"}
    generic = (
        "// abi-parity: expanse_sync_map_get\n"
        "pub fn get(&self, key: u64) -> Option<u64> { self.inner.get(key) }\n"
        "pub fn prev(&self, key: u64) -> Option<(u64, u64)> {\n"
        "    // expanse_sync_map_reader_prev_before is not bound here\n"
        "    self.inner.with_locked(|m| m.prev_before(key))\n"
        "}\n"
    )
    with tempfile.TemporaryDirectory() as td:
        d = Path(td)
        sync_rs = d / "sync.rs"
        for verify in (verify_python_bindings, verify_node_bindings):
            sync_rs.write_text(generic)
            covered, missing, errors = verify(d, symbols_under_test, mapping)
            assert planted.name in missing, (
                f"planted false match: {planted.name} reported covered by a generic token or a bare mention"
            )
            assert bound.name in covered and not errors, (covered, errors)

            # Its own marker, and nothing else, covers it.
            sync_rs.write_text(f"// abi-parity: {bound.name}, {planted.name}\n" + generic)
            covered, missing, errors = verify(d, symbols_under_test, mapping)
            assert covered == {planted.name, bound.name} and not missing and not errors, (covered, missing, errors)

            # A doc comment is not a marker.
            sync_rs.write_text(f"/// abi-parity: {planted.name}\n" + generic)
            covered, missing, errors = verify(d, symbols_under_test, mapping)
            assert planted.name in missing, "a /// doc comment must not count as a marker"

            # A marker for a longer name does not cover the shorter one, and is
            # itself an error (it names no declared symbol).
            sync_rs.write_text(f"// abi-parity: {planted.name}_ex\n" + generic)
            covered, missing, errors = verify(d, symbols_under_test, mapping)
            assert planted.name in missing
            assert any("not a wide-surface symbol" in e for e in errors), errors

            # A marker in a file its mapping does not name covers nothing.
            sync_rs.write_text(generic)
            (d / "map.rs").write_text(f"// abi-parity: {planted.name}\n")
            covered, missing, errors = verify(d, symbols_under_test, mapping)
            assert planted.name in missing
            assert any("its mapping names sync.rs" in e for e in errors), errors
            (d / "map.rs").unlink()

            # Malformed entries and mapping entries for undeclared symbols fail.
            sync_rs.write_text("// abi-parity: expanse_map_first; oops\n" + generic)
            _, _, errors = verify(d, symbols_under_test, {**mapping, "expanse_gone": "sync.rs"})
            assert any("malformed" in e for e in errors), errors
            assert any("mapping entry expanse_gone" in e for e in errors), errors

    # The shipped mappings name a file only; the marker is the evidence.
    for m in (PYTHON_FEATURE_MAPPING, NODE_FEATURE_MAPPING):
        assert all(isinstance(v, str) and v.endswith(".rs") for v in m.values()), m

    # The repository's own markers and mappings are consistent (coverage gaps
    # are the main check's business; inconsistencies are never acceptable).
    all_symbols = parse_c_header(root / "include" / "expanse.h")
    wide = [s for s in all_symbols if not s.narrow_only]
    for verify, sub in ((verify_python_bindings, "expanse-py"), (verify_node_bindings, "expanse-node")):
        _, _, errors = verify(root / "crates" / sub / "src", wide)
        assert not errors, errors

    # 6. Validate live STATED_DEFERRALS:
    deferral_errors = validate_stated_deferrals(STATED_DEFERRALS, wide)
    assert not deferral_errors, deferral_errors

    # 7. Discriminating self-test: verify fail-then-pass behavior and validation enforcement.
    test_sym = wide[0].name
    # 7a. Validation rejects invalid deferral entries
    assert validate_stated_deferrals({"nonexistent_symbol": {"issue": "#1", "reason": "r", "ecosystems": {"java"}}}, wide)
    assert validate_stated_deferrals({test_sym: {"issue": "", "reason": "r", "ecosystems": {"java"}}}, wide)
    assert validate_stated_deferrals({test_sym: {"issue": "#1", "reason": "   ", "ecosystems": {"java"}}}, wide)
    assert validate_stated_deferrals({test_sym: {"issue": "#1", "reason": "r", "ecosystems": {"ruby"}}}, wide)
    assert not validate_stated_deferrals({test_sym: {"issue": "#1", "reason": "valid", "ecosystems": {"java"}}}, wide)

    # 7b. Fail-then-pass check: an unlisted missing symbol reports in missing (FAIL),
    # while a stated deferral reports in deferred and not missing (PASS).
    raw_missing = {test_sym}
    deferred_empty, missing_unlisted = split_deferred(raw_missing, "java")
    assert test_sym in missing_unlisted, "Unlisted missing symbol must report as missing (fail loud)"
    assert test_sym not in deferred_empty

    # With deferral:
    test_deferral = {test_sym: {"issue": "#1397", "reason": "test deferral", "ecosystems": {"java"}}}
    deferred_listed, missing_listed = split_deferred(raw_missing, "java", test_deferral)
    assert test_sym in deferred_listed, "Listed deferred symbol must report in deferred"
    assert test_sym not in missing_listed, "Listed deferred symbol must not report in missing"

    # 7c. A deferral naming an ecosystem that binds the symbol is stale; one
    # naming only ecosystems that miss it is live.
    live = {"java": {test_sym}}
    assert not stale_deferrals(live, test_deferral)
    assert stale_deferrals({"java": set()}, test_deferral), "a bound symbol's deferral must be reported"
    two = {test_sym: {"issue": "#1397", "reason": "r", "ecosystems": {"java", "go"}}}
    assert len(stale_deferrals(live, two)) == 1, "each stale ecosystem is reported on its own"

    # 7d. A deferral stands only while its tracking issue is open. Closed is
    # an error naming the symbols; an unreadable state is reported, not failed.
    two_issues = {
        test_sym: {"issue": "#1", "reason": "r", "ecosystems": {"java"}},
        "other_symbol": {"issue": "#2", "reason": "r", "ecosystems": {"go"}},
    }
    errs, unknown = deferral_issue_problems(two_issues, state_of=lambda i: "OPEN")
    assert (errs, unknown) == ([], []), (errs, unknown)
    errs, unknown = deferral_issue_problems(two_issues, state_of=lambda i: "CLOSED" if i == "#2" else "OPEN")
    assert len(errs) == 1 and "#2" in errs[0] and "other_symbol" in errs[0] and unknown == [], (errs, unknown)
    errs, unknown = deferral_issue_problems(two_issues, state_of=lambda i: "UNKNOWN")
    assert errs == [] and unknown == ["#1", "#2"], (errs, unknown)
    # THE MOTIVATING STATE: all 19 ordered-bytes-map symbols cited #808 after it closed.
    was = {f"sym{i}": {"issue": "#808", "reason": "r", "ecosystems": {"java"}} for i in range(19)}
    errs, _ = deferral_issue_problems(was, state_of=lambda i: "CLOSED")
    assert len(errs) == 1 and "19 symbol(s)" in errs[0], errs
    assert issue_state("not-an-issue") == "UNKNOWN"

    print("check_abi_parity.py --self-test: all checks passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="libexpanse C ABI Symbol Parity Linter")
    parser.add_argument("--check", action="store_true", default=True, help="Validate 100%% parity and exit non-zero on mismatch")
    parser.add_argument("-v", "--verbose", action="store_true", help="Show verbose per-symbol coverage matrix")
    parser.add_argument("--json", action="store_true", help="Output machine-readable JSON")
    parser.add_argument("--markdown", action="store_true", help="Output markdown table for documentation")
    parser.add_argument("--base", help="Base ref to compare floor constant against (default: origin/$GITHUB_BASE_REF or origin/main)")
    parser.add_argument("--floor", type=int, default=MIN_C_SYMBOLS, help=f"Minimum required C ABI symbols (default: {MIN_C_SYMBOLS})")
    parser.add_argument("--pr-body-file", help="Path to file containing PR body text")
    parser.add_argument("--pr-body", help="PR body text as a string")
    parser.add_argument("--self-test", action="store_true", help="Run internal self-tests and exit")

    args = parser.parse_args()

    if args.self_test:
        return self_test()

    pr_body = ""
    if args.pr_body:
        pr_body = args.pr_body
    elif args.pr_body_file and os.path.exists(args.pr_body_file):
        try:
            pr_body = Path(args.pr_body_file).read_text(encoding="utf-8")
        except Exception as e:
            print(f"::warning::Failed to read PR body file '{args.pr_body_file}': {e}", file=sys.stderr)
    elif "PR_BODY" in os.environ:
        pr_body = os.environ["PR_BODY"]

    root = get_repo_root()

    # Determine base ref
    base_ref = args.base
    if not base_ref:
        if os.environ.get("GITHUB_BASE_REF"):
            base_ref = f"origin/{os.environ['GITHUB_BASE_REF']}"
        else:
            base_ref = "origin/main"

    # Base floor comparison: fail loud if base floor cannot be determined
    base_floor, err = get_base_floor_constant(base_ref, "scripts/check_abi_parity.py", "MIN_C_SYMBOLS", root=root)
    if err:
        print(f"::error::{err}", file=sys.stderr)
        return 1

    effective_floor = args.floor
    if base_floor is not None and effective_floor < base_floor:
        override = parse_allow_symbol_shrink(pr_body)
        if override:
            print(f"⚠️ Floor decrease detected (MIN_C_SYMBOLS: {base_floor} -> {effective_floor}), approved via PR override:")
            print(f"  Rationale: \"{override}\"")
        else:
            print(f"::error::C ABI symbol floor (MIN_C_SYMBOLS = {effective_floor}) is lower than base ref {base_ref} ({base_floor}) without an explicit override directive.")
            print("To approve lowering the floor, add an explicit directive to your PR body:")
            print("  allow-symbol-shrink: <nonempty reason>")
            return 1

    c_symbols, report = build_parity_report(root)

    # A deferral is an exemption from this gate, so its entries are checked on
    # every run, not only by --self-test.
    deferral_errors = validate_stated_deferrals(STATED_DEFERRALS, c_symbols) + stale_deferrals(
        {
            "java": report.java_deferred,
            "dotnet": report.dotnet_deferred,
            "python": report.python_deferred,
            "node": report.node_deferred,
            "go": report.go_deferred,
        }
    )
    issue_errors, unverified = deferral_issue_problems(STATED_DEFERRALS)
    deferral_errors += issue_errors
    if unverified:
        print(f"::notice::GitHub API unavailable (or GH_TOKEN unset) — open/closed state not "
              f"checked for the deferral issue(s): {', '.join(unverified)}")
    if deferral_errors:
        for e in deferral_errors:
            print(f"::error::{e}", file=sys.stderr)
        return 1

    if args.json:
        out = {
            "total_c_symbols": report.total_c_symbols,
            "narrow_only": report.narrow_only,
            "min_c_symbols_floor": effective_floor,
            "java": {
                "covered": len(report.java_covered),
                "missing": sorted(list(report.java_missing)),
                "deferred": sorted(list(report.java_deferred)),
            },
            "dotnet": {
                "covered": len(report.dotnet_covered),
                "missing": sorted(list(report.dotnet_missing)),
                "deferred": sorted(list(report.dotnet_deferred)),
            },
            "python": {
                "covered": len(report.python_covered),
                "missing": sorted(list(report.python_missing)),
                "deferred": sorted(list(report.python_deferred)),
                "errors": report.python_errors,
            },
            "node": {
                "covered": len(report.node_covered),
                "missing": sorted(list(report.node_missing)),
                "deferred": sorted(list(report.node_deferred)),
                "errors": report.node_errors,
            },
            "go": {
                "covered": len(report.go_covered),
                "missing": sorted(list(report.go_missing)),
                "deferred": sorted(list(report.go_deferred)),
            },
            "category_breakdown": report.category_breakdown,
        }
        print(json.dumps(out, indent=2))
    elif args.markdown:
        print(format_markdown_table(c_symbols, report))
    else:
        print_text_report(c_symbols, report, verbose=args.verbose)

    # Floor check
    floor_violation = False
    if report.total_c_symbols < effective_floor:
        override = parse_allow_symbol_shrink(pr_body)
        if override:
            print(f"⚠️ Total C ABI symbols ({report.total_c_symbols}) is below floor ({effective_floor}), but approved via PR override:")
            print(f"  Rationale: \"{override}\"")
        else:
            print(f"::error::Total declared C ABI functions ({report.total_c_symbols}) is below the pinned floor of {effective_floor}!")
            print("If symbols were intentionally removed or deprecated, add an explicit directive to the PR body:")
            print("  allow-symbol-shrink: <nonempty reason>")
            floor_violation = True

    for err in report.python_errors + report.node_errors:
        print(f"::error::{err}")

    has_errors = (
        len(report.python_errors) > 0
        or len(report.node_errors) > 0
        or len(report.java_missing) > 0
        or len(report.dotnet_missing) > 0
        or len(report.python_missing) > 0
        or len(report.node_missing) > 0
        or len(report.go_missing) > 0
        or floor_violation
    )

    if args.check and has_errors:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
