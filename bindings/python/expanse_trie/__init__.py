"""Expanse: Judy arrays and digital tries in Python.

Expanse provides cache-line-tuned, 64-bit Judy array data structures
reimplemented from first principles in clean-room pure Rust:

- `ExpanseSet`: Dynamic sparse 64-bit integer set (Judy1 equivalent).
- `ExpanseMap`: Dynamic sparse 64-bit integer key-to-value map (JudyL equivalent).
- `ExpanseStrMap`: Variable-length string/bytes-to-integer trie map (JudySL equivalent).
- `ExpanseBytesMap`: Arbitrary byte array-to-integer hash map (JudyHS equivalent).
- `ExpanseOrderedBytesMap`: Ordered arbitrary byte array-to-integer map with first/last/next/prev navigation.
- `ExpanseBlobMap`: 64-bit key-to-byte-payload map with inline packing and arena backing.
- `SyncExpanseSet`: Multithreaded optimistic OCC integer set with GIL-released queries.
- `SyncExpanseMap`: Multithreaded optimistic OCC integer map with GIL-released queries.
- `SyncExpanseStrMap`, `SyncExpanseBytesMap`, `SyncExpanseOrderedBytesMap`, `SyncExpanseBlobMap`:
  the shared string, bytes, ordered-bytes and blob maps, with GIL-released calls and atomic
  `compare_exchange` / `get_or_insert`.
"""

from ._expanse import (
    ExpanseBlobMap,
    ExpanseBytesMap,
    ExpanseMap,
    ExpanseOrderedBytesMap,
    ExpanseSet,
    ExpanseStrMap,
    SyncExpanseBlobMap,
    SyncExpanseBytesMap,
    SyncExpanseMap,
    SyncExpanseOrderedBytesMap,
    SyncExpanseSet,
    SyncExpanseStrMap,
    __version__,
)

__all__ = [
    "ExpanseBlobMap",
    "ExpanseBytesMap",
    "ExpanseMap",
    "ExpanseOrderedBytesMap",
    "ExpanseSet",
    "ExpanseStrMap",
    "SyncExpanseBlobMap",
    "SyncExpanseBytesMap",
    "SyncExpanseMap",
    "SyncExpanseOrderedBytesMap",
    "SyncExpanseSet",
    "SyncExpanseStrMap",
    "__version__",
]
