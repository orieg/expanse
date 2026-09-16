// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// test_memtable_park_points.cc -- deterministic interleavings and handle
// lifecycles on the Get, locate and iterator paths.
//
// The soundness gates of docs/benchmarks/rocksdb_memtable/METHODOLOGY.md
// section 5.16 that do not depend on race timing:
//
//   G-O3  park points on the locate path: (a) between the trie read and
//         SettleSeekCandidate, (b) between the tail_ load and the trie read,
//         (c) across a prefix remap, in both orders;
//   G-O4  the reader-handle registry: R1, R2, R3 (address reuse), R4
//         (eviction) and R5 (destruction and thread exit in either order);
//   G-O5  no mutex_ on the kOptimistic read path, with a writer parked inside
//         Insert holding it;
//   G-O6  no duplicate delivery from a Get across a split;
//   and the test that mutation 8 of section 5.16 is aimed at: a writer parked
//   between SplitLeafBlock's link and its trie insert.
//
// Each park test holds one thread at a named point (expanse_rocksdb::ParkPoint)
// while another runs to a checkpoint, then releases it. No test has timing on
// its passing path. Where a gate fails by not completing (G-O5, mutation 8), a
// wait on the failing path carries a bound so the failure is reported by name
// instead of only by the CI lane's timeout.
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
//
// `test_memtable_park_points [full|trie|opt|all] [test]` runs one scope, or one
// named test, so a failure under one does not hide the result of the next.

#ifndef EXPANSE_MEMTABLE_PARK_POINTS
#error "test_memtable_park_points.cc must be compiled with -DEXPANSE_MEMTABLE_PARK_POINTS"
#endif

#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdio>
#include <functional>
#include <map>
#include <memory>
#include <cstdlib>
#include <deque>
#include <functional>
#include <iostream>
#include <map>
#include <memory>
#include <mutex>
#include <new>
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
    switch (scope) {
        case Scope::kFullLocate: return "kFullLocate";
        case Scope::kTrieCall: return "kTrieCall";
        case Scope::kOptimistic: return "kOptimistic";
    }
    return "unknown";
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

// Eight bytes, so every such user key carries its own trie prefix.
std::string Key(int n) {
    char buf[16];
    std::snprintf(buf, sizeof(buf), "%08d", n);
    return std::string(buf, 8);
}

// Twelve bytes whose first eight are `prefix`, so keys share a trie prefix.
std::string SharedKey(const char* prefix, int n) {
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%.8s%04d", prefix, n);
    return std::string(buf, 12);
}

std::string Describe(const char* entry) {
    if (entry == nullptr) return "<none>";
    const Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
    uint64_t trailer = 0;
    for (int i = 0; i < 8; ++i) {
        trailer |= static_cast<uint64_t>(static_cast<unsigned char>(ikey.data()[ikey.size() - 8 + i])) << (i * 8);
    }
    std::ostringstream o;
    o << std::string(ikey.data(), ikey.size() - 8) << "@" << (trailer >> 8);
    return o.str();
}

uint64_t PrefixOf(const char* entry) { return expanse_rocksdb::ExtractKeyPrefix64(entry); }

// ---------------------------------------------------------------------------
// Parking
// ---------------------------------------------------------------------------

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
    // Blocks until the armed thread has parked. It has no bound on purpose: a
    // park point that is never reached hangs the binary, and the CI lane's
    // timeout records it.
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

struct HookInstalled {
    HookInstalled() { expanse_rocksdb::g_park_hook.store(&ParkHook, std::memory_order_release); }
    ~HookInstalled() { expanse_rocksdb::g_park_hook.store(nullptr, std::memory_order_release); }
};

// A latch a thread opens once and others wait on.
class Latch {
public:
    void Open() {
        std::lock_guard<std::mutex> l(m_);
        open_ = true;
        cv_.notify_all();
    }
    void Wait() {
        std::unique_lock<std::mutex> l(m_);
        cv_.wait(l, [this] { return open_; });
    }
    // The failing-path bound: returns false if the latch did not open in time.
    bool WaitFor(std::chrono::seconds bound) {
        std::unique_lock<std::mutex> l(m_);
        return cv_.wait_for(l, bound, [this] { return open_; });
    }

private:
    std::mutex m_;
    std::condition_variable cv_;
    bool open_ = false;
};

// Starts `body` on a new thread armed to park at `point`, and returns once it
// has parked. The thread waits on a gate until it is armed, so it cannot reach
// the point first.
std::thread RunUntilParked(ParkPoint point, std::function<void()> body) {
    auto gate = std::make_shared<Latch>();
    std::thread t([gate, body = std::move(body)] {
        gate->Wait();
        body();
    });
    g_parker.Arm(point, t.get_id());
    gate->Open();
    g_parker.WaitParked();
    return t;
}

