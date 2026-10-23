/*
 * expanse.h — the modern libexpanse C API.
 *
 * Additive to the legacy compat surface: Judy.h keeps the classic
 * Judy1/JudyL/JudySL/JudyHS families at unchanged semantics, and this
 * header exposes what the engine can do beyond them. Both live in the
 * same library — link libexpanse once, use either or both.
 *
 * What this API adds over the classic one:
 *   - Named, typed handles instead of a bare Pvoid_t root word.
 *   - O(depth) rank and select on ordered types (count_below,
 *     count_range, by_count) — classic Judy exposes Count/ByCount only
 *     for Judy1/JudyL and not on the string types.
 *   - Byte-exact memory accounting on every type.
 *   - Concurrent readers: one writer plus optimistic readers over the
 *     same tree (expanse_sync_set_t / expanse_sync_map_t), which the
 *     classic library has no equivalent for.
 *   - Explicit error-free value semantics: functions return what they
 *     mean (bool / count / slot pointer), no JError_t out-parameters.
 *
 * Conventions:
 *   - Handles are opaque pointers; NULL is never a valid live handle.
 *     Every _new() returns NULL only on allocation failure paths that
 *     abort (see docs/COMPAT.md D3), so in practice it is non-NULL.
 *   - Value-slot pointers (expanse_word_t*) stay valid until the next
 *     structural mutation of that container — the classic JudyL slot
 *     contract, unchanged.
 *   - Byte-string functions take (pointer, length) and treat the bytes
 *     as opaque: embedded NULs are ordinary bytes, and a zero length
 *     with a NULL pointer is the valid empty key.
 *   - Every function is thread-compatible: distinct handles are
 *     independent. Shared access to ONE handle across threads requires
 *     the expanse_sync_* types.
 */
#ifndef EXPANSE_H
#define EXPANSE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/*
 * Keys and values are one machine word, exactly as classic Judy's Word_t
 * is: 64-bit builds speak uint64_t, 32-bit builds uint32_t. This matches
 * the engine's own invariant — a value slot is a single machine word, so
 * eight of them fill a 64-byte cache line at either width.
 */
#if UINTPTR_MAX == 0xFFFFFFFFu
typedef uint32_t expanse_word_t;
#else
typedef uint64_t expanse_word_t;
#endif

/*
 * EXPANSE_WIDE_SURFACE marks the entry points that exist only in a 64-bit
 * libexpanse. The 32-bit engine is a real trie, not a reduced one, but it
 * has no rank/select, no value-slot accessors, and no byte-string, string
 * or blob containers — so those symbols are ABSENT from a 32-bit build
 * rather than present and stubbed, and a link error names the gap. The
 * 64-bit concurrent types (expanse_sync_*) additionally need a std build,
 * so they are absent from every no_std 64-bit libexpanse. The reverse also
 * exists: `!EXPANSE_WIDE_SURFACE` blocks declare the entry points only the
 * 32-bit engine has — including its own concurrent surface, expanse_sync32_*,
 * which needs no std and is present in every 32-bit build. docs/COMPAT.md
 * carries the surface matrix.
 */
#if UINTPTR_MAX == 0xFFFFFFFFu
#define EXPANSE_WIDE_SURFACE 0
#else
#define EXPANSE_WIDE_SURFACE 1
#endif

#if defined(__GNUC__) || defined(__clang__)
#define EXPANSE_DEPRECATED(msg) __attribute__((deprecated(msg)))
#define EXPANSE_NONNULL(...) __attribute__((nonnull(__VA_ARGS__)))
#elif defined(_MSC_VER)
#define EXPANSE_DEPRECATED(msg) __declspec(deprecated(msg))
#define EXPANSE_NONNULL(...)
#else
#define EXPANSE_DEPRECATED(msg)
#define EXPANSE_NONNULL(...)
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Library identity ---------------------------------------------- */

/* Version of the libexpanse build, "MAJOR.MINOR.PATCH". */
const char *expanse_version(void);

/* ---- expanse_set_t: ordered set of expanse_word_t keys (cf. Judy1) -------- */

typedef struct expanse_set expanse_set_t;

expanse_set_t *expanse_set_new(void);
void           expanse_set_free(expanse_set_t *set);

/* Returns true if the key was newly inserted / actually removed. */
bool     expanse_set_insert(expanse_set_t *set, expanse_word_t key);
bool     expanse_set_remove(expanse_set_t *set, expanse_word_t key);
bool     expanse_set_contains(const expanse_set_t *set, expanse_word_t key);
uint64_t expanse_set_len(const expanse_set_t *set);
size_t   expanse_set_mem_used(const expanse_set_t *set);
void     expanse_set_clear(expanse_set_t *set);

/*
 * Ordered navigation. Each writes the found key through `key_out` and
 * returns true; false means no such key (and `key_out` is untouched).
 * The _at_or_ variants include the given key itself.
 */
bool expanse_set_first(const expanse_set_t *set, expanse_word_t *key_out);
bool expanse_set_last(const expanse_set_t *set, expanse_word_t *key_out);
bool expanse_set_next_at_or_after(const expanse_set_t *set, expanse_word_t key,
                                  expanse_word_t *key_out);
bool expanse_set_next_after(const expanse_set_t *set, expanse_word_t key, expanse_word_t *key_out);
bool expanse_set_prev_at_or_before(const expanse_set_t *set, expanse_word_t key,
                                   expanse_word_t *key_out);
bool expanse_set_prev_before(const expanse_set_t *set, expanse_word_t key, expanse_word_t *key_out);

#if EXPANSE_WIDE_SURFACE
/* Rank and select, both O(depth). */
uint64_t expanse_set_count_below(const expanse_set_t *set, uint64_t key);
uint64_t expanse_set_count_range(const expanse_set_t *set, uint64_t lo, uint64_t hi);
bool     expanse_set_by_count(const expanse_set_t *set, uint64_t n, uint64_t *key_out);
bool     expanse_set_validate(const expanse_set_t *set);

#endif /* EXPANSE_WIDE_SURFACE */

/*
 * Held memory. mem_held: heap bytes the set holds from the system
 * allocator -- mem_used plus freed blocks kept for reuse and unused slab
 * space (on a 32-bit build: the node arena's slot table and free-handle
 * stack, which keep their peak capacity); 0 for NULL. A set keeps them
 * after removals; shrink_to_fit returns them to the system allocator and
 * returns the bytes released (0 for NULL), after which mem_held is lower by
 * exactly that much and mem_used is unchanged. Nothing moves: no key, value
 * or value pointer is invalidated. clear() and free() also return them.
 */
size_t   expanse_set_mem_held(const expanse_set_t *set);
size_t   expanse_set_shrink_to_fit(expanse_set_t *set);


/*
 * Batched membership query with memory-level parallelism prefetching.
 * Checks membership for `count` keys in `keys`, writing boolean presence (true/false)
 * into `out_present`. Returns the count of found keys.
 */
size_t expanse_set_contains_batch(const expanse_set_t *set, const expanse_word_t *keys,
                                  bool *out_present, size_t count);

/* ---- expanse_map_t: ordered expanse_word_t -> expanse_word_t map (cf. JudyL) ---- */

typedef struct expanse_map expanse_map_t;

expanse_map_t *expanse_map_new(void);
void           expanse_map_free(expanse_map_t *map);

/*
 * insert: stores key -> value. If the key was present, writes the
 * replaced value through `old_out` (when non-NULL) and returns false;
 * returns true when the key is new.
 */
bool     expanse_map_insert(expanse_map_t *map, expanse_word_t key,
                            expanse_word_t value, expanse_word_t *old_out) EXPANSE_NONNULL(1);
bool     expanse_map_get(const expanse_map_t *map, expanse_word_t key, expanse_word_t *value_out);
bool     expanse_map_contains(const expanse_map_t *map, expanse_word_t key);
bool     expanse_map_remove(expanse_map_t *map, expanse_word_t key, expanse_word_t *old_out);
uint64_t expanse_map_len(const expanse_map_t *map);
size_t   expanse_map_mem_used(const expanse_map_t *map);
void     expanse_map_clear(expanse_map_t *map);

/* Ordered navigation; `value_out` may be NULL if only the key matters. */
bool expanse_map_first(const expanse_map_t *map, expanse_word_t *key_out,
                       expanse_word_t *value_out);
bool expanse_map_last(const expanse_map_t *map, expanse_word_t *key_out,
                      expanse_word_t *value_out);
bool expanse_map_next_at_or_after(const expanse_map_t *map, expanse_word_t key,
                                  expanse_word_t *key_out, expanse_word_t *value_out);
bool expanse_map_next_after(const expanse_map_t *map, expanse_word_t key,
                            expanse_word_t *key_out, expanse_word_t *value_out);
bool expanse_map_prev_at_or_before(const expanse_map_t *map, expanse_word_t key,
                                   expanse_word_t *key_out, expanse_word_t *value_out);
bool expanse_map_prev_before(const expanse_map_t *map, expanse_word_t key,
                             expanse_word_t *key_out, expanse_word_t *value_out);

#if !EXPANSE_WIDE_SURFACE
/*
 * 32-bit-only surface (EXPANSE_WIDE_SURFACE == 0): entry points the 32-bit
 * engine has and the 64-bit engine does not, absent from a 64-bit build
 * exactly as the wide surface is absent from a 32-bit one. The language
 * bindings target 64-bit hosts, so scripts/check_abi_parity.py excludes
 * this block from binding coverage.
 *
 * remove_range: removes every entry whose key lies in [lo, hi], calling
 * `callback` (when non-NULL) once per removed entry in ascending key order
 * with `user_ctx`, and returns the count removed. One descent to the range
 * and one structural fix-up per touched node — the batched form of a
 * first/remove eviction loop. The callback must not touch `map`.
 *
 * Every callback below must return normally. Throwing a C++ exception through
 * it, or `longjmp`-ing out of it, unwinds through Rust frames that are mid
 * traversal or mid structural fix-up: no destructor runs, and the map is left
 * in a state that satisfies none of its invariants, with no diagnostic. This
 * is undefined behaviour and the library cannot detect it. A C++ caller
 * passing a lambda that can throw must catch inside the lambda. (A Rust panic
 * raised inside a callback is not an exception to this: `extern "C"` aborts
 * the process rather than unwinding, which is the defined -- and debuggable --
 * outcome.)
 */
typedef void (*expanse_map_remove_range_fn)(expanse_word_t key, expanse_word_t value,
                                            void *user_ctx);
size_t expanse_map_remove_range(expanse_map_t *map, expanse_word_t lo, expanse_word_t hi,
                                expanse_map_remove_range_fn callback, void *user_ctx);

