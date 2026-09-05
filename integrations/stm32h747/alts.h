/* Alternative ordered/unordered maps for the on-target comparison, behind one
 * vtable so every fixture runs the same code path for every implementation.
 * Keys and values are 32-bit words, matching the narrow expanse_map_t. */
#ifndef ALTS_H
#define ALTS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef void (*alt_range_cb)(uint32_t key, uint32_t value, void *ctx);

typedef struct {
    const char *name;
    bool ordered;                 /* first/remove_range are meaningful */
    void *(*create)(size_t cap);  /* cap: upper bound on live keys */
    void (*destroy)(void *m);
    void (*insert)(void *m, uint32_t k, uint32_t v);
    bool (*get)(void *m, uint32_t k, uint32_t *v);
    bool (*first)(void *m, uint32_t *k, uint32_t *v);
    void (*remove)(void *m, uint32_t k);
    size_t (*remove_range)(void *m, uint32_t lo, uint32_t hi, alt_range_cb cb, void *ctx);
    /* Unordered structures evict by scanning every live entry and asking the
     * predicate; returns the number removed. NULL for ordered structures. */
    size_t (*evict_scan)(void *m, bool (*expired)(uint32_t k, uint32_t v, void *ctx), void *ctx);
} alt_ops;

extern const alt_ops alt_expanse, alt_sorted_array, alt_open_hash, alt_tsearch;

/* Requested bytes held by an open-addressing table (header + key and value
 * arrays), for a live bytes-per-record readout; the LCD demo uses it. */
size_t alt_open_hash_bytes(const void *m);
/* Insert into an open-addressing table that is allowed to grow: when the
 * insert would take the load past 50% the table doubles first, moving every
 * live entry, and the DWT cycles that took are returned in *grow_cycles (0
 * when no doubling happened). The suite's fixtures pre-size their tables and
 * never call this; the LCD demo's growth step does. */
void alt_open_hash_insert_grow(void *m, uint32_t k, uint32_t v, uint32_t *grow_cycles);
/* Live entries and slot count, for a load-factor readout. */
void alt_open_hash_shape(const void *m, size_t *len, size_t *slots);

/* Live requested-bytes accounting shared by every implementation (the
 * expanse_host_* hooks route through it too): allocator overhead excluded
 * symmetrically, so bytes/key compares data-structure footprints. */
void *acct_malloc(size_t n);
void acct_free(void *p);
size_t acct_live(void);
size_t acct_peak(void);
void acct_reset_peak(void);

#endif
