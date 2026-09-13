// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// bench_memtable.cc — Microbenchmark comparing ExpanseMemTable against SkipList and VectorRep.
//
// # Workload shape
//
// | Property | Value |
// |---|---|
// | `workload_id` | `rocksdb_memtable_single_threaded` |
// | `group` | 8 |
// | `population` | 100,000 entries: 16-byte user key (`usr_` + 12 zero-padded digits of `rng() % 10^10`), 64-byte value, `mt19937_64` seed 1337. Each arm encodes its own copy into its own `Arena`, so no arm reads another's bytes |
// | `insertion_order` | generator — inserted in the draw order of the suite PRNG; no sort and no shuffle is applied |
// | `probes_and_reuse` | 50,000 probes drawn as `keys[rng() % N]`, so every probe names a key that IS in the structure. The same probe vector is reused by `readrandom` and `seekrandom`, and by all three arms |
// | `hit_rate` | **~9% (measured 8.98%)**, not ~100% as "every probe names a present key" suggests. Entries carry `seq = 1000 + i` for i in [0, 100000), so sequence numbers run 1,000..100,999, while every `LookupKey` is built at snapshot 10,000. MVCC hides every entry newer than the query, so only the 9,001 keys with `1000 + i <= 10000` can be found — 9.001% of the population, and 8.98% of probes hit in practice. `readrandom` is therefore predominantly a miss-path measurement |
// | `miss_gen_method` | n/a — no miss keys are generated. The ~91% of probes that miss are *present* keys made invisible by the snapshot above, which descends to the key's own leaf and fails on sequence rather than terminating early in a different expanse. That is a different shape from a key-space miss and is not the same thing AGENTS.md §8.6's miss-shape rule asks for |
// | `value_dereference` | none — the `Get` callback takes `const char*` unnamed and only increments a counter, so no arm reads the stored value. In `--arm` mode that counter is printed as the `consumed` column, so the probe loop's result is consumed; in the human-readable mode it is not printed at all, and the loop survives only because `Get` is a virtual call into a separate object file. `seekrandom` consumes nothing in either mode: `Seek` returns void and the cursor it moves is never read |
// | `measured_region` | `chrono::high_resolution_clock` around each benchmark's probe or insert loop. Entry encoding, `LookupKey` construction and iterator construction are outside it. The three reps live to the end of `main`, so no destructor runs inside a timed window. In `--arm <phase>` mode the fixture build still runs in full (it is what the read phases need) and only the named phase is timed and reported, so each phase starts from the same state instead of being warmed by the phases before it in a single process |
// | `arm_symmetry` | identical `BenchBytewiseComparator`, identical probe and key streams, one `Arena` per arm. `ReferenceSkipListRep` models a variable-height `InlineSkipList` tower (8 B key pointer + `height` × 8 B, E[height] = 4/3) after the #372 strawman retraction; `ReferenceVectorRep` models an unindexed append vector. All three implementations of a phase are timed inside ONE invocation, so a published ratio's two arms share one host state; the `consumed` column is the same for all of them and the driver refuses a round where it is not, which is what makes a scan that terminated early visible rather than merely fast. Within `prefixscan` the iterator arm runs before the batch arm and warms the data for it — pre-existing, and unchanged here. Single-threaded throughout: no arm is thread-safe and none is driven concurrently (see `bench_memtable_concurrent.cc` for the concurrent arm) |
// | `statistics` | point estimates only; this binary emits no interval. `docs/benchmarks/rocksdb_memtable/scripts/single_threaded_bench.py` owns the rounds — one `--arm <phase>` process per measured cell, phases interleaved within each round — and turns them into BCa 95% per-arm intervals plus **paired** per-round ratio intervals, with a load snapshot and a busy-CPU delta per cell (#868) |
// | `verdict` | published in `docs/benchmarks/rocksdb_memtable/METHODOLOGY.md` §2, and **stale**: measured at `6cb64b45`, since when `Insert`, `Get`, `ScanBatch` and the iterator have all changed against a newer engine. Re-measurement is tracked in #868 |
//
// ## The hit rate is a property of the fixture, not a choice
//
// The ~9% above is not a declared design target that this table is recording;
// it is what the seq/snapshot arithmetic produces, found by measuring the
// callback count when this declaration was written (#802 extended
// `scripts/check_bench_shapes.py` to the C++ harnesses, which is what forced
// the declaration to exist at all). The published `readrandom` and `seekrandom`
// ratios are symmetric across arms and so remain structurally sound as ratios
// (§8.10), but they do not describe a hit-heavy point lookup. Changing the
// fixture would change what the cells mean, so it is not done here.

#include <algorithm>
#include <chrono>
#include <cstdlib>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <memory>
#include <random>
#include <sstream>
#include <string>
#include <vector>

