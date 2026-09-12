// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// bench_memtable_concurrent.cc — concurrent read scaling for ExpanseMemTableRep (#802).
//
// # Workload shape
//
// | Property | Value |
// |---|---|
// | `workload_id` | `rocksdb_memtable_concurrent_read_scaling` |
// | `group` | 8 |
// | `population` | 100,000 keys, 16-byte user key, 64-byte value — the same fixture `bench_memtable.cc` builds, so both are read on one workload |
// | `insertion_order` | generator — keys are inserted in the draw order of the suite PRNG (mt19937_64, seed 1337), exactly as `bench_memtable.cc` does; no sort and no shuffle is applied |
// | `probes_and_reuse` | R reader threads cycle a per-thread shuffled stream of the pre-populated keys for the cell's whole window; each reader has its own stream and offset so readers do not share a cursor. Probes are drawn ONLY from the pre-populated set, never from keys the writer adds during the cell, so the hit rate does not drift with the writer's progress |
// | `hit_rate` | 50% by construction against the pre-populated set: half the probes are present keys, half are same-generator misses |
// | `miss_gen_method` | same-generator rejection sampling (§8.6) — miss keys are drawn from the same `mt19937_64` stream that built the population and rejected on membership, never a transform of a present key |
// | `value_dereference` | every `Get` callback reads the entry pointer and accumulates its first payload byte into a per-thread sink consumed after the join, so no probe is dead code |
// | `measured_region` | barrier release to the cell's deadline, readers only. Population build, thread spawn, probe-stream generation, the writer's pacing sleeps and all teardown are outside it |
// | `arm_symmetry` | single-arm: this is an Expanse self-scaling curve, not a comparison. `ReferenceSkipListRep` and `ReferenceVectorRep` in `bench_memtable.cc` are not thread-safe (raw `Node* next[]` with a non-atomic `count_`; `EnsureSorted` sorts through a `const_cast` in a `const` method with no lock), so a concurrent cell against either would be a data race rather than a baseline — see METHODOLOGY §5.5 |
// | `statistics` | ONE cell per process invocation, emitted as a single raw CSV row; the driver orders invocations so `(writer-mode × R)` interleaves within each round (§8.20.2) and computes the paired BCa 95% interval on `S(R) = T(R)/T(1)` across rounds (§8.4). This binary emits no interval and no ratio |
// | `verdict` | pending measurement |
//
// ## What this measures, and what it cannot
//
// `FindLeafBlockForSeek` takes the same `mutex_` that `Insert` holds for its
// whole body, so every read serialises on the writer's lock before it reaches
// the per-leaf seqlock. This sweeps reader count against a writer in three
// modes and reports aggregate read throughput; METHODOLOGY §5 is the
// pre-registration it is read against.
//
// The writer is PACED below saturation in the primary mode, and that is
// load-bearing rather than a convenience: one writer free-running at the
// measured insert rate holds the lock essentially all of the time, readers
// starve whatever the locate phase costs, and `S(R)` collapses to ~1 having
// measured nothing about the read path. The free-running mode is emitted for
// context and is never gated (METHODOLOGY §5.3).
//
// Pacing blocks, never spins: a spin-wait pacer would contend for the very
// lock under measurement. The achieved rate is counted and emitted per cell so
// the driver computes the writer's duty cycle from it rather than assuming the
// offered rate was met.
//
// ## One cell per process, and why
//
// Each invocation builds the population, runs exactly one (mode, R) cell, and
// exits. An earlier shape ran every cell of a round inside one process, and the
// writer's inserts accumulated across them: the cells that ran later read a
// bigger tree than the cells that ran earlier, so `S(R)` carried a monotone
// downward bias that had nothing to do with reader count. A fresh process per
// cell also gives the driver a real boundary to take its per-cell load snapshot
// across, which is what `load.foreign_busy_cpus` needs
// (scripts/check_bench_provenance.py). The cost is rebuilding the fixture per
// cell, which is outside the measured region either way.

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <memory>
#include <random>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

#include "expanse_memtable.h"

using namespace rocksdb;
using Clock = std::chrono::steady_clock;

