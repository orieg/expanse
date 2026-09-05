/* Implementations of the §8.3 comparison baselines. See twin_containers.h. */
#include "twin_containers.h"

#include <stdlib.h>
#include <string.h>

#if defined(ESP_PLATFORM)
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
typedef SemaphoreHandle_t twin_lock_t;
#define TWIN_LOCK_INIT(l)  do { (l) = xSemaphoreCreateRecursiveMutex(); } while (0)
#define TWIN_LOCK_TAKE(l)  do { if (l) xSemaphoreTakeRecursive((l), portMAX_DELAY); } while (0)
#define TWIN_LOCK_GIVE(l)  do { if (l) xSemaphoreGiveRecursive((l)); } while (0)
#define TWIN_LOCK_FREE(l)  do { if (l) vSemaphoreDelete(l); } while (0)
#else
typedef int twin_lock_t;
#define TWIN_LOCK_INIT(l)  do { (l) = 0; } while (0)
#define TWIN_LOCK_TAKE(l)  do { (void)(l); } while (0)
#define TWIN_LOCK_GIVE(l)  do { (void)(l); } while (0)
#define TWIN_LOCK_FREE(l)  do { (void)(l); } while (0)
#endif

/* Sentinel for an empty hash slot. The telemetry workload's keys are Unix
 * seconds around 1.7e9, so 0 is never a live key here; the slot's own
 * occupancy flag is what actually decides, and this only keys the probe. */
#define TWIN_EMPTY 0u

/*
 * The tracker's TTL predicate, mirrored exactly.
 *
 * expanse_ble_tracker_expire_stale builds `max_time_key` from the cutoff's
 * SECOND with the slab-index bits all set, so it retires every record whose
 * rel_sec is <= the cutoff second -- second granularity, inclusive. That
 * falls out of the composite `rel_sec:19 | idx:13` key, not from a choice
 * about TTL semantics.
 *
 * A twin using the obvious `last_seen_ms < cutoff_ms` retires a DIFFERENT
 * SET: on the benchmark's own sighting stream Expanse retired 300 records
 * where the millisecond predicate retired 250. Per-entry eviction costs
 * across arms doing unequal work are not comparable (§8.3), so the twins use
 * the tracker's predicate. The Unity case asserts all three arms agree.
 */
static inline bool twin_is_stale(uint32_t last_seen_ms, uint32_t cutoff_ms) {
    return (last_seen_ms / 1000u) <= (cutoff_ms / 1000u);
}

static void agg_init(expanse_memtable_agg_t *a) {
    a->min_val = UINT32_MAX;
    a->max_val = 0;
    a->sum_val = 0;
    a->count = 0;
}

static void agg_add(expanse_memtable_agg_t *a, uint32_t v) {
    if (v < a->min_val) a->min_val = v;
    if (v > a->max_val) a->max_val = v;
    a->sum_val += v;
    a->count++;
}

/* ---- open-addressing hash ------------------------------------------- */

struct twin_hash {
    uint32_t   *keys;
    uint32_t   *vals;
    uint8_t    *used;
    size_t      mask;      /* capacity - 1; capacity is a power of two */
    size_t      len;
    twin_lock_t lock;
};

/* Fibonacci hashing: one multiply and a shift, which is what a production
 * embedded table uses. Knuth's 2^32/phi. */
static inline size_t twin_hash_slot(const twin_hash_t *h, uint32_t key) {
    return (size_t)((key * 2654435761u) >> 8) & h->mask;
}

twin_hash_t *twin_hash_create(size_t expected_entries) {
    /* Reserved at <= 0.7 load, the realistic configuration (§8.3). */
    size_t want = (expected_entries * 10) / 7 + 1;
    size_t cap = 8;
    while (cap < want) cap <<= 1;

    twin_hash_t *h = (twin_hash_t *)calloc(1, sizeof(*h));
    if (!h) return NULL;
    h->keys = (uint32_t *)calloc(cap, sizeof(uint32_t));
    h->vals = (uint32_t *)calloc(cap, sizeof(uint32_t));
    h->used = (uint8_t *)calloc(cap, sizeof(uint8_t));
    if (!h->keys || !h->vals || !h->used) {
        free(h->keys); free(h->vals); free(h->used); free(h);
        return NULL;
    }
    h->mask = cap - 1;
    TWIN_LOCK_INIT(h->lock);
    return h;
}

