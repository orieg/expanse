// C ABI shim over Masstree (kohler/masstree-beta) — the Masstree comparison arm
// (#661, docs/benchmarks/masstree_comparison/METHODOLOGY.md).
//
// Compiled only under the `masstree` cargo feature, from the pinned submodule
// at third_party/masstree plus the hand-derived config.h in ./masstree_config
// (the library generates it with autoconf; the arm cannot run ./configure).
//
// What this file settles, each recorded in METHODOLOGY §3:
//
//  - Threading (§3.2). Every Masstree operation takes a `threadinfo&`. The
//    shim owns a fixed array of thread slots, created lazily under a mutex
//    (`threadinfo::make` pushes onto an unsynchronised global list), never
//    freed (the library has no destructor for them). A Rust thread takes one
//    slot for its lifetime; `enter`/`exit` map to `rcu_start`/`rcu_stop`, and
//    `quiesce` reproduces mttest's epoch advance exactly: the global epoch is
//    the wall clock >> 16, set under a lock when it moves, then the thread
//    quiesces its own limbo list.
//  - Key length (§3.4). MASSTREE_MAXKEYLEN = 255 is the library's contract —
//    insert and lookup of a longer key succeed mechanically, but scan writes
//    into MASSTREE_MAXKEYLEN-sized stack buffers with preconditions compiled
//    out. Every string entry point refuses a longer key with -2 and stores
//    nothing; the harness evaluates the predicate over the workload first and
//    withholds Masstree's column rather than narrowing the population.
//  - Census (§3.3). The allocator interposition lives in hot_shim.cpp and
//    reaches this translation unit through `-Wl,--wrap`: node slabs come from
//    `posix_memalign` in kvthread.cc, suffix bags and limbo groups from
//    `malloc`, the Json objects `json_stats` builds from the replaced
//    `operator new`. `exp_mt_stats` exposes Masstree's own node census so the
//    harness can publish structural bytes beside the slab-quantized figure.
//
// Integer keys are passed as 8-byte big-endian strings so that Masstree's
// byte-lexicographic order is numeric order and its 8-byte ikey slice is the
// whole key: no suffix, no layer.

#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>

#include "compiler.hh"
#include "masstree.hh"
#include "kvthread.hh"
#include "masstree_tcursor.hh"
#include "masstree_insert.hh"
#include "masstree_remove.hh"
#include "masstree_scan.hh"
#include "masstree_stats.hh"
#include "string.hh"

// Globals the library declares `extern` and leaves to the program, as
// mttest.cc defines them.
relaxed_atomic<mrcu_epoch_type> globalepoch(1);
relaxed_atomic<mrcu_epoch_type> active_epoch(1);
kvtimestamp_t initial_timestamp;
volatile bool recovering = false;

namespace {

// Two configurations of one template, the split HOT ships as
// HOTSingleThreaded / HOTRowex (§10.3): `concurrent = false` swaps Masstree's
// fenced, spin-locked `nodeversion` for `singlethreaded_nodeversion` and is
// the twin of `ExpanseMap` / `ExpanseStrMap`, which carry no protocol;
// `concurrent = true` is the twin of `SyncExpanseMap` / `SyncExpanseStrMap`.
template <bool CONCURRENT>
struct table_params : public Masstree::nodeparams<15, 15> {
    typedef uint64_t value_type;
    typedef Masstree::value_print<value_type> value_print_type;
    typedef ::threadinfo threadinfo_type;
    static constexpr bool concurrent = CONCURRENT;
};
typedef Masstree::Str Str;

// --- thread slots (§3.2) --------------------------------------------------

constexpr unsigned kSlots = 64;
threadinfo *g_slots[kSlots];
std::mutex g_slots_lock;
std::atomic<int> g_epoch_initialised{0};

std::mutex g_epoch_lock;
void set_global_epoch(mrcu_epoch_type e) {
    std::lock_guard<std::mutex> g(g_epoch_lock);
    if (mrcu_signed_epoch_type(e - globalepoch.load()) > 0) {
        globalepoch.store(e);
        active_epoch.store(threadinfo::min_active_epoch());
    }
}

inline size_t pool_round(size_t sz) { return ((sz + CACHE_LINE_SIZE - 1) / CACHE_LINE_SIZE) * CACHE_LINE_SIZE; }

inline threadinfo *ti_of(void *p) { return static_cast<threadinfo *>(p); }

inline Str key8(uint64_t k, uint64_t &be) {
    be = __builtin_bswap64(k);
    return Str(reinterpret_cast<const char *>(&be), 8);
}

// Visitor for basic_table::scan: folds values into a sink, stops after `limit`.
struct FoldScanner {
    size_t n = 0;
    size_t limit = ~size_t(0);
    uint64_t sink = 0;
    template <typename SS, typename K> void visit_leaf(const SS &, const K &, threadinfo &) {}
    template <typename K> bool visit_value(const K &, uint64_t v, threadinfo &) {
        sink ^= v;
        return ++n < limit;
    }
};

template <bool CONCURRENT>
struct api {
    typedef table_params<CONCURRENT> P;
    typedef Masstree::basic_table<P> table_type;
    typedef Masstree::tcursor<P> cursor_type;
    typedef Masstree::unlocked_tcursor<P> unlocked_cursor_type;
    typedef Masstree::leaf<P> leaf_type;
    typedef Masstree::internode<P> internode_type;

