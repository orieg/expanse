# Python Bindings & PyPI Distribution Guide (`expanse-trie`)

> Canonical documentation for Expanse Python bindings, PyPI distribution, type stubs, and GIL-free concurrent architecture.  
> Architecture: [ARCHITECTURE.md](ARCHITECTURE.md) · Packaging: [PACKAGING.md](PACKAGING.md) · CI Pipeline: [CI.md](CI.md)

`expanse-trie` provides high-performance Python bindings for **Expanse**, the clean-room, pure-Rust reimplementation of Judy arrays and digital tries modernized for 64-bit hardware.

---

## 1. Overview & Key Capabilities

- **Zero-Overhead Memory Compaction**: Consumes as low as **0.07–0.36 bytes/key** on clustered integer sets, compared to 64+ bytes/key for Python's standard `set` and `dict`.
- **Cache-Line Aligned Digital Tries**: $O(\text{depth})$ traversals (at most 8 digit steps for 64-bit keys) keeping branch and leaf evaluations within 64-byte L1 cache lines.
- **Ordered Traversal & Range Scans**: Native sorted iteration, $O(\text{depth})$ `first()`, `last()`, `next_at_or_after()`, `prev_at_or_before()`, rank (`count_below`), and select (`by_count`) without maintaining secondary index trees.
- **GIL-Free Optimistic Concurrency Control (OCC)**: `SyncExpanseSet` and `SyncExpanseMap` release the Python GIL (`py.detach`) during queries, enabling **linear multi-core CPU scaling** across Python `threading` and `ThreadPoolExecutor` workers with zero read locks.
- **Strict Typing & IDE Support**: Full PEP 561 compliance (`py.typed` and `__init__.pyi` stubs) for mypy, Pyright, and IDE autocompletion.

---

## 2. Installation

Precompiled binary wheels (built with Python `abi3` for Python 3.8+) are distributed on PyPI for Linux (`x86_64`, `aarch64`), macOS (`arm64`, `x86_64`), and Windows (`x86_64`):

```bash
pip install expanse-trie
```

### Local Development Build
To compile from source using `maturin`:

```bash
# Install maturin build backend
pip install "maturin>=1.5,<2.0" pytest

# Build and install into active virtualenv
maturin develop --release

# Execute test suite
pytest tests/test_python_bindings.py -v
```

---

## 3. Data Structures & Usage

```mermaid
graph TD
    A[expanse_trie] --> B[ExpanseSet (Judy1: 64-bit Integer Set)]
    A --> C[ExpanseMap (JudyL: 64-bit Key-Value Map)]
    A --> D[ExpanseStrMap (JudySL: String/Bytes Trie Map)]
    A --> E[ExpanseBytesMap (JudyHS: Arbitrary Bytes Hash Map)]
    A --> F[SyncExpanseSet (Thread-Safe OCC Set)]
    A --> G[SyncExpanseMap (Thread-Safe OCC Map)]
```

### 3.1 `ExpanseSet` (Sparse 64-bit Integer Set / Judy1)

`ExpanseSet` stores dynamic populations of 64-bit unsigned integers with adaptive compression:

```python
from expanse_trie import ExpanseSet

# Create and populate
s = ExpanseSet([10, 20, 30, 40, 50, 1000])

# O(depth) Membership tests
assert 20 in s
assert 25 not in s

# Mutations
s.add(60)
s.remove(20)
assert len(s) == 6

# Ordered Navigation & Proximity Searches
assert s.first() == 10
assert s.last() == 1000
assert s.next_at_or_after(25) == 30    # Smallest key >= 25
assert s.next_after(30) == 40          # Smallest key > 30
assert s.prev_at_or_before(35) == 30   # Largest key <= 35
assert s.prev_before(30) == 10         # Largest key < 30

# Rank & Select (0-based)
assert s.count_below(40) == 2          # Keys strictly < 40 (10, 30)
assert s.count_range(10, 50) == 4      # Keys in [10, 50] inclusive
assert s.by_count(0) == 10             # 0-th key in sorted order
assert s.by_count(3) == 50             # 3-rd key in sorted order

# Range Scanning
assert s.range(25, 60) == [30, 40, 50, 60]

# Exact memory footprint in bytes
print(f"Memory used: {s.mem_used()} bytes")
```

---

### 3.2 `ExpanseMap` (64-bit Key-Value Associative Map / JudyL)

`ExpanseMap` maps 64-bit integer keys to 64-bit integer values with dict-like semantics:

