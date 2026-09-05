// C ABI shim over HOT (Height Optimized Trie), plus the allocator census both
// comparison arms are measured with.
//
// Two things live here and they are deliberately in the same translation unit:
//
//  1. The `extern "C"` surface over `HOTSingleThreaded`, in the two value
//     models the suite pairs against Expanse (AGENTS.md 8.3):
//       - set arm: IdentityKeyExtractor<uint64_t>, value == key, stored inline
//         in HOT's tagged child pointer.
//       - map arm: PairPointerKeyExtractor over a heap std::pair, which is the
//         only way HOT carries a value distinct from its key.
//
//  2. The allocator census. HOT allocates nodes through a pooled
//     `posix_memalign` path and never through `operator new`, so an
//     `operator new` counter measures nothing (measured at the #660 Step 0
//     gate). The counters below interpose the C allocator family at link time
//     via `-Wl,--wrap=...`, which also captures Rust's allocations because
//     Rust's allocator bottoms out in the same symbols. That is the point:
//     ONE instrument measures both arms under one definition, so the symmetry
//     required by 8.3 holds by construction rather than by two counters
//     agreeing.
//
// The counters are `std::atomic` and the file is compiled `-fno-builtin-*` for
// the allocator family. Both are load-bearing: GCC knows `malloc`/`free` as
// builtins and may assume they do not touch globals, which lets it cache a
// plain counter across the call. That is exactly the defect the Step 0 gate
// program hit, where `free` was observed to run while the byte total did not
// move.

#include <atomic>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <malloc.h>
#include <new>
#include <utility>

extern "C" {
void *__real_malloc(size_t);
void *__real_calloc(size_t, size_t);
void *__real_realloc(void *, size_t);
void __real_free(void *);
int __real_posix_memalign(void **, size_t, size_t);
void *__real_aligned_alloc(size_t, size_t);
}

namespace {
std::atomic<long long> g_live{0};
std::atomic<long long> g_peak{0};
std::atomic<long long> g_allocs{0};
std::atomic<long long> g_frees{0};
std::atomic<int> g_armed{0};

inline void note_alloc(void *p) {
    if (p == nullptr || g_armed.load(std::memory_order_relaxed) == 0) return;
    long long n = static_cast<long long>(malloc_usable_size(p));
    long long now = g_live.fetch_add(n, std::memory_order_relaxed) + n;
    g_allocs.fetch_add(1, std::memory_order_relaxed);
    long long peak = g_peak.load(std::memory_order_relaxed);
    while (now > peak && !g_peak.compare_exchange_weak(peak, now, std::memory_order_relaxed)) {}
}

inline void note_free(void *p) {
    if (p == nullptr || g_armed.load(std::memory_order_relaxed) == 0) return;
    long long n = static_cast<long long>(malloc_usable_size(p));
    g_live.fetch_sub(n, std::memory_order_relaxed);
    g_frees.fetch_add(1, std::memory_order_relaxed);
}
}  // namespace

extern "C" {

void *__wrap_malloc(size_t s) {
    void *p = __real_malloc(s);
    note_alloc(p);
    return p;
}
void *__wrap_calloc(size_t n, size_t s) {
    void *p = __real_calloc(n, s);
    note_alloc(p);
    return p;
}
void *__wrap_realloc(void *old, size_t s) {
    note_free(old);
    void *p = __real_realloc(old, s);
    note_alloc(p);
    return p;
}
void __wrap_free(void *p) {
    note_free(p);
    __real_free(p);
}
int __wrap_posix_memalign(void **p, size_t a, size_t s) {
    int r = __real_posix_memalign(p, a, s);
    if (r == 0) note_alloc(*p);
    return r;
}
void *__wrap_aligned_alloc(size_t a, size_t s) {
    void *p = __real_aligned_alloc(a, s);
    note_alloc(p);
    return p;
}

}  // extern "C"

// Replaceable global allocation functions.
//
// `-Wl,--wrap=malloc` rewrites symbol resolution only for the objects being
// linked. libstdc++'s `operator new` lives in libstdc++.so and reaches malloc
// through the dynamic linker at runtime, so its allocations are NOT wrapped —
// measured: Arm B's 100,000 heap `std::pair`s produced 3 extra counted
// allocations instead of 100,000, hiding roughly 32 B/key of the map arm's real
// footprint and flattering HOT exactly where its value model is most expensive.
//
// Defining these here replaces the library's versions program-wide (C++
// [basic.stc.dynamic.allocation]), routing every `new`/`delete` through the
// counted path. This is the mirror image of the Step 0 finding: there, an
// `operator new` counter missed HOT's `posix_memalign` nodes; here, a malloc
// counter missed the C++ `operator new` pairs. The census needs both.
void *operator new(size_t s) {
    void *p = __wrap_malloc(s);
    if (p == nullptr) throw std::bad_alloc();
    return p;
}
void *operator new[](size_t s) { return operator new(s); }
void operator delete(void *p) noexcept { __wrap_free(p); }
void operator delete[](void *p) noexcept { __wrap_free(p); }
void operator delete(void *p, size_t) noexcept { __wrap_free(p); }
void operator delete[](void *p, size_t) noexcept { __wrap_free(p); }

extern "C" {

// --- census control -------------------------------------------------------

void exp_census_reset(void) {
    g_live.store(0, std::memory_order_relaxed);
    g_peak.store(0, std::memory_order_relaxed);
    g_allocs.store(0, std::memory_order_relaxed);
    g_frees.store(0, std::memory_order_relaxed);
}
void exp_census_arm(int on) { g_armed.store(on, std::memory_order_relaxed); }
long long exp_census_live(void) { return g_live.load(std::memory_order_relaxed); }
long long exp_census_peak(void) { return g_peak.load(std::memory_order_relaxed); }
long long exp_census_allocs(void) { return g_allocs.load(std::memory_order_relaxed); }
long long exp_census_frees(void) { return g_frees.load(std::memory_order_relaxed); }

}  // extern "C"