    static table_type *tab(void *p) { return static_cast<table_type *>(p); }

    static void *make(void *ti) {
        table_type *t = new table_type();
        t->initialize(*ti_of(ti));
        return t;
    }
    static void destroy(void *t, void *ti) {
        tab(t)->destroy(*ti_of(ti));
        delete tab(t);
    }
    static int insert(void *t, threadinfo &ti, Str key, uint64_t v) {
        cursor_type lp(*tab(t), key);
        bool found = lp.find_insert(ti);
        lp.value() = v;  // insert, or replace (ExpanseMap::insert semantics)
        fence();
        lp.finish(1, ti);
        return found ? 0 : 1;
    }
    static int get(void *t, threadinfo &ti, Str key, uint64_t *out) {
        unlocked_cursor_type lp(*tab(t), key);
        if (!lp.find_unlocked(ti)) return 0;
        *out = lp.value();
        return 1;
    }
    static size_t scan(void *t, threadinfo &ti, Str from, size_t limit, uint64_t *sink) {
        FoldScanner sc;
        sc.limit = limit;
        tab(t)->scan(from, true, sc, ti);
        *sink = sc.sink;
        return sc.n;
    }
    static size_t leaf_bytes() { return pool_round(sizeof(leaf_type)); }
    static size_t internode_bytes() { return pool_round(sizeof(internode_type)); }
    static lcdf::Json stats(void *t, threadinfo &ti) { return Masstree::json_stats(*tab(t), ti); }
};

inline size_t json_sum(const lcdf::Json &a) {
    size_t s = 0;
    if (a.is_array())
        for (auto it = a.array_data(); it != a.end_array_data(); ++it) s += static_cast<size_t>(it->to_u64());
    return s;
}

}  // namespace

extern "C" {

// --- constants the harness reads from the library rather than restating ------

size_t exp_mt_max_key_len(void) { return MASSTREE_MAXKEYLEN; }
// kvthread.cc refill_pool: superpage size when available, else 2 MiB; on the
// reference host both are 2 MiB. Reported so the census quantum is data.
size_t exp_mt_slab_bytes(void) { return size_t(2) << 20; }
size_t exp_mt_leaf_bytes(void) { return api<true>::leaf_bytes(); }
size_t exp_mt_internode_bytes(void) { return api<true>::internode_bytes(); }

// --- thread slots -------------------------------------------------------------

void *exp_mt_thread(uint32_t slot) {
    if (slot >= kSlots) {
        fprintf(stderr, "masstree shim: slot %u out of range (%u slots)\n", slot, kSlots);
        abort();
    }
    std::lock_guard<std::mutex> g(g_slots_lock);
    if (g_slots[slot] == nullptr) {
        if (g_epoch_initialised.exchange(1) == 0) {
            initial_timestamp = timestamp();
            mrcu_epoch_type e = timestamp() >> 16;
            globalepoch.store(e);
            active_epoch.store(e);
        }
        g_slots[slot] = threadinfo::make(threadinfo::TI_PROCESS, static_cast<int>(slot));
    }
    return g_slots[slot];
}

void exp_mt_thread_enter(void *ti) { ti_of(ti)->rcu_start(); }
void exp_mt_thread_exit(void *ti) { ti_of(ti)->rcu_stop(); }

// mttest.cc's rcu_quiesce, verbatim in behaviour.
void exp_mt_quiesce(void *ti) {
    mrcu_epoch_type e = timestamp() >> 16;
    if (e != globalepoch.load()) set_global_epoch(e);
    ti_of(ti)->rcu_quiesce();
}

// One reclamation step for a census (§10.4): advance the global epoch by one
// and quiesce this slot, so what the build deferred through RCU is freed
// rather than counted as held. `hard_rcu_quiesce` frees at most 128 entries
// per call; the harness repeats until the census sees no further frees.
void exp_mt_settle_step(void *ti) {
    set_global_epoch(globalepoch.load() + 1);
    ti_of(ti)->rcu_quiesce();
}

// --- Masstree's own node census (§3.3, engine-instrument column) ---------------

struct exp_mt_stats_t {
    uint64_t size;
    uint64_t leaves;
    uint64_t internodes;
    uint64_t layers;
    uint64_t ksuf_capacity;
    uint64_t overridden_ksuf_capacity;
    uint64_t leaf_bytes;
    uint64_t internode_bytes;
    uint64_t structural_bytes;
};

}  // extern "C"

