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
 *   - Concurrent readers: one writer plus lock-free readers over the
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
 * or concurrent containers — so those symbols are ABSENT from a 32-bit
 * build rather than present and stubbed, and a link error names the gap.
 * The concurrent types additionally need a std build, so they are absent
 * from every no_std libexpanse. docs/COMPAT.md carries the surface matrix.
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
#if EXPANSE_WIDE_SURFACE
bool expanse_set_next_at_or_after(const expanse_set_t *set, expanse_word_t key,
                                  expanse_word_t *key_out);
bool expanse_set_next_after(const expanse_set_t *set, expanse_word_t key, expanse_word_t *key_out);
bool expanse_set_prev_at_or_before(const expanse_set_t *set, expanse_word_t key,
                                   expanse_word_t *key_out);
bool expanse_set_prev_before(const expanse_set_t *set, expanse_word_t key, expanse_word_t *key_out);

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

/* ---- Concurrent types: one writer, lock-free readers ---------------- */

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