// A thread that runs closures one at a time and stays alive between them, so
// a test can read with it, destroy a rep on another thread, and read again.
class Worker {
public:
    Worker() : thread_([this] { Loop(); }) {}
    ~Worker() { Stop(); }
    void Run(std::function<void()> fn) {
        auto done = std::make_shared<Latch>();
        {
            std::lock_guard<std::mutex> l(m_);
            queue_.push_back([fn = std::move(fn), done] {
                fn();
                done->Open();
            });
        }
        cv_.notify_all();
        done->Wait();
    }
    void Stop() {
        if (!thread_.joinable()) return;
        {
            std::lock_guard<std::mutex> l(m_);
            stop_ = true;
        }
        cv_.notify_all();
        thread_.join();
    }

private:
    void Loop() {
        while (true) {
            std::function<void()> fn;
            {
                std::unique_lock<std::mutex> l(m_);
                cv_.wait(l, [this] { return stop_ || !queue_.empty(); });
                if (queue_.empty()) return;
                fn = std::move(queue_.front());
                queue_.pop_front();
            }
            fn();
        }
    }
    std::mutex m_;
    std::condition_variable cv_;
    std::deque<std::function<void()>> queue_;
    bool stop_ = false;
    std::thread thread_;
};

// ---------------------------------------------------------------------------
// The four reads every G-O3 test requires to find the targets
// ---------------------------------------------------------------------------

enum class Op { kGet, kContains, kSeek, kSeekForPrev };
constexpr Op kOps[] = {Op::kGet, Op::kContains, Op::kSeek, Op::kSeekForPrev};

const char* OpName(Op op) {
    switch (op) {
        case Op::kGet: return "Get";
        case Op::kContains: return "Contains";
        case Op::kSeek: return "Seek";
        case Op::kSeekForPrev: return "SeekForPrev";
    }
    return "?";
}

// Runs one read for `target` (an entry whose user key has no other version).
// Get and Seek take a LookupKey above every sequence, so the answer is the
// first entry at or after it; Contains and SeekForPrev take the entry itself.
// Returns what the read found.
const char* RunOp(ExpanseMemTableRep& rep, Op op, const char* target) {
    const Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(target);
    const Slice user_key(ikey.data(), ikey.size() - 8);
    switch (op) {
        case Op::kGet: {
            LookupKey lk(user_key, 1ull << 40);
            std::vector<const char*> delivered;
            rep.Get(lk, &delivered, [](void* arg, const char* entry) -> bool {
                static_cast<std::vector<const char*>*>(arg)->push_back(entry);
                return true;
            });
            for (const char* e : delivered) {
                if (e == target) return e;
            }
            return delivered.empty() ? nullptr : delivered.front();
        }
        case Op::kContains:
            return rep.Contains(target) ? target : nullptr;
        case Op::kSeek: {
            LookupKey lk(user_key, 1ull << 40);
            std::unique_ptr<MemTableRep::Iterator> it(rep.GetIterator());
            it->Seek(lk.internal_key(), lk.memtable_key().data());
            return it->Valid() ? it->key() : nullptr;
        }
        case Op::kSeekForPrev: {
            std::unique_ptr<MemTableRep::Iterator> it(rep.GetIterator());
            it->SeekForPrev(ikey, target);
            return it->Valid() ? it->key() : nullptr;
        }
    }
    return nullptr;
}

// The first point a reader under `scope` holds no lock after its trie read.
ParkPoint AfterTrieReadOutsideLock(Scope scope) {
    return scope == Scope::kFullLocate ? ParkPoint::kAfterLocate : ParkPoint::kLocateAfterTrieRead;
}

struct Rep {
    explicit Rep(size_t capacity, Scope scope) : rep(cmp, &arena, nullptr, nullptr, capacity, scope) {}
    const char* Insert(const std::string& user_key, SequenceNumber seq = 1) {
        const char* e = Encode(arena, user_key, seq);
        rep.Insert(const_cast<char*>(e));
        return e;
    }
    Comparator cmp;
    Arena arena;
    ExpanseMemTableRep rep;
};

// ---------------------------------------------------------------------------
// G-O3 (a): kOptimistic, parked between the trie read and SettleSeekCandidate
// ---------------------------------------------------------------------------