void twin_hash_destroy(twin_hash_t *h) {
    if (!h) return;
    TWIN_LOCK_FREE(h->lock);
    free(h->keys); free(h->vals); free(h->used); free(h);
}

bool twin_hash_insert(twin_hash_t *h, uint32_t key, uint32_t value) {
    if (!h) return false;
    TWIN_LOCK_TAKE(h->lock);
    size_t i = twin_hash_slot(h, key);
    for (;;) {
        if (!h->used[i]) {
            h->used[i] = 1; h->keys[i] = key; h->vals[i] = value; h->len++;
            TWIN_LOCK_GIVE(h->lock);
            return true;                      /* newly inserted */
        }
        if (h->keys[i] == key) {
            h->vals[i] = value;
            TWIN_LOCK_GIVE(h->lock);
            return false;                     /* overwrote */
        }
        i = (i + 1) & h->mask;
    }
}

bool twin_hash_get(twin_hash_t *h, uint32_t key, uint32_t *out_value) {
    if (!h) return false;
    TWIN_LOCK_TAKE(h->lock);
    size_t i = twin_hash_slot(h, key);
    while (h->used[i]) {
        if (h->keys[i] == key) {
            if (out_value) *out_value = h->vals[i];
            TWIN_LOCK_GIVE(h->lock);
            return true;
        }
        i = (i + 1) & h->mask;
    }
    TWIN_LOCK_GIVE(h->lock);
    return false;
}

bool twin_hash_remove(twin_hash_t *h, uint32_t key) {
    if (!h) return false;
    TWIN_LOCK_TAKE(h->lock);
    size_t i = twin_hash_slot(h, key);
    while (h->used[i]) {
        if (h->keys[i] == key) {
            /* Backward-shift deletion: keeps probe sequences intact without
             * tombstones, so a churn workload does not degrade the table. */
            size_t j = i;
            h->used[i] = 0; h->len--;
            size_t k = (j + 1) & h->mask;
            while (h->used[k]) {
                size_t home = twin_hash_slot(h, h->keys[k]);
                /* Move k back to j if j lies in k's probe path. */
                if ((j < k) ? (home <= j || home > k) : (home <= j && home > k)) {
                    h->keys[j] = h->keys[k]; h->vals[j] = h->vals[k];
                    h->used[j] = 1; h->used[k] = 0;
                    j = k;
                }
                k = (k + 1) & h->mask;
            }
            TWIN_LOCK_GIVE(h->lock);
            return true;
        }
        i = (i + 1) & h->mask;
    }
    TWIN_LOCK_GIVE(h->lock);
    return false;
}

bool twin_hash_aggregate(twin_hash_t *h, uint32_t lo, uint32_t hi, expanse_memtable_agg_t *out) {
    if (!h || !out) return false;
    TWIN_LOCK_TAKE(h->lock);
    agg_init(out);
    /* An unordered table cannot seek to `lo`; the whole table is the only
     * way to find the range. That is the structural cost of dropping order,
     * and it is exactly what this arm exists to measure. */
    for (size_t i = 0; i <= h->mask; ++i) {
        if (h->used[i] && h->keys[i] >= lo && h->keys[i] <= hi) agg_add(out, h->vals[i]);
    }
    TWIN_LOCK_GIVE(h->lock);
    return out->count > 0;
}

size_t twin_hash_len(const twin_hash_t *h) { return h ? h->len : 0; }

/* ---- sorted array ---------------------------------------------------- */

struct twin_sorted {
    uint32_t   *keys;
    uint32_t   *vals;
    size_t      len;
    size_t      cap;
    twin_lock_t lock;
};

/* Index of the first element >= key. */
static size_t twin_sorted_lb(const twin_sorted_t *s, uint32_t key) {
    size_t lo = 0, hi = s->len;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2;
        if (s->keys[mid] < key) lo = mid + 1; else hi = mid;
    }
    return lo;
}

twin_sorted_t *twin_sorted_create(size_t capacity) {
    twin_sorted_t *s = (twin_sorted_t *)calloc(1, sizeof(*s));
    if (!s) return NULL;
    s->keys = (uint32_t *)calloc(capacity, sizeof(uint32_t));
    s->vals = (uint32_t *)calloc(capacity, sizeof(uint32_t));
    if (!s->keys || !s->vals) { free(s->keys); free(s->vals); free(s); return NULL; }
    s->cap = capacity;
    TWIN_LOCK_INIT(s->lock);
    return s;
}