#include "expanse_memtable.h"

using namespace rocksdb;

// Standard Bytewise Comparator
class BenchBytewiseComparator : public MemTableRep::KeyComparator {
public:
    int operator()(const char* a, const char* b) const override {
        Slice slice_a = expanse_rocksdb::GetLengthPrefixedSlice(a);
        Slice slice_b = expanse_rocksdb::GetLengthPrefixedSlice(b);
        return expanse_rocksdb::CompareInternalKeys(slice_a, slice_b);
    }
};

// Simple Reference SkipList MemTable implementation for benchmarking comparison.
//
// Node layout note (#372): an earlier revision embedded the full
// `Node* next[kMaxHeight]` tower (16 pointers, 144 bytes) in every node
// regardless of its drawn height, *and* added `(height - 1) * sizeof(Node*)`
// on top — a strawman that inflated the skiplist's memory footprint to
// ~146.7 B/entry and with it the published "11.1× higher key density"
// headline. Real skiplists (RocksDB's `InlineSkipList`, LevelDB's `SkipList`)
// allocate variable-height nodes whose tower occupies exactly `height`
// pointer slots. This node mirrors that: per-node memory is the key pointer
// (8 B) plus `height` next-pointers (8 B each); with the geometric height
// distribution used here (P(grow) = 1/4, E[height] = 4/3) the expected
// overhead is ~18.7 B/entry. Any density or throughput figure derived from
// the old layout is invalid; results must be re-measured with this baseline.
class ReferenceSkipListRep : public MemTableRep {
public:
    static constexpr int kMaxHeight = 16;
    struct Node {
        const char* key;
        // Variable-height tower: allocated with `height` slots (>= 1); slots
        // beyond index 0 live in the over-allocation past the struct end,
        // exactly like InlineSkipList's trailing atomic pointer array.
        Node* next[1];
    };

    explicit ReferenceSkipListRep(const MemTableRep::KeyComparator& cmp, Allocator* alloc)
        : MemTableRep(alloc), cmp_(cmp), rng_(0xdeadbeef), max_height_(1) {
        head_ = AllocateNode("", 0, kMaxHeight);
        for (int i = 0; i < kMaxHeight; ++i) head_->next[i] = nullptr;
    }

    ~ReferenceSkipListRep() override = default;

    int RandomHeight() {
        int height = 1;
        while (height < kMaxHeight && (rng_() & 3) == 0) {
            height++;
        }
        return height;
    }

    Node* AllocateNode(const char* key, size_t /*size*/, int height) {
        // Key pointer + `height` tower pointers — the fair, InlineSkipList-like
        // per-node cost (no statically embedded kMaxHeight array).
        size_t bytes = sizeof(const char*) + static_cast<size_t>(height) * sizeof(Node*);
        Node* node = reinterpret_cast<Node*>(allocator_->AllocateAligned(bytes));
        node->key = key;
        allocated_bytes_ += bytes;
        return node;
    }

    void Insert(KeyHandle handle) override {
        const char* entry = static_cast<const char*>(handle);
        Node* update[kMaxHeight];
        Node* x = head_;
        for (int i = max_height_ - 1; i >= 0; --i) {
            while (x->next[i] != nullptr && cmp_(x->next[i]->key, entry) < 0) {
                x = x->next[i];
            }
            update[i] = x;
        }

        int height = RandomHeight();
        if (height > max_height_) {
            for (int i = max_height_; i < height; ++i) {
                update[i] = head_;
            }
            max_height_ = height;
        }

        Node* node = AllocateNode(entry, 0, height);
        for (int i = 0; i < height; ++i) {
            node->next[i] = update[i]->next[i];
            update[i]->next[i] = node;
        }
        count_++;
    }

    bool Contains(const char* key) const override {
        Node* x = head_;
        for (int i = max_height_ - 1; i >= 0; --i) {
            while (x->next[i] != nullptr && cmp_(x->next[i]->key, key) < 0) {
                x = x->next[i];
            }
        }
        x = x->next[0];
        return (x != nullptr && cmp_(x->key, key) == 0);
    }