// Capacity 8. [10 35 40 45 50 60 70 80] splits into head [10 35 40 45] and
// N1 [50 60 70 80]. A reader for 35 reads the trie (prefix 10 -> head) and
// parks. The writer inserts 11, 12, 13 and 46, which splits the head: it keeps
// [10 11 12 13] and 35 moves to the front of a new successor [35 40 45 46].
void TestLocateParkedBeforeSettle(Op op) {
    const std::string gate = std::string("G-O3a (kOptimistic, ") + OpName(op) + ")";
    std::cout << "[RUN] " << gate << " split of the trie's block while parked before the walk" << std::endl;
    Rep r(8, Scope::kOptimistic);
    const char* target = nullptr;
    for (int n : {10, 35, 40, 45, 50, 60, 70, 80}) {
        const char* e = r.Insert(Key(n));
        if (n == 35) target = e;
    }
    Require(r.rep.LeafBlockCountForTest() == 2, gate + ": fixture is not two blocks");
    Require(r.rep.TrieBlockIndexForTest(PrefixOf(target)) == 0, gate + ": the trie does not map 35 to the head");

    HookInstalled hook;
    const char* found = nullptr;
    std::thread reader = RunUntilParked(ParkPoint::kLocateAfterTrieRead, [&] { found = RunOp(r.rep, op, target); });
    for (int n : {11, 12, 13, 46}) r.Insert(Key(n));
    Require(r.rep.LeafBlockCountForTest() == 3, gate + ": the writer did not split the head");
    Require(r.rep.TrieBlockIndexForTest(PrefixOf(target)) == 1,
            gate + ": the target did not move to the head's new successor");
    g_parker.Release();
    reader.join();
    Require(g_parker.fired() == 1, gate + ": the park point fired " + std::to_string(g_parker.fired()) + " times");
    Require(found == target, gate + ": target " + Describe(target) + " not found after the resume (found " +
            Describe(found) + ")");
    std::cout << "  -> PASSED" << std::endl;
}

// ---------------------------------------------------------------------------
// G-O3 (b): kOptimistic, parked between the tail_ load and the trie read
// ---------------------------------------------------------------------------

// The same fixture. The reader loads head != tail (N1) and parks. The writer
// splits the head, which holds the target, then splits the tail twice, so
// tail_ moves on from the block the reader loaded.
void TestLocateParkedAfterTailLoad(Op op) {
    const std::string gate = std::string("G-O3b (kOptimistic, ") + OpName(op) + ")";
    std::cout << "[RUN] " << gate << " splits of the target's block and the tail while parked after the tail load"
              << std::endl;
    Rep r(8, Scope::kOptimistic);
    const char* target = nullptr;
    for (int n : {10, 35, 40, 45, 50, 60, 70, 80}) {
        const char* e = r.Insert(Key(n));
        if (n == 35) target = e;
    }
    HookInstalled hook;
    const char* found = nullptr;
    std::thread reader = RunUntilParked(ParkPoint::kLocateAfterTailLoad, [&] { found = RunOp(r.rep, op, target); });
    for (int n : {11, 12, 13, 46}) r.Insert(Key(n));  // splits the head; 35 moves
    Require(r.rep.LeafBlockCountForTest() == 3, gate + ": the writer did not split the target's block");
    for (int n : {81, 82, 83, 84}) r.Insert(Key(n));  // splits the tail
    for (int n : {85, 86, 87, 88}) r.Insert(Key(n));  // splits the new tail
    Require(r.rep.LeafBlockCountForTest() == 5, gate + ": the writer did not split the tail twice (" +
            std::to_string(r.rep.LeafBlockCountForTest()) + " blocks)");
    g_parker.Release();
    reader.join();
    Require(g_parker.fired() == 1, gate + ": the park point fired " + std::to_string(g_parker.fired()) + " times");
    Require(found == target, gate + ": target " + Describe(target) + " not found after the resume (found " +
            Describe(found) + ")");
    std::cout << "  -> PASSED" << std::endl;
}

// ---------------------------------------------------------------------------
// G-O3 (c): the prefix remap, in both orders
// ---------------------------------------------------------------------------

