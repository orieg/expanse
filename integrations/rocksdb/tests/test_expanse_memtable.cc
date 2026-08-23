// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// test_expanse_memtable.cc — Comprehensive unit test suite for ExpanseMemTable.

#include <cassert>
#include <chrono>
#include <iomanip>
#include <iostream>
#include <random>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

#include "expanse_memtable.h"

using namespace rocksdb;

// Standard Bytewise KeyComparator
class TestBytewiseComparator : public MemTableRep::KeyComparator {
public:
    int operator()(const char* a, const char* b) const override {
        Slice slice_a = expanse_rocksdb::GetLengthPrefixedSlice(a);
        Slice slice_b = expanse_rocksdb::GetLengthPrefixedSlice(b);
        return expanse_rocksdb::CompareInternalKeys(slice_a, slice_b);
    }
};

// Fixed-prefix SliceTransform (e.g. 4-byte prefix)
class FixedPrefixTransform : public SliceTransform {
private:
    size_t prefix_len_;

public:
    explicit FixedPrefixTransform(size_t prefix_len) : prefix_len_(prefix_len) {}
    const char* Name() const override { return "FixedPrefixTransform"; }

    Slice Transform(const Slice& key) const override {
        if (key.size() < prefix_len_) return key;
        return Slice(key.data(), prefix_len_);
    }

    bool InDomain(const Slice& key) const override {
        return key.size() >= prefix_len_;
    }
};

// Helper to encode a Memtable entry: [varint32(len)] [user_key] [trailer] [varint32(val_len)] [val]
static const char* EncodeEntry(
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

// Helper to extract value from entry
static std::string ExtractValue(const char* entry) {
    Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
    const char* p = ikey.data() + ikey.size();
    uint32_t val_len = 0;
    p = expanse_rocksdb::GetVarint32Ptr(p, p + 5, &val_len);
    return std::string(p, val_len);
}

// ----------------------------------------------------------------------------
// Unit Tests
// ----------------------------------------------------------------------------

void TestBasicInsertAndContains() {
    std::cout << "[RUN] TestBasicInsertAndContains" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 16);

    const char* e1 = EncodeEntry(arena, "key_a", 100, kTypeValue, "val_a");
    const char* e2 = EncodeEntry(arena, "key_b", 101, kTypeValue, "val_b");
    const char* e3 = EncodeEntry(arena, "key_c", 102, kTypeValue, "val_c");

    assert(!memtable.Contains(e1));
    memtable.Insert(const_cast<char*>(e1));
    assert(memtable.Contains(e1));

    memtable.Insert(const_cast<char*>(e2));
    memtable.Insert(const_cast<char*>(e3));

    assert(memtable.Contains(e1));
    assert(memtable.Contains(e2));
    assert(memtable.Contains(e3));
    assert(memtable.Count() == 3);

    const char* e_absent = EncodeEntry(arena, "key_d", 103, kTypeValue, "val_d");
    assert(!memtable.Contains(e_absent));
    std::cout << "  -> PASSED" << std::endl;
}