namespace {
template <bool C>
void fill_stats(void *t, void *ti, struct exp_mt_stats_t *out) {
    lcdf::Json j = api<C>::stats(t, *ti_of(ti));
    out->size = j["size"].to_u64();
    out->leaves = json_sum(j["leaf_by_depth"]) + json_sum(j["l1_leaf_by_depth"]);
    out->internodes = json_sum(j["internode_by_size"]) + json_sum(j["l1_internode_by_size"]);
    out->layers = j["l1_count"].to_u64();
    out->ksuf_capacity = j["ksuf_capacity"].to_u64();
    out->overridden_ksuf_capacity = j["overridden_ksuf_capacity"].to_u64();
    out->leaf_bytes = api<C>::leaf_bytes();
    out->internode_bytes = api<C>::internode_bytes();
    // The same arithmetic as scripts/masstree_envelope.py::structural_bytes.
    out->structural_bytes = out->leaves * out->leaf_bytes + out->internodes * out->internode_bytes
                          + out->ksuf_capacity + out->overridden_ksuf_capacity;
}
}  // namespace

// The C surface, once per configuration: `exp_mt_*` is `concurrent = true`
// (MC1/MC2), `exp_mts_*` is `concurrent = false` (M1/M2). Integer keys are
// 8-byte big-endian strings; string entry points refuse a key beyond
// MASSTREE_MAXKEYLEN with -2 (SIZE_MAX for a scan start) and store nothing.
#define EXP_MT_DEFINE_API(PFX, C)                                                                  \
    extern "C" {                                                                                   \
    void *PFX##_new(void *ti) { return api<C>::make(ti); }                                         \
    void PFX##_delete(void *t, void *ti) { api<C>::destroy(t, ti); }                               \
    int PFX##_insert(void *t, void *ti, uint64_t k, uint64_t v) {                                  \
        uint64_t be;                                                                               \
        return api<C>::insert(t, *ti_of(ti), key8(k, be), v);                                      \
    }                                                                                              \
    int PFX##_get(void *t, void *ti, uint64_t k, uint64_t *out) {                                  \
        uint64_t be;                                                                               \
        return api<C>::get(t, *ti_of(ti), key8(k, be), out);                                       \
    }                                                                                              \
    size_t PFX##_len(void *t, void *ti) {                                                          \
        uint64_t sink;                                                                             \
        return api<C>::scan(t, *ti_of(ti), Str("", 0), ~size_t(0), &sink);                         \
    }                                                                                              \
    uint64_t PFX##_iterate_xor(void *t, void *ti) {                                                \
        uint64_t sink;                                                                             \
        api<C>::scan(t, *ti_of(ti), Str("", 0), ~size_t(0), &sink);                                \
        return sink;                                                                               \
    }                                                                                              \
    size_t PFX##_scan(void *t, void *ti, uint64_t lo, size_t k, uint64_t *sink) {                  \
        uint64_t be;                                                                               \
        return api<C>::scan(t, *ti_of(ti), key8(lo, be), k, sink);                                 \
    }                                                                                              \
    int PFX##_str_insert(void *t, void *ti, const uint8_t *k, size_t len, uint64_t v) {            \
        if (len > MASSTREE_MAXKEYLEN) return -2;                                                   \
        return api<C>::insert(t, *ti_of(ti),                                                       \
                              Str(reinterpret_cast<const char *>(k), static_cast<int>(len)), v);   \
    }                                                                                              \
    int PFX##_str_get(void *t, void *ti, const uint8_t *k, size_t len, uint64_t *out) {            \
        if (len > MASSTREE_MAXKEYLEN) return -2;                                                   \
        return api<C>::get(t, *ti_of(ti),                                                          \
                           Str(reinterpret_cast<const char *>(k), static_cast<int>(len)), out);    \
    }                                                                                              \
    size_t PFX##_str_scan(void *t, void *ti, const uint8_t *lo, size_t lo_len, size_t k,           \
                          uint64_t *sink) {                                                        \
        if (lo_len > MASSTREE_MAXKEYLEN) return ~size_t(0);                                        \
        return api<C>::scan(t, *ti_of(ti),                                                         \
                            Str(reinterpret_cast<const char *>(lo), static_cast<int>(lo_len)), k,  \
                            sink);                                                                 \
    }                                                                                              \
    void PFX##_stats(void *t, void *ti, struct exp_mt_stats_t *out) { fill_stats<C>(t, ti, out); } \
    }

EXP_MT_DEFINE_API(exp_mt, true)
EXP_MT_DEFINE_API(exp_mts, false)