// Every key shares the 8-byte prefix p. Capacity 8: [p20 p22 p24 p26 p40 p42
// p44 p46] splits into A = head [p20 p22 p24 p26] and B [p40 p42 p44 p46], and
// the split remaps p to B. A reader for a target reads the trie (p -> B) and
// parks outside any lock. Then:
//
//   remap_first: the writer inserts p10 at slot 0 of A (p -> A), then p27,
//                p28 and p29, which split A (p -> the new block after A, and
//                p26 moves into it);
//   split_first: the writer inserts p27, p28, p29 and p30, which split A
//                (p -> the new block), then p10 at slot 0 of A (p -> A).
void TestPrefixRemap(Scope scope, bool remap_first, Op op, int target_suffix) {
    const std::string gate = std::string("G-O3c (") + ScopeName(scope) + ", " +
                             (remap_first ? "slot-0 remap then split" : "split then slot-0 remap") + ", " +
                             OpName(op) + ", p" + std::to_string(target_suffix) + ")";
    std::cout << "[RUN] " << gate << std::endl;
    Rep r(8, scope);
    const char* target = nullptr;
    for (int n : {20, 22, 24, 26, 40, 42, 44, 46}) {
        const char* e = r.Insert(SharedKey("PPPPPPPP", n));
        if (n == target_suffix) target = e;
    }
    Require(target != nullptr, gate + ": no such target in the fixture");
    const uint64_t p = PrefixOf(target);
    Require(r.rep.LeafBlockCountForTest() == 2 && r.rep.TrieBlockIndexForTest(p) == 1,
            gate + ": fixture does not map p to the second of two blocks");

    HookInstalled hook;
    const char* found = nullptr;
    std::thread reader = RunUntilParked(AfterTrieReadOutsideLock(scope), [&] { found = RunOp(r.rep, op, target); });
    if (remap_first) {
        r.Insert(SharedKey("PPPPPPPP", 10));
        Require(r.rep.TrieBlockIndexForTest(p) == 0, gate + ": the slot-0 insert did not remap p to A");
        for (int n : {27, 28, 29}) r.Insert(SharedKey("PPPPPPPP", n));
        Require(r.rep.LeafBlockCountForTest() == 3 && r.rep.TrieBlockIndexForTest(p) == 1,
                gate + ": the split did not remap p to the block after A");
    } else {
        for (int n : {27, 28, 29, 30}) r.Insert(SharedKey("PPPPPPPP", n));
        Require(r.rep.LeafBlockCountForTest() == 3 && r.rep.TrieBlockIndexForTest(p) == 1,
                gate + ": the split did not remap p to the block after A");
        r.Insert(SharedKey("PPPPPPPP", 10));
        Require(r.rep.TrieBlockIndexForTest(p) == 0, gate + ": the slot-0 insert did not remap p to A");
    }
    g_parker.Release();
    reader.join();
    Require(g_parker.fired() == 1, gate + ": the park point fired " + std::to_string(g_parker.fired()) + " times");
    Require(found == target, gate + ": target " + Describe(target) + " not found after the resume (found " +
            Describe(found) + ")");
    std::cout << "  -> PASSED" << std::endl;
}

// ---------------------------------------------------------------------------
// G-O4: the reader-handle registry (kOptimistic)
// ---------------------------------------------------------------------------

// A two-block rep, so a read runs the trie read and takes a handle.
std::unique_ptr<Rep> TwoBlockRep(std::vector<const char*>* entries = nullptr) {
    auto r = std::make_unique<Rep>(8, Scope::kOptimistic);
    for (int n = 1; n <= 12; ++n) {
        const char* e = r->Insert(Key(n * 10));
        if (entries) entries->push_back(e);
    }
    Require(r->rep.LeafBlockCountForTest() >= 2, "G-O4 fixture is not two blocks");
    return r;
}

void ReadAll(ExpanseMemTableRep& rep, const std::vector<const char*>& entries, const std::string& gate) {
    for (const char* e : entries) {
        Require(RunOp(rep, Op::kGet, e) == e, gate + ": Get missed " + Describe(e));
    }
}

// R1: two threads reading one rep hold different handles.
void TestHandlePerThread() {
    const std::string gate = "G-O4 R1 (kOptimistic)";
    std::cout << "[RUN] " << gate << " one handle per (rep, thread)" << std::endl;
    std::vector<const char*> entries;
    auto r = TwoBlockRep(&entries);
    const expanse_sync_map_reader_t* h[2] = {nullptr, nullptr};
    Latch read[2];
    Latch both_read;
    std::vector<std::thread> threads;
    for (int t = 0; t < 2; ++t) {
        threads.emplace_back([&, t] {
            ReadAll(r->rep, entries, gate);
            h[t] = r->rep.ReaderHandleForTest();
            read[t].Open();
            both_read.Wait();  // stay alive until both have read, so a thread id cannot be reused
        });
    }
    read[0].Wait();
    read[1].Wait();
    both_read.Open();
    for (auto& t : threads) t.join();
    Require(h[0] != nullptr && h[1] != nullptr, gate + ": a reading thread holds no handle");
    Require(h[0] != h[1], gate + ": two reader threads hold the same handle");
    Require(r->rep.ReaderHandleCount() == 2, gate + ": " + std::to_string(r->rep.ReaderHandleCount()) +
            " handles registered for two reader threads");
    std::cout << "  -> PASSED" << std::endl;
}

// R2: the destructor frees every registered handle before expanse_sync_map_free.
void TestDestructorFreesHandles() {
    const std::string gate = "G-O4 R2 (kOptimistic)";
    std::cout << "[RUN] " << gate << " no handle outlives the destructor" << std::endl;
    std::vector<const char*> entries;
    auto r = TwoBlockRep(&entries);
    std::vector<std::thread> threads;
    for (int t = 0; t < 3; ++t) threads.emplace_back([&] { ReadAll(r->rep, entries, gate); });
    for (auto& t : threads) t.join();
    Require(r->rep.ReaderHandleCount() == 3, gate + ": " + std::to_string(r->rep.ReaderHandleCount()) +
            " handles registered for three reader threads");
    r.reset();
    const long unfreed = ExpanseMemTableRep::UnfreedHandlesAtLastMapFreeForTest();
    Require(unfreed == 0, gate + ": the registry's count of unfreed handles read " + std::to_string(unfreed) +
            " before expanse_sync_map_free");
    std::cout << "  -> PASSED" << std::endl;
}

