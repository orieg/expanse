// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// bench_memtable.cc — Microbenchmark comparing ExpanseMemTable against SkipList and VectorRep.

#include <algorithm>
#include <chrono>
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

// Simple Reference SkipList MemTable implementation for benchmarking comparison
class ReferenceSkipListRep : public MemTableRep {
public:
    static constexpr int kMaxHeight = 16;
    struct Node {
        const char* key;
        Node* next[kMaxHeight];
        int height;
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
        size_t bytes = sizeof(Node) + (height - 1) * sizeof(Node*);
        Node* node = reinterpret_cast<Node*>(allocator_->AllocateAligned(bytes));
        node->key = key;
        node->height = height;
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

int main() {
    std::cout << "==========================================================================" << std::endl;
    std::cout << " RocksDB Pluggable MemTable Microbenchmark Suite: Expanse vs SkipList" << std::endl;
    std::cout << "==========================================================================" << std::endl;

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
    std::cout << "\n--- Benchmark 1: fillrandom (N = " << N << ") ---" << std::endl;

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

    std::cout << "  ExpanseMemTable: " << std::fixed << std::setprecision(2) << expanse_insert_mops << " Mops/s (" << (expanse_insert_sec * 1000.0) << " ms)" << std::endl;
    std::cout << "  SkipListRep:     " << std::fixed << std::setprecision(2) << skiplist_insert_mops << " Mops/s (" << (skiplist_insert_sec * 1000.0) << " ms)" << std::endl;
    std::cout << "  VectorRep:       " << std::fixed << std::setprecision(2) << vector_insert_mops << " Mops/s (" << (vector_insert_sec * 1000.0) << " ms)" << std::endl;

    // ------------------------------------------------------------------------
    // Benchmark 2: Read Random (Point Lookups)
    // ------------------------------------------------------------------------
    std::cout << "\n--- Benchmark 2: readrandom (Point Lookups, 50K queries) ---" << std::endl;
    const int query_count = 50000;
    std::vector<LookupKey> queries;
    queries.reserve(query_count);
    for (int i = 0; i < query_count; ++i) {
        queries.emplace_back(Slice(keys[rng() % N]), 10000);
    }

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

    std::cout << "  ExpanseMemTable: " << std::fixed << std::setprecision(2) << expanse_read_mops << " Mops/s (" << expanse_read_ns << " ns/op)" << std::endl;
    std::cout << "  SkipListRep:     " << std::fixed << std::setprecision(2) << skiplist_read_mops << " Mops/s (" << skiplist_read_ns << " ns/op)" << std::endl;
    std::cout << "  VectorRep:       " << std::fixed << std::setprecision(2) << vector_read_mops << " Mops/s (" << vector_read_ns << " ns/op)" << std::endl;

    // ------------------------------------------------------------------------
    // Benchmark 3: Seek Random (Range Seeks)
    // ------------------------------------------------------------------------
    std::cout << "\n--- Benchmark 3: seekrandom (Range Seeks, 50K queries) ---" << std::endl;

    std::unique_ptr<MemTableRep::Iterator> it_expanse(expanse_rep.GetIterator());
    std::unique_ptr<MemTableRep::Iterator> it_skiplist(skiplist_rep.GetIterator());
    std::unique_ptr<MemTableRep::Iterator> it_vector(vector_rep.GetIterator());

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

    std::cout << "  ExpanseMemTable: " << std::fixed << std::setprecision(2) << expanse_seek_mops << " Mops/s" << std::endl;
    std::cout << "  SkipListRep:     " << std::fixed << std::setprecision(2) << skiplist_seek_mops << " Mops/s" << std::endl;
    std::cout << "  VectorRep:       " << std::fixed << std::setprecision(2) << vector_seek_mops << " Mops/s" << std::endl;

    // ------------------------------------------------------------------------
    // Benchmark 4: Prefix Scan (Sequential Traversal)
    // ------------------------------------------------------------------------
    std::cout << "\n--- Benchmark 4: prefixscan (Sequential Scan across 100K entries) ---" << std::endl;

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

    std::cout << "  ExpanseMemTable (Iterator): " << std::fixed << std::setprecision(2) << expanse_scan_mops << " Mops/s" << std::endl;
    std::cout << "  ExpanseMemTable (Batch):    " << std::fixed << std::setprecision(2) << expanse_batch_mops << " Mops/s" << std::endl;
    std::cout << "  SkipListRep:                " << std::fixed << std::setprecision(2) << skiplist_scan_mops << " Mops/s" << std::endl;
    std::cout << "  VectorRep:                  " << std::fixed << std::setprecision(2) << vector_scan_mops << " Mops/s" << std::endl;

    // ------------------------------------------------------------------------
    // Benchmark 5: Memory Density & Footprint Analysis
    // ------------------------------------------------------------------------
    std::cout << "\n--- Memory Density & Footprint Analysis ---" << std::endl;
    size_t mem_expanse = expanse_rep.ApproximateMemoryUsage();
    size_t mem_skiplist = skiplist_rep.ApproximateMemoryUsage();
    size_t mem_vector = vector_rep.ApproximateMemoryUsage();

    double bytes_per_key_expanse = static_cast<double>(mem_expanse) / N;
    double bytes_per_key_skiplist = static_cast<double>(mem_skiplist) / N;
    double bytes_per_key_vector = static_cast<double>(mem_vector) / N;

    std::cout << "  ExpanseMemTable: " << (mem_expanse / (1024.0 * 1024.0)) << " MB (" << std::fixed << std::setprecision(1) << bytes_per_key_expanse << " B/entry)" << std::endl;
    std::cout << "  SkipListRep:     " << (mem_skiplist / (1024.0 * 1024.0)) << " MB (" << std::fixed << std::setprecision(1) << bytes_per_key_skiplist << " B/entry)" << std::endl;
    std::cout << "  VectorRep:       " << (mem_vector / (1024.0 * 1024.0)) << " MB (" << std::fixed << std::setprecision(1) << bytes_per_key_vector << " B/entry)" << std::endl;
    std::cout << "  => Key Density Advantage vs SkipList: " << std::fixed << std::setprecision(2)
              << (bytes_per_key_skiplist / bytes_per_key_expanse) << "x Higher Key Density in RAM!" << std::endl;

    std::cout << "\n==========================================================================" << std::endl;
    std::cout << " Microbenchmark Completed Successfully!" << std::endl;
    std::cout << "==========================================================================" << std::endl;

    return 0;
}