/*
 * for_each_range: walks every entry whose key lies in [lo, hi] in ascending
 * key order, calling `callback` on each until it returns false. Returns true
 * when the range was walked to the end, false when the callback stopped it.
 * A NULL map or callback, or an inverted range, walks nothing and returns
 * true.
 *
 * One descent to `lo` and then contiguous streaming through the leaves the
 * range spans, where an expanse_map_next_after loop pays a fresh O(depth)
 * root descent per key. Nothing is borrowed across the call boundary: the
 * callback receives keys and values by value, so no pointer outlives the
 * walk. The callback must not mutate `map` during the walk, and must return
 * normally -- see the note on `expanse_map_remove_range_fn` above.
 */
typedef bool (*expanse_map_for_each_range_fn)(expanse_word_t key, expanse_word_t value,
                                              void *user_ctx);
bool expanse_map_for_each_range(const expanse_map_t *map, expanse_word_t lo, expanse_word_t hi,
                                expanse_map_for_each_range_fn callback, void *user_ctx);
#endif /* !EXPANSE_WIDE_SURFACE */

#if !EXPANSE_WIDE_SURFACE
/* ---- expanse_sync32_*: one writer, single-attempt optimistic readers (32-bit only) ---- */

/*
 * The 32-bit engine's concurrent surface (docs/COMPAT.md, "The 32-bit
 * concurrent story"; crates/expanse/src/sync32.rs). Present in every
 * 32-bit build, std or not; absent from 64-bit builds, which carry the
 * separate expanse_sync_* family. Provisional: issue #573 item 3 measures
 * it against a mutex around the single-threaded map on hardware and is the
 * gate for keeping or retracting it.
 *
 * PROTOCOL. Blocking optimistic lock coupling, single attempt, no mutex
 * fallback — not lock-free. The writer brackets every mutation with a
 * version bump; a reader samples the version, walks, and validates. A read
 * that overlaps an open bracket, or sees torn content, returns
 * EXPANSE_SYNC32_BUSY at once: it never spins, because on a single-core
 * part BUSY inside an interrupt handler means the handler preempted the
 * writer inside its bracket, and the writer cannot progress until the
 * handler returns. Surface BUSY and retry on the next invocation. The whole
 * wrapper is atomic load/store plus fences — no compare-and-swap, which the
 * riscv32imc parts (ESP32-C2/C3) do not have.
 *
 * MEMORY. Fixed capacity by design: `node_cap` node slots and
 * `max_readers` reader slots (64 bytes each, cache-line padded) are reserved
 * at _new and never grow. A mutation that would need more than
 * expanse_sync32_mutation_headroom() free slots is refused with ARENA_FULL,
 * and one that cannot reclaim because a reader is inside a walk is refused
 * with RECLAIM_BACKLOG — in both cases BEFORE the tree is touched. Freed
 * nodes are parked until every reader is observed outside a walk; that
 * wait is not time-bounded. Writer calls allocate (a bracket may contain up
 * to expanse_sync32_mutation_headroom() allocator calls) and free; reader
 * try_* calls never allocate and never block.
 *
 * HANDLES AND EXECUTION CONTEXTS — the contract the C compiler cannot
 * enforce, so read it twice:
 *  - The writer handle (expanse_sync32_*_writer) is ONE per container and
 *    is used from ONE execution context at a time. Writer calls are not
 *    reentrant (no writer call from a callback or interrupt that preempts
 *    another writer call) and, because they allocate, never run in an
 *    interrupt handler.
 *  - A reader handle (expanse_sync32_*_reader(map, idx)) is ONE per
 *    execution context: a main loop and the interrupt handler that preempts
 *    it use different indices. Sharing one is a use-after-free in waiting —
 *    the inner walk's exit unpins the outer walk, and reclamation may then
 *    free memory the outer walk still dereferences. Debug builds assert on
 *    a re-entered handle.
 *  - reader try_* are the only interrupt-safe entry points: single attempt,
 *    no allocation, no blocking, BUSY as an ordinary answer.
 *  - Handles are owned by the container and live until _free; every handle
 *    pointer dangles after _free. Call _free only with reader interrupts
 *    masked, reader tasks joined, and no writer call in progress.
 *  - The contract is satisfied within one hart. Readers on another core
 *    (ESP32-C6 LP core, ESP32-P4 second HP core) are not supported by this
 *    version.
 *
 * Canonical shape: the main task owns the writer and reads through
 * _writer_get (always consistent, never BUSY); each interrupt context takes
 * one reader index; max_readers = number of interrupt contexts.
 *
 * STATUS CODES come in three bands; each function lists the values it can
 * return. New values may be added within a band.
 */
#define EXPANSE_HAS_SYNC32 1

typedef enum {
    /* outcomes */
    EXPANSE_SYNC32_OK              = 0,
    EXPANSE_SYNC32_NOT_FOUND       = 1,
    EXPANSE_SYNC32_BUSY            = 2,
    /* refusals: the tree is untouched */
    EXPANSE_SYNC32_ARENA_FULL      = 16,
    EXPANSE_SYNC32_RECLAIM_BACKLOG = 17,
    /* usage errors: a precondition violation, asserted in debug builds */
    EXPANSE_SYNC32_NULL_HANDLE     = 32
} expanse_sync32_status_t;

/* Append-only; pass sizeof(*stats) and receive the prefix you know. */
typedef struct {
    uint64_t len;
    size_t   mem_used;
    size_t   pending_len;
    size_t   pending_bytes;
    size_t   free_slots;
} expanse_sync32_stats_t;

size_t      expanse_sync32_mutation_headroom(void);
const char *expanse_sync32_status_str(int status);

typedef struct expanse_sync32_map        expanse_sync32_map_t;
typedef struct expanse_sync32_map_writer expanse_sync32_map_writer_t;
typedef struct expanse_sync32_map_reader expanse_sync32_map_reader_t;

/* NULL if node_cap < expanse_sync32_mutation_headroom() (or on abort paths). */
expanse_sync32_map_t *expanse_sync32_map_new(size_t node_cap, size_t max_readers);
void                  expanse_sync32_map_free(expanse_sync32_map_t *map);
/* Idempotent accessors: the same pointer every call; NULL for NULL or an
 * out-of-range index. No allocation, no atomics. */
expanse_sync32_map_writer_t *expanse_sync32_map_writer(expanse_sync32_map_t *map);
expanse_sync32_map_reader_t *expanse_sync32_map_reader(expanse_sync32_map_t *map, size_t idx);

/* OK (replaced_out/old_out written when the key was present) | ARENA_FULL |
 * RECLAIM_BACKLOG | NULL_HANDLE. Allocates. */
expanse_sync32_status_t expanse_sync32_map_writer_try_insert(
    expanse_sync32_map_writer_t *w, expanse_word_t key, expanse_word_t value,
    bool *replaced_out, expanse_word_t *old_out);
/* OK (old_out written) | NOT_FOUND | ARENA_FULL | RECLAIM_BACKLOG (headroom
 * is checked before the lookup) | NULL_HANDLE. Allocates. */
expanse_sync32_status_t expanse_sync32_map_writer_try_remove(
    expanse_sync32_map_writer_t *w, expanse_word_t key, expanse_word_t *old_out);
/*
 * Single-key read-modify-write through the writer handle (#1194). Atomic
 * against other writers by construction (one writer handle per container) and
 * published inside at most one write bracket so readers never observe a
 * half-applied change; needs no compare-and-swap (riscv32imc-compatible).
 *
 * compare_exchange: compares the current entry at `key` with
 * `(expected_present, expected_value)` (`expected_value` is ignored when
 * `!expected_present`). On EXPANSE_SYNC32_OK, `*exchanged_out` is set to true
 * when the entry matched and `(new_present, new_value)` took effect (insert,
 * overwrite, remove when `!new_present`, or a no-op when `new` equals the
 * observed entry), or false on mismatch; `*observed_present_out` is set to
 * whether `key` was present before the call and, when present, `*observed_out`
 * receives that previous value. Mismatches and no-op matches open no bracket
 * and consult no headroom. Returns OK | ARENA_FULL | RECLAIM_BACKLOG |
 * NULL_HANDLE. Allocates on a mutating match.
 *
 * update: checks mutation headroom first (so `callback` is never called on
 * ARENA_FULL or RECLAIM_BACKLOG), reads the current entry (`current_present`,
 * `current_value`, with `*new_value_out` pre-filled to `current_value` or 0
 * when absent), and invokes `callback` once. Returning true stores
 * `*new_value_out`; returning false removes `key` (no-op if `new` equals the
 * observed entry, opening no bracket). On OK, `*old_present_out` and (when
 * present) `*old_out` report the entry before `callback` ran. The callback
 * must not touch `w` or its owning map and must return normally (see the
 * callback note above). Returns OK | ARENA_FULL | RECLAIM_BACKLOG |
 * NULL_HANDLE (null `w` or null `callback`). Allocates when the entry changes.
 *
 * get_or_insert: if `key` is already present, returns OK with
 * `*inserted_out = false` and `*value_out` set to the existing value without
 * opening a bracket or consulting headroom; if absent, checks headroom,
 * inserts `value` in one bracket, and returns OK with `*inserted_out = true`
 * and `*value_out = value`. Returns OK | ARENA_FULL | RECLAIM_BACKLOG |
 * NULL_HANDLE. Allocates on insert.
 */
typedef bool (*expanse_sync32_map_update_fn)(bool current_present, expanse_word_t current_value,
                                             expanse_word_t *new_value_out, void *user_ctx);
expanse_sync32_status_t expanse_sync32_map_writer_compare_exchange(
    expanse_sync32_map_writer_t *w, expanse_word_t key, bool expected_present,
    expanse_word_t expected_value, bool new_present, expanse_word_t new_value,
    bool *exchanged_out, bool *observed_present_out, expanse_word_t *observed_out);
expanse_sync32_status_t expanse_sync32_map_writer_update(
    expanse_sync32_map_writer_t *w, expanse_word_t key, expanse_sync32_map_update_fn callback,
    void *user_ctx, bool *old_present_out, expanse_word_t *old_out);
expanse_sync32_status_t expanse_sync32_map_writer_get_or_insert(
    expanse_sync32_map_writer_t *w, expanse_word_t key, expanse_word_t value,
    bool *inserted_out, expanse_word_t *value_out);
/* OK (drained, or nothing pending) | RECLAIM_BACKLOG | NULL_HANDLE. */
expanse_sync32_status_t expanse_sync32_map_writer_try_reclaim(expanse_sync32_map_writer_t *w);
/* The writer's own consistent read: never BUSY, never allocates. */
bool expanse_sync32_map_writer_get(const expanse_sync32_map_writer_t *w, expanse_word_t key,
                                   expanse_word_t *value_out);
