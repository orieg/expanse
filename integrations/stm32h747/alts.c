/* Alternatives for the on-target comparison (see alts.h). Each is what a
 * firmware engineer would actually reach for, written plainly:
 *   sorted_array  parallel key/value arrays, bsearch + memmove
 *   open_hash     open addressing, linear probing, FNV-1a mix, <= 50% load
 *   tsearch       newlib's <search.h> binary tree (unbalanced), the ordered
 *                 structure the toolchain ships for free
 * Expanse itself is wrapped behind the same vtable. */
#include <search.h>
#include <stdlib.h>
#include <string.h>
#include <malloc.h>
#include "alts.h"
#include "expanse.h"

/* ---- accounting ------------------------------------------------------- */
static size_t live_bytes, peak_bytes;
void *acct_malloc(size_t n) {
    void *p = malloc(n);
    if (p) { live_bytes += n; if (live_bytes > peak_bytes) peak_bytes = live_bytes; }
    return p;
}
void acct_free(void *p) {
    if (!p) return;
    size_t n = malloc_usable_size(p);
    /* usable size >= requested; track requested by storing nothing extra:
     * newlib rounds to 8, so this over-subtracts by < 8 bytes per free.
     * Clamp so a long free sequence cannot underflow. */
    live_bytes = live_bytes > n ? live_bytes - n : 0;
    free(p);
}
size_t acct_live(void) { return live_bytes; }
size_t acct_peak(void) { return peak_bytes; }
void acct_reset_peak(void) { peak_bytes = live_bytes; }

/* ---- expanse ---------------------------------------------------------- */
static void *ex_create(size_t cap) { (void)cap; return expanse_map_new(); }
static void ex_destroy(void *m) { expanse_map_free(m); }
static void ex_insert(void *m, uint32_t k, uint32_t v) { expanse_word_t o; expanse_map_insert(m, k, v, &o); }
static bool ex_get(void *m, uint32_t k, uint32_t *v) { return expanse_map_get(m, k, v); }
static bool ex_first(void *m, uint32_t *k, uint32_t *v) { return expanse_map_first(m, k, v); }
static void ex_remove(void *m, uint32_t k) { expanse_word_t o; expanse_map_remove(m, k, &o); }
static size_t ex_remove_range(void *m, uint32_t lo, uint32_t hi, alt_range_cb cb, void *ctx) {
    return expanse_map_remove_range(m, lo, hi, cb, ctx);
}
const alt_ops alt_expanse = { "expanse", true, ex_create, ex_destroy, ex_insert, ex_get, ex_first,
                              ex_remove, ex_remove_range, 0 };

/* ---- sorted array ----------------------------------------------------- */
typedef struct { uint32_t *keys, *vals; size_t len, cap; } sarr;
static size_t sa_lower(const sarr *s, uint32_t k) { /* first index with key >= k */
    size_t lo = 0, hi = s->len;
    while (lo < hi) { size_t mid = (lo + hi) / 2; if (s->keys[mid] < k) lo = mid + 1; else hi = mid; }
    return lo;
}
static void *sa_create(size_t cap) {
    sarr *s = acct_malloc(sizeof *s); s->len = 0; s->cap = cap;
    s->keys = acct_malloc(cap * 4); s->vals = acct_malloc(cap * 4); return s;
}
static void sa_destroy(void *m) { sarr *s = m; acct_free(s->keys); acct_free(s->vals); acct_free(s); }
static void sa_insert(void *m, uint32_t k, uint32_t v) {
    sarr *s = m; size_t i = sa_lower(s, k);
    if (i < s->len && s->keys[i] == k) { s->vals[i] = v; return; }
    memmove(&s->keys[i + 1], &s->keys[i], (s->len - i) * 4);
    memmove(&s->vals[i + 1], &s->vals[i], (s->len - i) * 4);
    s->keys[i] = k; s->vals[i] = v; s->len++;
}
static bool sa_get(void *m, uint32_t k, uint32_t *v) {
    sarr *s = m; size_t i = sa_lower(s, k);
    if (i < s->len && s->keys[i] == k) { *v = s->vals[i]; return true; }
    return false;
}
static bool sa_first(void *m, uint32_t *k, uint32_t *v) {
    sarr *s = m; if (!s->len) return false; *k = s->keys[0]; *v = s->vals[0]; return true;
}
static void sa_erase(sarr *s, size_t i, size_t n) {
    memmove(&s->keys[i], &s->keys[i + n], (s->len - i - n) * 4);
    memmove(&s->vals[i], &s->vals[i + n], (s->len - i - n) * 4);
    s->len -= n;
}
static void sa_remove(void *m, uint32_t k) {
    sarr *s = m; size_t i = sa_lower(s, k);
    if (i < s->len && s->keys[i] == k) sa_erase(s, i, 1);
}
static size_t sa_remove_range(void *m, uint32_t lo, uint32_t hi, alt_range_cb cb, void *ctx) {
    sarr *s = m; size_t a = sa_lower(s, lo), b = hi == 0xFFFFFFFFu ? s->len : sa_lower(s, hi + 1);
    for (size_t i = a; i < b; i++) cb(s->keys[i], s->vals[i], ctx);
    sa_erase(s, a, b - a);
    return b - a;
}
const alt_ops alt_sorted_array = { "sorted_array", true, sa_create, sa_destroy, sa_insert, sa_get,
                                   sa_first, sa_remove, sa_remove_range, 0 };