namespace {

constexpr int kPopulation = 100000;
constexpr int kValueSize = 64;
constexpr uint64_t kSeed = 1337;

class BenchCmp : public MemTableRep::KeyComparator {
public:
    int operator()(const char* a, const char* b) const override {
        return expanse_rocksdb::CompareInternalKeys(
            expanse_rocksdb::GetLengthPrefixedSlice(a),
            expanse_rocksdb::GetLengthPrefixedSlice(b));
    }
};

const char* EncodeEntry(Arena& arena, const std::string& user_key, SequenceNumber seq,
                        const std::string& value) {
    size_t ikey_len = user_key.size() + 8;
    size_t val_len = value.size();
    char* buf = arena.Allocate(5 + ikey_len + 5 + val_len);
    char* p = expanse_rocksdb::EncodeVarint32(buf, static_cast<uint32_t>(ikey_len));
    memcpy(p, user_key.data(), user_key.size());
    p += user_key.size();
    uint64_t trailer = (seq << 8) | static_cast<uint64_t>(kTypeValue);
    for (int i = 0; i < 8; ++i) p[i] = static_cast<char>((trailer >> (i * 8)) & 0xff);
    p += 8;
    p = expanse_rocksdb::EncodeVarint32(p, static_cast<uint32_t>(val_len));
    if (val_len > 0) memcpy(p, value.data(), val_len);
    return buf;
}

std::string KeyAt(uint64_t raw) {
    std::ostringstream ss;
    ss << "usr_" << std::setw(12) << std::setfill('0') << (raw % 10000000000ULL);
    return ss.str();
}

// Writer modes. `idle` is the control that separates reader-vs-reader
// serialisation from reader-vs-writer blocking; without it a flat curve cannot
// be attributed to either (METHODOLOGY §5.2).
enum class WriterMode { kIdle, kPaced, kFree };

const char* ModeName(WriterMode m) {
    switch (m) {
        case WriterMode::kIdle: return "idle";
        case WriterMode::kPaced: return "paced";
        case WriterMode::kFree: return "free";
    }
    return "?";
}

struct CellResult {
    int readers = 0;
    WriterMode mode = WriterMode::kIdle;
    uint64_t read_ops = 0;
    uint64_t write_ops = 0;
    double elapsed_s = 0.0;
    uint64_t sink = 0;
};

}  // namespace