    void Get(const LookupKey& k, void* callback_args, bool (*callback_func)(void* arg, const char* entry)) override {
        Node* x = head_;
        for (int i = max_height_ - 1; i >= 0; --i) {
            while (x->next[i] != nullptr && cmp_(k.internal_key(), x->next[i]->key) > 0) {
                x = x->next[i];
            }
        }
        x = x->next[0];
        Slice user_key = k.user_key();
        while (x != nullptr) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(x->key);
            if (ikey.size() < 8) break;
            Slice ukey(ikey.data(), ikey.size() - 8);
            if (ukey != user_key) break;
            if (!callback_func(callback_args, x->key)) break;
            x = x->next[0];
        }
    }

    class Iterator : public MemTableRep::Iterator {
    public:
        Iterator(const ReferenceSkipListRep* list) : list_(list), node_(nullptr) {}
        bool Valid() const override { return node_ != nullptr; }
        const char* key() const override { return node_->key; }
        void Next() override { node_ = node_->next[0]; }
        void Prev() override { /* SkipList Prev is slow without backlink */ }
        void Seek(const Slice& internal_key, const char* memtable_key) override {
            Node* x = list_->head_;
            for (int i = list_->max_height_ - 1; i >= 0; --i) {
                while (x->next[i] != nullptr) {
                    int c = memtable_key != nullptr
                        ? list_->cmp_(x->next[i]->key, memtable_key)
                        : list_->cmp_(internal_key, x->next[i]->key);
                    if (c < 0) {
                        x = x->next[i];
                    } else {
                        break;
                    }
                }
            }
            node_ = x->next[0];
        }
        void SeekForPrev(const Slice&, const char*) override {}
        void SeekToFirst() override { node_ = list_->head_->next[0]; }
        void SeekToLast() override {}
    private:
        const ReferenceSkipListRep* list_;
        Node* node_;
    };

    MemTableRep::Iterator* GetIterator(Arena* arena = nullptr, bool = false) override {
        if (arena) {
            void* mem = arena->AllocateAligned(sizeof(Iterator));
            return new (mem) Iterator(this);
        }
        return new Iterator(this);
    }

    size_t ApproximateMemoryUsage() override {
        return sizeof(ReferenceSkipListRep) + allocated_bytes_;
    }

private:
    const MemTableRep::KeyComparator& cmp_;
    std::mt19937 rng_;
    Node* head_;
    int max_height_;
    size_t allocated_bytes_{0};
    uint64_t count_{0};
};

// Reference VectorRep MemTable implementation
class ReferenceVectorRep : public MemTableRep {
public:
    explicit ReferenceVectorRep(const MemTableRep::KeyComparator& cmp, Allocator* alloc)
        : MemTableRep(alloc), cmp_(cmp), is_sorted_(false) {}

    void Insert(KeyHandle handle) override {
        entries_.push_back(static_cast<const char*>(handle));
        is_sorted_ = false;
    }

    void EnsureSorted() const {
        if (!is_sorted_) {
            std::sort(const_cast<std::vector<const char*>&>(entries_).begin(),
                      const_cast<std::vector<const char*>&>(entries_).end(),
                      [&](const char* a, const char* b) {
                          return cmp_(a, b) < 0;
                      });
            is_sorted_ = true;
        }
    }

    bool Contains(const char* key) const override {
        EnsureSorted();
        auto it = std::lower_bound(entries_.begin(), entries_.end(), key,
                                   [&](const char* a, const char* b) {
                                       return cmp_(a, b) < 0;
                                   });
        return (it != entries_.end() && cmp_(*it, key) == 0);
    }

    void Get(const LookupKey& k, void* callback_args, bool (*callback_func)(void* arg, const char* entry)) override {
        EnsureSorted();
        auto it = std::lower_bound(entries_.begin(), entries_.end(), k.internal_key(),
                                   [&](const char* a, const Slice& b) {
                                       return cmp_(b, a) > 0;
                                   });
        Slice user_key = k.user_key();
        while (it != entries_.end()) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(*it);
            if (ikey.size() < 8) break;
            Slice ukey(ikey.data(), ikey.size() - 8);
            if (ukey != user_key) break;
            if (!callback_func(callback_args, *it)) break;
            ++it;
        }
    }

    class Iterator : public MemTableRep::Iterator {
    public:
        Iterator(const ReferenceVectorRep* rep) : rep_(rep), idx_(-1) { rep_->EnsureSorted(); }
        bool Valid() const override { return idx_ >= 0 && idx_ < static_cast<int>(rep_->entries_.size()); }
        const char* key() const override { return rep_->entries_[idx_]; }
        void Next() override { idx_++; }
        void Prev() override { idx_--; }
        void Seek(const Slice& internal_key, const char* memtable_key) override {
            rep_->EnsureSorted();
            if (memtable_key != nullptr) {
                auto it = std::lower_bound(rep_->entries_.begin(), rep_->entries_.end(), memtable_key,
                                           [&](const char* a, const char* b) { return rep_->cmp_(a, b) < 0; });
                idx_ = (it != rep_->entries_.end()) ? static_cast<int>(it - rep_->entries_.begin()) : static_cast<int>(rep_->entries_.size());
            } else {
                auto it = std::lower_bound(rep_->entries_.begin(), rep_->entries_.end(), internal_key,
                                           [&](const char* a, const Slice& b) { return rep_->cmp_(b, a) > 0; });
                idx_ = (it != rep_->entries_.end()) ? static_cast<int>(it - rep_->entries_.begin()) : static_cast<int>(rep_->entries_.size());
            }
        }
        void SeekForPrev(const Slice&, const char*) override {}
        void SeekToFirst() override { idx_ = 0; }
        void SeekToLast() override { idx_ = static_cast<int>(rep_->entries_.size()) - 1; }
    private:
        const ReferenceVectorRep* rep_;
        int idx_;
    };

    MemTableRep::Iterator* GetIterator(Arena* arena = nullptr, bool = false) override {
        if (arena) {
            void* mem = arena->AllocateAligned(sizeof(Iterator));
            return new (mem) Iterator(this);
        }
        return new Iterator(this);
    }

    size_t ApproximateMemoryUsage() override {
        return sizeof(ReferenceVectorRep) + entries_.capacity() * sizeof(const char*);
    }