void TestMvccDescendingSequenceOrder() {
    std::cout << "[RUN] TestMvccDescendingSequenceOrder" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 16);

    // Insert multiple versions of the same user key in mixed sequence order
    const char* e1 = EncodeEntry(arena, "user_alpha", 10, kTypeValue, "v10");
    const char* e3 = EncodeEntry(arena, "user_alpha", 30, kTypeValue, "v30");
    const char* e2 = EncodeEntry(arena, "user_alpha", 20, kTypeValue, "v20");
    const char* e4 = EncodeEntry(arena, "user_alpha", 40, kTypeDeletion, "");

    memtable.Insert(const_cast<char*>(e1));
    memtable.Insert(const_cast<char*>(e3));
    memtable.Insert(const_cast<char*>(e2));
    memtable.Insert(const_cast<char*>(e4));

    assert(memtable.Count() == 4);

    // Point lookup at snapshot 35 should find v30
    {
        LookupKey lk(Slice("user_alpha"), 35);
        std::string found_val;
        memtable.Get(lk, &found_val, [](void* arg, const char* entry) -> bool {
            auto* s = static_cast<std::string*>(arg);
            *s = ExtractValue(entry);
            return false; // Stop after first match
        });
        assert(found_val == "v30");
    }

    // Point lookup at snapshot 25 should find v20
    {
        LookupKey lk(Slice("user_alpha"), 25);
        std::string found_val;
        memtable.Get(lk, &found_val, [](void* arg, const char* entry) -> bool {
            auto* s = static_cast<std::string*>(arg);
            *s = ExtractValue(entry);
            return false;
        });
        assert(found_val == "v20");
    }

    // Point lookup at snapshot 45 should find deletion tombstone
    {
        LookupKey lk(Slice("user_alpha"), 45);
        Slice ikey;
        memtable.Get(lk, &ikey, [](void* arg, const char* entry) -> bool {
            auto* k = static_cast<Slice*>(arg);
            *k = expanse_rocksdb::GetLengthPrefixedSlice(entry);
            return false;
        });
        uint8_t type = ikey.data()[ikey.size() - 8];
        assert(type == kTypeDeletion);
    }
    std::cout << "  -> PASSED" << std::endl;
}

void TestForwardAndReverseIteration() {
    std::cout << "[RUN] TestForwardAndReverseIteration" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 4); // Small capacity to force splits

    std::vector<std::string> keys = {
        "apple", "banana", "cherry", "date", "elderberry",
        "fig", "grape", "honeydew", "kiwi", "lemon"
    };

    for (size_t i = 0; i < keys.size(); ++i) {
        const char* e = EncodeEntry(arena, keys[i], 100 + i, kTypeValue, "val_" + keys[i]);
        memtable.Insert(const_cast<char*>(e));
    }
    assert(memtable.Count() == keys.size());

    // Forward iteration
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        it->SeekToFirst();
        size_t idx = 0;
        while (it->Valid()) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
            Slice ukey(ikey.data(), ikey.size() - 8);
            assert(ukey.ToString() == keys[idx]);
            idx++;
            it->Next();
        }
        assert(idx == keys.size());
    }

    // Reverse iteration
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        it->SeekToLast();
        int idx = static_cast<int>(keys.size()) - 1;
        while (it->Valid()) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
            Slice ukey(ikey.data(), ikey.size() - 8);
            assert(ukey.ToString() == keys[idx]);
            idx--;
            it->Prev();
        }
        assert(idx == -1);
    }
    std::cout << "  -> PASSED" << std::endl;
}

void TestPrefixSeeksAndSeekForPrev() {
    std::cout << "[RUN] TestPrefixSeeksAndSeekForPrev" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    FixedPrefixTransform transform(4);
    ExpanseMemTableRep memtable(cmp, &arena, &transform, nullptr, 8);

    std::vector<std::string> keys = {
        "dept_eng_01", "dept_eng_02", "dept_fin_01", "dept_mkt_01", "dept_ops_01"
    };

    for (size_t i = 0; i < keys.size(); ++i) {
        const char* e = EncodeEntry(arena, keys[i], 100, kTypeValue, "v");
        memtable.Insert(const_cast<char*>(e));
    }

    // Seek exact
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        LookupKey lk(Slice("dept_fin_01"), 100);
        it->Seek(lk.internal_key(), lk.memtable_key().data());
        assert(it->Valid());
        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        Slice ukey(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "dept_fin_01");
    }

    // Seek between keys (dept_eng_03 -> lands on dept_fin_01)
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        LookupKey lk(Slice("dept_eng_03"), 100);
        it->Seek(lk.internal_key(), lk.memtable_key().data());
        assert(it->Valid());
        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        Slice ukey(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "dept_fin_01");
    }

    // SeekForPrev between keys (dept_eng_03 -> lands on dept_eng_02)
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        LookupKey lk(Slice("dept_eng_03"), 100);
        it->SeekForPrev(lk.internal_key(), lk.memtable_key().data());
        assert(it->Valid());
        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        Slice ukey(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "dept_eng_02");
    }

    // Seek past end
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        LookupKey lk(Slice("dept_zzz_99"), 100);
        it->Seek(lk.internal_key(), lk.memtable_key().data());
        assert(!it->Valid());
    }

    // SeekForPrev past end -> lands on dept_ops_01
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        LookupKey lk(Slice("dept_zzz_99"), 100);
        it->SeekForPrev(lk.internal_key(), lk.memtable_key().data());
        assert(it->Valid());
        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        Slice ukey(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "dept_ops_01");
    }
    std::cout << "  -> PASSED" << std::endl;
}