void twin_sorted_destroy(twin_sorted_t *s) {
    if (!s) return;
    TWIN_LOCK_FREE(s->lock);
    free(s->keys); free(s->vals); free(s);
}

bool twin_sorted_insert(twin_sorted_t *s, uint32_t key, uint32_t value) {
    if (!s) return false;
    TWIN_LOCK_TAKE(s->lock);
    size_t i = twin_sorted_lb(s, key);
    if (i < s->len && s->keys[i] == key) {
        s->vals[i] = value;
        TWIN_LOCK_GIVE(s->lock);
        return false;
    }
    if (s->len == s->cap) { TWIN_LOCK_GIVE(s->lock); return false; }
    /* Append is free; an out-of-order key pays the shift. Monotonic
     * telemetry always appends, which is this twin's winning regime. */
    if (i < s->len) {
        memmove(&s->keys[i + 1], &s->keys[i], (s->len - i) * sizeof(uint32_t));
        memmove(&s->vals[i + 1], &s->vals[i], (s->len - i) * sizeof(uint32_t));
    }
    s->keys[i] = key; s->vals[i] = value; s->len++;
    TWIN_LOCK_GIVE(s->lock);
    return true;
}

bool twin_sorted_get(twin_sorted_t *s, uint32_t key, uint32_t *out_value) {
    if (!s) return false;
    TWIN_LOCK_TAKE(s->lock);
    size_t i = twin_sorted_lb(s, key);
    bool found = (i < s->len && s->keys[i] == key);
    if (found && out_value) *out_value = s->vals[i];
    TWIN_LOCK_GIVE(s->lock);
    return found;
}

bool twin_sorted_remove(twin_sorted_t *s, uint32_t key) {
    if (!s) return false;
    TWIN_LOCK_TAKE(s->lock);
    size_t i = twin_sorted_lb(s, key);
    if (i >= s->len || s->keys[i] != key) { TWIN_LOCK_GIVE(s->lock); return false; }
    memmove(&s->keys[i], &s->keys[i + 1], (s->len - i - 1) * sizeof(uint32_t));
    memmove(&s->vals[i], &s->vals[i + 1], (s->len - i - 1) * sizeof(uint32_t));
    s->len--;
    TWIN_LOCK_GIVE(s->lock);
    return true;
}

bool twin_sorted_aggregate(twin_sorted_t *s, uint32_t lo, uint32_t hi, expanse_memtable_agg_t *out) {
    if (!s || !out) return false;
    TWIN_LOCK_TAKE(s->lock);
    agg_init(out);
    /* Seek once, then walk contiguous memory — the shape that should beat a
     * pointer chase through trie nodes. */
    for (size_t i = twin_sorted_lb(s, lo); i < s->len && s->keys[i] <= hi; ++i) {
        agg_add(out, s->vals[i]);
    }
    TWIN_LOCK_GIVE(s->lock);
    return out->count > 0;
}

bool twin_visit_agg(expanse_word_t key, expanse_word_t value, void *ctx) {
    (void)key;
    agg_add((expanse_memtable_agg_t *)ctx, (uint32_t)value);
    return true;
}

bool twin_sorted_aggregate_indirect(twin_sorted_t *s, uint32_t lo, uint32_t hi,
                                    twin_visit_fn visit,
                                    expanse_memtable_agg_t *out) {
    if (!s || !out || !visit) return false;
    TWIN_LOCK_TAKE(s->lock);
    agg_init(out);
    /* Deliberately character-for-character twin_sorted_aggregate above, with
     * the inline agg_add replaced by the indirect call. Keep them in step. */
    for (size_t i = twin_sorted_lb(s, lo); i < s->len && s->keys[i] <= hi; ++i) {
        if (!visit((expanse_word_t)s->keys[i], (expanse_word_t)s->vals[i], out)) break;
    }
    TWIN_LOCK_GIVE(s->lock);
    return out->count > 0;
}

size_t twin_sorted_len(const twin_sorted_t *s) { return s ? s->len : 0; }

/* ---- monotonic ring buffer ------------------------------------------- */

struct twin_ring {
    uint32_t   *keys;
    uint32_t   *vals;
    size_t      cap;
    size_t      head;   /* next write index */
    size_t      len;
    twin_lock_t lock;
};