// R2, P1: the destructor, run on a thread that never read the rep, frees the
// handles of reader threads that are still alive but have stopped reading it.
void TestDestructorOnAnotherThread() {
    const std::string gate = "G-O4 R2/P1 (kOptimistic)";
    std::cout << "[RUN] " << gate << " destructor on a thread that never read the rep" << std::endl;
    std::vector<const char*> entries;
    auto r = TwoBlockRep(&entries);
    Latch rep_destroyed;
    std::vector<std::unique_ptr<Latch>> done_reading;
    std::vector<std::thread> threads;
    for (int t = 0; t < 2; ++t) {
        done_reading.push_back(std::make_unique<Latch>());
        Latch* mine = done_reading.back().get();
        threads.emplace_back([&, mine] {
            ReadAll(r->rep, entries, gate);
            mine->Open();
            rep_destroyed.Wait();  // alive, not reading, while another thread destroys the rep
        });
    }
    for (auto& l : done_reading) l->Wait();
    Require(r->rep.ReaderHandleCount() == 2, gate + ": expected two registered handles");
    std::thread destroyer([&] { r.reset(); });
    destroyer.join();
    const long unfreed = ExpanseMemTableRep::UnfreedHandlesAtLastMapFreeForTest();
    rep_destroyed.Open();
    for (auto& t : threads) t.join();
    Require(unfreed == 0, gate + ": " + std::to_string(unfreed) + " handles unfreed before expanse_sync_map_free");
    std::cout << "  -> PASSED" << std::endl;
}

// R3: a rep constructed where a destroyed one lived must not match the
// destroyed rep's cached handle.
void TestAddressReuse() {
    const std::string gate = "G-O4 R3 (kOptimistic)";
    std::cout << "[RUN] " << gate << " a rep constructed in a destroyed rep's storage" << std::endl;
    Comparator cmp;
    Arena arena_a;
    Arena arena_b;
    alignas(ExpanseMemTableRep) std::byte storage[sizeof(ExpanseMemTableRep)];
    Worker worker;

    ExpanseMemTableRep* a = new (storage) ExpanseMemTableRep(cmp, &arena_a, nullptr, nullptr, 8, Scope::kOptimistic);
    std::vector<const char*> entries_a;
    for (int n = 1; n <= 12; ++n) {
        entries_a.push_back(Encode(arena_a, Key(n * 10), 1));
        a->Insert(const_cast<char*>(entries_a.back()));
    }
    worker.Run([&] { ReadAll(*a, entries_a, gate + " A"); });
    Require(a->ReaderHandleCount() == 1, gate + ": A registered " + std::to_string(a->ReaderHandleCount()) +
            " handles for one reader thread");
    a->~ExpanseMemTableRep();

    ExpanseMemTableRep* b = new (storage) ExpanseMemTableRep(cmp, &arena_b, nullptr, nullptr, 8, Scope::kOptimistic);
    Require(static_cast<void*>(b) == static_cast<void*>(a), gate + ": B was not constructed at A's address");
    std::vector<const char*> entries_b;
    for (int n = 1; n <= 12; ++n) {
        entries_b.push_back(Encode(arena_b, Key(n * 10 + 5), 1));
        b->Insert(const_cast<char*>(entries_b.back()));
    }
    worker.Run([&] { ReadAll(*b, entries_b, gate + " B"); });
    const size_t registered = b->ReaderHandleCount();
    worker.Stop();
    b->~ExpanseMemTableRep();
    Require(registered == 1, gate + ": B's registry holds " + std::to_string(registered) +
            " handles after the reader that read A read B; expected 1");
    std::cout << "  -> PASSED" << std::endl;
}

// R4: one thread reads K + 1 = 9 reps in rotation, twice round.
void TestCacheEviction() {
    const std::string gate = "G-O4 R4 (kOptimistic)";
    std::cout << "[RUN] " << gate << " 9 reps read in rotation by one thread, twice round" << std::endl;
    constexpr int kReps = 9;
    std::vector<std::unique_ptr<Rep>> reps;
    std::vector<std::vector<const char*>> entries(kReps);
    for (int i = 0; i < kReps; ++i) reps.push_back(TwoBlockRep(&entries[i]));
    std::thread reader([&] {
        for (int round = 0; round < 2; ++round) {
            for (int i = 0; i < kReps; ++i) {
                ReadAll(reps[i]->rep, entries[i], gate + " rep " + std::to_string(i) + " round " +
                        std::to_string(round));
            }
        }
    });
    reader.join();
    for (int i = 0; i < kReps; ++i) {
        Require(reps[i]->rep.ReaderHandleCount() == 1, gate + ": rep " + std::to_string(i) + " registered " +
                std::to_string(reps[i]->rep.ReaderHandleCount()) + " handles for one reader thread");
    }
    std::cout << "  -> PASSED" << std::endl;
}