int main(int argc, char** argv) {
    int readers = 1;
    int round = 0;
    double window_s = 2.0;
    double paced_rate = 250000.0;
    std::string mode_arg;
    bool header = false;

    for (int i = 1; i < argc; ++i) {
        std::string a = argv[i];
        auto next = [&]() -> std::string { return (i + 1 < argc) ? argv[++i] : std::string(); };
        if (a == "--readers") readers = std::stoi(next());
        else if (a == "--round") round = std::stoi(next());
        else if (a == "--window-seconds") window_s = std::stod(next());
        else if (a == "--paced-rate") paced_rate = std::stod(next());
        else if (a == "--mode") mode_arg = next();
        else if (a == "--header") header = true;
        else if (a == "--quick") window_s = 0.25;
        else {
            std::cerr << "unknown argument: " << a << "\n"
                      << "usage: bench_memtable_concurrent --mode <idle|paced|free> --readers R\n"
                      << "       [--round N] [--window-seconds S] [--paced-rate OPS]\n"
                      << "       [--header] [--quick]\n"
                      << "One cell per invocation; the driver owns the rounds and the order.\n";
            return 2;  // fail loud on an argument we do not understand (AGENTS.md 8.1)
        }
    }

    WriterMode mode;
    if (mode_arg == "idle") mode = WriterMode::kIdle;
    else if (mode_arg == "paced") mode = WriterMode::kPaced;
    else if (mode_arg == "free") mode = WriterMode::kFree;
    else {
        std::cerr << "--mode is required and must be idle, paced or free (got '"
                  << mode_arg << "')\n";
        return 2;
    }
    if (readers < 1) {
        std::cerr << "--readers must be >= 1, got " << readers << "\n";
        return 2;
    }
    if (!(window_s > 0.0)) {
        std::cerr << "--window-seconds must be > 0, got " << window_s << "\n";
        return 2;
    }

    // ---- population: identical construction to bench_memtable.cc ----------
    BenchCmp cmp;
    Arena arena(64 * 1024 * 1024);
    std::mt19937_64 rng(kSeed);
    std::string val(kValueSize, 'x');

    std::vector<std::string> present;
    present.reserve(kPopulation);
    for (int i = 0; i < kPopulation; ++i) present.push_back(KeyAt(rng()));

    // Miss keys: same generator, rejected on membership (AGENTS.md 8.6). Never a
    // transform of a present key -- such a probe lands in a different top-level
    // expanse and terminates at a systematically different depth than a hit.
    std::vector<std::string> sorted_present = present;
    std::sort(sorted_present.begin(), sorted_present.end());
    std::vector<std::string> misses;
    misses.reserve(kPopulation);
    while (static_cast<int>(misses.size()) < kPopulation) {
        std::string cand = KeyAt(rng());
        if (!std::binary_search(sorted_present.begin(), sorted_present.end(), cand)) {
            misses.push_back(std::move(cand));
        }
    }

    ExpanseMemTableRep rep(cmp, &arena, nullptr, nullptr, 64);
    for (int i = 0; i < kPopulation; ++i) {
        rep.Insert(const_cast<char*>(EncodeEntry(arena, present[i], 1000 + i, val)));
    }

    // Fresh keys for the writer: same generator, rejected on membership, so a
    // reader probing the pre-populated set never sees its hit rate drift.
    // Size the writer's key supply from the cell's own parameters, not a fixed
    // cap. A paced writer needs rate x window keys and must never run dry --
    // running dry makes its achieved rate understate what it was offering, and
    // the driver computes the duty cycle from the achieved rate. A 400,000 cap
    // against 250,000/s over 2 s was short by 100,000 and failed the run
    // (34718436485).
    //
    // A free-running writer cannot be covered this way: at the insert rate this
    // host measures it would want millions of entries, hundreds of MB of arena,
    // for a two-second window. So the free cell is allowed to exhaust its supply
    // and stop early; it is reported and never gated (METHODOLOGY section 5.2),
    // and the row carries writer_exhausted so that is visible rather than
    // inferred from a low rate.
    std::vector<const char*> fresh;
    size_t fresh_target = 0;
    if (mode == WriterMode::kPaced) {
        // 25% head-room: sleep_until can overshoot slightly, and a writer that
        // runs marginally ahead of schedule must not fall off the end.
        fresh_target = static_cast<size_t>(paced_rate * window_s * 1.25) + 1024;
    } else if (mode == WriterMode::kFree) {
        fresh_target = 2000000;  // ~190 MB of arena; exhaustion here is expected
    }
    if (fresh_target > 0) {
        fresh.reserve(fresh_target);
        uint64_t seq = 2000000;
        while (fresh.size() < fresh_target) {
            std::string cand = KeyAt(rng());
            if (std::binary_search(sorted_present.begin(), sorted_present.end(), cand)) continue;
            fresh.push_back(EncodeEntry(arena, cand, seq++, val));
        }
    }

    // Per-reader probe streams: 50% hits, 50% same-generator misses, shuffled
    // per thread so readers do not share a cursor.
    std::vector<std::vector<LookupKey>> probes(readers);
    for (int t = 0; t < readers; ++t) {
        std::mt19937_64 trng(kSeed + 101 + static_cast<uint64_t>(t));
        std::vector<std::string> stream;
        stream.reserve(20000);
        for (int i = 0; i < 10000; ++i) {
            stream.push_back(present[trng() % present.size()]);
            stream.push_back(misses[trng() % misses.size()]);
        }
        std::shuffle(stream.begin(), stream.end(), trng);
        probes[t].reserve(stream.size());
        for (const auto& k : stream) probes[t].emplace_back(Slice(k), 10000000);
    }

    if (header) {
        std::cout << "# rocksdb_memtable_concurrent_read_scaling\n";
        std::cout << "# population=" << kPopulation << " value_bytes=" << kValueSize
                  << " window_s=" << window_s << " paced_rate=" << paced_rate << "\n";
        std::cout << "round,writer_mode,readers,read_ops,write_ops,elapsed_s,read_mops,writer_exhausted\n";
    }

    std::atomic<bool> go{false};
    std::atomic<bool> stop{false};
    std::atomic<int> ready{0};
    std::atomic<size_t> fresh_cursor{0};
    std::atomic<uint64_t> writes{0};
    std::vector<uint64_t> reader_ops(readers, 0);
    std::vector<uint64_t> reader_sink(readers, 0);

    std::vector<std::thread> reader_threads;
    reader_threads.reserve(readers);
    for (int t = 0; t < readers; ++t) {
        reader_threads.emplace_back([&, t]() {
            const auto& stream = probes[t];
            uint64_t ops = 0, sink = 0;
            ready.fetch_add(1, std::memory_order_release);
            while (!go.load(std::memory_order_acquire)) std::this_thread::yield();
            size_t idx = 0;
            while (!stop.load(std::memory_order_relaxed)) {
                // Check the stop flag per batch, not per probe: a relaxed load
                // on every Get would be a measurable share of a ~260 ns read.
                for (int b = 0; b < 64; ++b) {
                    rep.Get(stream[idx], &sink,
                            [](void* arg, const char* entry) -> bool {
                                // Consume the entry so the probe is not dead
                                // code (AGENTS.md 8.6).
                                *static_cast<uint64_t*>(arg) +=
                                    static_cast<uint8_t>(entry[0]);
                                return false;
                            });
                    ops++;
                    if (++idx == stream.size()) idx = 0;
                }
            }
            reader_ops[t] = ops;
            reader_sink[t] = sink;
        });
    }

    std::thread writer_thread;
    if (mode != WriterMode::kIdle) {
        writer_thread = std::thread([&]() {
            ready.fetch_add(1, std::memory_order_release);
            while (!go.load(std::memory_order_acquire)) std::this_thread::yield();
            const auto start = Clock::now();
            uint64_t n = 0;
            const double interval_s =
                (mode == WriterMode::kPaced && paced_rate > 0.0) ? 1.0 / paced_rate : 0.0;
            while (!stop.load(std::memory_order_relaxed)) {
                size_t c = fresh_cursor.fetch_add(1, std::memory_order_relaxed);
                if (c >= fresh.size()) break;
                rep.InsertConcurrently(const_cast<char*>(fresh[c]));
                n++;
                if (interval_s > 0.0) {
                    // Pace by SLEEPING to an absolute schedule. A spin-wait
                    // here would contend for the very lock under measurement
                    // (METHODOLOGY 5.5).
                    const auto due =
                        start + std::chrono::duration_cast<Clock::duration>(
                                    std::chrono::duration<double>(
                                        static_cast<double>(n) * interval_s));
                    std::this_thread::sleep_until(due);
                }
            }
            writes.store(n, std::memory_order_relaxed);
        });
    }

    const int expect_ready = readers + (mode == WriterMode::kIdle ? 0 : 1);
    while (ready.load(std::memory_order_acquire) < expect_ready) std::this_thread::yield();

    // ---- measured region opens here --------------------------------------
    const auto t0 = Clock::now();
    go.store(true, std::memory_order_release);
    std::this_thread::sleep_for(std::chrono::duration<double>(window_s));
    stop.store(true, std::memory_order_relaxed);
    for (auto& th : reader_threads) th.join();
    if (writer_thread.joinable()) writer_thread.join();
    const auto t1 = Clock::now();
    // ---- measured region closes here -------------------------------------

    const double elapsed_s = std::chrono::duration<double>(t1 - t0).count();
    uint64_t read_ops = 0, sink = 0;
    for (int t = 0; t < readers; ++t) {
        read_ops += reader_ops[t];
        sink += reader_sink[t];
    }
    const uint64_t write_ops = writes.load(std::memory_order_relaxed);
    const double mops =
        (elapsed_s > 0.0) ? (static_cast<double>(read_ops) / elapsed_s) / 1e6 : 0.0;

    const bool exhausted = (mode != WriterMode::kIdle)
                           && fresh_cursor.load(std::memory_order_relaxed) >= fresh.size();
    std::cout << round << "," << ModeName(mode) << "," << readers << "," << read_ops << ","
              << write_ops << "," << std::fixed << std::setprecision(6) << elapsed_s << ","
              << std::setprecision(4) << mops << "," << (exhausted ? 1 : 0) << "\n";

    // A PACED writer that ran dry stopped inserting before the window closed, so
    // its achieved rate understates what it was offering and the duty cycle the
    // driver computes from it is wrong. That is a sizing bug in this harness, not
    // a property of the system, so it is fatal (AGENTS.md 8.1).
    //
    // A FREE writer running dry is expected -- see the supply sizing above -- and
    // is reported through writer_exhausted rather than failing the cell.
    if (mode == WriterMode::kPaced && exhausted) {
        std::cerr << "paced writer exhausted its key supply after " << write_ops
                  << " inserts of " << fresh.size() << " (offered " << paced_rate
                  << "/s over " << window_s << "s). The supply is sized from rate x window,"
                  << " so this means the writer ran ahead of schedule: widen the head-room"
                  << " in fresh_target.\n";
        return 1;
    }
    if (exhausted) {
        std::cerr << "note: free writer exhausted its " << fresh.size()
                  << "-key supply after " << write_ops
                  << " inserts and stopped before the window closed; reported via"
                  << " writer_exhausted (this cell is never gated)\n";
    }
    // Consume the sink so no reader loop is dead code (AGENTS.md 8.6).
    if (sink == 0xFFFFFFFFFFFFFFFFULL) {
        std::cerr << "sink sentinel\n";
        return 1;
    }
    return 0;
}
