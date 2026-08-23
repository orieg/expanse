// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// test_differential_memtable.cc — Differential fuzzing against oracle.

#include <cassert>
#include <chrono>
#include <iomanip>
#include <iostream>
#include <random>
#include <sstream>
#include <string>
#include <thread>
#include <vector>
#include <set>
#include <map>

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
    int operator()(const Slice& a, const char* b) const override {
        Slice slice_b = expanse_rocksdb::GetLengthPrefixedSlice(b);
        return expanse_rocksdb::CompareInternalKeys(a, slice_b);
    }
};

struct CompareEntry {
    bool operator()(const char* a, const char* b) const {
        TestBytewiseComparator cmp;
        return cmp(a, b) < 0;
    }
};

class ReferenceMemTable {
public:
    std::set<const char*, CompareEntry> entries;

    void Insert(const char* entry) {
        entries.insert(entry);
    }

    bool Contains(const char* entry) const {
        return entries.find(entry) != entries.end();
    }
    
    size_t Count() const {
        return entries.size();
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

// Compare Expanse Iterator and Reference Iterator state
void VerifyIteratorsMatch(MemTableRep::Iterator* exp_it, std::set<const char*, CompareEntry>::iterator ref_it, const std::set<const char*, CompareEntry>& ref_entries) {
    if (ref_it == ref_entries.end()) {
        assert(!exp_it->Valid());
    } else {
        assert(exp_it->Valid());
        Slice exp_ikey = expanse_rocksdb::GetLengthPrefixedSlice(exp_it->key());
        Slice ref_ikey = expanse_rocksdb::GetLengthPrefixedSlice(*ref_it);
        
        assert(exp_ikey.size() == ref_ikey.size());
        assert(memcmp(exp_ikey.data(), ref_ikey.data(), exp_ikey.size()) == 0);
        
        // Compare full entry byte-for-byte to ensure value equality
        // Extract val length and compare
        uint32_t val_len_exp = 0;
        const char* val_p_exp = expanse_rocksdb::GetVarint32Ptr(exp_ikey.data() + exp_ikey.size(), exp_ikey.data() + exp_ikey.size() + 5, &val_len_exp);
        uint32_t val_len_ref = 0;
        const char* val_p_ref = expanse_rocksdb::GetVarint32Ptr(ref_ikey.data() + ref_ikey.size(), ref_ikey.data() + ref_ikey.size() + 5, &val_len_ref);
        
        assert(val_len_exp == val_len_ref);
        if (val_len_exp > 0) {
            assert(memcmp(val_p_exp, val_p_ref, val_len_exp) == 0);
        }
    }
}

void TestDifferentialFuzz() {
    std::cout << "[RUN] Differential Fuzzing: ExpanseMemTable vs std::set" << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep memtable(cmp, &arena, nullptr, nullptr, 32);
    ReferenceMemTable ref_memtable;

    std::mt19937_64 rng(1337);
    
    // 1. Insert Phase
    const int NUM_KEYS = 500;
    const int NUM_VERSIONS = 5;
    
    std::vector<const char*> all_entries;
    
    for (int i = 0; i < NUM_KEYS; ++i) {
        std::ostringstream ss;
        ss << "key_" << std::setw(6) << std::setfill('0') << i;
        std::string user_key = ss.str();
        
        for (int v = 0; v < NUM_VERSIONS; ++v) {
            SequenceNumber seq = 1000 + v * 10 + (rng() % 5);
            ValueType type = (rng() % 10 == 0) ? kTypeDeletion : kTypeValue;
            std::string val = (type == kTypeValue) ? "val_" + std::to_string(seq) : "";
            const char* e = EncodeEntry(arena, user_key, seq, type, val);
            all_entries.push_back(e);
        }
    }
    
    // Shuffle inserts
    std::shuffle(all_entries.begin(), all_entries.end(), rng);
    
    for (const char* e : all_entries) {
        memtable.Insert(const_cast<char*>(e));
        ref_memtable.Insert(e);
    }
    
    assert(memtable.Count() == ref_memtable.Count());
    
    // 2. Iteration Phase
    {
        std::unique_ptr<MemTableRep::Iterator> exp_it(memtable.GetIterator());
        auto ref_it = ref_memtable.entries.begin();
        
        exp_it->SeekToFirst();
        while (ref_it != ref_memtable.entries.end()) {
            VerifyIteratorsMatch(exp_it.get(), ref_it, ref_memtable.entries);
            exp_it->Next();
            ref_it++;
        }
        assert(!exp_it->Valid());
        
        exp_it->SeekToLast();
        auto ref_it_rev = ref_memtable.entries.end();
        if (ref_it_rev != ref_memtable.entries.begin()) {
            ref_it_rev--;
            while (true) {
                VerifyIteratorsMatch(exp_it.get(), ref_it_rev, ref_memtable.entries);
                if (ref_it_rev == ref_memtable.entries.begin()) {
                    exp_it->Prev();
                    assert(!exp_it->Valid());
                    break;
                }
                exp_it->Prev();
                ref_it_rev--;
            }
        }
    }
    
    // 3. Point Lookups (Get)
    for (int i = 0; i < NUM_KEYS; ++i) {
        std::ostringstream ss;
        ss << "key_" << std::setw(6) << std::setfill('0') << i;
        std::string user_key = ss.str();
        
        SequenceNumber query_seq = 1000 + (rng() % 50);
        LookupKey lk(Slice(user_key), query_seq);
        
        // Ref lookup
        const char* ref_match = nullptr;
        for (auto it = ref_memtable.entries.begin(); it != ref_memtable.entries.end(); ++it) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(*it);
            if (cmp(ikey, lk.memtable_key().data()) >= 0) {
                // First key >= lookup_key. Check if user_key matches
                Slice ukey(ikey.data(), ikey.size() - 8);
                if (ukey == Slice(user_key)) {
                    ref_match = *it;
                }
                break;
            }
        }
        
        // Exp lookup
        const char* exp_match = nullptr;
        memtable.Get(lk, &exp_match, [](void* arg, const char* entry) -> bool {
            *static_cast<const char**>(arg) = entry;
            return false;
        });
        
        assert(exp_match == ref_match);
    }
    
    // 4. Seeks
    for (int i = 0; i < 100; ++i) {
        std::unique_ptr<MemTableRep::Iterator> exp_it(memtable.GetIterator());
        int target_idx = rng() % NUM_KEYS;
        std::ostringstream ss;
        ss << "key_" << std::setw(6) << std::setfill('0') << target_idx;
        SequenceNumber seq = 1000 + (rng() % 50);
        LookupKey lk(Slice(ss.str()), seq);
        
        // Seek
        exp_it->Seek(lk.internal_key(), lk.memtable_key().data());
        
        auto ref_it = ref_memtable.entries.begin();
        while (ref_it != ref_memtable.entries.end()) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(*ref_it);
            if (cmp(ikey, lk.memtable_key().data()) >= 0) {
                break;
            }
            ref_it++;
        }
        VerifyIteratorsMatch(exp_it.get(), ref_it, ref_memtable.entries);
        
        // SeekForPrev
        exp_it->SeekForPrev(lk.internal_key(), lk.memtable_key().data());
        
        auto ref_it_prev = ref_memtable.entries.begin();
        while (ref_it_prev != ref_memtable.entries.end()) {
            Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(*ref_it_prev);
            if (cmp(ikey, lk.memtable_key().data()) > 0) {
                break;
            }
            ref_it_prev++;
        }
        if (ref_it_prev == ref_memtable.entries.begin()) {
            assert(!exp_it->Valid());
        } else {
            ref_it_prev--;
            VerifyIteratorsMatch(exp_it.get(), ref_it_prev, ref_memtable.entries);
        }
    }
    