/* ---- open-addressing hash --------------------------------------------- */
#define OH_EMPTY 0xFFFFFFFFu
#define OH_TOMB  0xFFFFFFFEu
typedef struct { uint32_t *keys, *vals; size_t mask, len, used; } ohash;
static inline uint32_t oh_mix(uint32_t k) { /* FNV-1a over the 4 key bytes, same constants as the fixtures */
    uint32_t h = 2166136261u;
    for (int i = 0; i < 4; i++) { h ^= (k >> (8 * i)) & 0xFF; h *= 16777619u; }
    return h;
}
static void *oh_create(size_t cap) {
    size_t n = 16; while (n < cap * 2) n <<= 1;          /* load factor <= 50% */
    ohash *h = acct_malloc(sizeof *h); h->mask = n - 1; h->len = h->used = 0;
    h->keys = acct_malloc(n * 4); h->vals = acct_malloc(n * 4);
    for (size_t i = 0; i < n; i++) h->keys[i] = OH_EMPTY;
    return h;
}
static void oh_destroy(void *m) { ohash *h = m; acct_free(h->keys); acct_free(h->vals); acct_free(h); }
static void oh_insert(void *m, uint32_t k, uint32_t v) {
    ohash *h = m; size_t i = oh_mix(k) & h->mask, tomb = (size_t)-1;
    for (;;) {
        uint32_t c = h->keys[i];
        if (c == k) { h->vals[i] = v; return; }
        if (c == OH_TOMB && tomb == (size_t)-1) tomb = i;
        if (c == OH_EMPTY) { if (tomb != (size_t)-1) i = tomb; else h->used++; h->keys[i] = k; h->vals[i] = v; h->len++; return; }
        i = (i + 1) & h->mask;
    }
}
static bool oh_get(void *m, uint32_t k, uint32_t *v) {
    ohash *h = m; size_t i = oh_mix(k) & h->mask;
    for (;;) {
        uint32_t c = h->keys[i];
        if (c == k) { *v = h->vals[i]; return true; }
        if (c == OH_EMPTY) return false;
        i = (i + 1) & h->mask;
    }
}
static void oh_remove(void *m, uint32_t k) {
    ohash *h = m; size_t i = oh_mix(k) & h->mask;
    for (;;) {
        uint32_t c = h->keys[i];
        if (c == k) { h->keys[i] = OH_TOMB; h->len--; return; }
        if (c == OH_EMPTY) return;
        i = (i + 1) & h->mask;
    }
}
static size_t oh_evict_scan(void *m, bool (*expired)(uint32_t, uint32_t, void *), void *ctx) {
    ohash *h = m; size_t n = 0;
    for (size_t i = 0; i <= h->mask; i++) {
        uint32_t c = h->keys[i];
        if (c != OH_EMPTY && c != OH_TOMB && expired(c, h->vals[i], ctx)) { h->keys[i] = OH_TOMB; h->len--; n++; }
    }
    return n;
}
size_t alt_open_hash_bytes(const void *m) { const ohash *h = m; return sizeof *h + (h->mask + 1) * 8; }
void alt_open_hash_shape(const void *m, size_t *len, size_t *slots) { const ohash *h = m; *len = h->len; *slots = h->mask + 1; }
static inline uint32_t dwt_cyccnt(void) { return *(volatile uint32_t *)0xE0001004u; }
void alt_open_hash_insert_grow(void *m, uint32_t k, uint32_t v, uint32_t *grow_cycles) {
    ohash *h = m; *grow_cycles = 0;
    if ((h->used + 1) * 2 > h->mask + 1) {                      /* would pass 50% load: double */
        uint32_t t0 = dwt_cyccnt();
        size_t n = (h->mask + 1) * 2;
        uint32_t *nk = acct_malloc(n * 4), *nv = acct_malloc(n * 4);
        for (size_t i = 0; i < n; i++) nk[i] = OH_EMPTY;
        for (size_t i = 0; i <= h->mask; i++) {
            uint32_t key = h->keys[i];
            if (key == OH_EMPTY || key == OH_TOMB) continue;
            size_t j = oh_mix(key) & (n - 1);
            while (nk[j] != OH_EMPTY) j = (j + 1) & (n - 1);
            nk[j] = key; nv[j] = h->vals[i];
        }
        acct_free(h->keys); acct_free(h->vals);
        h->keys = nk; h->vals = nv; h->mask = n - 1; h->used = h->len;   /* tombstones dropped */
        *grow_cycles = dwt_cyccnt() - t0;
    }
    oh_insert(h, k, v);
}
const alt_ops alt_open_hash = { "open_hash", false, oh_create, oh_destroy, oh_insert, oh_get, 0,
                                oh_remove, 0, oh_evict_scan };

