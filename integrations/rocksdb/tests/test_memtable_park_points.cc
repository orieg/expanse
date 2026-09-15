// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// test_memtable_park_points.cc -- deterministic interleavings on the Get path.
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

#ifndef EXPANSE_MEMTABLE_PARK_POINTS
#error "test_memtable_park_points.cc must be compiled with -DEXPANSE_MEMTABLE_PARK_POINTS"
#endif

#include <condition_variable>
#include <cstdio>
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

}  // namespace

// `test_memtable_park_points [full|trie]`: with no argument every scope runs;
// with one, only that scope does, so a failure under one scope does not hide
// the result under the next.
int main(int argc, char** argv) {
    std::vector<Scope> scopes = {Scope::kFullLocate, Scope::kTrieCall};
    if (argc == 2) {
        const std::string only = argv[1];
        if (only == "full") {
            scopes = {Scope::kFullLocate};
        } else if (only == "trie") {
            scopes = {Scope::kTrieCall};
        } else {
            std::cerr << "unknown scope '" << only << "': expected full or trie" << std::endl;
            return 2;
        }
    } else if (argc > 2) {
        std::cerr << "usage: test_memtable_park_points [full|trie]" << std::endl;
        return 2;
    }

    std::cout << "============================================================" << std::endl;
    std::cout << "Running Expanse RocksDB MemTable park-point tests" << std::endl;
    std::cout << "============================================================" << std::endl;

    for (Scope scope : scopes) {
        TestGetDeliversEachEntryOnceAcrossSplit(scope);
    }

    std::cout << "============================================================" << std::endl;
    std::cout << "ALL PARK-POINT TESTS PASSED" << std::endl;
    std::cout << "============================================================" << std::endl;
    return 0;
}