/* OK | NULL_HANDLE. */
expanse_sync32_status_t expanse_sync32_map_writer_stats(const expanse_sync32_map_writer_t *w,
                                                        expanse_sync32_stats_t *stats,
                                                        size_t stats_size);
/* OK (value_out written) | NOT_FOUND | BUSY | NULL_HANDLE. Interrupt-safe. */
expanse_sync32_status_t expanse_sync32_map_reader_try_get(expanse_sync32_map_reader_t *r,
                                                          expanse_word_t key,
                                                          expanse_word_t *value_out);
/* OK (len_out written) | BUSY | NULL_HANDLE. Interrupt-safe. */
expanse_sync32_status_t expanse_sync32_map_reader_try_len(expanse_sync32_map_reader_t *r,
                                                          uint64_t *len_out);
/*
 * Ordered reads: the smallest entry, the largest, and the nearest entry
 * at-or-after / after / at-or-before / before `key`. OK (entry written
 * through key_out and value_out; either may be NULL) | NOT_FOUND (no such
 * entry, including try_next_after(UINT32_MAX) and try_prev_before(0)) |
 * BUSY | NULL_HANDLE. Single attempt, validated like _reader_try_get: never
 * blocks, never allocates. Interrupt-safe.
 */
expanse_sync32_status_t expanse_sync32_map_reader_try_first(expanse_sync32_map_reader_t *r,
                                                            expanse_word_t *key_out,
                                                            expanse_word_t *value_out);
expanse_sync32_status_t expanse_sync32_map_reader_try_last(expanse_sync32_map_reader_t *r,
                                                           expanse_word_t *key_out,
                                                           expanse_word_t *value_out);
expanse_sync32_status_t expanse_sync32_map_reader_try_next_at_or_after(
    expanse_sync32_map_reader_t *r, expanse_word_t key, expanse_word_t *key_out,
    expanse_word_t *value_out);
expanse_sync32_status_t expanse_sync32_map_reader_try_next_after(
    expanse_sync32_map_reader_t *r, expanse_word_t key, expanse_word_t *key_out,
    expanse_word_t *value_out);
expanse_sync32_status_t expanse_sync32_map_reader_try_prev_at_or_before(
    expanse_sync32_map_reader_t *r, expanse_word_t key, expanse_word_t *key_out,
    expanse_word_t *value_out);
expanse_sync32_status_t expanse_sync32_map_reader_try_prev_before(
    expanse_sync32_map_reader_t *r, expanse_word_t key, expanse_word_t *key_out,
    expanse_word_t *value_out);

typedef struct expanse_sync32_set        expanse_sync32_set_t;
typedef struct expanse_sync32_set_writer expanse_sync32_set_writer_t;
typedef struct expanse_sync32_set_reader expanse_sync32_set_reader_t;

expanse_sync32_set_t *expanse_sync32_set_new(size_t node_cap, size_t max_readers);
void                  expanse_sync32_set_free(expanse_sync32_set_t *set);
expanse_sync32_set_writer_t *expanse_sync32_set_writer(expanse_sync32_set_t *set);
expanse_sync32_set_reader_t *expanse_sync32_set_reader(expanse_sync32_set_t *set, size_t idx);

/* OK (inserted_out written: false if already present) | ARENA_FULL |
 * RECLAIM_BACKLOG | NULL_HANDLE. Allocates. */
expanse_sync32_status_t expanse_sync32_set_writer_try_insert(
    expanse_sync32_set_writer_t *w, expanse_word_t key, bool *inserted_out);
/* OK | NOT_FOUND | ARENA_FULL | RECLAIM_BACKLOG | NULL_HANDLE. Allocates. */
expanse_sync32_status_t expanse_sync32_set_writer_try_remove(
    expanse_sync32_set_writer_t *w, expanse_word_t key);
/* OK | RECLAIM_BACKLOG | NULL_HANDLE. */
expanse_sync32_status_t expanse_sync32_set_writer_try_reclaim(expanse_sync32_set_writer_t *w);
/* The writer's own consistent membership test: never BUSY, never allocates. */
bool expanse_sync32_set_writer_contains(const expanse_sync32_set_writer_t *w, expanse_word_t key);
/* OK | NULL_HANDLE. */
expanse_sync32_status_t expanse_sync32_set_writer_stats(const expanse_sync32_set_writer_t *w,
                                                        expanse_sync32_stats_t *stats,
                                                        size_t stats_size);
/* OK (present) | NOT_FOUND | BUSY | NULL_HANDLE. Interrupt-safe. */
expanse_sync32_status_t expanse_sync32_set_reader_try_contains(expanse_sync32_set_reader_t *r,
                                                               expanse_word_t key);
/* OK (len_out written) | BUSY | NULL_HANDLE. Interrupt-safe. */
expanse_sync32_status_t expanse_sync32_set_reader_try_len(expanse_sync32_set_reader_t *r,
                                                          uint64_t *len_out);
#endif /* !EXPANSE_WIDE_SURFACE */

#if EXPANSE_WIDE_SURFACE
/*
 * Batched key lookup with memory-level parallelism prefetching.
 * Looks up `count` keys in `keys`. For each key found, writes the value into `out_values`
 * and true into `out_found` (when non-NULL). Returns the count of found keys.
 */
size_t expanse_map_get_batch(const expanse_map_t *map, const expanse_word_t *keys,
                             expanse_word_t *out_values, bool *out_found, size_t count);

/*
 * Value slots (classic JudyL convention): _slot returns a writable
 * pointer to the stored value, NULL if the key is absent; _ins_slot
 * inserts the key with value 0 if absent (an existing value is kept)
 * and always returns its slot. Valid until the next mutation.
 */
expanse_word_t *expanse_map_slot(expanse_map_t *map, expanse_word_t key);
expanse_word_t *expanse_map_ins_slot(expanse_map_t *map, expanse_word_t key);

uint64_t expanse_map_count_below(const expanse_map_t *map, uint64_t key);
uint64_t expanse_map_count_range(const expanse_map_t *map, uint64_t lo, uint64_t hi);
bool     expanse_map_by_count(const expanse_map_t *map, uint64_t n,
                              expanse_word_t *key_out, expanse_word_t *value_out);
bool     expanse_map_validate(const expanse_map_t *map);
bool     expanse_map_validate_explain(const expanse_map_t *map, char *err_buf, size_t err_len);

#endif /* EXPANSE_WIDE_SURFACE */

/*
 * Held memory. mem_held: heap bytes the map holds from the system
 * allocator -- mem_used plus freed blocks kept for reuse and unused slab
 * space (on a 32-bit build: the node arena's slot table and free-handle
 * stack, which keep their peak capacity); 0 for NULL. A map keeps them
 * after removals; shrink_to_fit returns them to the system allocator and
 * returns the bytes released (0 for NULL), after which mem_held is lower by
 * exactly that much and mem_used is unchanged. Nothing moves: no key, value
 * or value pointer is invalidated. clear() and free() also return them.
 */
size_t   expanse_map_mem_held(const expanse_map_t *map);
size_t   expanse_map_shrink_to_fit(expanse_map_t *map);



#if EXPANSE_WIDE_SURFACE
/* ---- expanse_bytesmap_t: unordered bytes -> uint64_t (cf. JudyHS) --- */

typedef struct expanse_bytesmap expanse_bytesmap_t;

/* Callback function type for expanse_bytesmap_for_each. Return false to stop traversal. */
typedef bool (*expanse_bytesmap_iter_fn)(const void *key, size_t key_len, uint64_t value, void *user_ctx);

expanse_bytesmap_t *expanse_bytesmap_new(void);
void                expanse_bytesmap_free(expanse_bytesmap_t *map);

bool     expanse_bytesmap_insert(expanse_bytesmap_t *map, const void *key, size_t len,
                                 uint64_t value, uint64_t *old_out) EXPANSE_NONNULL(1);
bool     expanse_bytesmap_get(const expanse_bytesmap_t *map, const void *key, size_t len,
                              uint64_t *value_out);
bool     expanse_bytesmap_contains(const expanse_bytesmap_t *map, const void *key, size_t len);
bool     expanse_bytesmap_remove(expanse_bytesmap_t *map, const void *key, size_t len,
                                 uint64_t *old_out);
uint64_t *expanse_bytesmap_slot(const expanse_bytesmap_t *map, const void *key, size_t len);
uint64_t *expanse_bytesmap_ins_slot(expanse_bytesmap_t *map, const void *key, size_t len);
uint64_t expanse_bytesmap_len(const expanse_bytesmap_t *map);
size_t   expanse_bytesmap_mem_used(const expanse_bytesmap_t *map);
void     expanse_bytesmap_clear(expanse_bytesmap_t *map);
size_t   expanse_bytesmap_for_each(const expanse_bytesmap_t *map, expanse_bytesmap_iter_fn callback, void *user_ctx);

/* ---- expanse_strmap_t: ordered C-string -> uint64_t map (cf. JudySL) - */

typedef struct expanse_strmap expanse_strmap_t;

expanse_strmap_t *expanse_strmap_new(void);
void              expanse_strmap_free(expanse_strmap_t *map);

bool     expanse_strmap_insert(expanse_strmap_t *map, const char *key, uint64_t value,
                               uint64_t *old_out) EXPANSE_NONNULL(1);
bool     expanse_strmap_get(const expanse_strmap_t *map, const char *key, uint64_t *value_out);
bool     expanse_strmap_contains(const expanse_strmap_t *map, const char *key);
bool     expanse_strmap_remove(expanse_strmap_t *map, const char *key, uint64_t *old_out);
uint64_t *expanse_strmap_slot(const expanse_strmap_t *map, const char *key);
uint64_t *expanse_strmap_ins_slot(expanse_strmap_t *map, const char *key);
uint64_t expanse_strmap_len(const expanse_strmap_t *map);
size_t   expanse_strmap_mem_used(const expanse_strmap_t *map);
void     expanse_strmap_clear(expanse_strmap_t *map);
/* Held memory and its release, as expanse_map_mem_held / _shrink_to_fit. */
size_t   expanse_strmap_mem_held(const expanse_strmap_t *map);
size_t   expanse_strmap_shrink_to_fit(expanse_strmap_t *map);

/*
 * Ordered string navigation. The found key (NUL-terminated) is written to
 * `key_out` (up to `buf_len` bytes including NUL). Returns false if no key
 * is found or if `buf_len` is insufficient.
 */
EXPANSE_DEPRECATED("use expanse_strmap_first_ex to distinguish truncation from absent key")
bool expanse_strmap_first(const expanse_strmap_t *map, char *key_out, size_t buf_len,
                          uint64_t *value_out);