void TestSuggestCompactRange() {
    std::cout << "[RUN] TestSuggestCompactRange" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 16);

    const char* e1 = EncodeEntry(arena, "aaa_first", 1, kTypeValue, "v1");
    const char* e2 = EncodeEntry(arena, "mmm_mid", 2, kTypeValue, "v2");
    const char* e3 = EncodeEntry(arena, "zzz_last", 3, kTypeValue, "v3");

    memtable.Insert(const_cast<char*>(e2));
    memtable.Insert(const_cast<char*>(e1));
    memtable.Insert(const_cast<char*>(e3));

    Slice begin, end;
    memtable.SuggestCompactRange(&begin, &end);

    Slice ubegin(begin.data(), begin.size() - 8);
    Slice uend(end.data(), end.size() - 8);

    assert(ubegin.ToString() == "aaa_first");
    assert(uend.ToString() == "zzz_last");
    std::cout << "  -> PASSED" << std::endl;
}

void TestMultiThreadedConcurrentOperations() {
    std::cout << "[RUN] TestMultiThreadedConcurrentOperations" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 32);

    const int num_writers = 4;
    const int keys_per_writer = 1000;
    std::vector<std::thread> writers;
    std::mutex arena_mutex;

    // Concurrent writers
    for (int w = 0; w < num_writers; ++w) {
        writers.emplace_back([&, w]() {
            for (int i = 0; i < keys_per_writer; ++i) {
                std::ostringstream ss;
                ss << "user_" << std::setw(2) << std::setfill('0') << w
                   << "_key_" << std::setw(6) << std::setfill('0') << i;
                std::string k = ss.str();
                const char* entry;
                {
                    std::lock_guard<std::mutex> lock(arena_mutex);
                    entry = EncodeEntry(arena, k, 1000 + i, kTypeValue, "val_" + k);
                }
                memtable.InsertConcurrently(const_cast<char*>(entry));
            }
        });
    }

    // Concurrent reader
    std::atomic<bool> stop_reader{false};
    std::thread reader([&]() {
        while (!stop_reader.load(std::memory_order_relaxed)) {
            std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
            it->SeekToFirst();
            if (it->Valid()) {
                it->Next();
            }
            std::this_thread::yield();
        }
    });

    for (auto& t : writers) {
        t.join();
    }
    stop_reader.store(true);
    reader.join();

    assert(memtable.Count() == num_writers * keys_per_writer);

    // Verify all keys are present and sorted
    std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
    it->SeekToFirst();
    uint64_t count = 0;
    std::string prev_key = "";
    while (it->Valid()) {
        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        Slice ukey(ikey.data(), ikey.size() - 8);
        std::string curr_key = ukey.ToString();
        if (!prev_key.empty()) {
            assert(prev_key < curr_key);
        }
        prev_key = curr_key;
        count++;
        it->Next();
    }
    assert(count == num_writers * keys_per_writer);
    std::cout << "  -> PASSED (verified " << count << " concurrent entries)" << std::endl;
}

