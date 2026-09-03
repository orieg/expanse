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
#endif /* EXPANSE_WIDE_SURFACE */


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
                            expanse_word_t value, expanse_word_t *old_out);
bool     expanse_map_get(const expanse_map_t *map, expanse_word_t key, expanse_word_t *value_out);
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
 * walk. The callback must not mutate `map` during the walk.
 */
/**
 * Callback for expanse_map_remove_many(): one call per removed entry, in
 * ascending key order, with the caller's context pointer.
 */
typedef void (*expanse_map_remove_many_fn)(expanse_word_t key, expanse_word_t value,
                                           void *user_ctx);

/**
 * Removes a sorted, distinct set of scattered keys in one pass.
 *
 * expanse_map_remove_range() covers a contiguous interval; this covers a set
 * that shares none -- the retired half of a hash-keyed index, say. Removing
 * such a set one key at a time pays a fresh root descent per key, where this
 * visits each node on the path once however many keys pass through it.
 *
 * `keys` must be sorted ascending and distinct; an unsorted array removes only
 * what it finds in the runs it happens to form. Keys absent from the map are
 * skipped, so the return counts removals and not requests. `callback` may be
 * NULL, and must not mutate `map` (the call holds it).
 */
size_t expanse_map_remove_many(expanse_map_t *map, const expanse_word_t *keys, size_t len,
                               expanse_map_remove_many_fn callback, void *user_ctx);

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
#endif /* EXPANSE_WIDE_SURFACE */



#if EXPANSE_WIDE_SURFACE
/* ---- expanse_bytesmap_t: unordered bytes -> uint64_t (cf. JudyHS) --- */

typedef struct expanse_bytesmap expanse_bytesmap_t;

expanse_bytesmap_t *expanse_bytesmap_new(void);
void                expanse_bytesmap_free(expanse_bytesmap_t *map);

bool     expanse_bytesmap_insert(expanse_bytesmap_t *map, const void *key, size_t len,
                                 uint64_t value, uint64_t *old_out);
bool     expanse_bytesmap_get(const expanse_bytesmap_t *map, const void *key, size_t len,
                              uint64_t *value_out);
bool     expanse_bytesmap_remove(expanse_bytesmap_t *map, const void *key, size_t len,
                                 uint64_t *old_out);
uint64_t *expanse_bytesmap_slot(expanse_bytesmap_t *map, const void *key, size_t len);
uint64_t *expanse_bytesmap_ins_slot(expanse_bytesmap_t *map, const void *key, size_t len);
uint64_t expanse_bytesmap_len(const expanse_bytesmap_t *map);
size_t   expanse_bytesmap_mem_used(const expanse_bytesmap_t *map);
void     expanse_bytesmap_clear(expanse_bytesmap_t *map);

/* ---- expanse_strmap_t: ordered C-string -> uint64_t map (cf. JudySL) - */

typedef struct expanse_strmap expanse_strmap_t;

expanse_strmap_t *expanse_strmap_new(void);
void              expanse_strmap_free(expanse_strmap_t *map);

bool     expanse_strmap_insert(expanse_strmap_t *map, const char *key, uint64_t value,
                               uint64_t *old_out);
bool     expanse_strmap_get(const expanse_strmap_t *map, const char *key, uint64_t *value_out);
bool     expanse_strmap_remove(expanse_strmap_t *map, const char *key, uint64_t *old_out);
uint64_t *expanse_strmap_slot(expanse_strmap_t *map, const char *key);
uint64_t *expanse_strmap_ins_slot(expanse_strmap_t *map, const char *key);
uint64_t expanse_strmap_len(const expanse_strmap_t *map);
size_t   expanse_strmap_mem_used(const expanse_strmap_t *map);
void     expanse_strmap_clear(expanse_strmap_t *map);

/*
 * Ordered string navigation. The found key (NUL-terminated) is written to
 * `key_out` (up to `buf_len` bytes including NUL). Returns false if no key
 * is found or if `buf_len` is insufficient.
 */
bool expanse_strmap_first(expanse_strmap_t *map, char *key_out, size_t buf_len,
                          uint64_t *value_out);
bool expanse_strmap_last(expanse_strmap_t *map, char *key_out, size_t buf_len,
                         uint64_t *value_out);