twin_ring_t *twin_ring_create(size_t capacity) {
    twin_ring_t *r = (twin_ring_t *)calloc(1, sizeof(*r));
    if (!r) return NULL;
    r->keys = (uint32_t *)calloc(capacity, sizeof(uint32_t));
    r->vals = (uint32_t *)calloc(capacity, sizeof(uint32_t));
    if (!r->keys || !r->vals) { free(r->keys); free(r->vals); free(r); return NULL; }
    r->cap = capacity;
    TWIN_LOCK_INIT(r->lock);
    return r;
}

void twin_ring_destroy(twin_ring_t *r) {
    if (!r) return;
    TWIN_LOCK_FREE(r->lock);
    free(r->keys); free(r->vals); free(r);
}

bool twin_ring_insert(twin_ring_t *r, uint32_t key, uint32_t value) {
    if (!r) return false;
    TWIN_LOCK_TAKE(r->lock);
    r->keys[r->head] = key; r->vals[r->head] = value;
    r->head = (r->head + 1) % r->cap;
    if (r->len < r->cap) r->len++;
    TWIN_LOCK_GIVE(r->lock);
    return true;
}

/* No index: a point lookup is a scan. The ring wins ingest and loses this,
 * which is the trade it exists to show. */
bool twin_ring_get(twin_ring_t *r, uint32_t key, uint32_t *out_value) {
    if (!r) return false;
    TWIN_LOCK_TAKE(r->lock);
    for (size_t i = 0; i < r->len; ++i) {
        if (r->keys[i] == key) {
            if (out_value) *out_value = r->vals[i];
            TWIN_LOCK_GIVE(r->lock);
            return true;
        }
    }
    TWIN_LOCK_GIVE(r->lock);
    return false;
}

bool twin_ring_aggregate(twin_ring_t *r, uint32_t lo, uint32_t hi, expanse_memtable_agg_t *out) {
    if (!r || !out) return false;
    TWIN_LOCK_TAKE(r->lock);
    agg_init(out);
    for (size_t i = 0; i < r->len; ++i) {
        if (r->keys[i] >= lo && r->keys[i] <= hi) agg_add(out, r->vals[i]);
    }
    TWIN_LOCK_GIVE(r->lock);
    return out->count > 0;
}

size_t twin_ring_len(const twin_ring_t *r) { return r ? r->len : 0; }

/* ---- BLE twins ------------------------------------------------------- */

static inline uint32_t mac_hash(const uint8_t mac[6]) {
    /* FNV-1a over the 48-bit address, matching the tracker's own choice so
     * the two arms are not separated by hash quality. */
    uint32_t h = 2166136261u;
    for (int i = 0; i < 6; ++i) { h ^= mac[i]; h *= 16777619u; }
    return h;
}

struct twin_ble_hash {
    expanse_ble_record_t *recs;   /* identical 28-byte payload (§8.16) */
    uint8_t              *used;
    size_t                mask;
    size_t                len;
    twin_lock_t           lock;
};

twin_ble_hash_t *twin_ble_hash_create(size_t expected_entries) {
    size_t want = (expected_entries * 10) / 7 + 1;
    size_t cap = 8;
    while (cap < want) cap <<= 1;
    twin_ble_hash_t *t = (twin_ble_hash_t *)calloc(1, sizeof(*t));
    if (!t) return NULL;
    t->recs = (expanse_ble_record_t *)calloc(cap, sizeof(expanse_ble_record_t));
    t->used = (uint8_t *)calloc(cap, sizeof(uint8_t));
    if (!t->recs || !t->used) { free(t->recs); free(t->used); free(t); return NULL; }
    t->mask = cap - 1;
    TWIN_LOCK_INIT(t->lock);
    return t;
}

void twin_ble_hash_destroy(twin_ble_hash_t *t) {
    if (!t) return;
    TWIN_LOCK_FREE(t->lock);
    free(t->recs); free(t->used); free(t);
}

