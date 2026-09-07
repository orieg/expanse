# Python Bindings & PyPI Distribution Guide (`expanse-trie`)

> Canonical documentation for Expanse Python bindings, PyPI distribution, type stubs, and GIL-free concurrent architecture.  
> Architecture: [ARCHITECTURE.md](../ARCHITECTURE.md) · Packaging: [PACKAGING.md](../PACKAGING.md) · CI Pipeline: [CI.md](../CI.md)

`expanse-trie` provides high-performance Python bindings for **Expanse**, the clean-room, pure-Rust reimplementation of Judy arrays and digital tries modernized for 64-bit hardware.

---

## 1. Overview & Key Capabilities

- **Zero-Overhead Memory Compaction**: Consumes as low as **0.07–0.36 bytes/key** on clustered integer sets *(measured: Apple M1, `bytes_per_key` example, commit 6c63826a — deterministic byte accounting)*, versus tens of bytes/key for Python's standard `set` and `dict`.
- **Cache-Line Aligned Digital Tries**: $O(\text{depth})$ traversals (at most 8 digit steps for 64-bit keys) keeping branch and leaf evaluations within 64-byte L1 cache lines.
- **Ordered Traversal & Range Scans**: Native sorted iteration, $O(\text{depth})$ `first()`, `last()`, `next_at_or_after()`, `prev_at_or_before()`, rank (`count_below`), and select (`by_count`) without maintaining secondary index trees.
- **GIL-Free Optimistic Concurrency Control (OCC)**: `SyncExpanseSet` and `SyncExpanseMap` release the Python GIL (`py.detach`, pyo3 0.29's renamed `allow_threads`) during queries, so multiple Python `threading` / `ThreadPoolExecutor` workers execute reads concurrently across cores with zero read locks. **Which API you call decides whether you get multi-core reads.** Per-call `get` releases and reacquires the GIL around a lookup of tens of nanoseconds, so the handoff costs more than the work and concurrent callers convoy on reacquisition — pinned to the performance cores it measures **0.124x** its own single-thread throughput at 16 threads. `get_many` amortises one release across the batch and is the form to use for concurrent reads; on one thread it is **1.15x [1.04, 1.23]** a plain `dict`, while per-call `get` loses to one. Against the `dict` control only the single-thread comparison is settled; at two threads and above the control is multi-modal and the comparison is `INTERMEDIATE` (#774). The earlier `0.02x` figure for this bullet was measured unpinned and is superseded. Full table and provenance in [§ Concurrency](#concurrency).
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

### 3.5 `ExpanseBlobMap` (Large-Value / Off-Heap Blob Map)

`ExpanseBlobMap` maps a 64-bit key to an arbitrary byte payload, packing small payloads inline in the value slot and bump-allocating larger ones in the arena (see [large-values.md](../design/large-values.md); live arena ceiling is 16 MiB). `insert` takes an optional 32-bit **hot metadata** word stored alongside the locator for predicate filtering without dereferencing the payload:

```python
from expanse_trie import ExpanseBlobMap

blob = ExpanseBlobMap()                 # optional: ExpanseBlobMap(chunk_size=...)

# insert(key, data, hot_meta): hot_meta is a u32 (e.g. TTL / flags / tenant id)
blob.insert(1, b"arbitrary payload bytes", 0x01)

# get() returns (payload, hot_meta); get_bytes() / [] return just the payload
payload, meta = blob.get(1)
assert payload == b"arbitrary payload bytes" and meta == 0x01
assert blob[1] == b"arbitrary payload bytes"

# The dict-style setter stores with hot_meta = 0 (the inline-blob default contract):
blob[2] = b"no metadata"
assert blob.get(2) == (b"no metadata", 0)

# Persistence: save_to_file() writes an image; load_from_file() reads it back
# (full std::fs::read + index rebuild — not an mmap).
n = blob.save_to_file("blob.img")
reloaded = ExpanseBlobMap.load_from_file("blob.img")
assert reloaded[1] == b"arbitrary payload bytes"
```

---

## 4. Multithreaded GIL-Free Concurrency (`SyncExpanse*`)

In standard CPython, multithreaded CPU-bound data lookups often serialize on the Global Interpreter Lock (GIL).

`SyncExpanseSet` and `SyncExpanseMap` solve this by implementing **optimistic concurrency control (OCC)** in Rust and wrapping calls with `py.detach` (pyo3 0.29's renamed `allow_threads`):

1. **Lock-Free Reads**: Query operations (`contains`, `get`, `len`, `is_empty`) validate version seqlocks and read concurrently without holding mutexes or the Python GIL.
2. **Serialized Writes**: Mutations (`insert`, `remove`) synchronize internally while allowing readers to proceed optimistically.
3. **Multi-Core Reads** — through `get_many`, not `get`. Which API you call decides
   whether you get multi-core reads: per-call `get` releases and reacquires the GIL
   around a lookup of tens of nanoseconds, so the handoff costs more than the work and
   concurrent callers convoy on reacquisition, while `get_many` amortises one release
   across the batch.

   **Re-measured on the reference host (#755, #774).** The figures below replace the
   withheld ones. Two separate defects had to be cleared first, and the second was
   found by this re-run rather than by #755:

   - the harness drew its miss half as `k ^ (1 << 63)` of each hit key, which lands
     every miss in a different top-level expanse from the key it came from, so it
     terminates at a systematically different depth than a hit — the stream met its
     stated 50% hit rate while averaging two different descents into one number
     (section 8.6). Corrected: misses are rejection-sampled from the population's own
     generator and shuffled in.
   - the superseded run was taken **unpinned** on a hybrid host, and pin exposure grows
     with thread count — parity at one thread, 5.57x
     [5.31, 5.74]
     at sixteen (#774). That invalidated the within-arm scaling curve too, which #755
     had retained on the reasoning that its contamination was constant along the thread
     axis. It was, for the miss shape; it was not, for the pin.

   Throughput in Mops/s, `SyncExpanseMap` through per-call `get`, pinned to the
   performance cores *(workload: `python_concurrency`)*:

   | threads | `get` (per-call) | scaling vs 1 thread |
   |---:|---:|---:|
   | 1 | 5.686 [5.565, 5.738] | 1.000 |
   | 2 | 2.338 [2.317, 2.372] | 0.411 |
   | 4 | 0.819 [0.812, 0.839] | 0.144 |
   | 8 | 0.689 [0.686, 0.693] | 0.121 |
   | 16 | 0.708 [0.696, 0.712] | 0.124 |

   The per-call arm does not scale: it is slowest at 8-16 threads, about an eighth of
   its single-thread throughput. `get_many` is the form that reaches more than one
   core, peaking at two threads (16.204 [15.519, 16.685], 1.622x its own
   single-thread rate) and falling off after — at 16 threads each worker runs about
   three batches, so there is little left to amortise, and assembling the result list is
   itself GIL-bound Python work.

   **Against the `dict` control, only the single-thread comparison is settled.**
   `get_many` is 1.152x [1.040, 1.227] the
   `dict`, and per-call `get` is 0.656x [0.637, 0.691]
   — that is, per-call `get` **loses** to a plain `dict` on one thread. The earlier
   "1.64x faster single-threaded" and "beats the `dict` control at every width" are
   **formally retracted**, not merely re-measured: the measurement contradicts them.

   At two threads and above the comparison is `INTERMEDIATE` and no ratio is published.
   The `dict` control is multi-modal there — at W=4 and W=8, 30.0% of rounds run above
   1.5x the median with a coefficient of variation near 60%, while both Expanse arms
   stay unimodal — and a mean over a bimodal distribution describes neither mode
   (section 8.4). Every cell carries `mean_is_valid_estimator` in the artifact.

   *(measured: reference host — Intel i9-12900F, 8P+8E / 24 threads, benchmark pinned to
   CPUs 0-15; commit `eb404912`; 15 rounds per cell, BCa 95% over 10,000 resamples;
   artifact [`results/baseline_python_concurrency.json`](../../results/baseline_python_concurrency.json);
   load snapshotted before and after, busy-CPU delta 1.88 core-equivalents)*. The batch
   size is not tuned against any run — picking one to flatter the curve would be the
   mid-execution tuning section 8.7 warns about.

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

> The `expanse-trie` memory figure (0.07–0.36 bytes/key) is deterministic byte accounting *(measured: Apple M1, `bytes_per_key` example, commit 6c63826a)*; the `set`/`dict` and `roaring-bitmap` columns are approximate references. This table carries no timing measurements.

---

## 6. Architecture & Packaging Layout

- **Packaging Taxonomy**: Python package metadata and PEP 561 typed source files live in `bindings/python/expanse_trie/` (`py.typed` and `__init__.pyi`), configured via `pyproject.toml` with `python-source = "bindings/python"`.
- **PyO3 Extension**: Native compiled extension module built from `crates/expanse-py` into `expanse_trie._expanse`.
- **Wheel Architecture**: Compiled with stable `PyO3` targeting Python `abi3-py38` for cross-version binary compatibility across Python 3.8 through 3.13+.
- **Type Stubs**: Packaged with `py.typed` and `__init__.pyi` providing comprehensive annotations for modern Python type checkers (`mypy --strict`, Pyright, IDE linting).
