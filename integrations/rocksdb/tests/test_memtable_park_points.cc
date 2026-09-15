// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// test_memtable_park_points.cc -- deterministic interleavings on the Get and
// iterator paths.
//
// Each test holds one reader thread at a named park point
// (expanse_rocksdb::ParkPoint), runs a writer to completion on the main thread
// while it is held, then releases it. No test has timing on its passing path:
// the interleaving is forced, not hoped for (AGENTS.md section 2.1.5).
//
// Built with -DEXPANSE_MEMTABLE_PARK_POINTS, and so is src/expanse_memtable.cc
// in the same binary; a build without it has no park points and this file
// refuses to compile.
//
// G-O6 is the gate of docs/benchmarks/rocksdb_memtable/METHODOLOGY.md section
// 5.16: a Get whose matches run to the end of a block, held after that block
// validated and before its next_leaf load, while the writer splits the block.
//
// The iterator anchor tests hold each iterator operation after it has chosen
// the position it moves to and before the anchor on that position is captured
// (or, for key() and Valid(), after RevalidatePosition() accepted the position
// and before it is read), while an insert shifts or splits the block. The
// anchor must name the entry the operation chose, so the cursor reports it.

#ifndef EXPANSE_MEMTABLE_PARK_POINTS
#error "test_memtable_park_points.cc must be compiled with -DEXPANSE_MEMTABLE_PARK_POINTS"
#endif

#include <condition_variable>
#include <cstdio>
#include <functional>
#include <map>
#include <memory>
#include <cstdlib>
#include <iostream>
#include <mutex>
#include <set>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

#include "expanse_memtable.h"

using namespace rocksdb;
using expanse_rocksdb::ParkPoint;
using Scope = ExpanseMemTableRep::SeekLockScope;

