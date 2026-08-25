# Expanse Pluggable MemTable for RocksDB (`rocksdb-expanse`)

Official **RocksDB Pluggable MemTable** implementation backed by **Expanse's** 64-bit digital trie architecture and cache-line aligned leaf slabs.

---

## 1. Overview & Architectural Benefits

In LSM-tree storage engines like RocksDB, the **MemTable** buffers concurrent writes in RAM before flushing sorted runs to Level 0 SSTables on disk. 

The default RocksDB memtable implementation (`SkipListRep`) incurs significant performance and memory overheads:
- **High Pointer Overhead**: SkipLists allocate randomized height towers (1 to 32 forward pointers per node), consuming 32–64 bytes of indexing metadata per key.
- **CPU Cache Misses**: Traversing a SkipList causes pointer chasing across scattered heap allocations, thrashing L1/L2 CPU caches.
- **Premature SSTable Flushes**: Because SkipList metadata consumes up to 40%–50% of the allocated `write_buffer_size`, fewer user keys fit in RAM, triggering frequent L0 flushes and cascading compaction write amplification.

### The Expanse MemTable Advantage

`ExpanseMemTableRep` organizes key entries into **64-byte cache-line aligned leaf blocks** indexed by an adaptive **Expanse 64-bit Digital Trie**:

```
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                      RocksDB Write / Read / Scan Path                       │
 └──────────────────────────────────────┬──────────────────────────────────────┘
                                        │
                                        ▼
                  ┌───────────────────────────────────────────┐
                  │ Expanse 64-bit Prefix Trie (JudyL Engine) │
                  │  • O(depth) bounds (<= 8 levels)          │
                  │  • Zero tree rotations / rebalancing      │
                  └─────────────────────┬─────────────────────┘
                                        │
          ┌─────────────────────────────┼─────────────────────────────┐
          ▼                             ▼                             ▼
 ┌──────────────────┐          ┌──────────────────┐          ┌──────────────────┐
 │ LeafBlock 0      │ ◄──────► │ LeafBlock 1      │ ◄──────► │ LeafBlock 2      │
 │  • 64-byte align │          │  • 64-byte align │          │  • 64-byte align │
 │  • SIMD searched │          │  • SIMD searched │          │  • SIMD searched │
 └──────────────────┘          └──────────────────┘          └──────────────────┘
```

1. **~11× Higher Key Density in RAM** (vs the reference SkipList):
   - Leaf blocks store entry pointers in contiguous 64-byte spans. Indexing overhead is **13.2 bytes/entry (Expanse) vs 146.7 bytes/entry (reference SkipList)** for 100k entries — a deterministic accounting figure.
   - More user data fits into the same memtable memory budget, reducing L0 SSTable flush frequency and downstream compaction pressure.
2. **Reduced Compaction Write Amplification**:
   - Larger effective in-memory batching before flush leads to wider sorted runs and fewer overlapping L0 SSTables, lowering write amplification on NVMe/SSD storage.
3. **~10× Faster Sequential Iteration** (vs the reference SkipList):
   - Forward range scans traverse contiguous leaf blocks via intrusive sibling leaf chaining at **160.8 Mops/sec** (vs 16.4 Mops/sec for the pointer-hopping SkipList). An unordered `VectorRep` append-vector scans faster still (599 Mops/sec) but cannot serve ordered seeks.
4. **$O(\text{depth})$ Prefix Seeks**:
   - Prefix lookups skip non-matching branches in a single digit comparison without descending empty key spaces.

---

## 2. Benchmark Results

![RocksDB MemTable Benchmark: ExpanseMemTable vs SkipList vs VectorRep](../../docs/assets/bench_rocksdb.svg)

*(Measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit 7d87dff7; `benches/bench_memtable.cc` built `-O3` against release `libexpanse.so`; 100,000 keys, 16-byte key, 64-byte value payload; median of 3 back-to-back runs on an idle host (load < 0.4). The memory-footprint / B-entry column is deterministic byte accounting; throughput columns are wall-clock on this host. `SkipListRep`/`VectorRep` are the in-file reference implementations, not stock RocksDB.)*

| Benchmark Metric | ExpanseMemTable | Reference SkipListRep | VectorRep | Expanse vs SkipList |
|---|---:|---:|---:|---|
| **Memory Footprint** (100K keys) | **1.26 MB** (13.2 B/entry) | 14.0 MB (146.7 B/entry) | 1.0 MB (10.5 B/entry) | **11.11× Higher Key Density** |
| **Fill Random** (`fillrandom` insert) | **4.65 Mops/s** | 2.43 Mops/s | 204.81 Mops/s | **1.91× Faster Inserts** |
| **Point Lookup** (`readrandom`) | **273 ns/op** (3.66 Mops/s) | 541 ns/op (1.85 Mops/s) | 546 ns/op (1.83 Mops/s) | **1.98× Faster Lookups** |
| **Range Seek** (`seekrandom`) | **3.68 Mops/s** | 1.73 Mops/s | 3.90 Mops/s | **2.13× Faster Seeks** |
| **Sequential Scan** (`prefixscan` Iterator) | **160.84 Mops/s** | 16.40 Mops/s | 598.95 Mops/s | **9.81× Faster Ordered Scans** |
| **Batch Scan** (`ScanBatch` 1024-chunk) | **114.92 Mops/s** | N/A | N/A | **High-Throughput Batch Extraction** |