EXPANSE_DEPRECATED("use expanse_strmap_last_ex to distinguish truncation from absent key")
bool expanse_strmap_last(const expanse_strmap_t *map, char *key_out, size_t buf_len,
                         uint64_t *value_out);
EXPANSE_DEPRECATED("use expanse_strmap_next_at_or_after_ex to distinguish truncation from absent key")
bool expanse_strmap_next_at_or_after(const expanse_strmap_t *map, const char *key,
                                     char *key_out, size_t buf_len, uint64_t *value_out);
EXPANSE_DEPRECATED("use expanse_strmap_next_after_ex to distinguish truncation from absent key")
bool expanse_strmap_next_after(const expanse_strmap_t *map, const char *key,
                               char *key_out, size_t buf_len, uint64_t *value_out);
EXPANSE_DEPRECATED("use expanse_strmap_prev_at_or_before_ex to distinguish truncation from absent key")
bool expanse_strmap_prev_at_or_before(const expanse_strmap_t *map, const char *key,
                                      char *key_out, size_t buf_len, uint64_t *value_out);
EXPANSE_DEPRECATED("use expanse_strmap_prev_before_ex to distinguish truncation from absent key")
bool expanse_strmap_prev_before(const expanse_strmap_t *map, const char *key,
                                char *key_out, size_t buf_len, uint64_t *value_out);

/*
 * Truncation-aware string navigation (the `_ex` variants).
 *
 * The plain expanse_strmap_first/last/next/prev above return `false` for BOTH
 * "no such key" and "key found but buf_len too small", so a caller cannot tell
 * a missing key from a truncated one and may silently drop long keys. These
 * variants disambiguate via an explicit status and report the needed buffer
 * size through `required_len`. The original symbols are unchanged.
 *
 * On EXPANSE_STR_NAV_OK the NUL-terminated key is written to `key_out`, the
 * value to `*value_out` (if non-NULL), and `*required_len` (if non-NULL) is set
 * to the byte length needed (key length + 1 for the NUL). On
 * EXPANSE_STR_NAV_BUFFER_TOO_SMALL nothing is written to `key_out` but
 * `*required_len` (if non-NULL) is set so the caller can retry with a big
 * enough buffer. On EXPANSE_STR_NAV_NOT_FOUND no key matched and nothing is
 * written.
 */
typedef enum {
    EXPANSE_STR_NAV_OK = 0,
    EXPANSE_STR_NAV_NOT_FOUND = 1,
    EXPANSE_STR_NAV_BUFFER_TOO_SMALL = 2
} expanse_str_nav_status;
typedef expanse_str_nav_status expanse_str_nav_status_t;

