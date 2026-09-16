// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// test_differential_memtable.cc — Differential tests against a std::set oracle.
//
// G-O1 of docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section 5.16: under
// quiescence, every seek lock scope agrees with ReferenceMemTable on Get,
// Contains, IteratorImpl::Seek and SeekForPrev. Get is checked on the whole
// sequence it passes to its callback, with a callback that returns true, so a
// match delivered twice, out of order or not at all is a difference. Every
// case runs under every scope; agreement with the oracle is agreement across
// the scopes.
//
// `test_differential_memtable [full|trie|opt]` runs one scope only.

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <iomanip>
#include <iostream>
#include <map>
#include <memory>
#include <random>
#include <set>
#include <sstream>
#include <string>
#include <vector>

#include "expanse_memtable.h"

using namespace rocksdb;
using Scope = ExpanseMemTableRep::SeekLockScope;

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

namespace {

[[noreturn]] void Fail(const std::string& msg) {
    std::cerr << "FAILED: " << msg << std::endl;
    std::abort();
}

void Require(bool cond, const std::string& msg) {
    if (!cond) Fail(msg);
}

const char* ScopeName(Scope scope) {
    switch (scope) {
        case Scope::kFullLocate: return "kFullLocate";
        case Scope::kTrieCall: return "kTrieCall";
        case Scope::kOptimistic: return "kOptimistic";
    }
    return "unknown";
}

// Helper to encode a Memtable entry: [varint32(len)] [user_key] [trailer] [varint32(val_len)] [val]
const char* EncodeEntry(
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

std::string Describe(const char* entry) {
    if (entry == nullptr) return "<end>";
    const Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
    uint64_t trailer = 0;
    for (int i = 0; i < 8; ++i) {
        trailer |= static_cast<uint64_t>(static_cast<unsigned char>(ikey.data()[ikey.size() - 8 + i])) << (i * 8);
    }
    return std::string(ikey.data(), ikey.size() - 8) + "@" + std::to_string(trailer >> 8);
}

std::string DescribeAll(const std::vector<const char*>& entries) {
    std::string out = "[";
    for (size_t i = 0; i < entries.size(); ++i) {
        out += (i ? ", " : "") + Describe(entries[i]);
    }
    return out + "]";
}

Slice UserKey(const char* entry) {
    const Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
    return Slice(ikey.data(), ikey.size() - 8);
}

// Compare the full internal key and value of the iterator's entry with the reference's.
void VerifyIteratorsMatch(MemTableRep::Iterator* exp_it, std::set<const char*, CompareEntry>::iterator ref_it,
                          const std::set<const char*, CompareEntry>& ref_entries, const std::string& what) {
    if (ref_it == ref_entries.end()) {
        Require(!exp_it->Valid(), what + ": expected end, got " + Describe(exp_it->key()));
        return;
    }
    Require(exp_it->Valid(), what + ": expected " + Describe(*ref_it) + ", got end");
    Slice exp_ikey = expanse_rocksdb::GetLengthPrefixedSlice(exp_it->key());
    Slice ref_ikey = expanse_rocksdb::GetLengthPrefixedSlice(*ref_it);
    Require(exp_ikey.size() == ref_ikey.size() && memcmp(exp_ikey.data(), ref_ikey.data(), exp_ikey.size()) == 0,
            what + ": expected " + Describe(*ref_it) + ", got " + Describe(exp_it->key()));

    uint32_t val_len_exp = 0;
    const char* val_p_exp = expanse_rocksdb::GetVarint32Ptr(exp_ikey.data() + exp_ikey.size(), exp_ikey.data() + exp_ikey.size() + 5, &val_len_exp);
    uint32_t val_len_ref = 0;
    const char* val_p_ref = expanse_rocksdb::GetVarint32Ptr(ref_ikey.data() + ref_ikey.size(), ref_ikey.data() + ref_ikey.size() + 5, &val_len_ref);
    Require(val_len_exp == val_len_ref && (val_len_exp == 0 || memcmp(val_p_exp, val_p_ref, val_len_exp) == 0),
            what + ": value differs at " + Describe(*ref_it));
}

bool Collect(void* arg, const char* entry) {
    static_cast<std::vector<const char*>*>(arg)->push_back(entry);
    return true;  // every match is delivered
}

// One G-O1 probe: a user key and a snapshot. Get, Seek and SeekForPrev are
// read with the LookupKey; Contains with every present entry separately.
void CheckProbe(ExpanseMemTableRep& rep, const ReferenceMemTable& ref, const std::string& user_key,
                SequenceNumber snapshot, const std::string& where) {
    LookupKey lk(Slice(user_key), snapshot);
    const std::string probe = where + " probe " + user_key + "@" + std::to_string(snapshot);

    // Get: every reference entry from the lookup key's lower bound while the user key matches, in order.
    std::vector<const char*> want;
    for (auto it = ref.entries.lower_bound(lk.memtable_key().data());
         it != ref.entries.end() && UserKey(*it) == Slice(user_key); ++it) {
        want.push_back(*it);
    }
    std::vector<const char*> got;
    rep.Get(lk, &got, &Collect);
    Require(got == want, probe + ": Get delivered " + DescribeAll(got) + ", expected " + DescribeAll(want));

    std::unique_ptr<MemTableRep::Iterator> it(rep.GetIterator());
    it->Seek(lk.internal_key(), lk.memtable_key().data());
    VerifyIteratorsMatch(it.get(), ref.entries.lower_bound(lk.memtable_key().data()), ref.entries, probe + ": Seek");

    it->SeekForPrev(lk.internal_key(), lk.memtable_key().data());
    auto last_le = ref.entries.upper_bound(lk.memtable_key().data());
    if (last_le == ref.entries.begin()) {
        Require(!it->Valid(), probe + ": SeekForPrev expected nothing at or before, got " + Describe(it->key()));
    } else {
        --last_le;
        VerifyIteratorsMatch(it.get(), last_le, ref.entries, probe + ": SeekForPrev");
    }
}

struct Case {
    std::string name;
    size_t leaf_capacity;
    std::vector<std::pair<std::string, SequenceNumber>> inserts;  // in insertion order
    std::vector<std::string> absent_user_keys;
};

void RunCase(const Case& c, Scope scope, uint64_t seed) {
    const std::string where = std::string("G-O1 (") + ScopeName(scope) + ", " + c.name + ")";
    std::cout << "[RUN] " << where << ": " << c.inserts.size() << " entries, leaf capacity " << c.leaf_capacity
              << std::endl;
    TestBytewiseComparator cmp;
    Arena arena;
    ExpanseMemTableRep rep(cmp, &arena, nullptr, nullptr, c.leaf_capacity, scope);
    ReferenceMemTable ref;
    std::mt19937_64 rng(seed);

    std::set<std::string> user_keys;
    std::vector<const char*> present;
    for (const auto& [user_key, seq] : c.inserts) {
        const ValueType type = (rng() % 10 == 0) ? kTypeDeletion : kTypeValue;
        const std::string val = (type == kTypeValue) ? "val_" + user_key + "_" + std::to_string(seq) : "";
        const char* e = EncodeEntry(arena, user_key, seq, type, val);
        rep.Insert(const_cast<char*>(e));
        ref.Insert(e);
        present.push_back(e);
        user_keys.insert(user_key);
    }
    Require(rep.Count() == ref.Count(), where + ": count " + std::to_string(rep.Count()) + " vs reference " +
            std::to_string(ref.Count()));

    // Contains, on every present entry and on absent ones.
    for (const char* e : present) {
        Require(rep.Contains(e), where + ": Contains(" + Describe(e) + ") returned false for a present entry");
    }
    for (const std::string& k : c.absent_user_keys) {
        const char* e = EncodeEntry(arena, k, 1, kTypeValue, "");
        Require(!rep.Contains(e), where + ": Contains(" + Describe(e) + ") returned true for an absent entry");
    }

    // Get, Seek and SeekForPrev: every user key and every absent one, at the
    // newest snapshot, below every sequence, and at each present sequence.
    std::map<std::string, std::vector<SequenceNumber>> seqs;
    for (const auto& [user_key, seq] : c.inserts) seqs[user_key].push_back(seq);
    for (const auto& [user_key, list] : seqs) {
        CheckProbe(rep, ref, user_key, (1ull << 55), where);
        CheckProbe(rep, ref, user_key, 0, where);
        for (SequenceNumber s : list) CheckProbe(rep, ref, user_key, s, where);
    }
    for (const std::string& k : c.absent_user_keys) {
        CheckProbe(rep, ref, k, (1ull << 55), where);
    }

    // Forward and reverse iteration.
    {
        std::unique_ptr<MemTableRep::Iterator> exp_it(rep.GetIterator());
        exp_it->SeekToFirst();
        for (auto ref_it = ref.entries.begin(); ref_it != ref.entries.end(); ++ref_it) {
            VerifyIteratorsMatch(exp_it.get(), ref_it, ref.entries, where + ": forward iteration");
            exp_it->Next();
        }
        Require(!exp_it->Valid(), where + ": forward iteration did not end");
        exp_it->SeekToLast();
        for (auto ref_it = ref.entries.rbegin(); ref_it != ref.entries.rend(); ++ref_it) {
            VerifyIteratorsMatch(exp_it.get(), std::prev(ref_it.base()), ref.entries, where + ": reverse iteration");
            exp_it->Prev();
        }
        Require(!exp_it->Valid(), where + ": reverse iteration did not end");
    }

    // Batch scan.
    {
        std::unique_ptr<MemTableRep::Iterator> exp_it(rep.GetIterator());
        exp_it->SeekToFirst();
        auto* concrete_it = dynamic_cast<ExpanseMemTableRep::IteratorImpl*>(exp_it.get());
        std::vector<Slice> batch_keys(100);
        std::vector<Slice> batch_vals(100);
        auto ref_it = ref.entries.begin();
        while (true) {
            size_t n = concrete_it->ScanBatch(100, batch_keys.data(), batch_vals.data());
            if (n == 0) break;
            for (size_t i = 0; i < n; ++i) {
                Require(ref_it != ref.entries.end(), where + ": ScanBatch returned more entries than the reference");
                Slice ref_ikey = expanse_rocksdb::GetLengthPrefixedSlice(*ref_it);
                Require(batch_keys[i].size() == ref_ikey.size() &&
                        memcmp(batch_keys[i].data(), ref_ikey.data(), batch_keys[i].size()) == 0,
                        where + ": ScanBatch key differs at " + Describe(*ref_it));
                uint32_t val_len_ref = 0;
                const char* val_p_ref = expanse_rocksdb::GetVarint32Ptr(ref_ikey.data() + ref_ikey.size(), ref_ikey.data() + ref_ikey.size() + 5, &val_len_ref);
                Require(batch_vals[i].size() == val_len_ref &&
                        (val_len_ref == 0 || memcmp(batch_vals[i].data(), val_p_ref, val_len_ref) == 0),
                        where + ": ScanBatch value differs at " + Describe(*ref_it));
                ++ref_it;
            }
        }
        Require(ref_it == ref.entries.end(), where + ": ScanBatch stopped early");
    }
    std::cout << "  -> PASSED" << std::endl;
}

std::string Padded(const char* prefix, int n, int width) {
    std::ostringstream ss;
    ss << prefix << std::setw(width) << std::setfill('0') << n;
    return ss.str();
}

std::vector<Case> Cases() {
    std::vector<Case> cases;

    // The random fuzz: 500 user keys, 5 versions each, shuffled, capacity 32.
    {
        Case c{"random", 32, {}, {}};
        std::mt19937_64 rng(1337);
        for (int i = 0; i < 500; ++i) {
            for (int v = 0; v < 5; ++v) {
                c.inserts.emplace_back(Padded("key_", i, 6), 1000 + v * 10 + (rng() % 5));
            }
        }
        std::shuffle(c.inserts.begin(), c.inserts.end(), rng);
        for (int i = 0; i < 20; ++i) c.absent_user_keys.push_back(Padded("key_", i * 25, 6) + "x");
        cases.push_back(std::move(c));
    }

    // Keys that share their first 8 bytes across block boundaries. Inserted in
    // ascending order at capacity 8, every split remaps the shared prefix to the
    // newest block, so the trie answers a target in an earlier block with a block
    // after it and the backward walk has to run. Keys with other prefixes sit on
    // both sides.
    {
        Case c{"shared 8-byte prefix across blocks", 8, {}, {}};
        for (int i = 0; i < 6; ++i) c.inserts.emplace_back(Padded("AAAAAAAA", i, 4), 1);
        for (int i = 0; i < 48; ++i) c.inserts.emplace_back(Padded("PPPPPPPP", i * 2, 4), 1);
        for (int i = 0; i < 6; ++i) c.inserts.emplace_back(Padded("ZZZZZZZZ", i, 4), 1);
        for (int i = 0; i < 12; ++i) c.absent_user_keys.push_back(Padded("PPPPPPPP", i * 8 + 1, 4));
        c.absent_user_keys.push_back("PPPPPPPO");
        c.absent_user_keys.push_back("PPPPPPPQ");
        cases.push_back(std::move(c));
    }

    // One user key with more versions than half a block's capacity, so its
    // versions span a split: 7 versions at capacity 8, interleaved with keys on
    // both sides.
    {
        Case c{"versions spanning a split", 8, {}, {}};
        for (int round = 0; round < 7; ++round) {
            c.inserts.emplace_back(Padded("k", 10 + round, 7), 5);
            c.inserts.emplace_back("m0000000", 100 + round);
            c.inserts.emplace_back(Padded("z", 10 + round, 7), 5);
        }
        c.absent_user_keys.push_back("m0000001");
        c.absent_user_keys.push_back("l9999999");
        cases.push_back(std::move(c));
    }

    // A rep with a single block, so the locate phase returns at h == t.
    {
        Case c{"single block", 64, {}, {}};
        for (int i = 0; i < 40; ++i) c.inserts.emplace_back(Padded("one_", (i * 7) % 40, 4), 1 + i % 3);
        c.absent_user_keys.push_back("one_0041");
        c.absent_user_keys.push_back("aaaa");
        cases.push_back(std::move(c));
    }
    return cases;
}

}  // namespace

int main(int argc, char** argv) {
    std::vector<Scope> scopes = {Scope::kFullLocate, Scope::kTrieCall, Scope::kOptimistic};
    if (argc == 2) {
        const std::string only = argv[1];
        if (only == "full") scopes = {Scope::kFullLocate};
        else if (only == "trie") scopes = {Scope::kTrieCall};
        else if (only == "opt") scopes = {Scope::kOptimistic};
        else {
            std::cerr << "unknown scope '" << only << "': expected full, trie or opt" << std::endl;
            return 2;
        }
    } else if (argc > 2) {
        std::cerr << "usage: test_differential_memtable [full|trie|opt]" << std::endl;
        return 2;
    }

    std::cout << "============================================================" << std::endl;
    std::cout << "Running Expanse RocksDB Differential MemTable Tests" << std::endl;
    std::cout << "============================================================" << std::endl;

    uint64_t seed = 7;
    for (const Case& c : Cases()) {
        for (Scope scope : scopes) {
            RunCase(c, scope, seed);
        }
        ++seed;
    }

    std::cout << "============================================================" << std::endl;
    std::cout << "DIFFERENTIAL FUZZ TESTS PASSED!" << std::endl;
    std::cout << "============================================================" << std::endl;
    return 0;
}