private:
    const MemTableRep::KeyComparator& cmp_;
    mutable std::vector<const char*> entries_;
    mutable bool is_sorted_;
};

// Helper to encode a Memtable entry
static const char* BenchEncodeEntry(
    Arena& arena,
    const std::string& user_key,
    SequenceNumber seq,
    ValueType type,
    const std::string& value
) {
    size_t ikey_len = user_key.size() + 8;
    size_t val_len = value.size();
    size_t total_buf_size = 5 + ikey_len + 5 + val_len;

    char* buf = arena.Allocate(total_buf_size);
    char* p = expanse_rocksdb::EncodeVarint32(buf, static_cast<uint32_t>(ikey_len));
    memcpy(p, user_key.data(), user_key.size());
    p += user_key.size();

    uint64_t trailer = (seq << 8) | static_cast<uint64_t>(type);
    for (int i = 0; i < 8; ++i) {
        p[i] = static_cast<char>((trailer >> (i * 8)) & 0xff);
    }
    p += 8;

    p = expanse_rocksdb::EncodeVarint32(p, static_cast<uint32_t>(val_len));
    if (val_len > 0) {
        memcpy(p, value.data(), val_len);
        p += val_len;
    }
    return buf;
}

// ---------------------------------------------------------------------------
// Invocation modes (#868)
// ---------------------------------------------------------------------------
//
// No arguments: the full human-readable table, byte-identical to what this
// binary printed before `--arm` existed, so `make bench` and
// `scripts/generate_bench_svg.py` are untouched.
//
// `--arm <name> [--round N]`: build the fixture, time **only** that phase, and
// print one CSV row per implementation. One phase per invocation is what gives
// the Python driver a process boundary per measured cell, which is the only way
// `load.foreign_busy_cpus` can be attributed to a cell rather than to a whole
// sweep (`scripts/bench_provenance.py`). `bench_memtable_concurrent.cc` was
// built that way for the same reason.
//
// The three implementations stay together inside one invocation: a published
// ratio's two arms must be timed under the same host state, and a boundary per
// implementation would make each ratio a comparison across two contention
// windows rather than one.
static std::string g_arm;          // empty => every phase (human-readable mode)
static bool g_csv = false;
static int g_round = 0;

// True when the named phase is to be timed and reported this invocation.
static bool want(const char* arm) { return g_arm.empty() || g_arm == arm; }

// One CSV row. `ops`/`elapsed_s`/`mops` are zero on a census row and
// `bytes_total`/`bytes_per_entry` are zero on a timed row; the driver rejects a
// row that carries both or neither, so a phase that silently produced nothing
// cannot reach an artifact as a zero (AGENTS.md section 8.1).
struct CsvRow {
    std::string arm;
    std::string implementation;
    uint64_t ops;
    double elapsed_s;
    double mops;
    // The phase's own output counter, written to stdout so the timed loop's
    // result is consumed rather than discarded (AGENTS.md section 8.6).
    // `readrandom`'s is the `Get` callback count — incremented but never
    // printed in the human-readable mode, which is the one mode this column
    // fixes. `seekrandom` has none: `Seek` returns void and only moves the
    // iterator's cursor, so it is 0 and that remains a DCE exposure held off
    // only by the virtual call into a separate object file.
    uint64_t consumed;
    uint64_t bytes_total;
    double bytes_per_entry;
};
static std::vector<CsvRow> g_rows;

static void emit_timed(const char* arm, const char* impl, uint64_t ops, double secs,
                       uint64_t consumed) {
    if (!g_csv) return;
    g_rows.push_back({arm, impl, ops, secs, (ops / secs) / 1e6, consumed, 0, 0.0});
}

static void emit_census(const char* impl, uint64_t entries, uint64_t bytes) {
    if (!g_csv) return;
    g_rows.push_back({"memory", impl, 0, 0.0, 0.0, 0, bytes,
                      static_cast<double>(bytes) / static_cast<double>(entries)});
}