expanse_str_nav_status expanse_strmap_first_ex(const expanse_strmap_t *map,
                                               char *key_out, size_t buf_len,
                                               size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_last_ex(const expanse_strmap_t *map,
                                              char *key_out, size_t buf_len,
                                              size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_next_at_or_after_ex(const expanse_strmap_t *map, const char *key,
                                                          char *key_out, size_t buf_len,
                                                          size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_next_after_ex(const expanse_strmap_t *map, const char *key,
                                                    char *key_out, size_t buf_len,
                                                    size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_prev_at_or_before_ex(const expanse_strmap_t *map, const char *key,
                                                           char *key_out, size_t buf_len,
                                                           size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_prev_before_ex(const expanse_strmap_t *map, const char *key,
                                                     char *key_out, size_t buf_len,
                                                     size_t *required_len, uint64_t *value_out);

/* ---- expanse_ordered_bytesmap_t: ordered arbitrary bytes -> uint64_t map --- */

typedef struct expanse_ordered_bytesmap expanse_ordered_bytesmap_t;

/*
 * Status returned by expanse_ordered_bytesmap navigation functions.
 * Disambiguates an absent key from a caller destination buffer that is too small.
 */
typedef enum {
    EXPANSE_ORDERED_BYTES_NAV_OK = 0,
    EXPANSE_ORDERED_BYTES_NAV_NOT_FOUND = 1,
    EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL = 2
} expanse_ordered_bytes_nav_status;
typedef expanse_ordered_bytes_nav_status expanse_ordered_bytes_nav_status_t;

expanse_ordered_bytesmap_t *expanse_ordered_bytesmap_new(void);
void                        expanse_ordered_bytesmap_free(expanse_ordered_bytesmap_t *map);

bool     expanse_ordered_bytesmap_insert(expanse_ordered_bytesmap_t *map, const void *key, size_t len,
                                         uint64_t value, uint64_t *old_out) EXPANSE_NONNULL(1);
bool     expanse_ordered_bytesmap_get(const expanse_ordered_bytesmap_t *map, const void *key, size_t len,
                                      uint64_t *value_out);
bool     expanse_ordered_bytesmap_contains(const expanse_ordered_bytesmap_t *map, const void *key, size_t len);
bool     expanse_ordered_bytesmap_remove(expanse_ordered_bytesmap_t *map, const void *key, size_t len,
                                         uint64_t *old_out);
/*
 * Value slots (classic JudyL convention): _slot returns a writable pointer
 * to the stored value, NULL if the key is absent; _ins_slot inserts the
 * key with value 0 if absent (an existing value is kept) and always returns
 * its slot. Valid until the next structural mutation of that map.
 * Zero rotating ring buffers are used; pointers refer directly to the stored
 * value slot.
 */
uint64_t *expanse_ordered_bytesmap_slot(const expanse_ordered_bytesmap_t *map, const void *key, size_t len);
uint64_t *expanse_ordered_bytesmap_ins_slot(expanse_ordered_bytesmap_t *map, const void *key, size_t len);
uint64_t expanse_ordered_bytesmap_len(const expanse_ordered_bytesmap_t *map);
size_t   expanse_ordered_bytesmap_mem_used(const expanse_ordered_bytesmap_t *map);
size_t   expanse_ordered_bytesmap_mem_held(const expanse_ordered_bytesmap_t *map);
size_t   expanse_ordered_bytesmap_shrink_to_fit(expanse_ordered_bytesmap_t *map);
void     expanse_ordered_bytesmap_clear(expanse_ordered_bytesmap_t *map);

/*
 * Ordered navigation over arbitrary byte keys using caller-allocated buffers.
 *
 * Keys are written to `key_out` (up to `buf_len` bytes).
 * On EXPANSE_ORDERED_BYTES_NAV_OK, the decoded key is written to `key_out`,
 * `*value_out` (if non-NULL) receives the value, and `*required_len` (if non-NULL)
 * receives the exact byte length of the key.
 * On EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL, nothing is written to `key_out`
 * (avoiding silent truncation), but `*required_len` (if non-NULL) receives the
 * needed buffer size so the caller can reallocate and retry.
 * On EXPANSE_ORDERED_BYTES_NAV_NOT_FOUND, no matching key was found.
 *
 * Passing `key_out == NULL` and `buf_len == 0` is valid to probe the required
 * buffer length for the matching key without writing. A matching key of zero
 * length needs no buffer, so that probe returns EXPANSE_ORDERED_BYTES_NAV_OK
 * with `*required_len == 0`.
 *
 * In the functions that take a search `key`, `key` and `key_out` may be the
 * same buffer: the search key is read before `key_out` is written.
 */
expanse_ordered_bytes_nav_status expanse_ordered_bytesmap_first(
    const expanse_ordered_bytesmap_t *map,
    void *key_out,
    size_t buf_len,
    size_t *required_len,
    uint64_t *value_out
);
expanse_ordered_bytes_nav_status expanse_ordered_bytesmap_last(
    const expanse_ordered_bytesmap_t *map,
    void *key_out,
    size_t buf_len,
    size_t *required_len,
    uint64_t *value_out
);
expanse_ordered_bytes_nav_status expanse_ordered_bytesmap_next_at_or_after(
    const expanse_ordered_bytesmap_t *map,
    const void *key,
    size_t len,
    void *key_out,
    size_t buf_len,
    size_t *required_len,
    uint64_t *value_out
);
expanse_ordered_bytes_nav_status expanse_ordered_bytesmap_next_after(
    const expanse_ordered_bytesmap_t *map,
    const void *key,
    size_t len,
    void *key_out,
    size_t buf_len,
    size_t *required_len,
    uint64_t *value_out
);
expanse_ordered_bytes_nav_status expanse_ordered_bytesmap_prev_at_or_before(
    const expanse_ordered_bytesmap_t *map,
    const void *key,
    size_t len,
    void *key_out,
    size_t buf_len,
    size_t *required_len,
    uint64_t *value_out
);
expanse_ordered_bytes_nav_status expanse_ordered_bytesmap_prev_before(
    const expanse_ordered_bytesmap_t *map,
    const void *key,
    size_t len,
    void *key_out,
    size_t buf_len,
    size_t *required_len,
    uint64_t *value_out
);


/* ---- Concurrent types: one writer, optimistic readers ---------------- */

/*
 * The capability classic Judy has no answer for. Writers serialize
 * internally; readers run an optimistic validated walk with epoch-based
 * reclamation, so a reader never dereferences freed memory and, on the
 * common path, takes no lock and does not block a writer. The protocol is
 * blocking optimistic lock coupling, not lock-free: a read that exhausts its
 * bounded retries falls back to the writer-excluding path (it quiesces
 * writers and takes the writer mutex), and writers wait for that read to
 * finish (docs/ARCHITECTURE.md §4, docs/BENCHMARKING.md for scaling).
 *
 * A container handle is safe to use from any number of threads at once.
 * Readers SHOULD take a reader handle (expanse_sync_*_reader_new) once per
 * thread and reuse it: the one-shot _contains/_get calls register and
 * drop a reader on every call.
 *
 * Reader-handle ownership:
 *  - One handle per reading thread. A handle owns a single epoch slot and
 *    its pins are not reentrant, so calls on ONE handle from two threads at
 *    once are undefined; give each reading thread its own.
 *  - A handle may be passed to, and freed (_reader_free) from, a thread
 *    other than the one that created it, once no call on it is in progress:
 *    every call releases its epoch pin before returning, and freeing
 *    deregisters the handle through the container's registry lock, reading
 *    no thread-local state.
 *  - Freeing a handle while another thread is inside a call on it is
 *    undefined. Freeing it after its parent container is undefined: free
 *    every reader handle before the container.
 */

typedef struct expanse_sync_set expanse_sync_set_t;
typedef struct expanse_sync_set_reader expanse_sync_set_reader_t;
typedef struct expanse_sync_map expanse_sync_map_t;
typedef struct expanse_sync_map_reader expanse_sync_map_reader_t;

expanse_sync_set_t *expanse_sync_set_new(void);
void                expanse_sync_set_free(expanse_sync_set_t *set);
bool                expanse_sync_set_insert(expanse_sync_set_t *set, uint64_t key);
bool                expanse_sync_set_remove(expanse_sync_set_t *set, uint64_t key);
bool                expanse_sync_set_contains(const expanse_sync_set_t *set, uint64_t key);
uint64_t            expanse_sync_set_len(const expanse_sync_set_t *set);
/*
 * Held memory of a concurrent container, and its release. mem_held: heap
 * bytes it holds from the global allocator: the tree's own share plus what
 * its epoch collector holds for it, freed blocks past their grace period
 * kept for reuse and retired blocks still in their grace period; 0 for NULL.
 * Read with writers excluded, like expanse_sync_map_mem_used: not a call for
 * a hot loop. shrink_to_fit returns the collector's freed blocks to the
 * system allocator and returns the bytes released (0 for NULL), after which
 * mem_held is lower by exactly that much. It runs beside readers and writers
 * without excluding either; blocks still in their grace period stay held
 * until a later call, so mem_held may still exceed mem_used afterwards.
 */
size_t              expanse_sync_set_mem_held(const expanse_sync_set_t *set);
size_t              expanse_sync_set_shrink_to_fit(const expanse_sync_set_t *set);

expanse_sync_set_reader_t *expanse_sync_set_reader_new(const expanse_sync_set_t *set);
void                       expanse_sync_set_reader_free(expanse_sync_set_reader_t *reader);
bool expanse_sync_set_reader_contains(const expanse_sync_set_reader_t *reader, uint64_t key);

expanse_sync_map_t *expanse_sync_map_new(void);
void                expanse_sync_map_free(expanse_sync_map_t *map);
bool                expanse_sync_map_insert(expanse_sync_map_t *map, uint64_t key,
                                            uint64_t value, uint64_t *old_out);
bool                expanse_sync_map_get(const expanse_sync_map_t *map, uint64_t key,
                                         uint64_t *value_out);
bool                expanse_sync_map_remove(expanse_sync_map_t *map, uint64_t key,
                                            uint64_t *old_out);
uint64_t            expanse_sync_map_len(const expanse_sync_map_t *map);
/*
 * Atomic read-modify-write on one key, callable from any thread on the
 * shared handle like the calls above; each is one linearizable step.
 *
 * compare_exchange stores new_value under key only if the key's current word
 * is expected_value. "Absent" is a flag, never a value, so every uint64_t is
 * a legal value: expected_present == false means "only if the key is
 * absent"; new_present == false means "remove the key". Returns true when
 * the store happened. On both outcomes *observed_present_out and
 * *observed_out report the word the compare saw (observed_out is written
 * only when that word was present; either output may be NULL), so a failed
 * call hands the caller the word to retry against. A NULL map returns
 * false and writes nothing.
 *
 * get_or_insert stores value under key only if key is absent and returns
 * true. If the key was present it returns false, leaves it untouched and
 * writes its value to *existing_out (may be NULL), as expanse_sync_map_insert
 * reports an old value. Of any number of callers racing on one absent key,
 * exactly one gets true.
 *
 * There is no callback form: an upcall per retry, and an exception unwinding
 * through C, are what the C boundary rules out. The read-modify-write "update"
 * is a loop over compare_exchange, which may run the computation several
 * times, so it must be free of side effects:
 *
 *     uint64_t v = 0; bool present = expanse_sync_map_get(m, k, &v);
 *     for (;;) {
 *         uint64_t nv = v + 1;                  // f(current)
 *         bool seen_present; uint64_t seen;
 *         if (expanse_sync_map_compare_exchange(m, k, present, v, true, nv,
 *                                               &seen_present, &seen)) break;
 *         present = seen_present; v = seen;     // retry on the observed word
 *     }
 */
bool                expanse_sync_map_compare_exchange(expanse_sync_map_t *map, uint64_t key,
                                                      bool expected_present, uint64_t expected_value,
                                                      bool new_present, uint64_t new_value,
                                                      bool *observed_present_out,
                                                      uint64_t *observed_out);
bool                expanse_sync_map_get_or_insert(expanse_sync_map_t *map, uint64_t key,
                                                   uint64_t value, uint64_t *existing_out);
/*
 * Heap bytes used by the map's nodes and leaves, as expanse_map_mem_used
 * counts them; 0 for a NULL map. Read with writers excluded (it quiesces
 * writers and takes the writer mutex), so writers wait for it: not a call for
 * a hot loop. Nodes retired to the epoch collector but not yet reclaimed are
 * no longer counted, although their allocations are still resident.
 */
size_t              expanse_sync_map_mem_used(const expanse_sync_map_t *map);
/* Held memory and its release, as expanse_sync_set_mem_held / _shrink_to_fit. */
size_t              expanse_sync_map_mem_held(const expanse_sync_map_t *map);
size_t              expanse_sync_map_shrink_to_fit(const expanse_sync_map_t *map);

expanse_sync_map_reader_t *expanse_sync_map_reader_new(const expanse_sync_map_t *map);
void                       expanse_sync_map_reader_free(expanse_sync_map_reader_t *reader);
bool expanse_sync_map_reader_get(const expanse_sync_map_reader_t *reader, uint64_t key,
                                 uint64_t *value_out);

/*
 * Ordered reads through a reader handle: the smallest entry, the largest,
 * and the nearest entry at-or-after / after / at-or-before / before `key`.
 * They return true with the entry written through key_out and value_out
 * (either may be NULL), and false when there is no such entry — including
 * next_after(UINT64_MAX) and prev_before(0) — or the reader is NULL.
 * Validated like _reader_get, with the same retry budget and fallback: the
 * entry returned was present, and every key the search passed over was
 * absent, at one instant.
 */
bool expanse_sync_map_reader_first(const expanse_sync_map_reader_t *reader, uint64_t *key_out,
                                   uint64_t *value_out);
bool expanse_sync_map_reader_last(const expanse_sync_map_reader_t *reader, uint64_t *key_out,
                                  uint64_t *value_out);
bool expanse_sync_map_reader_next_at_or_after(const expanse_sync_map_reader_t *reader,
                                              uint64_t key, uint64_t *key_out,
                                              uint64_t *value_out);
bool expanse_sync_map_reader_next_after(const expanse_sync_map_reader_t *reader, uint64_t key,
                                        uint64_t *key_out, uint64_t *value_out);
bool expanse_sync_map_reader_prev_at_or_before(const expanse_sync_map_reader_t *reader,
                                               uint64_t key, uint64_t *key_out,
                                               uint64_t *value_out);
bool expanse_sync_map_reader_prev_before(const expanse_sync_map_reader_t *reader, uint64_t key,
                                         uint64_t *key_out, uint64_t *value_out);

/*
 * expanse_sync_strmap_t: the concurrent ordered C-string -> uint64_t map
 * (the shared form of expanse_strmap_t). Same protocol, thread rules and
 * reader-handle ownership as expanse_sync_map_t above: the container handle
 * is safe from any number of threads at once except _free, which needs no
 * call in progress and every reader freed first; a reader handle is used by
 * one thread at a time, may be freed from another thread once no call on it
 * is in progress, and is freed before its map.
 *
 * Keys are NUL-terminated C strings, as expanse_strmap_* takes them; a key
 * with an embedded NUL byte goes to the bytes or ordered-bytes maps. A NULL
 * key behaves like a NULL handle: false (or 0), nothing written. Values,
 * out-pointers and return conventions are those of expanse_sync_map_*: insert
 * returns true when the key is new and otherwise writes the replaced value
 * through old_out; get, remove and the reader's get write through their
 * out-pointer only on true; every out-pointer may be NULL. mem_used,
 * mem_held and shrink_to_fit behave as the expanse_sync_map_* calls of the
 * same name: the first two read with writers excluded.
 */
typedef struct expanse_sync_strmap expanse_sync_strmap_t;
typedef struct expanse_sync_strmap_reader expanse_sync_strmap_reader_t;

expanse_sync_strmap_t *expanse_sync_strmap_new(void);
void                   expanse_sync_strmap_free(expanse_sync_strmap_t *map);
bool                   expanse_sync_strmap_insert(expanse_sync_strmap_t *map, const char *key,
                                                  uint64_t value, uint64_t *old_out);
bool                   expanse_sync_strmap_get(const expanse_sync_strmap_t *map, const char *key,
                                               uint64_t *value_out);
bool                   expanse_sync_strmap_contains(const expanse_sync_strmap_t *map,
                                                    const char *key);
bool                   expanse_sync_strmap_remove(expanse_sync_strmap_t *map, const char *key,
                                                  uint64_t *old_out);
uint64_t               expanse_sync_strmap_len(const expanse_sync_strmap_t *map);
/* A sum of the allocator's per-writer counters, taken without excluding writers:
 * no writer waits for it. Exact once every write has finished; while writers
 * run it can differ by the bytes of the operations in flight. */
size_t                 expanse_sync_strmap_mem_used(const expanse_sync_strmap_t *map);
/* Read with writers excluded, so writers wait for it; take a resident-memory
 * ratio from here. */
size_t                 expanse_sync_strmap_mem_held(const expanse_sync_strmap_t *map);
size_t                 expanse_sync_strmap_shrink_to_fit(const expanse_sync_strmap_t *map);
/*
 * Atomic read-modify-write on one key, with the convention, outputs and
 * retry loop of expanse_sync_map_compare_exchange / _get_or_insert above:
 * presence flags on both sides, the observed word reported on both outcomes
 * of compare_exchange, the present value through *existing_out when
 * get_or_insert returns false. Each is one linearizable step; of any number
 * of callers racing get_or_insert on one absent key, exactly one gets true.
 * A NULL map or key returns false and writes nothing.
 */
bool expanse_sync_strmap_compare_exchange(expanse_sync_strmap_t *map, const char *key,
                                          bool expected_present, uint64_t expected_value,
                                          bool new_present, uint64_t new_value,
                                          bool *observed_present_out, uint64_t *observed_out);
bool expanse_sync_strmap_get_or_insert(expanse_sync_strmap_t *map, const char *key,
                                       uint64_t value, uint64_t *existing_out);

expanse_sync_strmap_reader_t *expanse_sync_strmap_reader_new(const expanse_sync_strmap_t *map);
void                          expanse_sync_strmap_reader_free(expanse_sync_strmap_reader_t *reader);
bool expanse_sync_strmap_reader_get(const expanse_sync_strmap_reader_t *reader, const char *key,
                                    uint64_t *value_out);
bool expanse_sync_strmap_reader_contains(const expanse_sync_strmap_reader_t *reader,
                                         const char *key);

/*
 * Ordered reads through a string-map reader handle: the smallest entry, the
 * largest, and the nearest entry at-or-after / after / at-or-before / before
 * `key`. Each is one validated read with the retry budget and fallback of
 * _reader_get: the entry returned was present, and every key the search
 * passed over was absent, at one instant. A walk built from repeated
 * next_after calls is not a snapshot: it sees every key present throughout
 * it, in order, and may or may not see keys inserted or removed meanwhile.
 *
 * Status and outputs follow the expanse_strmap_*_ex contract above. OK: the
 * NUL-terminated key is in key_out, *required_len (if non-NULL) is its length
 * plus 1, and *value_out (if non-NULL) its value. BUFFER_TOO_SMALL: a key was
 * found but buf_len is below *required_len (or key_out is NULL; NULL with 0
 * is a valid length probe); only *required_len is written. NOT_FOUND: no such
 * entry, which includes prev_before(""), or a NULL reader or key; nothing is
 * written. A retry after BUFFER_TOO_SMALL is a new read, so it may return a
 * different key if a writer ran in between.
 */
expanse_str_nav_status expanse_sync_strmap_reader_first(const expanse_sync_strmap_reader_t *reader,
                                                        char *key_out, size_t buf_len,
                                                        size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_sync_strmap_reader_last(const expanse_sync_strmap_reader_t *reader,
                                                       char *key_out, size_t buf_len,
                                                       size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_sync_strmap_reader_next_at_or_after(
    const expanse_sync_strmap_reader_t *reader, const char *key, char *key_out, size_t buf_len,
    size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_sync_strmap_reader_next_after(
    const expanse_sync_strmap_reader_t *reader, const char *key, char *key_out, size_t buf_len,
    size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_sync_strmap_reader_prev_at_or_before(
    const expanse_sync_strmap_reader_t *reader, const char *key, char *key_out, size_t buf_len,
    size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_sync_strmap_reader_prev_before(
    const expanse_sync_strmap_reader_t *reader, const char *key, char *key_out, size_t buf_len,
    size_t *required_len, uint64_t *value_out);

/*
 * expanse_sync_bytesmap_t and expanse_sync_ordered_bytesmap_t: the concurrent
 * byte-string -> uint64_t maps (the shared forms of expanse_bytesmap_t, which
 * is unordered, and expanse_ordered_bytesmap_t). Same protocol, thread rules
 * and reader-handle ownership as expanse_sync_map_t above: the container
 * handle is safe from any number of threads at once except _free, which
 * needs no call in progress and every reader freed first; a reader handle is
 * used by one thread at a time, may be freed from another thread once no
 * call on it is in progress, and is freed before its map.
 *
 * Keys are (key, len) byte strings, as expanse_bytesmap_* takes them: opaque
 * bytes, embedded NULs ordinary, NULL with len 0 the empty key. A NULL key
 * with a non-zero len behaves like a NULL handle: false (or 0), nothing
 * written. Values, out-pointers and return conventions are those of
 * expanse_sync_strmap_* above; mem_used and mem_held read with writers
 * excluded.
 */
typedef struct expanse_sync_bytesmap expanse_sync_bytesmap_t;
typedef struct expanse_sync_bytesmap_reader expanse_sync_bytesmap_reader_t;

expanse_sync_bytesmap_t *expanse_sync_bytesmap_new(void);
void     expanse_sync_bytesmap_free(expanse_sync_bytesmap_t *map);
bool     expanse_sync_bytesmap_insert(expanse_sync_bytesmap_t *map, const void *key, size_t len,
                                      uint64_t value, uint64_t *old_out);
bool     expanse_sync_bytesmap_get(const expanse_sync_bytesmap_t *map, const void *key, size_t len,
                                   uint64_t *value_out);
bool     expanse_sync_bytesmap_contains(const expanse_sync_bytesmap_t *map, const void *key,
                                        size_t len);
bool     expanse_sync_bytesmap_remove(expanse_sync_bytesmap_t *map, const void *key, size_t len,
                                      uint64_t *old_out);
uint64_t expanse_sync_bytesmap_len(const expanse_sync_bytesmap_t *map);
size_t   expanse_sync_bytesmap_mem_used(const expanse_sync_bytesmap_t *map);
size_t   expanse_sync_bytesmap_mem_held(const expanse_sync_bytesmap_t *map);
size_t   expanse_sync_bytesmap_shrink_to_fit(const expanse_sync_bytesmap_t *map);
/*
 * Atomic read-modify-write on one key, for both byte-string maps: the
 * convention of expanse_sync_strmap_compare_exchange / _get_or_insert, with a
 * (key, len) byte-string key. A NULL map, or a NULL key with len > 0, returns
 * false and writes nothing.
 */
bool     expanse_sync_bytesmap_compare_exchange(expanse_sync_bytesmap_t *map, const void *key,
                                                size_t len, bool expected_present,
                                                uint64_t expected_value, bool new_present,
                                                uint64_t new_value, bool *observed_present_out,
                                                uint64_t *observed_out);
bool     expanse_sync_bytesmap_get_or_insert(expanse_sync_bytesmap_t *map, const void *key,
                                             size_t len, uint64_t value, uint64_t *existing_out);

expanse_sync_bytesmap_reader_t *expanse_sync_bytesmap_reader_new(const expanse_sync_bytesmap_t *map);
void expanse_sync_bytesmap_reader_free(expanse_sync_bytesmap_reader_t *reader);
bool expanse_sync_bytesmap_reader_get(const expanse_sync_bytesmap_reader_t *reader,
                                      const void *key, size_t len, uint64_t *value_out);
bool expanse_sync_bytesmap_reader_contains(const expanse_sync_bytesmap_reader_t *reader,
                                           const void *key, size_t len);

typedef struct expanse_sync_ordered_bytesmap expanse_sync_ordered_bytesmap_t;
typedef struct expanse_sync_ordered_bytesmap_reader expanse_sync_ordered_bytesmap_reader_t;

expanse_sync_ordered_bytesmap_t *expanse_sync_ordered_bytesmap_new(void);
void     expanse_sync_ordered_bytesmap_free(expanse_sync_ordered_bytesmap_t *map);
bool     expanse_sync_ordered_bytesmap_insert(expanse_sync_ordered_bytesmap_t *map, const void *key,
                                              size_t len, uint64_t value, uint64_t *old_out);
bool     expanse_sync_ordered_bytesmap_get(const expanse_sync_ordered_bytesmap_t *map,
                                           const void *key, size_t len, uint64_t *value_out);
bool     expanse_sync_ordered_bytesmap_contains(const expanse_sync_ordered_bytesmap_t *map,
                                                const void *key, size_t len);
bool     expanse_sync_ordered_bytesmap_remove(expanse_sync_ordered_bytesmap_t *map, const void *key,
                                              size_t len, uint64_t *old_out);
uint64_t expanse_sync_ordered_bytesmap_len(const expanse_sync_ordered_bytesmap_t *map);
/* As expanse_sync_strmap_mem_used / _mem_held: _mem_used waits for nothing,
 * _mem_held excludes writers. */
size_t   expanse_sync_ordered_bytesmap_mem_used(const expanse_sync_ordered_bytesmap_t *map);
size_t   expanse_sync_ordered_bytesmap_mem_held(const expanse_sync_ordered_bytesmap_t *map);
size_t   expanse_sync_ordered_bytesmap_shrink_to_fit(const expanse_sync_ordered_bytesmap_t *map);
/* Read-modify-write, as expanse_sync_bytesmap_compare_exchange / _get_or_insert. */
bool     expanse_sync_ordered_bytesmap_compare_exchange(expanse_sync_ordered_bytesmap_t *map,
                                                        const void *key, size_t len,
                                                        bool expected_present,
                                                        uint64_t expected_value, bool new_present,
                                                        uint64_t new_value,
                                                        bool *observed_present_out,
                                                        uint64_t *observed_out);
bool     expanse_sync_ordered_bytesmap_get_or_insert(expanse_sync_ordered_bytesmap_t *map,
                                                     const void *key, size_t len, uint64_t value,
                                                     uint64_t *existing_out);

expanse_sync_ordered_bytesmap_reader_t *expanse_sync_ordered_bytesmap_reader_new(
    const expanse_sync_ordered_bytesmap_t *map);
void expanse_sync_ordered_bytesmap_reader_free(expanse_sync_ordered_bytesmap_reader_t *reader);
bool expanse_sync_ordered_bytesmap_reader_get(const expanse_sync_ordered_bytesmap_reader_t *reader,
                                              const void *key, size_t len, uint64_t *value_out);
bool expanse_sync_ordered_bytesmap_reader_contains(
    const expanse_sync_ordered_bytesmap_reader_t *reader, const void *key, size_t len);

/*
 * Ordered reads of the shared ordered-bytes map: the smallest entry, the
 * largest, and the nearest entry at-or-after / after / at-or-before / before
 * `key`. They are calls on the MAP handle, not on a reader, and they BLOCK
 * WRITERS for their duration: each runs with every writer excluded (it
 * quiesces writers and takes the writer mutex), so the entry returned is the
 * answer at one instant, and every write waits for the call. They are not
 * calls for a hot loop beside writers. A walk built from repeated next_after
 * calls is not a snapshot: it sees every key present throughout it, in
 * order, and may or may not see keys inserted or removed between calls.
 *
 * Status and outputs follow the expanse_ordered_bytesmap_* navigation
 * contract above. OK: the key is in key_out[0 .. *required_len) (nothing for
 * the empty key), *required_len (if non-NULL) is its length, and *value_out
 * (if non-NULL) its value. BUFFER_TOO_SMALL: a key was found but buf_len is
 * below its length (or key_out is NULL; NULL with 0 is a valid length probe,
 * which returns OK with *required_len == 0 when the key found is empty); only
 * *required_len is written, and key_out is untouched. NOT_FOUND: no such
 * entry, which includes prev_before of the empty key, or a NULL map, or a
 * NULL key with a non-zero len; nothing is written. A retry after
 * BUFFER_TOO_SMALL is a new read, so it may return a different key if a
 * writer ran in between. In the functions that take a search `key`, `key`
 * and `key_out` may be the same buffer.
 */
expanse_ordered_bytes_nav_status expanse_sync_ordered_bytesmap_first(
    const expanse_sync_ordered_bytesmap_t *map, void *key_out, size_t buf_len,
    size_t *required_len, uint64_t *value_out);
expanse_ordered_bytes_nav_status expanse_sync_ordered_bytesmap_last(
    const expanse_sync_ordered_bytesmap_t *map, void *key_out, size_t buf_len,
    size_t *required_len, uint64_t *value_out);
expanse_ordered_bytes_nav_status expanse_sync_ordered_bytesmap_next_at_or_after(
    const expanse_sync_ordered_bytesmap_t *map, const void *key, size_t len, void *key_out,
    size_t buf_len, size_t *required_len, uint64_t *value_out);
expanse_ordered_bytes_nav_status expanse_sync_ordered_bytesmap_next_after(
    const expanse_sync_ordered_bytesmap_t *map, const void *key, size_t len, void *key_out,
    size_t buf_len, size_t *required_len, uint64_t *value_out);
expanse_ordered_bytes_nav_status expanse_sync_ordered_bytesmap_prev_at_or_before(
    const expanse_sync_ordered_bytesmap_t *map, const void *key, size_t len, void *key_out,
    size_t buf_len, size_t *required_len, uint64_t *value_out);
expanse_ordered_bytes_nav_status expanse_sync_ordered_bytesmap_prev_before(
    const expanse_sync_ordered_bytesmap_t *map, const void *key, size_t len, void *key_out,
    size_t buf_len, size_t *required_len, uint64_t *value_out);

/* ---- ExpanseBlobMap: polymorphic large-value map with inline/arena backing ---- */

typedef struct ExpanseBlobMap ExpanseBlobMap;
typedef struct ExpanseBlobMap expanse_blob_map_t;

/*
 * A zero-copy view of a stored payload.
 *
 * INVALIDATION CONTRACT (same spirit as the JudyL value-slot contract):
 * `ptr` borrows directly into the map's inline slot memory or arena slab and
 * stays valid ONLY until the next structural mutation of that map. For
 * compressed inline payloads, `ptr` is NULL and callers use
 * `expanse_blob_map_get_into` to decompress into a destination buffer. Any
 * expanse_blob_map_insert / _remove / _clear / _compact / _free invalidates
 * every previously returned ExpanseBlobView.ptr — reading through it afterwards
 * is undefined behavior. Copy the bytes out before mutating if you need them to
 * outlive the next mutation. Views delivered to a scan callback are valid only
 * for the duration of that callback invocation.
 *
 * An insert that the arena's capacity cap refuses may compact the arena before
 * it returns, whether it then succeeds or fails: every arena payload moves and
 * the chunks earlier views pointed into are freed. A view held across any
 * insert can therefore read freed memory, not merely stale bytes.
 */
typedef struct {
    const uint8_t *ptr;
    size_t         len;
    uint32_t       hot_meta;
    bool           is_inline;
} ExpanseBlobView;

typedef ExpanseBlobView expanse_blob_view_t;

/*
 * Scan callbacks. Both must return normally: throwing a C++ exception through
 * one, or `longjmp`-ing out of it, unwinds through Rust frames mid traversal,
 * runs no destructor, and leaves the map in a state that satisfies none of its
 * invariants, with no diagnostic. Undefined behaviour, undetectable by the
 * library; catch inside the callback. A Rust panic in a callback aborts the
 * process instead of unwinding, which is the defined outcome.
 */
typedef bool (*expanse_predicate_fn)(uint64_t key, uint32_t hot_meta, void *user_ctx);
typedef bool (*expanse_scan_cb_fn)(uint64_t key, ExpanseBlobView view, void *user_ctx);

/* chunk_size 0 selects the default 2 MiB chunk; the capacity cap is 1 GiB. */
ExpanseBlobMap *expanse_blob_map_new(size_t chunk_size);
/*
 * As expanse_blob_map_new, with a capacity cap of max_capacity bytes of
 * allocated chunks, dead and live (0 selects the default 1 GiB). The cap is
 * clamped to [chunk_size, 64 GiB] and fixed for the map's life. It bounds the
 * arena, not the process: a compaction holds the old and the new chunks at
 * once, plus 16 bytes per entry. The reclaim rule applies at any cap.
 */
ExpanseBlobMap *expanse_blob_map_new_with_capacity(size_t chunk_size, size_t max_capacity);
void            expanse_blob_map_free(ExpanseBlobMap *map);
/*
 * On (the default), an insert the capacity cap refuses may compact the arena
 * under the reclaim rule and retry once; off, it is refused with nothing
 * compacted, and only expanse_blob_map_compact copies payloads. NULL is a no-op.
 */
void expanse_blob_map_set_reclaim_at_cap(ExpanseBlobMap *map, bool on);

/*
 * Metadata: a value is stored inline in its slot word, which has no metadata
 * field, only when hot_meta is 0 (a payload of 7 bytes or fewer, or one the
 * codec compresses into the slot word). A payload of any length inserted with
 * non-zero hot_meta is stored in the arena and keeps it (issue #1548).
 *
 * Returns false when the insert is refused: hot_meta wider than 24 bits (at
 * any payload length), a payload larger than a chunk, an allocation failure, or an arena at its
 * capacity cap that the reclaim rule cannot free enough of. At the cap the
 * insert may first compact the arena (see the view contract above), which
 * copies every live payload; the map's contents are unchanged on refusal.
 * expanse_blob_map_insert_ex returns which of these it was.
 */
bool expanse_blob_map_insert(
    ExpanseBlobMap *map,
    uint64_t key,
    const uint8_t *data,
    size_t len,
    uint32_t hot_meta
);

/*
 * STATUS CODES of expanse_blob_map_insert_ex, expanse_blob_map_arena_stats
 * and the shared blob map's expanse_sync_blob_map_* calls (below), in bands.
 * New values may be added within a band; existing values never change.
 * expanse_blob_map_insert_ex returns none of NOT_FOUND, BUFFER_TOO_SMALL,
 * MISMATCH and NO_META_FIELD.
 */
typedef enum {
    EXPANSE_BLOB_OK                = 0,
    /* outcomes of a shared-map read (expanse_sync_blob_map_*get_into) */
    EXPANSE_BLOB_NOT_FOUND         = 1,  /* key absent; nothing written */
    EXPANSE_BLOB_BUFFER_TOO_SMALL  = 2,  /* key present, payload longer than buf_len: nothing
                                            copied, required length and metadata reported */
    /* outcome of a shared-map read-modify-write (expanse_sync_blob_map_compare_exchange*,
       _get_or_insert): nothing stored, the current value reported */
    EXPANSE_BLOB_MISMATCH          = 3,
    /* refusals: the map's contents are unchanged */
    EXPANSE_BLOB_META_OVERFLOW     = 16, /* hot_meta wider than 24 bits */
    EXPANSE_BLOB_ALLOCATION_FAILED = 17, /* payload larger than a chunk, or out of memory */
    EXPANSE_BLOB_CAP_REFUSED       = 18, /* the cap refused a chunk; nothing was compacted */
    EXPANSE_BLOB_ARENA_FULL        = 19, /* the cap refused a chunk after this insert compacted */
    EXPANSE_BLOB_NO_META_FIELD     = 20, /* metadata offered to a value with no metadata field */
    /* usage errors: nothing was done */
    EXPANSE_BLOB_INVALID_ARGUMENT  = 32, /* NULL handle or stats, NULL data with len > 0, ... */
    /* any other engine error; insert returns none today */
    EXPANSE_BLOB_ERROR             = 48
} expanse_blob_status_t;

/*
 * expanse_blob_map_insert with the reason for a refusal. CAP_REFUSED: no
 * compaction ran (the reclaim rule declined, or it is off), so dead bytes may
 * remain; allocated_bytes - live_bytes from expanse_blob_map_arena_stats bounds
 * what expanse_blob_map_compact can free. ARENA_FULL: this insert compacted
 * the arena (every view is invalidated) and the record still does not fit, so
 * only a removal makes room. Any refusal leaves the contents unchanged.
 */
expanse_blob_status_t expanse_blob_map_insert_ex(
    ExpanseBlobMap *map,
    uint64_t key,
    const uint8_t *data,
    size_t len,
    uint32_t hot_meta
);

/* Append-only; pass sizeof(*stats) and receive the prefix you know. */
typedef struct {
    uint64_t live_bytes;      /* live payload bytes plus an 8-byte header each */
    uint64_t allocated_bytes; /* chunk bytes allocated, dead and live: what the cap counts */
    uint64_t max_capacity;    /* the capacity cap, as clamped */
    uint64_t chunk_size;
    uint64_t reclaim_at_cap;  /* 1 if the reclaim at the cap is on, else 0 */
} expanse_blob_arena_stats_t;

/* OK | INVALID_ARGUMENT (map or stats NULL). */
expanse_blob_status_t expanse_blob_map_arena_stats(const ExpanseBlobMap *map,
                                                   expanse_blob_arena_stats_t *stats,
                                                   size_t stats_size);

bool expanse_blob_map_remove(ExpanseBlobMap *map, uint64_t key);

bool expanse_blob_map_get(
    const ExpanseBlobMap *map,
    uint64_t key,
    ExpanseBlobView *out_view
);

/*
 * Copies the whole payload into buf, or nothing. Returns true only when the key
 * is present and the payload fits in buf_len bytes: buf[0..*out_len) holds it,
 * the rest of buf is untouched, *out_meta is the hot metadata (0 for inline
 * values). Returns false when the key is absent (*out_len = 0, *out_meta
 * untouched) or when buf is too short: then buf is untouched, *out_len is the
 * required length (> buf_len) and *out_meta is the hot metadata. Tell the two
 * apart with (*out_len > buf_len) and retry with a buffer of *out_len bytes.
 * buf == NULL counts as length 0, so NULL with buf_len 0 queries the length.
 * A zero-length payload is present and fits any buffer. out_len and out_meta
 * may be NULL.
 */
bool expanse_blob_map_get_into(
    const ExpanseBlobMap *map,
    uint64_t key,
    uint8_t *buf,
    size_t buf_len,
    size_t *out_len,
    uint32_t *out_meta
);

size_t expanse_blob_map_scan_filtered(
    const ExpanseBlobMap *map,
    uint64_t start_key,
    uint64_t end_key,
    expanse_predicate_fn predicate,
    expanse_scan_cb_fn callback,
    void *user_ctx
);

bool     expanse_blob_map_compact(ExpanseBlobMap *map);
uint64_t expanse_blob_map_len(const ExpanseBlobMap *map);
size_t   expanse_blob_map_mem_used(const ExpanseBlobMap *map);
void     expanse_blob_map_clear(ExpanseBlobMap *map);
bool     expanse_blob_map_contains_key(const ExpanseBlobMap *map, uint64_t key);
bool     expanse_blob_map_contains(const ExpanseBlobMap *map, uint64_t key);

/*
 * expanse_sync_blob_map_t: the concurrent uint64_t -> blob map (the shared
 * form of ExpanseBlobMap above). Same protocol, thread rules and reader-handle
 * ownership as expanse_sync_map_t: the container handle is safe from any
 * number of threads at once except _free, which needs no call in progress and
 * every reader freed first; a reader handle is used by one thread at a time
 * (its calls take it non-const for that reason), may be freed from another
 * thread once no call on it is in progress, and is freed before its map.
 *
 * VALUES ARE COPIED, NEVER BORROWED. No call returns a pointer into the map:
 * _get_into copies the payload into the caller's buffer within one call, so
 * nothing a C caller holds is invalidated by a later insert, remove or
 * compaction. Insert and its refusals are those of expanse_blob_map_insert_ex
 * (OK, META_OVERFLOW, ALLOCATION_FAILED, CAP_REFUSED, ARENA_FULL;
 * INVALID_ARGUMENT for a NULL map, NULL data with len > 0, or len above
 * PTRDIFF_MAX). Hot metadata is the 24-bit field of an arena-stored value. A
 * value is stored in its slot word, without that field, only when hot_meta is
 * 0 (a payload of 7 bytes or fewer, or one the map compresses into the slot
 * word), and reads back metadata 0. A payload of any length inserted with
 * non-zero hot_meta is arena-stored and keeps it, and META_OVERFLOW refuses
 * hot_meta wider than 24 bits at every length, as expanse_blob_map_insert
 * does (issue #1548).
 *
 * _get_into / _reader_get_into, one read each:
 *   OK                key present and its payload fits: it is in
 *                     buf[0 .. *len_out), *meta_out is its metadata, the rest
 *                     of buf is untouched.
 *   NOT_FOUND         key absent; nothing written.
 *   BUFFER_TOO_SMALL  key present, payload longer than buf_len: NOTHING is
 *                     copied, buf is untouched; *len_out is the payload's
 *                     length and *meta_out its metadata, both from the same
 *                     read that found the key.
 *   INVALID_ARGUMENT  NULL map or reader, NULL buf with buf_len > 0, or
 *                     buf_len above PTRDIFF_MAX; nothing written.
 * NULL buf with buf_len 0 is the length probe (OK only for an empty payload).
 * len_out and meta_out may be NULL. A retry with a buffer of *len_out bytes
 * is a new read: it succeeds unless a writer replaced or removed the value in
 * between, when it reports the new state (BUFFER_TOO_SMALL with the new
 * length, or NOT_FOUND), so a caller loops until OK or NOT_FOUND.
 *
 * _get_meta / _contains report presence as bool (_get_meta writes the
 * metadata on true, nothing on false). The one-shot _get_into, _get_meta and
 * _contains on the map register and drop a reader per call; hot loops take a
 * reader handle.
 *
 * Memory: _mem_used and _mem_held read with writers excluded and wait for a
 * running compaction; _mem_used_estimate takes no lock and waits for nothing.
 * _shrink_to_fit returns the collector's freed index blocks to the system.
 * _set_reclaim_at_cap and _compact are those of expanse_blob_map_* above;
 * _compact returns OK, a refusal, or INVALID_ARGUMENT for a NULL map, and
 * readers keep reading while it copies.
 */
typedef struct expanse_sync_blob_map expanse_sync_blob_map_t;
typedef struct expanse_sync_blob_map_reader expanse_sync_blob_map_reader_t;

/* chunk_size and max_capacity as expanse_blob_map_new / _new_with_capacity. */
expanse_sync_blob_map_t *expanse_sync_blob_map_new(size_t chunk_size);
expanse_sync_blob_map_t *expanse_sync_blob_map_new_with_capacity(size_t chunk_size,
                                                                 size_t max_capacity);
void expanse_sync_blob_map_free(expanse_sync_blob_map_t *map);
expanse_blob_status_t expanse_sync_blob_map_insert(expanse_sync_blob_map_t *map, uint64_t key,
                                                   const uint8_t *data, size_t len,
                                                   uint32_t hot_meta);
bool expanse_sync_blob_map_remove(expanse_sync_blob_map_t *map, uint64_t key);
expanse_blob_status_t expanse_sync_blob_map_get_into(const expanse_sync_blob_map_t *map,
                                                     uint64_t key, uint8_t *buf, size_t buf_len,
                                                     size_t *len_out, uint32_t *meta_out);
bool     expanse_sync_blob_map_get_meta(const expanse_sync_blob_map_t *map, uint64_t key,
                                        uint32_t *meta_out);
bool     expanse_sync_blob_map_contains(const expanse_sync_blob_map_t *map, uint64_t key);
uint64_t expanse_sync_blob_map_len(const expanse_sync_blob_map_t *map);
/* Waits for a running compaction and makes writers wait; poll with _mem_used_estimate. */
size_t   expanse_sync_blob_map_mem_used(const expanse_sync_blob_map_t *map);
/* No lock, no wait; may lag a running compaction (see expanse_sync(3)). */
size_t   expanse_sync_blob_map_mem_used_estimate(const expanse_sync_blob_map_t *map);
/* Waits for a running compaction and makes writers wait; has no non-blocking form. */
size_t   expanse_sync_blob_map_mem_held(const expanse_sync_blob_map_t *map);
size_t   expanse_sync_blob_map_shrink_to_fit(const expanse_sync_blob_map_t *map);
void     expanse_sync_blob_map_set_reclaim_at_cap(expanse_sync_blob_map_t *map, bool on);
expanse_blob_status_t expanse_sync_blob_map_compact(expanse_sync_blob_map_t *map);

/*
 * Atomic read-modify-write on one key; each call is one linearizable step.
 * A value is its payload bytes AND its metadata, and compare_exchange compares
 * both. "Absent" is a flag: *_present == false ignores the data, length and
 * metadata beside it; expected absent stores only into an absent key, new
 * absent removes the key.
 *
 * compare_exchange / get_or_insert:
 *   OK                stored (or, for compare_exchange with new absent,
 *                     removed). Nothing is written: the value compare_exchange
 *                     replaced is the caller's expected, and the value
 *                     get_or_insert stored is the caller's data.
 *   MISMATCH          nothing stored. compare_exchange: the current value is
 *                     not expected; *observed_present_out says whether the
 *                     key is present. get_or_insert: the key is present and
 *                     untouched. A present value's payload is in
 *                     buf[0 .. *len_out) and its metadata in *meta_out; an
 *                     absent one writes 0 to both.
 *   BUFFER_TOO_SMALL  a MISMATCH whose present payload is longer than
 *                     buf_len: NOTHING is copied, buf is untouched; *len_out
 *                     and *meta_out (and *observed_present_out) are written,
 *                     from the same step that compared. Nothing was stored:
 *                     get_or_insert writes the buffer only when the key was
 *                     present, so a too-small buffer never accompanies an
 *                     insert.
 *   refusal band      the current value WAS expected (get_or_insert: the key
 *                     was absent), but new cannot be stored: META_OVERFLOW,
 *                     ALLOCATION_FAILED, CAP_REFUSED (these calls never run
 *                     the reclaim at the cap that _insert runs; _compact and
 *                     a retry is the caller's choice), ARENA_FULL. The map is
 *                     unchanged and nothing is written.
 *   INVALID_ARGUMENT  NULL map, a present value with NULL data and len > 0
 *                     or len above PTRDIFF_MAX, NULL buf with buf_len > 0, or
 *                     buf_len above PTRDIFF_MAX; nothing done.
 * NULL buf with buf_len 0 is valid (MISMATCH only for an empty payload). The
 * outputs may be NULL. buf may be the buffer passed as expected_data or data:
 * the payload is copied out after the comparison, so a retry loop can reuse
 * one buffer. A retry loop retries on MISMATCH (and on BUFFER_TOO_SMALL,
 * after growing buf) and stops on OK or a refusal; a refusal is reported only
 * when the comparison held, so the loop ends.
 *
 * Metadata follows insert (above): a value reads back the hot_meta it was
 * stored with at every payload length, so expecting it matches; only a
 * value stored with hot_meta 0 sits in its slot word (issue #1548).
 *
 * compare_exchange_meta swaps the 24-bit metadata in the slot with no payload
 * copy and no allocation; it is the form for a retry loop whose state lives
 * in the metadata (a sequence number, a status tag). The current metadata of
 * a present key is its field, or 0 for a payload stored in its slot word.
 * OK: swapped (or, new absent, removed). MISMATCH: *observed_present_out and,
 * when present, *observed_meta_out written. META_OVERFLOW: new_meta is wider
 * than 24 bits. NO_META_FIELD: the comparison held but the value has no field:
 * the key is absent (metadata alone cannot create it) or new_meta is non-zero
 * for a payload stored in its slot word. INVALID_ARGUMENT: NULL map. A refusal
 * writes nothing.
 *
 *     uint32_t meta = 0; bool present = expanse_sync_blob_map_get_meta(m, k, &meta);
 *     for (;;) {
 *         bool seen_present; uint32_t seen;
 *         expanse_blob_status_t st = expanse_sync_blob_map_compare_exchange_meta(
 *             m, k, present, meta, true, meta + 1, &seen_present, &seen);
 *         if (st != EXPANSE_BLOB_MISMATCH) break;   // OK, or a refusal
 *         present = seen_present; meta = seen;      // retry on the observed metadata
 *     }
 */
expanse_blob_status_t expanse_sync_blob_map_compare_exchange(
    expanse_sync_blob_map_t *map, uint64_t key, bool expected_present,
    const uint8_t *expected_data, size_t expected_len, uint32_t expected_meta, bool new_present,
    const uint8_t *new_data, size_t new_len, uint32_t new_meta, bool *observed_present_out,
    uint8_t *buf, size_t buf_len, size_t *len_out, uint32_t *meta_out);
expanse_blob_status_t expanse_sync_blob_map_compare_exchange_meta(
    expanse_sync_blob_map_t *map, uint64_t key, bool expected_present, uint32_t expected_meta,
    bool new_present, uint32_t new_meta, bool *observed_present_out, uint32_t *observed_meta_out);
expanse_blob_status_t expanse_sync_blob_map_get_or_insert(expanse_sync_blob_map_t *map,
                                                          uint64_t key, const uint8_t *data,
                                                          size_t len, uint32_t hot_meta,
                                                          uint8_t *buf, size_t buf_len,
                                                          size_t *len_out, uint32_t *meta_out);

expanse_sync_blob_map_reader_t *expanse_sync_blob_map_reader_new(
    const expanse_sync_blob_map_t *map);
void expanse_sync_blob_map_reader_free(expanse_sync_blob_map_reader_t *reader);
expanse_blob_status_t expanse_sync_blob_map_reader_get_into(expanse_sync_blob_map_reader_t *reader,
                                                            uint64_t key, uint8_t *buf,
                                                            size_t buf_len, size_t *len_out,
                                                            uint32_t *meta_out);
bool expanse_sync_blob_map_reader_get_meta(expanse_sync_blob_map_reader_t *reader, uint64_t key,
                                           uint32_t *meta_out);
bool expanse_sync_blob_map_reader_contains(expanse_sync_blob_map_reader_t *reader, uint64_t key);


#endif /* EXPANSE_WIDE_SURFACE */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* EXPANSE_H */
