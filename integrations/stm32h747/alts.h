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

/* Live requested-bytes accounting shared by every implementation (the
 * expanse_host_* hooks route through it too): allocator overhead excluded
 * symmetrically, so bytes/key compares data-structure footprints. */
void *acct_malloc(size_t n);
void acct_free(void *p);
size_t acct_live(void);
size_t acct_peak(void);
void acct_reset_peak(void);

#endif