/* ---- newlib tsearch --------------------------------------------------- */
typedef struct { uint32_t k, v; } tnode;
typedef struct { void *root; size_t len; uint32_t *scratch; size_t scap; } tmap;
static int t_cmp(const void *a, const void *b) {
    uint32_t x = ((const tnode *)a)->k, y = ((const tnode *)b)->k; return x < y ? -1 : x > y;
}
static void *t_create(size_t cap) {
    tmap *t = acct_malloc(sizeof *t); t->root = 0; t->len = 0; t->scap = cap;
    t->scratch = acct_malloc(cap * 4); return t;
}
static void t_free_node(void *n) { acct_free(n); }
static void t_destroy(void *m) {
    tmap *t = m;
    /* tdestroy is glibc-only; delete by repeatedly removing the root. */
    while (t->root) { tnode *r = *(tnode **)t->root; tdelete(r, &t->root, t_cmp); t_free_node(r); }
    acct_free(t->scratch); acct_free(t);
}
static void t_insert(void *m, uint32_t k, uint32_t v) {
    tmap *t = m; tnode probe = { k, v };
    tnode **slot = tsearch(&probe, &t->root, t_cmp);
    if (*slot == &probe) { tnode *n = acct_malloc(sizeof *n); *n = probe; *slot = n; t->len++; }
    else (*slot)->v = v;
}
static bool t_get(void *m, uint32_t k, uint32_t *v) {
    tmap *t = m; tnode probe = { k, 0 }; tnode **slot = tfind(&probe, &t->root, t_cmp);
    if (!slot) return false;
    *v = (*slot)->v;
    return true;
}
static void t_remove(void *m, uint32_t k) {
    tmap *t = m; tnode probe = { k, 0 }; tnode **slot = tfind(&probe, &t->root, t_cmp);
    if (!slot) return;
    tnode *n = *slot;
    tdelete(&probe, &t->root, t_cmp);
    t_free_node(n);
    t->len--;
}
/* newlib has no ordered iterator with early exit; the library's way to find
 * the minimum is a full in-order walk. Descend the leftmost spine instead —
 * that is what tsearch's own implementation stores (node = {key, left, right}). */
typedef struct tnode_s { void *key; struct tnode_s *left, *right; } tnode_s;
static bool t_first(void *m, uint32_t *k, uint32_t *v) {
    tmap *t = m; tnode_s *n = t->root; if (!n) return false;
    while (n->left) n = n->left;
    *k = ((tnode *)n->key)->k; *v = ((tnode *)n->key)->v; return true;
}
static size_t t_remove_range(void *m, uint32_t lo, uint32_t hi, alt_range_cb cb, void *ctx) {
    size_t n = 0; uint32_t k, v;
    while (t_first(m, &k, &v) && k >= lo && k <= hi) { cb(k, v, ctx); t_remove(m, k); n++; }
    return n;
}
const alt_ops alt_tsearch = { "tsearch", true, t_create, t_destroy, t_insert, t_get, t_first,
                              t_remove, t_remove_range, 0 };
