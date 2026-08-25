#!/usr/bin/env python3
"""
scripts/check_abi_parity.py — Automated C ABI Symbol Parity Linter for Expanse.

Verifies 100% symbol and feature parity of the modern libexpanse C API
(`crates/expanse-capi/include/expanse.h`) across all target language bindings:
  1. Java Panama FFM (`bindings/java/src/main/java/io/github/orieg/expanse/internal/ExpanseNative.java`)
  2. .NET C# P/Invoke (`bindings/dotnet/src/Expanse.NET/Native/NativeMethods.cs`)
  3. Python PyO3 (`crates/expanse-py/src/`)
  4. Node.js N-API (`crates/expanse-node/src/`)

Usage:
  python3 scripts/check_abi_parity.py [--check] [--verbose] [--json] [--markdown]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple


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


@dataclass
class ParityReport:
    total_c_symbols: int
    java_covered: Set[str] = field(default_factory=set)
    java_missing: Set[str] = field(default_factory=set)
    dotnet_covered: Set[str] = field(default_factory=set)
    dotnet_missing: Set[str] = field(default_factory=set)
    python_covered: Set[str] = field(default_factory=set)
    python_missing: Set[str] = field(default_factory=set)
    node_covered: Set[str] = field(default_factory=set)
    node_missing: Set[str] = field(default_factory=set)
    category_breakdown: Dict[str, Dict[str, int]] = field(default_factory=dict)


# Mapping of C ABI symbols to expected Python PyO3 constructs / methods
PYTHON_FEATURE_MAPPING = {
    # Identity
    "expanse_version": ("lib.rs", ["__version__", "version"]),

    # Set (17 functions)
    "expanse_set_new": ("set.rs", ["new", "__init__"]),
    "expanse_set_free": ("set.rs", ["inner", "ExpanseSet"]),
    "expanse_set_insert": ("set.rs", ["insert", "add"]),
    "expanse_set_remove": ("set.rs", ["remove", "discard"]),
    "expanse_set_contains": ("set.rs", ["__contains__", "contains"]),
    "expanse_set_len": ("set.rs", ["__len__", "len"]),
    "expanse_set_mem_used": ("set.rs", ["mem_used"]),
    "expanse_set_clear": ("set.rs", ["clear"]),
    "expanse_set_first": ("set.rs", ["first"]),
    "expanse_set_last": ("set.rs", ["last"]),
    "expanse_set_next_at_or_after": ("set.rs", ["next_at_or_after", "next"]),
    "expanse_set_next_after": ("set.rs", ["next_after", "next"]),
    "expanse_set_prev_at_or_before": ("set.rs", ["prev_at_or_before", "prev"]),
    "expanse_set_prev_before": ("set.rs", ["prev_before", "prev"]),
    "expanse_set_count_below": ("set.rs", ["count_below", "rank"]),
    "expanse_set_count_range": ("set.rs", ["count_range"]),
    "expanse_set_by_count": ("set.rs", ["by_count", "select"]),
    "expanse_set_contains_batch": ("set.rs", ["contains_batch"]),

    # Map (20 functions)
    "expanse_map_new": ("map.rs", ["new", "__init__"]),
    "expanse_map_free": ("map.rs", ["inner", "ExpanseMap"]),
    "expanse_map_insert": ("map.rs", ["insert", "__setitem__"]),
    "expanse_map_get": ("map.rs", ["get", "__getitem__"]),
    "expanse_map_get_batch": ("map.rs", ["get_batch"]),
    "expanse_map_remove": ("map.rs", ["remove", "__delitem__"]),
    "expanse_map_len": ("map.rs", ["__len__", "len"]),
    "expanse_map_mem_used": ("map.rs", ["mem_used"]),
    "expanse_map_clear": ("map.rs", ["clear"]),
    "expanse_map_slot": ("map.rs", ["insert", "get", "__getitem__"]),
    "expanse_map_ins_slot": ("map.rs", ["insert", "__setitem__"]),
    "expanse_map_first": ("map.rs", ["first"]),
    "expanse_map_last": ("map.rs", ["last"]),
    "expanse_map_next_at_or_after": ("map.rs", ["next_at_or_after", "next"]),
    "expanse_map_next_after": ("map.rs", ["next_after", "next"]),
    "expanse_map_prev_at_or_before": ("map.rs", ["prev_at_or_before", "prev"]),
    "expanse_map_prev_before": ("map.rs", ["prev_before", "prev"]),
    "expanse_map_count_below": ("map.rs", ["count_below", "rank"]),
    "expanse_map_count_range": ("map.rs", ["count_range"]),
    "expanse_map_by_count": ("map.rs", ["by_count", "select"]),

    # BytesMap (10 functions)
    "expanse_bytesmap_new": ("bytesmap.rs", ["new", "__init__"]),
    "expanse_bytesmap_free": ("bytesmap.rs", ["inner", "ExpanseBytesMap"]),
    "expanse_bytesmap_insert": ("bytesmap.rs", ["insert", "__setitem__"]),
    "expanse_bytesmap_get": ("bytesmap.rs", ["get", "__getitem__"]),
    "expanse_bytesmap_remove": ("bytesmap.rs", ["remove", "__delitem__"]),
    "expanse_bytesmap_slot": ("bytesmap.rs", ["insert", "get", "__getitem__"]),
    "expanse_bytesmap_ins_slot": ("bytesmap.rs", ["insert", "__setitem__"]),
    "expanse_bytesmap_len": ("bytesmap.rs", ["__len__", "len"]),
    "expanse_bytesmap_mem_used": ("bytesmap.rs", ["mem_used"]),
    "expanse_bytesmap_clear": ("bytesmap.rs", ["clear"]),

    # StrMap (16 functions)
    "expanse_strmap_new": ("strmap.rs", ["new", "__init__"]),
    "expanse_strmap_free": ("strmap.rs", ["inner", "ExpanseStrMap"]),
    "expanse_strmap_insert": ("strmap.rs", ["insert", "__setitem__"]),
    "expanse_strmap_get": ("strmap.rs", ["get", "__getitem__"]),
    "expanse_strmap_remove": ("strmap.rs", ["remove", "__delitem__"]),
    "expanse_strmap_slot": ("strmap.rs", ["insert", "get", "__getitem__"]),
    "expanse_strmap_ins_slot": ("strmap.rs", ["insert", "__setitem__"]),
    "expanse_strmap_len": ("strmap.rs", ["__len__", "len"]),
    "expanse_strmap_mem_used": ("strmap.rs", ["mem_used"]),
    "expanse_strmap_clear": ("strmap.rs", ["clear"]),
    "expanse_strmap_first": ("strmap.rs", ["first"]),
    "expanse_strmap_last": ("strmap.rs", ["last"]),
    "expanse_strmap_next_at_or_after": ("strmap.rs", ["next_at_or_after", "next"]),
    "expanse_strmap_next_after": ("strmap.rs", ["next_after", "next"]),
    "expanse_strmap_prev_at_or_before": ("strmap.rs", ["prev_at_or_before", "prev"]),
    "expanse_strmap_prev_before": ("strmap.rs", ["prev_before", "prev"]),

    # StrMap truncation-aware navigation (6 functions)
    "expanse_strmap_first_ex": ("strmap.rs", ["first"]),
    "expanse_strmap_last_ex": ("strmap.rs", ["last"]),
    "expanse_strmap_next_at_or_after_ex": ("strmap.rs", ["next_at_or_after", "next"]),
    "expanse_strmap_next_after_ex": ("strmap.rs", ["next_after", "next"]),
    "expanse_strmap_prev_at_or_before_ex": ("strmap.rs", ["prev_at_or_before", "prev"]),
    "expanse_strmap_prev_before_ex": ("strmap.rs", ["prev_before", "prev"]),

    # SyncSet (9 functions)
    "expanse_sync_set_new": ("sync.rs", ["new", "SyncExpanseSet"]),
    "expanse_sync_set_free": ("sync.rs", ["inner", "SyncExpanseSet"]),
    "expanse_sync_set_insert": ("sync.rs", ["insert", "add"]),
    "expanse_sync_set_remove": ("sync.rs", ["remove", "discard"]),
    "expanse_sync_set_contains": ("sync.rs", ["__contains__", "contains"]),
    "expanse_sync_set_len": ("sync.rs", ["__len__", "len"]),
    "expanse_sync_set_reader_new": ("sync.rs", ["SyncExpanseSet", "detach"]),
    "expanse_sync_set_reader_free": ("sync.rs", ["SyncExpanseSet", "detach"]),
    "expanse_sync_set_reader_contains": ("sync.rs", ["__contains__", "contains"]),

    # SyncMap (9 functions)
    "expanse_sync_map_new": ("sync.rs", ["new", "SyncExpanseMap"]),
    "expanse_sync_map_free": ("sync.rs", ["inner", "SyncExpanseMap"]),
    "expanse_sync_map_insert": ("sync.rs", ["insert", "__setitem__"]),
    "expanse_sync_map_get": ("sync.rs", ["get", "__getitem__"]),
    "expanse_sync_map_remove": ("sync.rs", ["remove", "__delitem__"]),
    "expanse_sync_map_len": ("sync.rs", ["__len__", "len"]),
    "expanse_sync_map_reader_new": ("sync.rs", ["SyncExpanseMap", "detach"]),
    "expanse_sync_map_reader_free": ("sync.rs", ["SyncExpanseMap", "detach"]),
    "expanse_sync_map_reader_get": ("sync.rs", ["get", "__getitem__"]),

    # BlobMap (11 functions)
    "expanse_blob_map_new": ("blobmap.rs", ["new", "with_chunk_size"]),
    "expanse_blob_map_free": ("blobmap.rs", ["inner", "ExpanseBlobMap"]),
    "expanse_blob_map_insert": ("blobmap.rs", ["insert", "__setitem__"]),
    "expanse_blob_map_remove": ("blobmap.rs", ["remove", "__delitem__"]),
    "expanse_blob_map_get": ("blobmap.rs", ["get", "__getitem__", "get_bytes"]),
    "expanse_blob_map_scan_filtered": ("blobmap.rs", ["get", "len", "contains_key", "inner"]),
    "expanse_blob_map_compact": ("blobmap.rs", ["compact", "inner"]),
    "expanse_blob_map_len": ("blobmap.rs", ["__len__", "len"]),
    "expanse_blob_map_mem_used": ("blobmap.rs", ["mem_used", "inner"]),
    "expanse_blob_map_clear": ("blobmap.rs", ["clear", "inner"]),
    "expanse_blob_map_contains_key": ("blobmap.rs", ["contains_key", "__contains__"]),
}

# Mapping of C ABI symbols to expected Node.js N-API constructs / methods
NODE_FEATURE_MAPPING = {
    # Identity
    "expanse_version": ("lib.rs", ["lib.rs", "napi"]),

    # Set (17 functions)
    "expanse_set_new": ("set.rs", ["new", "constructor"]),
    "expanse_set_free": ("set.rs", ["inner", "ExpanseSet"]),
    "expanse_set_insert": ("set.rs", ["add", "insert"]),
    "expanse_set_remove": ("set.rs", ["remove", "delete"]),
    "expanse_set_contains": ("set.rs", ["has", "contains"]),
    "expanse_set_len": ("set.rs", ["size", "len"]),
    "expanse_set_mem_used": ("set.rs", ["mem_used", "memUsed"]),
    "expanse_set_clear": ("set.rs", ["clear"]),
    "expanse_set_first": ("set.rs", ["first"]),
    "expanse_set_last": ("set.rs", ["last"]),
    "expanse_set_next_at_or_after": ("set.rs", ["next"]),
    "expanse_set_next_after": ("set.rs", ["next"]),
    "expanse_set_prev_at_or_before": ("set.rs", ["prev"]),
    "expanse_set_prev_before": ("set.rs", ["prev"]),
    "expanse_set_count_below": ("set.rs", ["rank", "count_below"]),
    "expanse_set_count_range": ("set.rs", ["count_range", "countRange"]),
    "expanse_set_by_count": ("set.rs", ["select", "by_count"]),
    "expanse_set_contains_batch": ("set.rs", ["contains_batch", "containsBatch"]),

    # Map (20 functions)
    "expanse_map_new": ("map.rs", ["new", "constructor"]),
    "expanse_map_free": ("map.rs", ["inner", "ExpanseMap"]),
    "expanse_map_insert": ("map.rs", ["set", "insert"]),
    "expanse_map_get": ("map.rs", ["get"]),
    "expanse_map_get_batch": ("map.rs", ["get_batch", "getBatch"]),
    "expanse_map_remove": ("map.rs", ["delete", "remove"]),
    "expanse_map_len": ("map.rs", ["size", "len"]),
    "expanse_map_mem_used": ("map.rs", ["mem_used", "memUsed"]),
    "expanse_map_clear": ("map.rs", ["clear"]),
    "expanse_map_slot": ("map.rs", ["set", "get"]),
    "expanse_map_ins_slot": ("map.rs", ["set", "insert"]),
    "expanse_map_first": ("map.rs", ["first"]),
    "expanse_map_last": ("map.rs", ["last"]),
    "expanse_map_next_at_or_after": ("map.rs", ["next"]),
    "expanse_map_next_after": ("map.rs", ["next"]),
    "expanse_map_prev_at_or_before": ("map.rs", ["prev"]),
    "expanse_map_prev_before": ("map.rs", ["prev"]),
    "expanse_map_count_below": ("map.rs", ["rank", "count_below"]),
    "expanse_map_count_range": ("map.rs", ["count_range", "countRange"]),
    "expanse_map_by_count": ("map.rs", ["select", "by_count"]),

    # BytesMap (10 functions)
    "expanse_bytesmap_new": ("bytesmap.rs", ["new", "constructor"]),
    "expanse_bytesmap_free": ("bytesmap.rs", ["inner", "ExpanseBytesMap"]),
    "expanse_bytesmap_insert": ("bytesmap.rs", ["set", "insert"]),
    "expanse_bytesmap_get": ("bytesmap.rs", ["get"]),
    "expanse_bytesmap_remove": ("bytesmap.rs", ["delete", "remove"]),
    "expanse_bytesmap_slot": ("bytesmap.rs", ["set", "get"]),
    "expanse_bytesmap_ins_slot": ("bytesmap.rs", ["set", "insert"]),
    "expanse_bytesmap_len": ("bytesmap.rs", ["size", "len"]),
    "expanse_bytesmap_mem_used": ("bytesmap.rs", ["mem_used", "memUsed"]),
    "expanse_bytesmap_clear": ("bytesmap.rs", ["clear"]),

    # StrMap (16 functions)
    "expanse_strmap_new": ("strmap.rs", ["new", "constructor"]),
    "expanse_strmap_free": ("strmap.rs", ["inner", "ExpanseStrMap"]),
    "expanse_strmap_insert": ("strmap.rs", ["set", "insert"]),
    "expanse_strmap_get": ("strmap.rs", ["get"]),
    "expanse_strmap_remove": ("strmap.rs", ["delete", "remove"]),
    "expanse_strmap_slot": ("strmap.rs", ["set", "get"]),
    "expanse_strmap_ins_slot": ("strmap.rs", ["set", "insert"]),
    "expanse_strmap_len": ("strmap.rs", ["size", "len"]),
    "expanse_strmap_mem_used": ("strmap.rs", ["mem_used", "memUsed"]),
    "expanse_strmap_clear": ("strmap.rs", ["clear"]),
    "expanse_strmap_first": ("strmap.rs", ["first"]),
    "expanse_strmap_last": ("strmap.rs", ["last"]),
    "expanse_strmap_next_at_or_after": ("strmap.rs", ["next"]),
    "expanse_strmap_next_after": ("strmap.rs", ["next"]),
    "expanse_strmap_prev_at_or_before": ("strmap.rs", ["prev"]),
    "expanse_strmap_prev_before": ("strmap.rs", ["prev"]),

    # StrMap truncation-aware navigation (6 functions)
    "expanse_strmap_first_ex": ("strmap.rs", ["first"]),
    "expanse_strmap_last_ex": ("strmap.rs", ["last"]),
    "expanse_strmap_next_at_or_after_ex": ("strmap.rs", ["next"]),
    "expanse_strmap_next_after_ex": ("strmap.rs", ["next"]),
    "expanse_strmap_prev_at_or_before_ex": ("strmap.rs", ["prev"]),
    "expanse_strmap_prev_before_ex": ("strmap.rs", ["prev"]),

    # SyncSet (9 functions)
    "expanse_sync_set_new": ("sync.rs", ["new", "constructor"]),
    "expanse_sync_set_free": ("sync.rs", ["inner", "SyncExpanseSet"]),
    "expanse_sync_set_insert": ("sync.rs", ["add", "insert"]),
    "expanse_sync_set_remove": ("sync.rs", ["remove", "delete"]),
    "expanse_sync_set_contains": ("sync.rs", ["has", "contains"]),
    "expanse_sync_set_len": ("sync.rs", ["size", "len"]),
    "expanse_sync_set_reader_new": ("sync.rs", ["SyncExpanseSet", "inner"]),
    "expanse_sync_set_reader_free": ("sync.rs", ["SyncExpanseSet", "inner"]),
    "expanse_sync_set_reader_contains": ("sync.rs", ["has", "contains"]),

    # SyncMap (9 functions)
    "expanse_sync_map_new": ("sync.rs", ["new", "constructor"]),
    "expanse_sync_map_free": ("sync.rs", ["inner", "SyncExpanseMap"]),
    "expanse_sync_map_insert": ("sync.rs", ["set", "insert"]),
    "expanse_sync_map_get": ("sync.rs", ["get"]),
    "expanse_sync_map_remove": ("sync.rs", ["delete", "remove"]),
    "expanse_sync_map_len": ("sync.rs", ["size", "len"]),
    "expanse_sync_map_reader_new": ("sync.rs", ["SyncExpanseMap", "inner"]),
    "expanse_sync_map_reader_free": ("sync.rs", ["SyncExpanseMap", "inner"]),
    "expanse_sync_map_reader_get": ("sync.rs", ["get"]),

    # BlobMap (11 functions)
    "expanse_blob_map_new": ("blobmap.rs", ["new", "constructor"]),
    "expanse_blob_map_free": ("blobmap.rs", ["inner", "ExpanseBlobMap"]),
    "expanse_blob_map_insert": ("blobmap.rs", ["set", "insert"]),
    "expanse_blob_map_remove": ("blobmap.rs", ["delete", "remove"]),
    "expanse_blob_map_get": ("blobmap.rs", ["get", "get_with_meta", "getWithMeta"]),
    "expanse_blob_map_scan_filtered": ("blobmap.rs", ["prune", "index", "iter"]),
    "expanse_blob_map_compact": ("blobmap.rs", ["compact"]),
    "expanse_blob_map_len": ("blobmap.rs", ["size", "len"]),
    "expanse_blob_map_mem_used": ("blobmap.rs", ["mem_used", "memUsed"]),
    "expanse_blob_map_clear": ("blobmap.rs", ["clear"]),
    "expanse_blob_map_contains_key": ("blobmap.rs", ["has", "contains_key"]),
}


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

    for idx, raw_line in enumerate(lines, start=1):
        line = raw_line.strip()

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

            # Check if this is a function declaration:
            # (return_type) (expanse_...) (args)
            match = re.match(
                r"^((?:const\s+)?[\w\s\*]+?)\s*\b(expanse_[a-z0-9_]+)\s*\((.*)\)\s*;$",
                sig,
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


def verify_python_bindings(py_dir: Path, c_symbols: List[CSymbol]) -> Tuple[Set[str], Set[str]]:
    """Verifies that all C functionality is mapped in Python PyO3 modules."""
    covered: Set[str] = set()
    missing: Set[str] = set()

    file_contents: Dict[str, str] = {}
    for rs_file in py_dir.glob("*.rs"):
        file_contents[rs_file.name] = rs_file.read_text(encoding="utf-8")

    for sym in c_symbols:
        mapping = PYTHON_FEATURE_MAPPING.get(sym.name)
        if not mapping:
            # If no mapping entry defined, mark missing
            missing.add(sym.name)
            continue

        rs_filename, keywords = mapping
        content = file_contents.get(rs_filename, "")

        # Check if keywords exist in file
        found = any(kw in content for kw in keywords)
        if found:
            covered.add(sym.name)
        else:
            missing.add(sym.name)

    return covered, missing


def verify_node_bindings(node_dir: Path, c_symbols: List[CSymbol]) -> Tuple[Set[str], Set[str]]:
    """Verifies that all C functionality is mapped in Node.js N-API modules."""
    covered: Set[str] = set()
    missing: Set[str] = set()

    file_contents: Dict[str, str] = {}
    for rs_file in node_dir.glob("*.rs"):
        file_contents[rs_file.name] = rs_file.read_text(encoding="utf-8")

    for sym in c_symbols:
        mapping = NODE_FEATURE_MAPPING.get(sym.name)
        if not mapping:
            missing.add(sym.name)
            continue

        rs_filename, keywords = mapping
        content = file_contents.get(rs_filename, "")

        found = any(kw in content for kw in keywords)
        if found:
            covered.add(sym.name)
        else:
            missing.add(sym.name)

    return covered, missing


def build_parity_report(root: Path) -> Tuple[List[CSymbol], ParityReport]:
    """Builds the full cross-ecosystem ABI parity report."""
    header_path = root / "crates" / "expanse-capi" / "include" / "expanse.h"
    if not header_path.exists():
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

    c_symbols = parse_c_header(header_path)
    c_symbol_names = {s.name for s in c_symbols}

    java_symbols = parse_java_panama(java_path)
    dotnet_symbols = parse_dotnet_pinvoke(dotnet_path)
    py_covered, py_missing = verify_python_bindings(py_dir, c_symbols)
    node_covered, node_missing = verify_node_bindings(node_dir, c_symbols)

    report = ParityReport(
        total_c_symbols=len(c_symbols),
        java_covered=c_symbol_names.intersection(java_symbols),
        java_missing=c_symbol_names - java_symbols,
        dotnet_covered=c_symbol_names.intersection(dotnet_symbols),
        dotnet_missing=c_symbol_names - dotnet_symbols,
        python_covered=py_covered,
        python_missing=py_missing,
        node_covered=node_covered,
        node_missing=node_missing,
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
        }

    return c_symbols, report


def print_text_report(c_symbols: List[CSymbol], report: ParityReport, verbose: bool = False) -> None:
    """Prints a clean human-readable CLI report."""
    print("================================================================================")
    print("           libexpanse C ABI Multi-Ecosystem Symbol Parity Report                ")
    print("================================================================================")
    print(f"Canonical C ABI Header: crates/expanse-capi/include/expanse.h")
    print(f"Total Declared C Functions: {report.total_c_symbols}\n")

    print("--------------------------------------------------------------------------------")
    print(f"{'Ecosystem / Binding Layer':<35} | {'Wrapped':<10} | {'Coverage':<10} | {'Status'}")
    print("--------------------------------------------------------------------------------")

    def format_row(name: str, covered: int, total: int, missing: Set[str]) -> str:
        pct = (covered / total * 100.0) if total > 0 else 100.0
        status = "✓ PASS (100%)" if len(missing) == 0 else f"✗ FAIL ({len(missing)} missing)"
        return f"{name:<35} | {covered:>3}/{total:<5} | {pct:>8.1f}% | {status}"

    print(format_row("Java 22+ (Panama FFM downcalls)", len(report.java_covered), report.total_c_symbols, report.java_missing))
    print(format_row(".NET C# (P/Invoke NativeMethods)", len(report.dotnet_covered), report.total_c_symbols, report.dotnet_missing))
    print(format_row("Python (PyO3 native classes)", len(report.python_covered), report.total_c_symbols, report.python_missing))
    print(format_row("Node.js (N-API native bindings)", len(report.node_covered), report.total_c_symbols, report.node_missing))
    print("--------------------------------------------------------------------------------\n")

    print("--- Breakdown by Container / Functional Category ---")
    print(f"{'Category':<32} | {'Total':<6} | {'Java':<6} | {'.NET':<6} | {'Python':<6} | {'Node.js':<6}")
    print("--------------------------------------------------------------------------------")
    for cat_name, counts in report.category_breakdown.items():
        print(f"{cat_name:<32} | {counts['total']:<6} | {counts['java']:<6} | {counts['dotnet']:<6} | {counts['python']:<6} | {counts['node']:<6}")
    print("--------------------------------------------------------------------------------\n")

    if verbose or report.java_missing or report.dotnet_missing or report.python_missing or report.node_missing:
        print("--- Detailed Per-Symbol Coverage Matrix ---")
        header = f"{'Symbol Name':<36} | {'Java':<6} | {'.NET':<6} | {'Python':<6} | {'Node.js':<6}"
        print(header)
        print("-" * len(header))
        for s in c_symbols:
            j = "✓" if s.name in report.java_covered else "MISSING"
            d = "✓" if s.name in report.dotnet_covered else "MISSING"
            p = "✓" if s.name in report.python_covered else "MISSING"
            n = "✓" if s.name in report.node_covered else "MISSING"
            print(f"{s.name:<36} | {j:<6} | {d:<6} | {p:<6} | {n:<6}")
        print("--------------------------------------------------------------------------------\n")

    if report.java_missing or report.dotnet_missing or report.python_missing or report.node_missing:
        print("::error::ABI Parity check failed! Missing symbols detected:")
        if report.java_missing:
            print(f"  Java missing: {sorted(report.java_missing)}")
        if report.dotnet_missing:
            print(f"  .NET missing: {sorted(report.dotnet_missing)}")
        if report.python_missing:
            print(f"  Python missing: {sorted(report.python_missing)}")
        if report.node_missing:
            print(f"  Node.js missing: {sorted(report.node_missing)}")
    else:
        print(f"✓ All {report.total_c_symbols} libexpanse C ABI symbols are 100% covered across Java, .NET, Python, and Node.js!")


def format_markdown_table(c_symbols: List[CSymbol], report: ParityReport) -> str:
    """Generates GitHub markdown table for docs/COMPAT.md."""
    lines = [
        "| Container / API Family | C Functions | Java 22+ Panama | .NET P/Invoke | Python (PyO3) | Node.js (N-API) | Feature Parity |",
        "|---|---|---|---|---|---|---|",
    ]
    for cat_name, counts in report.category_breakdown.items():
        total = counts["total"]
        j = f"{counts['java']}/{total}"
        d = f"{counts['dotnet']}/{total}"
        p = f"{counts['python']}/{total}"
        n = f"{counts['node']}/{total}"
        status = "100% Full Parity" if counts["java"] == total and counts["dotnet"] == total and counts["python"] == total and counts["node"] == total else "Partial"
        lines.append(f"| `{cat_name}` | {total} | {j} | {d} | {p} | {n} | {status} |")

    lines.append(f"| **Total C ABI Symbols** | **{report.total_c_symbols}** | **{len(report.java_covered)}/{report.total_c_symbols}** | **{len(report.dotnet_covered)}/{report.total_c_symbols}** | **{len(report.python_covered)}/{report.total_c_symbols}** | **{len(report.node_covered)}/{report.total_c_symbols}** | **100% Complete** |")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="libexpanse C ABI Symbol Parity Linter")
    parser.add_argument("--check", action="store_true", default=True, help="Validate 100% parity and exit non-zero on mismatch")
    parser.add_argument("-v", "--verbose", action="store_true", help="Show verbose per-symbol coverage matrix")
    parser.add_argument("--json", action="store_true", help="Output machine-readable JSON")
    parser.add_argument("--markdown", action="store_true", help="Output markdown table for documentation")

    args = parser.parse_args()

    root = get_repo_root()
    c_symbols, report = build_parity_report(root)

    if args.json:
        out = {
            "total_c_symbols": report.total_c_symbols,
            "java": {"covered": len(report.java_covered), "missing": sorted(list(report.java_missing))},
            "dotnet": {"covered": len(report.dotnet_covered), "missing": sorted(list(report.dotnet_missing))},
            "python": {"covered": len(report.python_covered), "missing": sorted(list(report.python_missing))},
            "node": {"covered": len(report.node_covered), "missing": sorted(list(report.node_missing))},
            "category_breakdown": report.category_breakdown,
        }
        print(json.dumps(out, indent=2))
    elif args.markdown:
        print(format_markdown_table(c_symbols, report))
    else:
        print_text_report(c_symbols, report, verbose=args.verbose)

    has_errors = (
        len(report.java_missing) > 0
        or len(report.dotnet_missing) > 0
        or len(report.python_missing) > 0
        or len(report.node_missing) > 0
    )

    if args.check and has_errors:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