namespace {

// Fails the binary with a message a CI step can match on. Not assert(): the
// message names the gate, the scope and what was observed, and it does not
// depend on NDEBUG.
[[noreturn]] void Fail(const std::string& msg) {
    std::cerr << "FAILED: " << msg << std::endl;
    std::abort();
}

void Require(bool cond, const std::string& msg) {
    if (!cond) Fail(msg);
}

const char* ScopeName(Scope scope) {
    return scope == Scope::kFullLocate ? "kFullLocate" : "kTrieCall";
}

class Comparator : public MemTableRep::KeyComparator {
public:
    int operator()(const char* a, const char* b) const override {
        return expanse_rocksdb::CompareInternalKeys(expanse_rocksdb::GetLengthPrefixedSlice(a),
                                                    expanse_rocksdb::GetLengthPrefixedSlice(b));
    }
};

// [varint32(ikey_len)] [user_key] [8-byte trailer] [varint32(val_len)] [value]
const char* Encode(Arena& arena, const std::string& user_key, SequenceNumber seq) {
    const size_t ikey_len = user_key.size() + 8;
    char* buf = arena.Allocate(5 + ikey_len + 5);
    char* p = expanse_rocksdb::EncodeVarint32(buf, static_cast<uint32_t>(ikey_len));
    memcpy(p, user_key.data(), user_key.size());
    p += user_key.size();
    const uint64_t trailer = (seq << 8) | kTypeValue;
    for (int i = 0; i < 8; ++i) p[i] = static_cast<char>((trailer >> (i * 8)) & 0xff);
    p += 8;
    expanse_rocksdb::EncodeVarint32(p, 0);
    return buf;
}

// Eight bytes, so every user key carries its own trie prefix.
std::string Key(int n) {
    char buf[16];
    std::snprintf(buf, sizeof(buf), "%08d", n);
    return std::string(buf, 8);
}

std::string Describe(const char* entry) {
    const Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
    uint64_t trailer = 0;
    for (int i = 0; i < 8; ++i) {
        trailer |= static_cast<uint64_t>(static_cast<unsigned char>(ikey.data()[ikey.size() - 8 + i])) << (i * 8);
    }
    std::ostringstream o;
    o << std::string(ikey.data(), ikey.size() - 8) << "@" << (trailer >> 8);
    return o.str();
}

// One-shot park: the first time `target` reaches `point` after Arm(), it
// parks until Release(). Any other thread, point or later pass runs on.
class Parker {
public:
    // Called by the test thread before the target runs, so the state a
    // previous test left behind is cleared before anything can wait on it.
    void Arm(ParkPoint point, std::thread::id target) {
        std::lock_guard<std::mutex> l(m_);
        point_ = point;
        target_ = target;
        armed_ = true;
        parked_ = false;
        released_ = false;
        fired_ = 0;
    }
    void Hook(ParkPoint point) {
        std::unique_lock<std::mutex> l(m_);
        if (!armed_ || point != point_ || std::this_thread::get_id() != target_) return;
        armed_ = false;
        parked_ = true;
        ++fired_;
        cv_.notify_all();
        cv_.wait(l, [this] { return released_; });
    }
    // Blocks until the armed thread has parked. The only wait in a test, and
    // it has no timeout on purpose: a park point that is never reached hangs
    // the binary, and the CI lane's timeout records it.
    void WaitParked() {
        std::unique_lock<std::mutex> l(m_);
        cv_.wait(l, [this] { return parked_; });
    }
    void Release() {
        std::lock_guard<std::mutex> l(m_);
        released_ = true;
        cv_.notify_all();
    }
    int fired() {
        std::lock_guard<std::mutex> l(m_);
        return fired_;
    }

private:
    std::mutex m_;
    std::condition_variable cv_;
    ParkPoint point_{};
    std::thread::id target_{};
    bool armed_ = false;
    bool parked_ = false;
    bool released_ = false;
    int fired_ = 0;
};

Parker g_parker;

void ParkHook(ParkPoint point) { g_parker.Hook(point); }

// Starts `body` on a new thread armed to park at `point`, and returns once it
// has parked. The thread waits on a gate until it is armed, so it cannot reach
// the point first.
std::thread RunUntilParked(ParkPoint point, std::function<void()> body) {
    auto gate = std::make_shared<std::pair<std::mutex, std::condition_variable>>();
    auto open = std::make_shared<bool>(false);
    std::thread t([gate, open, body = std::move(body)] {
        {
            std::unique_lock<std::mutex> l(gate->first);
            gate->second.wait(l, [&] { return *open; });
        }
        body();
    });
    g_parker.Arm(point, t.get_id());
    {
        std::lock_guard<std::mutex> l(gate->first);
        *open = true;
    }
    gate->second.notify_all();
    g_parker.WaitParked();
    return t;
}

struct Delivered {
    std::vector<const char*> entries;
};

bool Collect(void* arg, const char* entry) {
    static_cast<Delivered*>(arg)->entries.push_back(entry);
    return true;  // every match is delivered, so a duplicate cannot hide behind an early stop
}

// G-O6. The head block ends in three versions of one user key `u`, so a Get
// for `u` delivers them and then loads next_leaf. Held there, the writer
// inserts one smaller key into the head, which fills it and splits it: the
// versions move into the new successor, which the resumed Get scans next. Each
// version must reach the callback once.
void TestGetDeliversEachEntryOnceAcrossSplit(Scope scope) {
    const std::string gate = std::string("G-O6 (") + ScopeName(scope) + ")";
    std::cout << "[RUN] " << gate << " Get across a split of the block it validated" << std::endl;

    Comparator cmp;
    Arena arena;
    ExpanseMemTableRep rep(cmp, &arena, nullptr, nullptr, 8, scope);

    // 10, 20, 30, 40 and 90..93: the eighth insert splits the head into
    // [10, 20, 30, 40] and [90, 91, 92, 93].
    for (int n : {10, 20, 30, 40, 90, 91, 92, 93}) {
        rep.Insert(const_cast<char*>(Encode(arena, Key(n), 1)));
    }
    // Three versions of 50 land at the end of the head: [10, 20, 30, 40, 50@9, 50@8, 50@7].
    const std::string u = Key(50);
    std::vector<const char*> versions;
    for (SequenceNumber seq : {9, 8, 7}) {
        versions.push_back(Encode(arena, u, seq));
        rep.Insert(const_cast<char*>(versions.back()));
    }
    Require(rep.LeafBlockCountForTest() == 2, gate + ": fixture has " +
            std::to_string(rep.LeafBlockCountForTest()) + " blocks before the writer, expected 2");

    Delivered delivered;
    expanse_rocksdb::g_park_hook.store(&ParkHook, std::memory_order_release);
    std::thread reader = RunUntilParked(ParkPoint::kGetBeforeNextLeaf, [&] {
        LookupKey lk(Slice(u), 1000);
        rep.Get(lk, &delivered, &Collect);
    });

    // One key below the versions fills the head to 8 and splits it: the head
    // keeps [10, 20, 30, 40] and the versions move with 41 into its successor.
    rep.Insert(const_cast<char*>(Encode(arena, Key(41), 1)));
    Require(rep.LeafBlockCountForTest() == 3, gate + ": the writer's insert did not split the block the reader "
            "validated (" + std::to_string(rep.LeafBlockCountForTest()) + " blocks, expected 3)");

    g_parker.Release();
    reader.join();
    expanse_rocksdb::g_park_hook.store(nullptr, std::memory_order_release);
    Require(g_parker.fired() == 1, gate + ": the park point fired " + std::to_string(g_parker.fired()) +
            " times, expected once");

    std::set<const char*> seen;
    for (const char* e : delivered.entries) {
        if (!seen.insert(e).second) {
            Fail(gate + ": Get delivered entry " + Describe(e) + " twice (" +
                 std::to_string(delivered.entries.size()) + " deliveries for " +
                 std::to_string(versions.size()) + " versions)");
        }
    }
    Require(delivered.entries == versions, gate + ": Get delivered " + std::to_string(delivered.entries.size()) +
            " entries, expected the " + std::to_string(versions.size()) + " versions in internal-key order");
    std::cout << "  -> PASSED" << std::endl;
}

// ---------------------------------------------------------------------------
// The iterator anchor
// ---------------------------------------------------------------------------

std::string Name(const char* entry) { return entry == nullptr ? "<none>" : Describe(entry); }

// One single-block or split-bound rep with named entries.
struct IterFixture {
    IterFixture(size_t capacity, Scope scope, std::initializer_list<int> keys)
        : rep(cmp, &arena, nullptr, nullptr, capacity, scope) {
        for (int n : keys) entry[n] = Insert(n);
    }
    const char* Insert(int n) {
        const char* e = Encode(arena, Key(n), 1);
        rep.Insert(const_cast<char*>(e));
        return e;
    }
    Comparator cmp;
    Arena arena;
    ExpanseMemTableRep rep;
    std::map<int, const char*> entry;
};

void SeekTo(MemTableRep::Iterator* it, const char* e) {
    it->Seek(expanse_rocksdb::GetLengthPrefixedSlice(e), e);
}

// Runs `op` on a reader thread, parked at `point`; while it is held the main
// thread inserts `insert_key`; then releases it and returns. `op` positions the
// iterator first if it needs to, and records what the parked call returned.
void RunParkedIteratorOp(IterFixture& f, ParkPoint point, int insert_key, const std::function<void()>& op,
                         const std::string& gate) {
    expanse_rocksdb::g_park_hook.store(&ParkHook, std::memory_order_release);
    std::thread reader = RunUntilParked(point, op);
    f.entry[insert_key] = f.Insert(insert_key);
    g_parker.Release();
    reader.join();
    expanse_rocksdb::g_park_hook.store(nullptr, std::memory_order_release);
    Require(g_parker.fired() == 1, gate + ": the park point fired " + std::to_string(g_parker.fired()) +
            " times, expected once");
}

// [10 20 30 40] in one block of capacity 16; the insert of 15 shifts 20, 30
// and 40 one slot right without splitting.
void TestSeekAnchor(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", Seek)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    RunParkedIteratorOp(f, ParkPoint::kSeekBeforeAnchor, 15, [&] { SeekTo(it.get(), f.entry[30]); }, gate);
    const char* got = it->Valid() ? it->key() : nullptr;
    Require(got == f.entry[30], gate + ": Seek(" + Name(f.entry[30]) + ") reports " + Name(got) +
            " after an insert shifted the block between its validated scan and its anchor");
    std::cout << "  -> PASSED" << std::endl;
}

void TestSeekForPrevAnchor(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", SeekForPrev)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    RunParkedIteratorOp(f, ParkPoint::kSeekForPrevBeforeAnchor, 15, [&] {
        it->SeekForPrev(expanse_rocksdb::GetLengthPrefixedSlice(f.entry[30]), f.entry[30]);
    }, gate);
    const char* got = it->Valid() ? it->key() : nullptr;
    Require(got == f.entry[30], gate + ": SeekForPrev(" + Name(f.entry[30]) + ") reports " + Name(got) +
            " after an insert shifted the block before its anchor");
    std::cout << "  -> PASSED" << std::endl;
}