// R5, thread first: a reader thread exits, then the rep is destroyed.
void TestThreadExitsFirst() {
    const std::string gate = "G-O4 R5 thread first (kOptimistic)";
    std::cout << "[RUN] " << gate << std::endl;
    std::vector<const char*> entries;
    auto r = TwoBlockRep(&entries);
    std::thread reader([&] { ReadAll(r->rep, entries, gate); });
    reader.join();
    Require(r->rep.ReaderHandleCount() == 1, gate + ": the exited thread's handle is not registered");
    r.reset();
    Require(ExpanseMemTableRep::UnfreedHandlesAtLastMapFreeForTest() == 0,
            gate + ": handles unfreed before expanse_sync_map_free");
    std::cout << "  -> PASSED" << std::endl;
}

// R5, rep first: a rep is destroyed while its reader thread lives; the thread
// then reads a new rep and exits.
void TestRepDestroyedFirst() {
    const std::string gate = "G-O4 R5 rep first (kOptimistic)";
    std::cout << "[RUN] " << gate << std::endl;
    Worker worker;
    std::vector<const char*> entries1;
    auto r1 = TwoBlockRep(&entries1);
    worker.Run([&] { ReadAll(r1->rep, entries1, gate + " first rep"); });
    r1.reset();
    std::vector<const char*> entries2;
    auto r2 = TwoBlockRep(&entries2);
    worker.Run([&] { ReadAll(r2->rep, entries2, gate + " second rep"); });
    Require(r2->rep.ReaderHandleCount() == 1, gate + ": the second rep registered " +
            std::to_string(r2->rep.ReaderHandleCount()) + " handles");
    worker.Stop();
    r2.reset();
    Require(ExpanseMemTableRep::UnfreedHandlesAtLastMapFreeForTest() == 0,
            gate + ": handles unfreed before expanse_sync_map_free");
    std::cout << "  -> PASSED" << std::endl;
}

// ---------------------------------------------------------------------------
// G-O5: no mutex_ on the kOptimistic read path
// ---------------------------------------------------------------------------

// A writer holds mutex_, parked inside Insert before any bracket or trie call.
// Get, Contains and Seek on present keys must return correct results while it
// is parked, and the test joins them before releasing the writer. A reader that
// took mutex_ would wait for the writer, and the bounded wait below reports it.
void TestReadersDoNotWaitForInsert() {
    const std::string gate = "G-O5 (kOptimistic)";
    std::cout << "[RUN] " << gate << " reads while a writer is parked inside Insert holding mutex_" << std::endl;
    std::vector<const char*> entries;
    auto r = TwoBlockRep(&entries);
    Require(r->rep.LeafBlockCountForTest() >= 2, gate + ": the fixture must consult the trie");

    HookInstalled hook;
    std::thread writer = RunUntilParked(ParkPoint::kInsertAfterLock, [&] { r->Insert(Key(55)); });

    Latch readers_done;
    std::atomic<int> remaining{3};
    std::vector<std::thread> readers;
    for (Op op : {Op::kGet, Op::kContains, Op::kSeek}) {
        readers.emplace_back([&, op] {
            for (const char* e : entries) {
                Require(RunOp(r->rep, op, e) == e, gate + ": " + OpName(op) + " missed " + Describe(e) +
                        " while the writer was parked");
            }
            if (remaining.fetch_sub(1, std::memory_order_acquire) == 1) readers_done.Open();
        });
    }
    if (!readers_done.WaitFor(std::chrono::seconds(60))) {
        Fail(gate + ": readers did not return within 60 s while a writer was parked inside Insert holding mutex_");
    }
    for (auto& t : readers) t.join();
    g_parker.Release();
    writer.join();
    std::cout << "  -> PASSED" << std::endl;
}

// ---------------------------------------------------------------------------
// G-O6: Get delivers each entry once across a split
// ---------------------------------------------------------------------------