#include <hot/singlethreaded/HOTSingleThreaded.hpp>
#include <idx/contenthelpers/IdentityKeyExtractor.hpp>
#include <idx/contenthelpers/OptionalValue.hpp>
#include <idx/contenthelpers/PairPointerKeyExtractor.hpp>

using SetTrie = hot::singlethreaded::HOTSingleThreaded<uint64_t, idx::contenthelpers::IdentityKeyExtractor>;
using MapPair = std::pair<uint64_t, uint64_t>;
using MapTrie = hot::singlethreaded::HOTSingleThreaded<MapPair *, idx::contenthelpers::PairPointerKeyExtractor>;

// HOT tags leaves in bit 0 and recovers the payload with an arithmetic shift,
// so its INLINE VALUE is 63 bits wide. That is a payload width, not a key
// width, and it binds only where the stored value is the key itself -- the set
// arm's IdentityKeyExtractor. The map arm stores a heap pointer and takes the
// full 64-bit domain: measured 6/6 found with correct values on keys spanning
// bit 63, including ~0ull, against 3/6 for the set arm.
//
// So this guard belongs on the set arm alone. It returns -2 so the caller can
// report "this arm cannot represent this key" as an outcome; it must never be
// used to quietly narrow the workload, and it is deliberately absent from the
// map arm, which an earlier revision wrongly guarded.
#define EXP_INLINE_PAYLOAD_GUARD(k) \
    do { if (((k) >> 63) != 0u) return -2; } while (0)

extern "C" {

// HOT's node pool is a function-local `static` inside
// `HOTSingleThreadedNodeBase::getMemoryPool()`, so it is process-global and
// outlives every trie instance. A trie that is built and dropped leaves its
// nodes on the pool's free lists, and the next trie reuses them WITHOUT calling
// `posix_memalign` — so an allocator census taken after any earlier HOT trie
// undercounts. Measured: 3.61 B/key with a warm pool against 11.76 B/key cold,
// same workload, a 3.3x understatement. The Expanse arm has no such pool and is
// unaffected (13.39 vs 13.38 B/key), so the error is asymmetric and flatters
// HOT.
//
// Exposing the pool's own counter lets the census refuse to run on a warm pool
// instead of trusting the caller to remember (AGENTS.md 8.1).
size_t exp_hot_pool_allocations(void) {
    return hot::singlethreaded::HOTSingleThreadedNodeBase::getNumberAllocations();
}

// --- set arm --------------------------------------------------------------

void *exp_hot_set_new(void) { return new SetTrie(); }
void exp_hot_set_delete(void *t) { delete static_cast<SetTrie *>(t); }

int exp_hot_set_insert(void *t, uint64_t k) {
    EXP_INLINE_PAYLOAD_GUARD(k);
    return static_cast<SetTrie *>(t)->insert(k) ? 1 : 0;
}

int exp_hot_set_contains(void *t, uint64_t k) {
    EXP_INLINE_PAYLOAD_GUARD(k);
    return static_cast<SetTrie *>(t)->lookup(k).mIsValid ? 1 : 0;
}

// Population is counted by walking, never inferred from insert() return values:
// the Step 0 gate proved insert() returns true for keys the trie cannot find.
size_t exp_hot_set_len(void *t) {
    size_t n = 0;
    SetTrie *trie = static_cast<SetTrie *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) ++n;
    return n;
}

uint64_t exp_hot_set_iterate_xor(void *t) {
    uint64_t sink = 0;
    SetTrie *trie = static_cast<SetTrie *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) sink ^= *it;
    return sink;
}

size_t exp_hot_set_scan(void *t, uint64_t lo, size_t k, uint64_t *sink) {
    size_t n = 0;
    uint64_t acc = 0;
    SetTrie *trie = static_cast<SetTrie *>(t);
    for (auto it = trie->lower_bound(lo); it != trie->end() && n < k; ++it, ++n) acc ^= *it;
    *sink = acc;
    return n;
}

// --- map arm --------------------------------------------------------------

void *exp_hot_map_new(void) { return new MapTrie(); }

void exp_hot_map_delete(void *t) {
    MapTrie *trie = static_cast<MapTrie *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) delete *it;
    delete trie;
}

int exp_hot_map_insert(void *t, uint64_t k, uint64_t v) {
    MapPair *entry = new MapPair(k, v);
    if (static_cast<MapTrie *>(t)->insert(entry)) return 1;
    delete entry;
    return 0;
}

int exp_hot_map_get(void *t, uint64_t k, uint64_t *out) {
    auto r = static_cast<MapTrie *>(t)->lookup(k);
    if (!r.mIsValid) return 0;
    *out = r.mValue->second;
    return 1;
}

size_t exp_hot_map_len(void *t) {
    size_t n = 0;
    MapTrie *trie = static_cast<MapTrie *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) ++n;
    return n;
}

uint64_t exp_hot_map_iterate_xor(void *t) {
    uint64_t sink = 0;
    MapTrie *trie = static_cast<MapTrie *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) sink ^= (*it)->second;
    return sink;
}

size_t exp_hot_map_scan(void *t, uint64_t lo, size_t k, uint64_t *sink) {
    size_t n = 0;
    uint64_t acc = 0;
    MapTrie *trie = static_cast<MapTrie *>(t);
    for (auto it = trie->lower_bound(lo); it != trie->end() && n < k; ++it, ++n) acc ^= (*it)->second;
    *sink = acc;
    return n;
}

}  // extern "C"
