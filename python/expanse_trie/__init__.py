"""Expanse: Modern Judy arrays and high-performance digital tries in Python.

Expanse provides cache-line-tuned, 64-bit Judy array data structures
reimplemented from first principles in clean-room pure Rust:

- `ExpanseSet`: Dynamic sparse 64-bit integer set (Judy1 equivalent).
- `ExpanseMap`: Dynamic sparse 64-bit integer key-to-value map (JudyL equivalent).
- `ExpanseStrMap`: Variable-length string/bytes-to-integer trie map (JudySL equivalent).
- `ExpanseBytesMap`: Arbitrary byte array-to-integer hash map (JudyHS equivalent).
- `SyncExpanseSet`: Multithreaded lock-free OCC integer set with GIL-released queries.
- `SyncExpanseMap`: Multithreaded lock-free OCC integer map with GIL-released queries.
"""

from ._expanse import (
    ExpanseBytesMap,
    ExpanseMap,
    ExpanseSet,
    ExpanseStrMap,
    SyncExpanseMap,
    SyncExpanseSet,
    __version__,
)

__all__ = [
    "ExpanseBytesMap",
    "ExpanseMap",
    "ExpanseSet",
    "ExpanseStrMap",
    "SyncExpanseMap",
    "SyncExpanseSet",
    "__version__",
]