void TestNextAnchor(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", Next)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    RunParkedIteratorOp(f, ParkPoint::kNextBeforeAnchor, 15, [&] {
        SeekTo(it.get(), f.entry[20]);
        it->Next();
    }, gate);
    const char* got = it->Valid() ? it->key() : nullptr;
    Require(got == f.entry[30], gate + ": Next from " + Name(f.entry[20]) + " reports " + Name(got) +
            ", expected " + Name(f.entry[30]));
    std::cout << "  -> PASSED" << std::endl;
}

void TestPrevAnchor(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", Prev)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    RunParkedIteratorOp(f, ParkPoint::kPrevBeforeAnchor, 15, [&] {
        SeekTo(it.get(), f.entry[40]);
        it->Prev();
    }, gate);
    const char* got = it->Valid() ? it->key() : nullptr;
    Require(got == f.entry[30], gate + ": Prev from " + Name(f.entry[40]) + " reports " + Name(got) +
            ", expected " + Name(f.entry[30]));
    std::cout << "  -> PASSED" << std::endl;
}

// No interleaving is known to make SeekToFirst report a wrong entry: slot 0
// changes only when a smaller key is inserted, and that key is then the first
// entry. The test pins that the answer is the first entry before or after the
// insert of 5; it is not a fail-then-pass case.
void TestSeekToFirstAnchor(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", SeekToFirst)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    RunParkedIteratorOp(f, ParkPoint::kSeekToFirstBeforeAnchor, 5, [&] { it->SeekToFirst(); }, gate);
    const char* got = it->Valid() ? it->key() : nullptr;
    Require(got == f.entry[10] || got == f.entry[5], gate + ": SeekToFirst reports " + Name(got) +
            ", expected " + Name(f.entry[10]) + " or " + Name(f.entry[5]));
    std::cout << "  -> PASSED" << std::endl;
}