bool expanse_strmap_next_at_or_after(expanse_strmap_t *map, const char *key,
                                     char *key_out, size_t buf_len, uint64_t *value_out);
bool expanse_strmap_next_after(expanse_strmap_t *map, const char *key,
                               char *key_out, size_t buf_len, uint64_t *value_out);
bool expanse_strmap_prev_at_or_before(expanse_strmap_t *map, const char *key,
                                      char *key_out, size_t buf_len, uint64_t *value_out);
bool expanse_strmap_prev_before(expanse_strmap_t *map, const char *key,
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

expanse_str_nav_status expanse_strmap_first_ex(expanse_strmap_t *map,
                                               char *key_out, size_t buf_len,
                                               size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_last_ex(expanse_strmap_t *map,
                                              char *key_out, size_t buf_len,
                                              size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_next_at_or_after_ex(expanse_strmap_t *map, const char *key,
                                                          char *key_out, size_t buf_len,
                                                          size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_next_after_ex(expanse_strmap_t *map, const char *key,
                                                    char *key_out, size_t buf_len,
                                                    size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_prev_at_or_before_ex(expanse_strmap_t *map, const char *key,
                                                           char *key_out, size_t buf_len,
                                                           size_t *required_len, uint64_t *value_out);
expanse_str_nav_status expanse_strmap_prev_before_ex(expanse_strmap_t *map, const char *key,
                                                     char *key_out, size_t buf_len,
                                                     size_t *required_len, uint64_t *value_out);

/* ---- Concurrent types: one writer, optimistic readers ---------------- */

/*
 * The capability classic Judy has no answer for. Writers serialize
 * internally; readers run an optimistic validated walk with epoch-based
 * reclamation, so a reader never blocks a writer or dereferences freed
 * memory (docs/ARCHITECTURE.md §4, docs/BENCHMARKING.md for scaling).
 *
 * A handle is safe to use from any number of threads at once. Readers
 * SHOULD take a reader handle (expanse_sync_*_reader_new) once per
 * thread and reuse it: the one-shot _contains/_get calls register and
 * drop a reader on every call. A reader handle belongs to the thread
 * that created it and must be freed before its parent container.
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

expanse_sync_map_reader_t *expanse_sync_map_reader_new(const expanse_sync_map_t *map);
void                       expanse_sync_map_reader_free(expanse_sync_map_reader_t *reader);
bool expanse_sync_map_reader_get(const expanse_sync_map_reader_t *reader, uint64_t key,
                                 uint64_t *value_out);

/* ---- ExpanseBlobMap: polymorphic large-value map with inline/arena backing ---- */

typedef struct ExpanseBlobMap ExpanseBlobMap;

/*
 * A zero-copy view of a stored payload.
 *
 * INVALIDATION CONTRACT (same spirit as the JudyL value-slot contract):
 * `ptr` borrows directly into the map's inline slot memory or arena slab and
 * stays valid ONLY until the next structural mutation of that map. Any
 * expanse_blob_map_insert / _remove / _clear / _compact / _free invalidates
 * every previously returned ExpanseBlobView.ptr — reading through it afterwards
 * is undefined behavior. Copy the bytes out before mutating if you need them to
 * outlive the next mutation. Views delivered to a scan callback are valid only
 * for the duration of that callback invocation.
 */
typedef struct {
    const uint8_t *ptr;
    size_t         len;
    uint32_t       hot_meta;
    bool           is_inline;
} ExpanseBlobView;

typedef bool (*expanse_predicate_fn)(uint64_t key, uint32_t hot_meta, void *user_ctx);
typedef bool (*expanse_scan_cb_fn)(uint64_t key, ExpanseBlobView view, void *user_ctx);

ExpanseBlobMap *expanse_blob_map_new(size_t chunk_size);
void            expanse_blob_map_free(ExpanseBlobMap *map);

bool expanse_blob_map_insert(
    ExpanseBlobMap *map,
    uint64_t key,
    const uint8_t *data,
    size_t len,
    uint32_t hot_meta
);

bool expanse_blob_map_remove(ExpanseBlobMap *map, uint64_t key);

bool expanse_blob_map_get(
    const ExpanseBlobMap *map,
    uint64_t key,
    ExpanseBlobView *out_view
);

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

#endif /* EXPANSE_WIDE_SURFACE */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* EXPANSE_H */