> `VectorRep` (an unordered append-only vector) wins on insert and unordered scan by construction but cannot serve ordered range seeks — it is included as a throughput ceiling, not an ordered-index competitor.

---

## 3. Quickstart & Integration Guide

### 3.1 Enabling ExpanseMemTable in RocksDB C++

Include `expanse_memtable.h` and set `options.memtable_factory`:

```cpp
#include <rocksdb/db.h>
#include <rocksdb/options.h>
#include "expanse_memtable.h"

int main() {
    rocksdb::DB* db = nullptr;
    rocksdb::Options options;
    options.create_if_missing = true;

    // Configure Expanse Pluggable MemTable Factory
    // Parameters:
    //   - leaf_capacity: number of entry pointers per cache-line leaf block (default: 64)
    //   - enable_prefix_trie: enable digital prefix indexing (default: true)
    options.memtable_factory = rocksdb::NewExpanseMemTableRepFactory(
        /*leaf_capacity=*/64,
        /*enable_prefix_trie=*/true
    );

    // Open RocksDB database
    rocksdb::Status s = rocksdb::DB::Open(options, "/tmp/rocksdb_expanse_demo", &db);
    assert(s.ok());

    // Standard RocksDB Operations
    db->Put(rocksdb::WriteOptions(), "user:1001", "{\"name\":\"Alice\"}");
    
    std::string val;
    db->Get(rocksdb::ReadOptions(), "user:1001", &val);

    // Range Scan using Expanse Iterator
    rocksdb::Iterator* it = db->NewIterator(rocksdb::ReadOptions());
    for (it->Seek("user:"); it->Valid() && it->key().starts_with("user:"); it->Next()) {
        std::cout << it->key().ToString() << " -> " << it->value().ToString() << std::endl;
    }
    delete it;
    delete db;
    return 0;
}
```

---

## 4. Building & Running Tests

### Prerequisites
- C++20 compatible compiler (`clang++` or `g++`)
- Rust toolchain (`cargo`) to build `libexpanse`

### Build and Run Unit Tests

```bash
# 1. Build libexpanse
cargo build -p expanse-capi

# 2. Build and run Expanse RocksDB MemTable tests
make -C integrations/rocksdb test

# 3. Run microbenchmarks
make -C integrations/rocksdb bench
```

### CMake Integration

```bash
mkdir -p integrations/rocksdb/build
cd integrations/rocksdb/build
cmake ..
make -j
ctest --output-on-failure
```

---

## 5. API Reference

### `rocksdb::ExpanseMemTableRep`
Implements `rocksdb::MemTableRep` with full support for:
- `Insert(KeyHandle handle)` / `InsertConcurrently(KeyHandle handle)`: Synchronized leaf insertion with automatic trie block split.
- `Contains(const char* key)`: Binary search within target leaf block.
- `Get(const LookupKey& k, ...)`: MVCC snapshot point lookups with descending sequence number resolution.
- `GetIterator(Arena* arena, bool is_reverse)`: Returns `ExpanseMemTableIterator` supporting `Seek`, `SeekForPrev`, `SeekToFirst`, `SeekToLast`, `Next`, `Prev`.
- `ScanBatch(size_t max_keys, Slice* out_keys, Slice* out_values)`: High-throughput batch scan extraction across chained sibling leaves.
- `internal_key()`, `user_key()`, `value()`: Zero-copy cached slice accessors on iterator avoiding redundant varint decoding.
- `SuggestCompactRange(Slice* begin, Slice* end)`: Bounds detection for efficient memtable flushes.

### `rocksdb::NewExpanseMemTableRepFactory(size_t leaf_capacity = 64, bool enable_prefix_trie = true)`
Returns a `std::shared_ptr<rocksdb::MemTableRepFactory>` ready to be assigned to `rocksdb::Options::memtable_factory`.

## Concurrent-read safety

The lock-free reader protocol in `ExpanseMemTableRep` is safe under concurrent writes. The earlier race — `SplitLeafBlock` nulling source entries before lowering the block count (stale-count `nullptr` deref in `Get`), and `Insert`'s in-place shift leaving the leaf array transiently unsorted (spurious binary-search miss) — was resolved by a **per-leaf seqlock with publication ordering** (#234) and **release/acquire happens-before on the reader path** (#236): each `LeafBlock` carries a `version` counter and its `entries`/`count` are published with `memory_order_release` and read with `memory_order_acquire`, so a reader either observes a consistent snapshot or retries. This is exercised in CI by the RocksDB memtable matrix under ThreadSanitizer (`test-rocksdb-memtable`, `sanitizer: tsan`) and a differential test. Issue [#229](https://github.com/orieg/expanse/issues/229) is closed.