void TestSeekToLastAnchor(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", SeekToLast)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    RunParkedIteratorOp(f, ParkPoint::kSeekToLastBeforeAnchor, 15, [&] { it->SeekToLast(); }, gate);
    const char* got = it->Valid() ? it->key() : nullptr;
    Require(got == f.entry[40], gate + ": SeekToLast reports " + Name(got) + ", expected " + Name(f.entry[40]));
    std::cout << "  -> PASSED" << std::endl;
}

// SeekToFirst, then ScanBatch(2) delivers 10 and 20 and is held before its
// anchor on 30. After the insert of 15, the next batch must continue at 30.
void TestScanBatchAnchor(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", ScanBatch)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    auto* impl = dynamic_cast<ExpanseMemTableRep::IteratorImpl*>(it.get());
    Require(impl != nullptr, gate + ": GetIterator did not return an IteratorImpl");
    std::vector<Slice> first(2), rest(8);
    size_t n_first = 0;
    RunParkedIteratorOp(f, ParkPoint::kScanBatchBeforeAnchor, 15, [&] {
        it->SeekToFirst();
        n_first = impl->ScanBatch(2, first.data());
    }, gate);
    auto same = [](const Slice& s, const char* e) { return s == expanse_rocksdb::GetLengthPrefixedSlice(e); };
    Require(n_first == 2 && same(first[0], f.entry[10]) && same(first[1], f.entry[20]),
            gate + ": the first batch did not deliver 10 and 20");
    const size_t n_rest = impl->ScanBatch(8, rest.data());
    std::string got;
    for (size_t i = 0; i < n_rest; ++i) got += (i ? ", " : "") + std::string(rest[i].data(), rest[i].size() - 8);
    Require(n_rest == 2 && same(rest[0], f.entry[30]) && same(rest[1], f.entry[40]),
            gate + ": after the insert, the next batch delivered [" + got + "], expected [00000030, 00000040]");
    std::cout << "  -> PASSED" << std::endl;
}

