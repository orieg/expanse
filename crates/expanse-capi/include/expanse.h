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
 *   - Value-slot pointers (uint64_t*) stay valid until the next
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

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Library identity ---------------------------------------------- */

/* Version of the libexpanse build, "MAJOR.MINOR.PATCH". */
const char *expanse_version(void);

/* ---- expanse_set_t: ordered set of uint64_t keys (cf. Judy1) -------- */

typedef struct expanse_set expanse_set_t;

expanse_set_t *expanse_set_new(void);
void           expanse_set_free(expanse_set_t *set);

/* Returns true if the key was newly inserted / actually removed. */
bool     expanse_set_insert(expanse_set_t *set, uint64_t key);
bool     expanse_set_remove(expanse_set_t *set, uint64_t key);
bool     expanse_set_contains(const expanse_set_t *set, uint64_t key);
uint64_t expanse_set_len(const expanse_set_t *set);
size_t   expanse_set_mem_used(const expanse_set_t *set);
void     expanse_set_clear(expanse_set_t *set);

/*
 * Ordered navigation. Each writes the found key through `key_out` and
 * returns true; false means no such key (and `key_out` is untouched).
 * The _at_or_ variants include the given key itself.
 */
bool expanse_set_first(const expanse_set_t *set, uint64_t *key_out);
bool expanse_set_last(const expanse_set_t *set, uint64_t *key_out);
bool expanse_set_next_at_or_after(const expanse_set_t *set, uint64_t key, uint64_t *key_out);
bool expanse_set_next_after(const expanse_set_t *set, uint64_t key, uint64_t *key_out);
bool expanse_set_prev_at_or_before(const expanse_set_t *set, uint64_t key, uint64_t *key_out);
bool expanse_set_prev_before(const expanse_set_t *set, uint64_t key, uint64_t *key_out);

/* Rank and select, both O(depth). */
uint64_t expanse_set_count_below(const expanse_set_t *set, uint64_t key);
uint64_t expanse_set_count_range(const expanse_set_t *set, uint64_t lo, uint64_t hi);
bool     expanse_set_by_count(const expanse_set_t *set, uint64_t n, uint64_t *key_out);

/* ---- expanse_map_t: ordered uint64_t -> uint64_t map (cf. JudyL) ---- */

typedef struct expanse_map expanse_map_t;

expanse_map_t *expanse_map_new(void);
void           expanse_map_free(expanse_map_t *map);

/*
 * insert: stores key -> value. If the key was present, writes the
 * replaced value through `old_out` (when non-NULL) and returns false;
 * returns true when the key is new.
 */
bool     expanse_map_insert(expanse_map_t *map, uint64_t key, uint64_t value, uint64_t *old_out);
bool     expanse_map_get(const expanse_map_t *map, uint64_t key, uint64_t *value_out);
bool     expanse_map_remove(expanse_map_t *map, uint64_t key, uint64_t *old_out);
uint64_t expanse_map_len(const expanse_map_t *map);
size_t   expanse_map_mem_used(const expanse_map_t *map);
void     expanse_map_clear(expanse_map_t *map);

/*
 * Value slots (classic JudyL convention): _slot returns a writable
 * pointer to the stored value, NULL if the key is absent; _ins_slot
 * inserts the key with value 0 if absent (an existing value is kept)
 * and always returns its slot. Valid until the next mutation.
 */
uint64_t *expanse_map_slot(expanse_map_t *map, uint64_t key);
uint64_t *expanse_map_ins_slot(expanse_map_t *map, uint64_t key);

/* Ordered navigation; `value_out` may be NULL if only the key matters. */
bool expanse_map_first(const expanse_map_t *map, uint64_t *key_out, uint64_t *value_out);
bool expanse_map_last(const expanse_map_t *map, uint64_t *key_out, uint64_t *value_out);
bool expanse_map_next_at_or_after(const expanse_map_t *map, uint64_t key,
                                  uint64_t *key_out, uint64_t *value_out);
bool expanse_map_next_after(const expanse_map_t *map, uint64_t key,
                            uint64_t *key_out, uint64_t *value_out);
bool expanse_map_prev_at_or_before(const expanse_map_t *map, uint64_t key,
                                   uint64_t *key_out, uint64_t *value_out);
bool expanse_map_prev_before(const expanse_map_t *map, uint64_t key,
                             uint64_t *key_out, uint64_t *value_out);

uint64_t expanse_map_count_below(const expanse_map_t *map, uint64_t key);
uint64_t expanse_map_count_range(const expanse_map_t *map, uint64_t lo, uint64_t hi);
bool     expanse_map_by_count(const expanse_map_t *map, uint64_t n,
                              uint64_t *key_out, uint64_t *value_out);

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

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* EXPANSE_H */