```python
from expanse_trie import ExpanseMap

# Create map from dict or pairs
m = ExpanseMap({1: 100, 2: 200, 5: 500, 10: 1000})

# Dict-like item access
assert m[1] == 100
m[20] = 2000
assert m.get(99, default=0) == 0

# Pop & Remove
old = m.pop(2)
assert old == 200
assert 2 not in m

# Ordered iteration & Range Queries
assert list(m) == [1, 5, 10, 20]
assert m.items() == [(1, 100), (5, 500), (10, 1000), (20, 2000)]
assert m.range(5, 15) == [(5, 500), (10, 1000)]

# Navigation
assert m.next_at_or_after(6) == (10, 1000)
```

---

### 3.3 `ExpanseStrMap` (String & Byte Key Trie Map / JudySL)

`ExpanseStrMap` indexes arbitrary variable-length string or byte keys into 64-bit values using digital expanse trie branching:

```python
from expanse_trie import ExpanseStrMap

sm = ExpanseStrMap()
sm["http://api.internal/users"] = 1
sm["http://api.internal/orders"] = 2
sm["http://api.internal/payments"] = 3

# Exact lookups
assert sm["http://api.internal/orders"] == 2
assert "http://api.internal/catalog" not in sm

# Lexicographical range queries
routes = sm.range(start="http://api.internal/o", end="http://api.internal/p~")
# Returns [("http://api.internal/orders", 2), ("http://api.internal/payments", 3)]
```

---

### 3.4 `ExpanseBytesMap` (Arbitrary Byte Key Hash Map / JudyHS)

`ExpanseBytesMap` hashes arbitrary binary keys (including binary blobs with embedded `\x00` null bytes):

```python
from expanse_trie import ExpanseBytesMap

bm = ExpanseBytesMap()
raw_key = b"session\x00auth\x00token\xff\xfe"
bm[raw_key] = 8888

assert raw_key in bm
assert bm[raw_key] == 8888
```

---

## 4. Multithreaded GIL-Free Concurrency (`SyncExpanse*`)

In standard CPython, multithreaded CPU-bound data lookups often serialize on the Global Interpreter Lock (GIL).

`SyncExpanseSet` and `SyncExpanseMap` solve this by implementing **lock-free Optimistic Concurrency Control (OCC)** in Rust and wrapping calls with `py.detach` (pyo3 0.29's renamed `allow_threads`):

1. **Lock-Free Reads**: Query operations (`contains`, `get`, `len`, `is_empty`) validate version seqlocks and read concurrently without holding mutexes or the Python GIL.
2. **Serialized Writes**: Mutations (`insert`, `remove`) synchronize internally while allowing readers to proceed optimistically.
3. **True Multi-Core Scaling**: Multiple Python threads saturate all CPU cores without lock contention.

### Multithreaded Concurrency Example:

```python
import concurrent.futures
from expanse_trie import SyncExpanseMap

# Shared thread-safe OCC map
cache = SyncExpanseMap()
for i in range(100_000):
    cache.insert(i, i * 10)

def reader_worker(thread_id: int) -> int:
    hits = 0
    # Queries execute GIL-free in parallel across CPU cores
    for k in range(thread_id, 100_000, 8):
        if cache.get(k) is not None:
            hits += 1
    return hits

# Execute across 8 parallel OS threads
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
    results = list(executor.map(reader_worker, range(8)))

print(f"Total verified items across threads: {sum(results)}")
```

---

## 5. Performance & Memory Comparison

| Feature / Metric | Python `set` / `dict` | `roaring-bitmap` | `expanse-trie` |
|---|---|---|---|
| **Memory (Dense/Clustered Integers)** | ~64 bytes / key | 0.12–0.50 bytes / key | **0.07–0.36 bytes / key** |
| **Random Point Lookup** | Hash table lookup | Container binary search | **Direct Tagged Pointer / Trie** |
| **Ordered Range Scan (`range()`)** | $O(N \log N)$ (Unsorted) | $O(\text{containers})$ | **$O(\text{depth})$ Cache-line Traversal** |
| **Rank / Select (`count_below`)** | Not natively supported | Bit-count traversal | **$O(\text{depth})$ Direct Trie Rank** |
| **Multithreaded GIL Release** | No (holds GIL) | Partial | **Yes (`SyncExpanseMap`/`Set`)** |
| **Memory Safety** | C / Python runtime | C / Rust backend | **100% Pure Rust (#![no_std] core)** |

---

## 6. Architecture & Typing Summary

- **Wheel Architecture**: Compiled with stable `PyO3` targeting Python `abi3-py38` for cross-version binary compatibility.
- **Type Stubs**: Packaged with `py.typed` and `__init__.pyi` providing comprehensive annotations for modern Python type checkers (`mypy --strict`, Pyright, IDE linting).