void TestLargeVolumeRandomOperations() {
    std::cout << "[RUN] TestLargeVolumeRandomOperations (10,000 keys)" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 64);

    const int total_keys = 10000;
    std::vector<std::string> keys;
    keys.reserve(total_keys);

    std::mt19937_64 rng(42);
    for (int i = 0; i < total_keys; ++i) {
        std::ostringstream ss;
        ss << "k_" << std::setw(8) << std::setfill('0') << (rng() % 100000000);
        keys.push_back(ss.str());
    }

    for (size_t i = 0; i < keys.size(); ++i) {
        const char* e = EncodeEntry(arena, keys[i], 100, kTypeValue, "val_" + std::to_string(i));
        memtable.Insert(const_cast<char*>(e));
    }

    assert(memtable.Count() == total_keys);

    // Verify memory usage calculation
    size_t mem = memtable.ApproximateMemoryUsage();
    assert(mem > 0);

    // Verify random lookups
    for (int i = 0; i < 500; ++i) {
        size_t idx = rng() % keys.size();
        LookupKey lk(Slice(keys[idx]), 100);
        bool found = false;
        memtable.Get(lk, &found, [](void* arg, const char* /*entry*/) -> bool {
            *static_cast<bool*>(arg) = true;
            return false;
        });
        assert(found);
    }

    // Verify full sorted order
    std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
    it->SeekToFirst();
    std::string last_key = "";
    uint64_t iterated = 0;
    while (it->Valid()) {
        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        Slice ukey(ikey.data(), ikey.size() - 8);
        std::string current = ukey.ToString();
        if (!last_key.empty()) {
            assert(last_key <= current);
        }
        last_key = current;
        iterated++;
        it->Next();
    }
    assert(iterated == total_keys);
    std::cout << "  -> PASSED (10,000 keys verified, memory: " << mem << " bytes)" << std::endl;
}

void TestBatchScanApi() {
    std::cout << "[RUN] TestBatchScanApi" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 16); // small capacity to create multiple leaves

    const int total = 500;
    std::vector<std::string> keys;
    keys.reserve(total);
    for (int i = 0; i < total; ++i) {
        std::ostringstream ss;
        ss << "batch_key_" << std::setw(5) << std::setfill('0') << i;
        keys.push_back(ss.str());
        const char* e = EncodeEntry(arena, keys.back(), 100 + i, kTypeValue, "val_" + keys.back());
        memtable.Insert(const_cast<char*>(e));
    }

    std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
    auto* exp_it = dynamic_cast<ExpanseMemTableRep::IteratorImpl*>(it.get());
    assert(exp_it != nullptr);

    exp_it->SeekToFirst();
    assert(exp_it->Valid());

    // Test zero-copy accessors
    Slice ikey = exp_it->internal_key();
    Slice ukey = exp_it->user_key();
    Slice val = exp_it->value();
    assert(ukey.ToString() == keys[0]);
    assert(val.ToString() == "val_" + keys[0]);
    assert(ikey.size() == ukey.size() + 8);

    // Batch extraction 1: 100 keys
    std::vector<Slice> batch_keys(100);
    std::vector<Slice> batch_vals(100);
    size_t n1 = exp_it->ScanBatch(100, batch_keys.data(), batch_vals.data());
    assert(n1 == 100);

    for (size_t i = 0; i < 100; ++i) {
        Slice u(batch_keys[i].data(), batch_keys[i].size() - 8);
        assert(u.ToString() == keys[i]);
        assert(batch_vals[i].ToString() == "val_" + keys[i]);
    }

    // Batch extraction 2: 250 keys via helper function
    std::vector<Slice> batch_keys2(250);
    std::vector<Slice> batch_vals2(250);
    size_t n2 = ScanBatch(exp_it, 250, batch_keys2.data(), batch_vals2.data());
    assert(n2 == 250);

    for (size_t i = 0; i < 250; ++i) {
        Slice u(batch_keys2[i].data(), batch_keys2[i].size() - 8);
        assert(u.ToString() == keys[100 + i]);
        assert(batch_vals2[i].ToString() == "val_" + keys[100 + i]);
    }

    // Batch extraction 3: remaining (150 keys requested with buffer for 200)
    std::vector<Slice> batch_keys3(200);
    std::vector<Slice> batch_vals3(200);
    size_t n3 = exp_it->ScanBatch(200, batch_keys3.data(), batch_vals3.data());
    assert(n3 == 150);

    for (size_t i = 0; i < 150; ++i) {
        Slice u(batch_keys3[i].data(), batch_keys3[i].size() - 8);
        assert(u.ToString() == keys[350 + i]);
        assert(batch_vals3[i].ToString() == "val_" + keys[350 + i]);
    }

    // Further scans at end return 0
    size_t n4 = exp_it->ScanBatch(10, batch_keys.data(), batch_vals.data());
    assert(n4 == 0);
    assert(!exp_it->Valid());

    std::cout << "  -> PASSED" << std::endl;
}