int main(int argc, char** argv) {
    static const char* kArms[] = {"fillrandom", "readrandom", "seekrandom",
                                  "prefixscan", "memory"};
    for (int i = 1; i < argc; ++i) {
        std::string a = argv[i];
        if (a == "--arm" && i + 1 < argc) {
            g_arm = argv[++i];
            g_csv = true;
        } else if (a == "--round" && i + 1 < argc) {
            g_round = std::atoi(argv[++i]);
        } else {
            std::cerr << "unknown argument: " << a << "\n"
                      << "usage: bench_memtable [--arm <"
                      << "fillrandom|readrandom|seekrandom|prefixscan|memory"
                      << "> [--round N]]\n";
            return 2;
        }
    }
    if (g_csv) {
        bool known = false;
        for (const char* a : kArms) known = known || (g_arm == a);
        if (!known) {
            // Refuse by name rather than running the whole suite under an
            // unrecognised arm (AGENTS.md section 8.1).
            std::cerr << "unknown --arm " << g_arm << "\n";
            return 2;
        }
    }

    if (!g_csv) {
    std::cout << "==========================================================================" << std::endl;
    std::cout << " RocksDB Pluggable MemTable Microbenchmark Suite: Expanse vs SkipList" << std::endl;
    std::cout << "==========================================================================" << std::endl;
    }

    const int N = 100000; // 100K entries
    const int val_size = 64; // 64-byte payload
    std::string val(val_size, 'x');

    std::vector<std::string> keys;
    keys.reserve(N);
    std::mt19937_64 rng(1337);

    for (int i = 0; i < N; ++i) {
        std::ostringstream ss;
        ss << "usr_" << std::setw(12) << std::setfill('0') << (rng() % 10000000000ULL);
        keys.push_back(ss.str());
    }

    BenchBytewiseComparator cmp;

    // Prepare encoded entries in arena
    Arena arena_expanse(4 * 1024 * 1024);
    Arena arena_skiplist(4 * 1024 * 1024);
    Arena arena_vector(4 * 1024 * 1024);

    std::vector<const char*> expanse_entries;
    std::vector<const char*> skiplist_entries;
    std::vector<const char*> vector_entries;
    expanse_entries.reserve(N);
    skiplist_entries.reserve(N);
    vector_entries.reserve(N);

    for (int i = 0; i < N; ++i) {
        expanse_entries.push_back(BenchEncodeEntry(arena_expanse, keys[i], 1000 + i, kTypeValue, val));
        skiplist_entries.push_back(BenchEncodeEntry(arena_skiplist, keys[i], 1000 + i, kTypeValue, val));
        vector_entries.push_back(BenchEncodeEntry(arena_vector, keys[i], 1000 + i, kTypeValue, val));
    }

    // ------------------------------------------------------------------------
    // Benchmark 1: Fill Random (Inserts)
    // ------------------------------------------------------------------------
    // Always executed: the fill IS the build, so every read phase needs it.
    // Timed in every mode; reported only when it is the selected arm.
    if (!g_csv) std::cout << "\n--- Benchmark 1: fillrandom (N = " << N << ") ---" << std::endl;

    ExpanseMemTableRep expanse_rep(cmp, &arena_expanse, nullptr, nullptr, 64);
    ReferenceSkipListRep skiplist_rep(cmp, &arena_skiplist);
    ReferenceVectorRep vector_rep(cmp, &arena_vector);

    // Expanse Insert
    auto t0 = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < N; ++i) {
        expanse_rep.Insert(const_cast<char*>(expanse_entries[i]));
    }
    auto t1 = std::chrono::high_resolution_clock::now();
    double expanse_insert_sec = std::chrono::duration<double>(t1 - t0).count();
    double expanse_insert_mops = (N / expanse_insert_sec) / 1e6;

    // SkipList Insert
    t0 = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < N; ++i) {
        skiplist_rep.Insert(const_cast<char*>(skiplist_entries[i]));
    }
    t1 = std::chrono::high_resolution_clock::now();
    double skiplist_insert_sec = std::chrono::duration<double>(t1 - t0).count();
    double skiplist_insert_mops = (N / skiplist_insert_sec) / 1e6;

    // Vector Insert
    t0 = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < N; ++i) {
        vector_rep.Insert(const_cast<char*>(vector_entries[i]));
    }
    t1 = std::chrono::high_resolution_clock::now();
    double vector_insert_sec = std::chrono::duration<double>(t1 - t0).count();
    double vector_insert_mops = (N / vector_insert_sec) / 1e6;

    if (!g_csv) {
    std::cout << "  ExpanseMemTable: " << std::fixed << std::setprecision(2) << expanse_insert_mops << " Mops/s (" << (expanse_insert_sec * 1000.0) << " ms)" << std::endl;
    std::cout << "  SkipListRep:     " << std::fixed << std::setprecision(2) << skiplist_insert_mops << " Mops/s (" << (skiplist_insert_sec * 1000.0) << " ms)" << std::endl;
    std::cout << "  VectorRep:       " << std::fixed << std::setprecision(2) << vector_insert_mops << " Mops/s (" << (vector_insert_sec * 1000.0) << " ms)" << std::endl;
    }
    if (want("fillrandom")) {
        emit_timed("fillrandom", "ExpanseMemTable", N, expanse_insert_sec, N);
        emit_timed("fillrandom", "SkipListRep", N, skiplist_insert_sec, N);
        emit_timed("fillrandom", "VectorRep", N, vector_insert_sec, N);
    }

    // ------------------------------------------------------------------------
    // Benchmark 2: Read Random (Point Lookups)
    // ------------------------------------------------------------------------
    if (!g_csv) std::cout << "\n--- Benchmark 2: readrandom (Point Lookups, 50K queries) ---" << std::endl;
    const int query_count = 50000;
    std::vector<LookupKey> queries;
    queries.reserve(query_count);
    for (int i = 0; i < query_count; ++i) {
        queries.emplace_back(Slice(keys[rng() % N]), 10000);
    }

    if (want("readrandom")) {
    // Expanse Read
    t0 = std::chrono::high_resolution_clock::now();
    uint64_t expanse_found = 0;
    for (int i = 0; i < query_count; ++i) {
        expanse_rep.Get(queries[i], &expanse_found, [](void* arg, const char*) -> bool {
            (*static_cast<uint64_t*>(arg))++;
            return false;
        });
    }
    t1 = std::chrono::high_resolution_clock::now();
    double expanse_read_sec = std::chrono::duration<double>(t1 - t0).count();
    double expanse_read_mops = (query_count / expanse_read_sec) / 1e6;
    double expanse_read_ns = (expanse_read_sec * 1e9) / query_count;

    // SkipList Read
    t0 = std::chrono::high_resolution_clock::now();
    uint64_t skiplist_found = 0;
    for (int i = 0; i < query_count; ++i) {
        skiplist_rep.Get(queries[i], &skiplist_found, [](void* arg, const char*) -> bool {
            (*static_cast<uint64_t*>(arg))++;
            return false;
        });
    }
    t1 = std::chrono::high_resolution_clock::now();
    double skiplist_read_sec = std::chrono::duration<double>(t1 - t0).count();
    double skiplist_read_mops = (query_count / skiplist_read_sec) / 1e6;
    double skiplist_read_ns = (skiplist_read_sec * 1e9) / query_count;

    // Vector Read
    t0 = std::chrono::high_resolution_clock::now();
    uint64_t vector_found = 0;
    for (int i = 0; i < query_count; ++i) {
        vector_rep.Get(queries[i], &vector_found, [](void* arg, const char*) -> bool {
            (*static_cast<uint64_t*>(arg))++;
            return false;
        });
    }
    t1 = std::chrono::high_resolution_clock::now();
    double vector_read_sec = std::chrono::duration<double>(t1 - t0).count();
    double vector_read_mops = (query_count / vector_read_sec) / 1e6;
    double vector_read_ns = (vector_read_sec * 1e9) / query_count;

    if (!g_csv) {
    std::cout << "  ExpanseMemTable: " << std::fixed << std::setprecision(2) << expanse_read_mops << " Mops/s (" << expanse_read_ns << " ns/op)" << std::endl;
    std::cout << "  SkipListRep:     " << std::fixed << std::setprecision(2) << skiplist_read_mops << " Mops/s (" << skiplist_read_ns << " ns/op)" << std::endl;
    std::cout << "  VectorRep:       " << std::fixed << std::setprecision(2) << vector_read_mops << " Mops/s (" << vector_read_ns << " ns/op)" << std::endl;
    }
    emit_timed("readrandom", "ExpanseMemTable", query_count, expanse_read_sec, expanse_found);
    emit_timed("readrandom", "SkipListRep", query_count, skiplist_read_sec, skiplist_found);
    emit_timed("readrandom", "VectorRep", query_count, vector_read_sec, vector_found);
    }

    // ------------------------------------------------------------------------
    // Benchmark 3: Seek Random (Range Seeks)
    // ------------------------------------------------------------------------
    if (!g_csv) std::cout << "\n--- Benchmark 3: seekrandom (Range Seeks, 50K queries) ---" << std::endl;

    // Built unconditionally: `prefixscan` walks the same iterators, and the
    // construction is outside every timed window in either mode.
    std::unique_ptr<MemTableRep::Iterator> it_expanse(expanse_rep.GetIterator());
    std::unique_ptr<MemTableRep::Iterator> it_skiplist(skiplist_rep.GetIterator());
    std::unique_ptr<MemTableRep::Iterator> it_vector(vector_rep.GetIterator());

    if (want("seekrandom")) {
    t0 = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < query_count; ++i) {
        it_expanse->Seek(queries[i].internal_key(), queries[i].memtable_key().data());
    }
    t1 = std::chrono::high_resolution_clock::now();
    double expanse_seek_sec = std::chrono::duration<double>(t1 - t0).count();
    double expanse_seek_mops = (query_count / expanse_seek_sec) / 1e6;

    t0 = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < query_count; ++i) {
        it_skiplist->Seek(queries[i].internal_key(), queries[i].memtable_key().data());
    }
    t1 = std::chrono::high_resolution_clock::now();
    double skiplist_seek_sec = std::chrono::duration<double>(t1 - t0).count();
    double skiplist_seek_mops = (query_count / skiplist_seek_sec) / 1e6;

    t0 = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < query_count; ++i) {
        it_vector->Seek(queries[i].internal_key(), queries[i].memtable_key().data());
    }
    t1 = std::chrono::high_resolution_clock::now();
    double vector_seek_sec = std::chrono::duration<double>(t1 - t0).count();
    double vector_seek_mops = (query_count / vector_seek_sec) / 1e6;

    if (!g_csv) {
    std::cout << "  ExpanseMemTable: " << std::fixed << std::setprecision(2) << expanse_seek_mops << " Mops/s" << std::endl;
    std::cout << "  SkipListRep:     " << std::fixed << std::setprecision(2) << skiplist_seek_mops << " Mops/s" << std::endl;
    std::cout << "  VectorRep:       " << std::fixed << std::setprecision(2) << vector_seek_mops << " Mops/s" << std::endl;
    }
    // `consumed` is 0 on every seek arm: `Seek` returns void and the cursor it
    // moves is never read, so there is no output to write out.
    emit_timed("seekrandom", "ExpanseMemTable", query_count, expanse_seek_sec, 0);
    emit_timed("seekrandom", "SkipListRep", query_count, skiplist_seek_sec, 0);
    emit_timed("seekrandom", "VectorRep", query_count, vector_seek_sec, 0);
    }

    // ------------------------------------------------------------------------
    // Benchmark 4: Prefix Scan (Sequential Traversal)
    // ------------------------------------------------------------------------
    if (!g_csv) std::cout << "\n--- Benchmark 4: prefixscan (Sequential Scan across 100K entries) ---" << std::endl;

    if (want("prefixscan")) {
    t0 = std::chrono::high_resolution_clock::now();
    it_expanse->SeekToFirst();
    uint64_t expanse_scan_count = 0;
    while (it_expanse->Valid()) {
        expanse_scan_count++;
        it_expanse->Next();
    }
    t1 = std::chrono::high_resolution_clock::now();
    double expanse_scan_sec = std::chrono::duration<double>(t1 - t0).count();
    double expanse_scan_mops = (expanse_scan_count / expanse_scan_sec) / 1e6;

    // Expanse Batch Scan (1024 keys per batch)
    t0 = std::chrono::high_resolution_clock::now();
    it_expanse->SeekToFirst();
    uint64_t expanse_batch_scan_count = 0;
    constexpr size_t kBatchSize = 1024;
    std::vector<Slice> batch_keys(kBatchSize);
    std::vector<Slice> batch_vals(kBatchSize);
    while (it_expanse->Valid()) {
        size_t n = ScanBatch(it_expanse.get(), kBatchSize, batch_keys.data(), batch_vals.data());
        if (n == 0) break;
        expanse_batch_scan_count += n;
    }
    t1 = std::chrono::high_resolution_clock::now();
    double expanse_batch_sec = std::chrono::duration<double>(t1 - t0).count();
    double expanse_batch_mops = (expanse_batch_scan_count / expanse_batch_sec) / 1e6;

    t0 = std::chrono::high_resolution_clock::now();
    it_skiplist->SeekToFirst();
    uint64_t skiplist_scan_count = 0;
    while (it_skiplist->Valid()) {
        skiplist_scan_count++;
        it_skiplist->Next();
    }
    t1 = std::chrono::high_resolution_clock::now();
    double skiplist_scan_sec = std::chrono::duration<double>(t1 - t0).count();
    double skiplist_scan_mops = (skiplist_scan_count / skiplist_scan_sec) / 1e6;

    t0 = std::chrono::high_resolution_clock::now();
    it_vector->SeekToFirst();
    uint64_t vector_scan_count = 0;
    while (it_vector->Valid()) {
        vector_scan_count++;
        it_vector->Next();
    }
    t1 = std::chrono::high_resolution_clock::now();
    double vector_scan_sec = std::chrono::duration<double>(t1 - t0).count();
    double vector_scan_mops = (vector_scan_count / vector_scan_sec) / 1e6;

    if (!g_csv) {
    std::cout << "  ExpanseMemTable (Iterator): " << std::fixed << std::setprecision(2) << expanse_scan_mops << " Mops/s" << std::endl;
    std::cout << "  ExpanseMemTable (Batch):    " << std::fixed << std::setprecision(2) << expanse_batch_mops << " Mops/s" << std::endl;
    std::cout << "  SkipListRep:                " << std::fixed << std::setprecision(2) << skiplist_scan_mops << " Mops/s" << std::endl;
    std::cout << "  VectorRep:                  " << std::fixed << std::setprecision(2) << vector_scan_mops << " Mops/s" << std::endl;
    }
    // Each scan arm's op count IS its entry count, so `ops` and `consumed`
    // agree here; the driver checks that they do, which is what makes a scan
    // that stopped early visible rather than merely fast.
    emit_timed("prefixscan", "ExpanseMemTable (Iterator)", expanse_scan_count, expanse_scan_sec, expanse_scan_count);
    emit_timed("prefixscan", "ExpanseMemTable (Batch)", expanse_batch_scan_count, expanse_batch_sec, expanse_batch_scan_count);
    emit_timed("prefixscan", "SkipListRep", skiplist_scan_count, skiplist_scan_sec, skiplist_scan_count);
    emit_timed("prefixscan", "VectorRep", vector_scan_count, vector_scan_sec, vector_scan_count);
    }

    // ------------------------------------------------------------------------
    // Benchmark 5: Memory Density & Footprint Analysis
    // ------------------------------------------------------------------------
    // Always computed: deterministic allocator accounting, no timed window.
    if (!g_csv) std::cout << "\n--- Memory Density & Footprint Analysis ---" << std::endl;
    size_t mem_expanse = expanse_rep.ApproximateMemoryUsage();
    size_t mem_skiplist = skiplist_rep.ApproximateMemoryUsage();
    size_t mem_vector = vector_rep.ApproximateMemoryUsage();

    double bytes_per_key_expanse = static_cast<double>(mem_expanse) / N;
    double bytes_per_key_skiplist = static_cast<double>(mem_skiplist) / N;
    double bytes_per_key_vector = static_cast<double>(mem_vector) / N;

    if (!g_csv) {
    std::cout << "  ExpanseMemTable: " << (mem_expanse / (1024.0 * 1024.0)) << " MB (" << std::fixed << std::setprecision(1) << bytes_per_key_expanse << " B/entry)" << std::endl;
    std::cout << "  SkipListRep:     " << (mem_skiplist / (1024.0 * 1024.0)) << " MB (" << std::fixed << std::setprecision(1) << bytes_per_key_skiplist << " B/entry)" << std::endl;
    std::cout << "  VectorRep:       " << (mem_vector / (1024.0 * 1024.0)) << " MB (" << std::fixed << std::setprecision(1) << bytes_per_key_vector << " B/entry)" << std::endl;
    std::cout << "  => Key Density Advantage vs SkipList: " << std::fixed << std::setprecision(2)
              << (bytes_per_key_skiplist / bytes_per_key_expanse) << "x Higher Key Density in RAM!" << std::endl;
    }
    if (want("memory")) {
        emit_census("ExpanseMemTable", N, mem_expanse);
        emit_census("SkipListRep", N, mem_skiplist);
        emit_census("VectorRep", N, mem_vector);
    }

    if (!g_csv) {
    std::cout << "\n==========================================================================" << std::endl;
    std::cout << " Microbenchmark Completed Successfully!" << std::endl;
    std::cout << "==========================================================================" << std::endl;
    }

    if (g_csv) {
        // Full precision on `elapsed_s`: the driver recomputes Mops/s from
        // `ops / elapsed_s` and cross-checks it against the `mops` column, so
        // a rounded seconds field would make the two disagree by construction.
        std::cout << "# rocksdb_memtable_single_threaded\n";
        std::cout << "arm,implementation,round,ops,elapsed_s,mops,consumed,bytes_total,bytes_per_entry\n";
        for (const CsvRow& r : g_rows) {
            std::cout << r.arm << "," << r.implementation << "," << g_round << ","
                      << r.ops << ","
                      << std::setprecision(12) << std::fixed << r.elapsed_s << ","
                      << std::setprecision(9) << r.mops << ","
                      << r.consumed << "," << r.bytes_total << ","
                      << std::setprecision(6) << r.bytes_per_entry << "\n";
        }
        if (g_rows.empty()) {
            // Unreachable while every declared arm emits rows; if it is ever
            // reached the arm produced nothing and must not look like a pass.
            std::cerr << "arm " << g_arm << " produced no rows\n";
            return 1;
        }
    }

    return 0;
}