// The head block ends in three versions of one user key `u`, so a Get for `u`
// delivers them and then loads next_leaf. Held there, the writer inserts one
// smaller key into the head, which fills it and splits it: the versions move
// into the new successor, which the resumed Get scans next. Each version must
// reach the callback once.
void TestGetDeliversEachEntryOnceAcrossSplit(Scope scope) {
    const std::string gate = std::string("G-O6 (") + ScopeName(scope) + ")";
    std::cout << "[RUN] " << gate << " Get across a split of the block it validated" << std::endl;

    Rep r(8, scope);
    // 10, 20, 30, 40 and 90..93: the eighth insert splits the head into
    // [10, 20, 30, 40] and [90, 91, 92, 93].
    for (int n : {10, 20, 30, 40, 90, 91, 92, 93}) r.Insert(Key(n));
    // Three versions of 50 land at the end of the head: [10, 20, 30, 40, 50@9, 50@8, 50@7].
    const std::string u = Key(50);
    std::vector<const char*> versions;
    for (SequenceNumber seq : {9, 8, 7}) versions.push_back(r.Insert(u, seq));
    Require(r.rep.LeafBlockCountForTest() == 2, gate + ": fixture has " +
            std::to_string(r.rep.LeafBlockCountForTest()) + " blocks before the writer, expected 2");

    HookInstalled hook;
    std::vector<const char*> delivered;
    std::thread reader = RunUntilParked(ParkPoint::kGetBeforeNextLeaf, [&] {
        LookupKey lk(Slice(u), 1000);
        r.rep.Get(lk, &delivered, [](void* arg, const char* entry) -> bool {
            static_cast<std::vector<const char*>*>(arg)->push_back(entry);
            return true;  // every match is delivered, so a duplicate cannot hide behind an early stop
        });
    });

    // One key below the versions fills the head to 8 and splits it: the head
    // keeps [10, 20, 30, 40] and the versions move with 41 into its successor.
    r.Insert(Key(41));
    Require(r.rep.LeafBlockCountForTest() == 3, gate + ": the writer's insert did not split the block the reader "
            "validated (" + std::to_string(r.rep.LeafBlockCountForTest()) + " blocks, expected 3)");

    g_parker.Release();
    reader.join();
    Require(g_parker.fired() == 1, gate + ": the park point fired " + std::to_string(g_parker.fired()) +
            " times, expected once");

    std::set<const char*> seen;
    for (const char* e : delivered) {
        if (!seen.insert(e).second) {
            Fail(gate + ": Get delivered entry " + Describe(e) + " twice (" + std::to_string(delivered.size()) +
                 " deliveries for " + std::to_string(versions.size()) + " versions)");
        }
    }
    Require(delivered == versions, gate + ": Get delivered " + std::to_string(delivered.size()) +
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

// ---------------------------------------------------------------------------
// Mutation 8's test: a writer parked between SplitLeafBlock's link and trie insert
// ---------------------------------------------------------------------------

// Capacity 8. Blocks H [10 20 30 40], B [50 51 52 53 60 70 80] and S [90 100
// 110 120]. The writer inserts 54, which fills B and splits it into B [50 51 52
// 53] and NB [54 60 70 80], and parks at kSplitBetweenLinkAndTrieInsert: B's
// version is odd, NB is linked, and the trie does not yet map 54.
//
// While it is parked, one reader runs to a checkpoint on blocks whose version
// is even: Get, Seek and SeekForPrev on the upper half (60, 70), and Prev from
// S's first key (90), which must step back into NB and read 80. Another reader
// starts Get, Seek, SeekForPrev and Prev on the lower half (51, 52, and Prev
// from 54), which validate B and so wait on its version until the writer is
// released. Every answer is checked after both readers join.
//
// Section 5.16's mutation 8 moves the trie insert before the link with this
// park point between them: the trie maps 54 to NB while B and S are not yet
// linked to it, and Prev from 90 steps from S into B instead of NB.
void TestWriterParkedBetweenLinkAndTrieInsert() {
    const std::string gate = "Mutation-8 test (kOptimistic)";
    std::cout << "[RUN] " << gate << " reads while a split is parked between its link and its trie insert" << std::endl;
    Rep r(8, Scope::kOptimistic);
    std::vector<int> keys = {10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 51, 52, 53};
    std::map<int, const char*> entry;
    for (int n : keys) entry[n] = r.Insert(Key(n));
    Require(r.rep.LeafBlockCountForTest() == 3, gate + ": fixture is not three blocks");

    HookInstalled hook;
    std::thread writer = RunUntilParked(ParkPoint::kSplitBetweenLinkAndTrieInsert,
                                        [&] { entry[54] = nullptr; r.Insert(Key(54)); });
    Require(r.rep.LeafBlockCountForTest() == 4, gate + ": the parked split has not linked its new block");

    struct Answer { std::string what; const char* want; const char* got; };
    std::vector<Answer> during, after;
    Latch upper_done;
    std::thread upper([&] {
        for (int n : {60, 70}) {
            during.push_back({"Get " + Key(n), entry[n], RunOp(r.rep, Op::kGet, entry[n])});
            during.push_back({"Seek " + Key(n), entry[n], RunOp(r.rep, Op::kSeek, entry[n])});
            during.push_back({"SeekForPrev " + Key(n), entry[n], RunOp(r.rep, Op::kSeekForPrev, entry[n])});
        }
        std::unique_ptr<MemTableRep::Iterator> it(r.rep.GetIterator());
        it->SeekForPrev(expanse_rocksdb::GetLengthPrefixedSlice(entry[90]), entry[90]);
        it->Prev();
        const char* got = it->Valid() ? it->key() : nullptr;
        during.push_back({"Prev from " + Key(90), entry[80], got});
        upper_done.Open();
    });
    if (!upper_done.WaitFor(std::chrono::seconds(60))) {
        Fail(gate + ": reads on unlocked blocks did not return within 60 s while the split was parked");
    }
    upper.join();

    Latch lower_started;
    std::thread lower([&] {
        lower_started.Open();
        for (int n : {51, 52}) {
            after.push_back({"Get " + Key(n), entry[n], RunOp(r.rep, Op::kGet, entry[n])});
            after.push_back({"Seek " + Key(n), entry[n], RunOp(r.rep, Op::kSeek, entry[n])});
            after.push_back({"SeekForPrev " + Key(n), entry[n], RunOp(r.rep, Op::kSeekForPrev, entry[n])});
        }
    });
    lower_started.Wait();
    g_parker.Release();
    writer.join();
    lower.join();
    // Prev from the split's first moved key, after the release.
    {
        LookupKey lk(Slice(Key(54)), 1ull << 40);
        std::unique_ptr<MemTableRep::Iterator> it(r.rep.GetIterator());
        it->Seek(lk.internal_key(), lk.memtable_key().data());
        it->Prev();
        after.push_back({"Prev from " + Key(54), entry[53], it->Valid() ? it->key() : nullptr});
    }
    for (const auto& a : during) {
        Require(a.got == a.want, gate + ": while parked, " + a.what + " read " + Describe(a.got) + ", expected " +
                Describe(a.want));
    }
    for (const auto& a : after) {
        Require(a.got == a.want, gate + ": " + a.what + " read " + Describe(a.got) + ", expected " + Describe(a.want));
    }
    std::cout << "  -> PASSED" << std::endl;
}

}  // namespace

// `test_memtable_park_points [full|trie|opt|all] [test]`: with no argument every
// scope runs every test; a scope runs only that scope, and a test name only that
// test, so a failure under one does not hide the result of the next. The tests
// that take no scope -- the locate park points, the handle registry and
// mutation 8's -- belong to kOptimistic and run only when no test name is given.
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
    std::vector<Scope> scopes = {Scope::kFullLocate, Scope::kTrieCall, Scope::kOptimistic};
    std::string only_test;
    if (argc >= 2) {
        const std::string only = argv[1];
        if (only == "full") {
            scopes = {Scope::kFullLocate};
        } else if (only == "trie") {
            scopes = {Scope::kTrieCall};
        } else if (only == "opt") {
            scopes = {Scope::kOptimistic};
        } else if (only != "all") {
            std::cerr << "unknown scope '" << only << "': expected full, trie, opt or all" << std::endl;
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
        std::cerr << "usage: test_memtable_park_points [full|trie|opt|all] [test]" << std::endl;
        return 2;
    }

    bool opt = false;
    for (Scope scope : scopes) opt = opt || scope == Scope::kOptimistic;
    const bool every_test = only_test.empty();

    std::cout << "============================================================" << std::endl;
    std::cout << "Running Expanse RocksDB MemTable park-point tests" << std::endl;
    std::cout << "============================================================" << std::endl;

    if (every_test) {
        if (opt) {
            for (Op op : kOps) TestLocateParkedBeforeSettle(op);
            for (Op op : kOps) TestLocateParkedAfterTailLoad(op);
        }
        for (Scope scope : scopes) {
            for (bool remap_first : {true, false}) {
                for (Op op : kOps) {
                    TestPrefixRemap(scope, remap_first, op, 24);
                    TestPrefixRemap(scope, remap_first, op, 26);
                    TestPrefixRemap(scope, remap_first, op, 44);
                }
            }
        }
        if (opt) {
            TestHandlePerThread();
            TestDestructorFreesHandles();
            TestDestructorOnAnotherThread();
            TestAddressReuse();
            TestCacheEviction();
            TestThreadExitsFirst();
            TestRepDestroyedFirst();
            TestReadersDoNotWaitForInsert();
        }
    }
    for (Scope scope : scopes) {
        for (const auto& t : tests) {
            if (every_test || t.first == only_test) t.second(scope);
        }
    }
    if (opt && every_test) TestWriterParkedBetweenLinkAndTrieInsert();

    std::cout << "============================================================" << std::endl;
    std::cout << "ALL PARK-POINT TESTS PASSED" << std::endl;
    std::cout << "============================================================" << std::endl;
    return 0;
}