void TestIntrusiveLeafChainingAndPrefetch() {
    std::cout << "[RUN] TestIntrusiveLeafChainingAndPrefetch" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 8); // Force high number of leaf splits

    const int total = 1000;
    std::vector<std::string> keys;
    keys.reserve(total);
    for (int i = 0; i < total; ++i) {
        std::ostringstream ss;
        ss << "chain_" << std::setw(6) << std::setfill('0') << i;
        keys.push_back(ss.str());
        const char* e = EncodeEntry(arena, keys.back(), 1000 + i, kTypeValue, "val");
        memtable.Insert(const_cast<char*>(e));
    }

    // Step-by-step forward traversal
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        it->SeekToFirst();
        int idx = 0;
        while (it->Valid()) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
            Slice ukey(ikey.data(), ikey.size() - 8);
            assert(ukey.ToString() == keys[idx]);
            idx++;
            it->Next();
        }
        assert(idx == total);
    }

    // Step-by-step backward traversal
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        it->SeekToLast();
        int idx = total - 1;
        while (it->Valid()) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
            Slice ukey(ikey.data(), ikey.size() - 8);
            assert(ukey.ToString() == keys[idx]);
            idx--;
            it->Prev();
        }
        assert(idx == -1);
    }

    // Seek to middle and alternate Next/Prev across leaf boundaries
    {
        std::unique_ptr<MemTableRep::Iterator> it(memtable.GetIterator());
        LookupKey lk(Slice("chain_000500"), 1500);
        it->Seek(lk.internal_key(), lk.memtable_key().data());
        assert(it->Valid());

        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        Slice ukey(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "chain_000500");

        it->Next();
        ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        ukey = Slice(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "chain_000501");

        it->Prev();
        ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        ukey = Slice(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "chain_000500");

        it->Prev();
        ikey = expanse_rocksdb::GetLengthPrefixedSlice(it->key());
        ukey = Slice(ikey.data(), ikey.size() - 8);
        assert(ukey.ToString() == "chain_000499");
    }

    std::cout << "  -> PASSED" << std::endl;
}

void TestFactory() {
    std::cout << "[RUN] TestFactory" << std::endl;
    auto factory = NewExpanseMemTableRepFactory(32, true);
    assert(std::string(factory->Name()) == "ExpanseMemTableRepFactory");
    assert(factory->IsConcurrentlyWritable());
    assert(factory->IsPrefixExtractorSupported());

    TestBytewiseComparator cmp;
    Arena arena;
    MemTableRep* rep = factory->CreateMemTableRep(cmp, &arena, nullptr, nullptr);
    assert(rep != nullptr);

    const char* e = EncodeEntry(arena, "hello", 1, kTypeValue, "world");
    rep->Insert(const_cast<char*>(e));
    assert(rep->Contains(e));

    delete rep;
    std::cout << "  -> PASSED" << std::endl;
}

int main() {
    std::cout << "============================================================" << std::endl;
    std::cout << "Running Expanse RocksDB MemTable Unit Tests" << std::endl;
    std::cout << "============================================================" << std::endl;

    TestBasicInsertAndContains();
    TestMvccDescendingSequenceOrder();
    TestForwardAndReverseIteration();
    TestPrefixSeeksAndSeekForPrev();
    TestSuggestCompactRange();
    TestMultiThreadedConcurrentOperations();
    TestLargeVolumeRandomOperations();
    TestBatchScanApi();
    TestIntrusiveLeafChainingAndPrefetch();
    TestFactory();

    std::cout << "============================================================" << std::endl;
    std::cout << "ALL EXPANSE MEMTABLE TESTS PASSED (100% PASS RATE)!" << std::endl;
    std::cout << "============================================================" << std::endl;
    return 0;
}