void TestKeyAfterRevalidate(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", key)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(16, scope, {10, 20, 30, 40});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    const char* got = nullptr;
    RunParkedIteratorOp(f, ParkPoint::kKeyAfterRevalidate, 15, [&] {
        SeekTo(it.get(), f.entry[30]);
        got = it->key();
    }, gate);
    Require(got == f.entry[30], gate + ": key() on a cursor at " + Name(f.entry[30]) + " returned " + Name(got) +
            " after an insert shifted the block between its revalidation and its read");
    std::cout << "  -> PASSED" << std::endl;
}

// Capacity 8, [10 .. 70] in one block. The cursor sits on 70 (slot 6); the
// insert of 15 fills the block and splits it, so 70 moves to the new block.
void TestValidAfterRevalidate(Scope scope) {
    const std::string gate = std::string("Iterator anchor (") + ScopeName(scope) + ", Valid)";
    std::cout << "[RUN] " << gate << std::endl;
    IterFixture f(8, scope, {10, 20, 30, 40, 50, 60, 70});
    std::unique_ptr<MemTableRep::Iterator> it(f.rep.GetIterator());
    bool valid = false;
    RunParkedIteratorOp(f, ParkPoint::kValidAfterRevalidate, 15, [&] {
        SeekTo(it.get(), f.entry[70]);
        valid = it->Valid();
    }, gate);
    Require(f.rep.LeafBlockCountForTest() == 2, gate + ": the insert did not split the block");
    Require(valid, gate + ": Valid() on a cursor at " + Name(f.entry[70]) +
            " returned false after a split moved its entry to the next block");
    const char* got = it->key();
    Require(got == f.entry[70], gate + ": the cursor then reports " + Name(got) + ", expected " + Name(f.entry[70]));
    std::cout << "  -> PASSED" << std::endl;
}

}  // namespace

// `test_memtable_park_points [full|trie|all] [test]`: with no argument every
// scope runs every test; a scope runs only that scope, and a test name only
// that test, so a failure under one does not hide the result of the next.
int main(int argc, char** argv) {
    // Tests by name, so one site's failure can be shown on its own.
    const std::vector<std::pair<std::string, void (*)(Scope)>> tests = {
        {"get", TestGetDeliversEachEntryOnceAcrossSplit},
        {"seek", TestSeekAnchor},
        {"seekforprev", TestSeekForPrevAnchor},
        {"next", TestNextAnchor},
        {"prev", TestPrevAnchor},
        {"seektofirst", TestSeekToFirstAnchor},
        {"seektolast", TestSeekToLastAnchor},
        {"scanbatch", TestScanBatchAnchor},
        {"key", TestKeyAfterRevalidate},
        {"valid", TestValidAfterRevalidate},
    };
    std::vector<Scope> scopes = {Scope::kFullLocate, Scope::kTrieCall};
    std::string only_test;
    if (argc >= 2) {
        const std::string only = argv[1];
        if (only == "full") {
            scopes = {Scope::kFullLocate};
        } else if (only == "trie") {
            scopes = {Scope::kTrieCall};
        } else if (only != "all") {
            std::cerr << "unknown scope '" << only << "': expected full, trie or all" << std::endl;
            return 2;
        }
    }
    if (argc == 3) {
        only_test = argv[2];
        bool known = false;
        for (const auto& t : tests) known = known || t.first == only_test;
        if (!known) {
            std::cerr << "unknown test '" << only_test << "'" << std::endl;
            return 2;
        }
    } else if (argc > 3) {
        std::cerr << "usage: test_memtable_park_points [full|trie|all] [test]" << std::endl;
        return 2;
    }

    std::cout << "============================================================" << std::endl;
    std::cout << "Running Expanse RocksDB MemTable park-point tests" << std::endl;
    std::cout << "============================================================" << std::endl;

    for (Scope scope : scopes) {
        for (const auto& t : tests) {
            if (only_test.empty() || t.first == only_test) t.second(scope);
        }
    }

    std::cout << "============================================================" << std::endl;
    std::cout << "ALL PARK-POINT TESTS PASSED" << std::endl;
    std::cout << "============================================================" << std::endl;
    return 0;
}