bool twin_ble_hash_record(twin_ble_hash_t *t, const expanse_ble_record_t *rec) {
    if (!t || !rec) return false;
    TWIN_LOCK_TAKE(t->lock);
    size_t i = (size_t)(mac_hash(rec->mac) >> 8) & t->mask;
    for (;;) {
        if (!t->used[i]) {
            t->recs[i] = *rec; t->used[i] = 1; t->len++;
            TWIN_LOCK_GIVE(t->lock);
            return true;
        }
        if (memcmp(t->recs[i].mac, rec->mac, 6) == 0) {
            t->recs[i] = *rec;                 /* refresh the sighting */
            TWIN_LOCK_GIVE(t->lock);
            return false;
        }
        i = (i + 1) & t->mask;
    }
}

bool twin_ble_hash_get(twin_ble_hash_t *t, const uint8_t mac[6], expanse_ble_record_t *out) {
    if (!t || !mac) return false;
    TWIN_LOCK_TAKE(t->lock);
    size_t i = (size_t)(mac_hash(mac) >> 8) & t->mask;
    while (t->used[i]) {
        if (memcmp(t->recs[i].mac, mac, 6) == 0) {
            if (out) *out = t->recs[i];
            TWIN_LOCK_GIVE(t->lock);
            return true;
        }
        i = (i + 1) & t->mask;
    }
    TWIN_LOCK_GIVE(t->lock);
    return false;
}

size_t twin_ble_hash_expire_stale(twin_ble_hash_t *t, uint32_t cutoff_ms) {
    if (!t) return 0;
    TWIN_LOCK_TAKE(t->lock);
    /* No secondary time index, so every slot is visited whether or not any
     * entry is stale. Expanse's by_time trie is what this contrasts with. */
    size_t expired = 0;
    for (size_t i = 0; i <= t->mask; ++i) {
        if (t->used[i] && twin_is_stale(t->recs[i].last_seen_ms, cutoff_ms)) {
            t->used[i] = 0; t->len--; expired++;
        }
    }
    TWIN_LOCK_GIVE(t->lock);
    return expired;
}

struct twin_ble_scan {
    expanse_ble_record_t *recs;
    size_t                len;
    size_t                cap;
    twin_lock_t           lock;
};

twin_ble_scan_t *twin_ble_scan_create(size_t capacity) {
    twin_ble_scan_t *t = (twin_ble_scan_t *)calloc(1, sizeof(*t));
    if (!t) return NULL;
    t->recs = (expanse_ble_record_t *)calloc(capacity, sizeof(expanse_ble_record_t));
    if (!t->recs) { free(t); return NULL; }
    t->cap = capacity;
    TWIN_LOCK_INIT(t->lock);
    return t;
}

void twin_ble_scan_destroy(twin_ble_scan_t *t) {
    if (!t) return;
    TWIN_LOCK_FREE(t->lock);
    free(t->recs); free(t);
}

bool twin_ble_scan_record(twin_ble_scan_t *t, const expanse_ble_record_t *rec) {
    if (!t || !rec) return false;
    TWIN_LOCK_TAKE(t->lock);
    for (size_t i = 0; i < t->len; ++i) {
        if (memcmp(t->recs[i].mac, rec->mac, 6) == 0) {
            t->recs[i] = *rec;
            TWIN_LOCK_GIVE(t->lock);
            return false;
        }
    }
    if (t->len == t->cap) { TWIN_LOCK_GIVE(t->lock); return false; }
    t->recs[t->len++] = *rec;
    TWIN_LOCK_GIVE(t->lock);
    return true;
}

bool twin_ble_scan_get(twin_ble_scan_t *t, const uint8_t mac[6], expanse_ble_record_t *out) {
    if (!t || !mac) return false;
    TWIN_LOCK_TAKE(t->lock);
    for (size_t i = 0; i < t->len; ++i) {
        if (memcmp(t->recs[i].mac, mac, 6) == 0) {
            if (out) *out = t->recs[i];
            TWIN_LOCK_GIVE(t->lock);
            return true;
        }
    }
    TWIN_LOCK_GIVE(t->lock);
    return false;
}

size_t twin_ble_scan_expire_stale(twin_ble_scan_t *t, uint32_t cutoff_ms) {
    if (!t) return 0;
    TWIN_LOCK_TAKE(t->lock);
    size_t w = 0, expired = 0;
    for (size_t i = 0; i < t->len; ++i) {
        if (twin_is_stale(t->recs[i].last_seen_ms, cutoff_ms)) { expired++; continue; }
        if (w != i) t->recs[w] = t->recs[i];
        w++;
    }
    t->len = w;
    TWIN_LOCK_GIVE(t->lock);
    return expired;
}
