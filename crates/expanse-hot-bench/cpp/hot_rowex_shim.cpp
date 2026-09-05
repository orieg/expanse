// C ABI shim over HOT's ROWEX (Read-Optimized Write EXclusion) variant — the
// concurrent arm of the suite (#692, METHODOLOGY.md §11).
//
// Compiled only under the `rowex` cargo feature, because it drags in TBB:
// ROWEX keeps each thread's epoch-based-reclamation state in a
// `tbb::enumerable_thread_specific`, and nothing else in HOT needs the
// library. `build.rs` builds libtbb from HOT's own pinned nested submodule
// (TBB 2018) into the cargo build directory; no system package is involved.
//
// The allocator census lives in `hot_shim.cpp` and reaches this translation
// unit unchanged: ROWEX nodes come from `posix_memalign` in header code that is
// instantiated *here*, so the link-time `--wrap` sees them, and its per-thread
// free lists are `std::vector`s through the replaced `operator new`. What the
// census cannot see is libtbb.so's own per-thread bookkeeping, allocated
// through the dynamic linker (§9.7's mechanism) — once per registering thread,
// independent of N (§11.3, decision 1).
//
// Every entry point below that says "concurrent" may be called from any number
// of threads at once on the same trie; that is the property the arm measures.
// `len` and the iterators are quiescent-only: they walk the structure and are
// called after the writers have joined.

#include <cstdint>
#include <cstdlib>
#include <utility>

#include <hot/rowex/HOTRowex.hpp>
#include <idx/contenthelpers/IdentityKeyExtractor.hpp>
#include <idx/contenthelpers/OptionalValue.hpp>
#include <idx/contenthelpers/PairPointerKeyExtractor.hpp>

using RowexSet = hot::rowex::HOTRowex<uint64_t, idx::contenthelpers::IdentityKeyExtractor>;
using RowexPair = std::pair<uint64_t, uint64_t>;
using RowexMap = hot::rowex::HOTRowex<RowexPair *, idx::contenthelpers::PairPointerKeyExtractor>;

// ROWEX's child pointer tags leaves in bit 0 and recovers the payload with a
// shift, exactly as the single-threaded variant does, so the inline payload is
// 63 bits and the predicate binds the set arm alone (measured at the §11.2
// gate: 2^63 inserted and not found). Same contract as `hot_shim.cpp`: -2
// means "this arm cannot represent this key", reported, never used to narrow
// the workload.
#define EXP_ROWEX_INLINE_PAYLOAD_GUARD(k) \
    do { if (((k) >> 63) != 0u) return -2; } while (0)

extern "C" {

// --- set arm (concurrent) -------------------------------------------------

void *exp_rowex_set_new(void) { return new RowexSet(); }
void exp_rowex_set_delete(void *t) { delete static_cast<RowexSet *>(t); }

// Concurrent.
int exp_rowex_set_insert(void *t, uint64_t k) {
    EXP_ROWEX_INLINE_PAYLOAD_GUARD(k);
    return static_cast<RowexSet *>(t)->insert(k) ? 1 : 0;
}

// Concurrent.
int exp_rowex_set_contains(void *t, uint64_t k) {
    EXP_ROWEX_INLINE_PAYLOAD_GUARD(k);
    return static_cast<RowexSet *>(t)->lookup(k).mIsValid ? 1 : 0;
}

// Quiescent-only. Counted by walking, never from `insert()` return values.
size_t exp_rowex_set_len(void *t) {
    size_t n = 0;
    RowexSet *trie = static_cast<RowexSet *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) ++n;
    return n;
}

// --- map arm (concurrent) -------------------------------------------------

void *exp_rowex_map_new(void) { return new RowexMap(); }

// Quiescent-only: the pairs are owned by the arm, not by HOT.
void exp_rowex_map_delete(void *t) {
    RowexMap *trie = static_cast<RowexMap *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) delete *it;
    delete trie;
}

// Concurrent. One heap pair per attempted insert, as in the single-threaded
// map arm — the value model HOT imposes when the value is not the key.
int exp_rowex_map_insert(void *t, uint64_t k, uint64_t v) {
    RowexPair *entry = new RowexPair(k, v);
    if (static_cast<RowexMap *>(t)->insert(entry)) return 1;
    delete entry;
    return 0;
}

// Concurrent. Fetches the stored value through the pair pointer, so the
// dereference is billed to the arm rather than stopping at `mIsValid` (§9.8).
int exp_rowex_map_get(void *t, uint64_t k, uint64_t *out) {
    auto r = static_cast<RowexMap *>(t)->lookup(k);
    if (!r.mIsValid) return 0;
    *out = r.mValue->second;
    return 1;
}

// Quiescent-only.
size_t exp_rowex_map_len(void *t) {
    size_t n = 0;
    RowexMap *trie = static_cast<RowexMap *>(t);
    for (auto it = trie->begin(); it != trie->end(); ++it) ++n;
    return n;
}

}  // extern "C"