    // 5. Batch Scan
    {
        std::unique_ptr<MemTableRep::Iterator> exp_it(memtable.GetIterator());
        exp_it->SeekToFirst();
        
        auto* concrete_it = dynamic_cast<ExpanseMemTableRep::IteratorImpl*>(exp_it.get());
        
        std::vector<Slice> batch_keys(100);
        std::vector<Slice> batch_vals(100);
        
        auto ref_it = ref_memtable.entries.begin();
        
        while (true) {
            size_t n = concrete_it->ScanBatch(100, batch_keys.data(), batch_vals.data());
            if (n == 0) break;
            
            for (size_t i = 0; i < n; ++i) {
                assert(ref_it != ref_memtable.entries.end());
                Slice ref_ikey = expanse_rocksdb::GetLengthPrefixedSlice(*ref_it);
                Slice ref_ukey(ref_ikey.data(), ref_ikey.size() - 8);
                
                assert(batch_keys[i].size() == ref_ikey.size());
                assert(memcmp(batch_keys[i].data(), ref_ikey.data(), batch_keys[i].size()) == 0);
                
                uint32_t val_len_ref = 0;
                const char* val_p_ref = expanse_rocksdb::GetVarint32Ptr(ref_ikey.data() + ref_ikey.size(), ref_ikey.data() + ref_ikey.size() + 5, &val_len_ref);
                
                assert(batch_vals[i].size() == val_len_ref);
                if (val_len_ref > 0) {
                    assert(memcmp(batch_vals[i].data(), val_p_ref, val_len_ref) == 0);
                }
                ref_it++;
            }
        }
        assert(ref_it == ref_memtable.entries.end());
    }

    std::cout << "  -> PASSED" << std::endl;
}

int main() {
    std::cout << "============================================================" << std::endl;
    std::cout << "Running Expanse RocksDB Differential MemTable Tests" << std::endl;
    std::cout << "============================================================" << std::endl;

    TestDifferentialFuzz();

    std::cout << "============================================================" << std::endl;
    std::cout << "DIFFERENTIAL FUZZ TESTS PASSED!" << std::endl;
    std::cout << "============================================================" << std::endl;
    return 0;
}
