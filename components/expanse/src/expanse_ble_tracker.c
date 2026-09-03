#include "expanse_ble_tracker.h"
#include "expanse.h"

#if defined(__has_include)
#if __has_include(<stdlib.h>)
#include <stdlib.h>
#else
void *malloc(size_t size);
void *calloc(size_t nmemb, size_t size);
void free(void *ptr);
#endif
#if __has_include(<string.h>)
#include <string.h>
#else
void *memset(void *s, int c, size_t n);
int memcmp(const void *s1, const void *s2, size_t n);
#endif
#else
#include <stdlib.h>
#include <string.h>
#endif

#if defined(ESP_PLATFORM)
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
typedef SemaphoreHandle_t expanse_lock_t;
#define EXPANSE_LOCK_INIT(lock)   do { (lock) = xSemaphoreCreateRecursiveMutex(); } while (0)
#define EXPANSE_LOCK_TAKE(lock)   do { if (lock) xSemaphoreTakeRecursive((lock), portMAX_DELAY); } while (0)
#define EXPANSE_LOCK_GIVE(lock)   do { if (lock) xSemaphoreGiveRecursive((lock)); } while (0)
#define EXPANSE_LOCK_FREE(lock)   do { if (lock) vSemaphoreDelete((lock)); } while (0)
#elif defined(_WIN32)
#include <windows.h>
typedef CRITICAL_SECTION expanse_lock_t;
#define EXPANSE_LOCK_INIT(lock)   InitializeCriticalSection(&(lock))
#define EXPANSE_LOCK_TAKE(lock)   EnterCriticalSection(&(lock))
#define EXPANSE_LOCK_GIVE(lock)   LeaveCriticalSection(&(lock))
#define EXPANSE_LOCK_FREE(lock)   DeleteCriticalSection(&(lock))
#elif defined(__unix__) || defined(__APPLE__)
#include <pthread.h>
typedef pthread_mutex_t expanse_lock_t;
#define EXPANSE_LOCK_INIT(lock)   do { \
    pthread_mutexattr_t attr; \
    pthread_mutexattr_init(&attr); \
    pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_RECURSIVE); \
    pthread_mutex_init(&(lock), &attr); \
    pthread_mutexattr_destroy(&attr); \
} while (0)
#define EXPANSE_LOCK_TAKE(lock)   pthread_mutex_lock(&(lock))
#define EXPANSE_LOCK_GIVE(lock)   pthread_mutex_unlock(&(lock))
#define EXPANSE_LOCK_FREE(lock)   pthread_mutex_destroy(&(lock))
#elif defined(EXPANSE_SINGLE_THREADED_BAREMETAL)
typedef int expanse_lock_t;
#define EXPANSE_LOCK_INIT(lock)   do { (lock) = 0; } while (0)
#define EXPANSE_LOCK_TAKE(lock)   do { } while (0)
#define EXPANSE_LOCK_GIVE(lock)   do { } while (0)
#define EXPANSE_LOCK_FREE(lock)   do { } while (0)
#else
#error "Unsupported platform for threading in expanse_ble_tracker — define EXPANSE_SINGLE_THREADED_BAREMETAL for single-threaded bare-metal."
#endif

#define TIME_KEY_SHIFT 13
#define SLAB_IDX_MASK  0x1FFF

struct expanse_ble_tracker {
    expanse_map_t         *by_mac;              /* mac_hash -> slab_idx */
    expanse_map_t         *by_time;             /* (rel_sec << 13) | slab_idx -> slab_idx */
    expanse_ble_record_t  *entries;             /* contiguous slab: 28B */
    uint32_t              *mono_last_seen_sec;  /* monotonic seconds per slot: 4B */
    uint16_t              *free_indices;        /* freelist stack: 2B */
    uint32_t              *active_bitmap;       /* 1 bit per slot liveness: 0.125B */
    size_t                 free_count;
    size_t                 max_capacity;
    size_t                 count;
    uint32_t               last_raw_ms;
    uint64_t               mono_offset_ms;
    uint32_t               base_epoch_sec;
    expanse_lock_t         lock;
};

static inline bool is_slot_active(const expanse_ble_tracker_t *tracker, size_t idx) {
    return (tracker->active_bitmap[idx >> 5] & (1u << (idx & 31))) != 0;
}

static inline void set_slot_active(expanse_ble_tracker_t *tracker, size_t idx) {
    tracker->active_bitmap[idx >> 5] |= (1u << (idx & 31));
}

static inline void clear_slot_active(expanse_ble_tracker_t *tracker, size_t idx) {
    tracker->active_bitmap[idx >> 5] &= ~(1u << (idx & 31));
}

static inline uint32_t fnv1a_32(const uint8_t *data, size_t len) {
    uint32_t hash = 2166136261u;
    for (size_t i = 0; i < len; ++i) {
        hash ^= data[i];
        hash *= 16777619u;
    }
    return hash;
}

static inline uint64_t unwrap_mono_ms(expanse_ble_tracker_t *tracker, uint32_t raw_ms) {
    if (tracker->count > 0 || tracker->last_raw_ms > 0) {
        if (raw_ms < tracker->last_raw_ms && (tracker->last_raw_ms - raw_ms) > 0x80000000u) {
            /* 49.7-day wrap detected: advance offset by 2^32 ms */
            tracker->mono_offset_ms += 0x100000000ULL;
        }
    }
    tracker->last_raw_ms = raw_ms;
    return tracker->mono_offset_ms + (uint64_t)raw_ms;
}

static inline uint32_t make_time_key(uint32_t mono_sec, uint32_t base_epoch_sec, uint16_t slab_idx) {
    uint32_t rel_sec = 0;
    if (mono_sec >= base_epoch_sec) {
        uint32_t diff = mono_sec - base_epoch_sec;
        rel_sec = (diff > 0x7FFFF) ? 0x7FFFF : diff;
    } else {
        /* Pre-epoch or wrapped entry: rel_sec = 0 sorts as oldest */
        rel_sec = 0;
    }
    return (rel_sec << TIME_KEY_SHIFT) | (slab_idx & SLAB_IDX_MASK);
}

static void rebase_epoch_if_needed(expanse_ble_tracker_t *tracker, uint32_t current_mono_sec) {
    if (current_mono_sec < tracker->base_epoch_sec) {
        tracker->base_epoch_sec = current_mono_sec;
    }
    uint32_t span = current_mono_sec - tracker->base_epoch_sec;
    if (span < 400000) { /* ~4.6 days */
        return;
    }

    uint32_t shift_delta = span - 100000;
    tracker->base_epoch_sec += shift_delta;

    /* Rebuild by_time with updated base_epoch_sec over active initialized slots only */
    expanse_map_clear(tracker->by_time);
    for (size_t i = 0; i < tracker->max_capacity; ++i) {
        if (!is_slot_active(tracker, i)) {
            continue;
        }
        uint32_t tk = make_time_key(tracker->mono_last_seen_sec[i], tracker->base_epoch_sec, (uint16_t)i);
        expanse_map_insert(tracker->by_time, tk, (expanse_word_t)i, NULL);
    }
}

expanse_ble_tracker_t *expanse_ble_tracker_create(size_t max_capacity) {
    if (max_capacity == 0 || max_capacity > EXPANSE_BLE_MAX_CAPACITY) {
        return NULL;
    }

    expanse_ble_tracker_t *tracker = (expanse_ble_tracker_t *)calloc(1, sizeof(expanse_ble_tracker_t));
    if (!tracker) {
        return NULL;
    }

    size_t bitmap_words = (max_capacity + 31) / 32;
    tracker->by_mac = expanse_map_new();
    tracker->by_time = expanse_map_new();
    tracker->entries = (expanse_ble_record_t *)calloc(max_capacity, sizeof(expanse_ble_record_t));
    tracker->mono_last_seen_sec = (uint32_t *)calloc(max_capacity, sizeof(uint32_t));
    tracker->active_bitmap = (uint32_t *)calloc(bitmap_words, sizeof(uint32_t));
    tracker->free_indices = (uint16_t *)malloc(max_capacity * sizeof(uint16_t));

    if (!tracker->by_mac || !tracker->by_time || !tracker->entries ||
        !tracker->mono_last_seen_sec || !tracker->active_bitmap || !tracker->free_indices) {
        if (tracker->by_mac) expanse_map_free(tracker->by_mac);
        if (tracker->by_time) expanse_map_free(tracker->by_time);
        if (tracker->entries) free(tracker->entries);
        if (tracker->mono_last_seen_sec) free(tracker->mono_last_seen_sec);
        if (tracker->active_bitmap) free(tracker->active_bitmap);
        if (tracker->free_indices) free(tracker->free_indices);
        free(tracker);
        return NULL;
    }

    for (size_t i = 0; i < max_capacity; ++i) {
        tracker->free_indices[i] = (uint16_t)(max_capacity - 1 - i);
    }
    tracker->free_count = max_capacity;
    tracker->max_capacity = max_capacity;
    tracker->count = 0;
    tracker->last_raw_ms = 0;
    tracker->mono_offset_ms = 0;
    tracker->base_epoch_sec = 0;
    EXPANSE_LOCK_INIT(tracker->lock);

    return tracker;
}

void expanse_ble_tracker_destroy(expanse_ble_tracker_t *tracker) {
    if (!tracker) {
        return;
    }
    EXPANSE_LOCK_TAKE(tracker->lock);
    if (tracker->by_mac) {
        expanse_map_free(tracker->by_mac);
        tracker->by_mac = NULL;
    }
    if (tracker->by_time) {
        expanse_map_free(tracker->by_time);
        tracker->by_time = NULL;
    }
    if (tracker->entries) {
        free(tracker->entries);
        tracker->entries = NULL;
    }
    if (tracker->mono_last_seen_sec) {
        free(tracker->mono_last_seen_sec);
        tracker->mono_last_seen_sec = NULL;
    }
    if (tracker->active_bitmap) {
        free(tracker->active_bitmap);
        tracker->active_bitmap = NULL;
    }
    if (tracker->free_indices) {
        free(tracker->free_indices);
        tracker->free_indices = NULL;
    }
    EXPANSE_LOCK_GIVE(tracker->lock);
    EXPANSE_LOCK_FREE(tracker->lock);
    free(tracker);
}

int expanse_ble_tracker_record(expanse_ble_tracker_t *tracker, const expanse_ble_record_t *record) {
    if (!tracker || !record) {
        return EXPANSE_BLE_ERR_INVAL;
    }

    EXPANSE_LOCK_TAKE(tracker->lock);
    uint64_t mono_ms = unwrap_mono_ms(tracker, record->last_seen_ms);
    uint32_t mono_sec = (uint32_t)(mono_ms / 1000ULL);
    rebase_epoch_if_needed(tracker, mono_sec);

    uint32_t mac_hash = fnv1a_32(record->mac, 6);
    expanse_word_t existing_idx = 0;
    if (expanse_map_get(tracker->by_mac, mac_hash, &existing_idx)) {
        uint16_t idx = (uint16_t)existing_idx;
        if (memcmp(tracker->entries[idx].mac, record->mac, 6) != 0) {
            EXPANSE_LOCK_GIVE(tracker->lock);
            return EXPANSE_BLE_ERR_COLLISION;
        }

        /* Re-sighting: remove old time key, update record and mono timestamp, insert new time key */
        uint32_t old_tk = make_time_key(tracker->mono_last_seen_sec[idx], tracker->base_epoch_sec, idx);
        expanse_map_remove(tracker->by_time, old_tk, NULL);

        tracker->entries[idx] = *record;
        tracker->mono_last_seen_sec[idx] = mono_sec;

        uint32_t new_tk = make_time_key(mono_sec, tracker->base_epoch_sec, idx);
        expanse_map_insert(tracker->by_time, new_tk, (expanse_word_t)idx, NULL);

        EXPANSE_LOCK_GIVE(tracker->lock);
        return EXPANSE_BLE_OK;
    }

    /* New sighting */
    if (tracker->free_count == 0) {
        EXPANSE_LOCK_GIVE(tracker->lock);
        return EXPANSE_BLE_ERR_NOMEM;
    }

    uint16_t idx = tracker->free_indices[--tracker->free_count];
    tracker->entries[idx] = *record;
    tracker->mono_last_seen_sec[idx] = mono_sec;
    set_slot_active(tracker, idx);

    uint32_t tk = make_time_key(mono_sec, tracker->base_epoch_sec, idx);
    expanse_map_insert(tracker->by_mac, mac_hash, (expanse_word_t)idx, NULL);
    expanse_map_insert(tracker->by_time, tk, (expanse_word_t)idx, NULL);
    tracker->count++;

    EXPANSE_LOCK_GIVE(tracker->lock);
    return EXPANSE_BLE_OK;
}

bool expanse_ble_tracker_get(const expanse_ble_tracker_t *tracker, const uint8_t mac[6], expanse_ble_record_t *out_record) {
    if (!tracker || !mac) {
        return false;
    }
    expanse_ble_tracker_t *non_const = (expanse_ble_tracker_t *)tracker;
    EXPANSE_LOCK_TAKE(non_const->lock);

    uint32_t mac_hash = fnv1a_32(mac, 6);
    expanse_word_t idx_word = 0;
    if (!expanse_map_get(tracker->by_mac, mac_hash, &idx_word)) {
        EXPANSE_LOCK_GIVE(non_const->lock);
        return false;
    }

    uint16_t idx = (uint16_t)idx_word;
    if (!is_slot_active(tracker, idx) || memcmp(tracker->entries[idx].mac, mac, 6) != 0) {
        EXPANSE_LOCK_GIVE(non_const->lock);
        return false;
    }

    if (out_record) {
        *out_record = tracker->entries[idx];
    }

    EXPANSE_LOCK_GIVE(non_const->lock);
    return true;
}

bool expanse_ble_tracker_remove(expanse_ble_tracker_t *tracker, const uint8_t mac[6], expanse_ble_record_t *out_record) {
    if (!tracker || !mac) {
        return false;
    }
    EXPANSE_LOCK_TAKE(tracker->lock);

    uint32_t mac_hash = fnv1a_32(mac, 6);
    expanse_word_t idx_word = 0;
    if (!expanse_map_get(tracker->by_mac, mac_hash, &idx_word)) {
        EXPANSE_LOCK_GIVE(tracker->lock);
        return false;
    }

    uint16_t idx = (uint16_t)idx_word;
    if (!is_slot_active(tracker, idx) || memcmp(tracker->entries[idx].mac, mac, 6) != 0) {
        EXPANSE_LOCK_GIVE(tracker->lock);
        return false;
    }

    if (out_record) {
        *out_record = tracker->entries[idx];
    }

    uint32_t tk = make_time_key(tracker->mono_last_seen_sec[idx], tracker->base_epoch_sec, idx);
    expanse_map_remove(tracker->by_mac, mac_hash, NULL);
    expanse_map_remove(tracker->by_time, tk, NULL);

    clear_slot_active(tracker, idx);
    tracker->free_indices[tracker->free_count++] = idx;
    tracker->count--;

    EXPANSE_LOCK_GIVE(tracker->lock);
    return true;
}

/* Releases the slab slot behind an expired by_time entry: by_mac index,
 * activity bit, free list, count. Shared by both eviction paths. */
static void expire_slot(expanse_ble_tracker_t *tracker, uint16_t idx) {
    uint32_t mac_hash = fnv1a_32(tracker->entries[idx].mac, 6);
    expanse_map_remove(tracker->by_mac, mac_hash, NULL);
    clear_slot_active(tracker, idx);
    tracker->free_indices[tracker->free_count++] = idx;
    tracker->count--;
}

#if !EXPANSE_WIDE_SURFACE
#define EXPIRE_BATCH_SIZE 64

typedef struct {
    expanse_ble_tracker_t *tracker;
    uint16_t batch[EXPIRE_BATCH_SIZE];
    size_t count;
} expire_batch_ctx_t;

static void flush_expire_batch(expire_batch_ctx_t *bctx) {
    for (size_t i = 0; i < bctx->count; i++) {
        expire_slot(bctx->tracker, bctx->batch[i]);
    }
    bctx->count = 0;
}

static void expire_one(expanse_word_t time_key, expanse_word_t idx_word, void *ctx) {
    (void)time_key;
    expire_batch_ctx_t *bctx = (expire_batch_ctx_t *)ctx;
    bctx->batch[bctx->count++] = (uint16_t)idx_word;
    if (bctx->count == EXPIRE_BATCH_SIZE) {
        flush_expire_batch(bctx);
    }
}
#endif

size_t expanse_ble_tracker_expire_stale(expanse_ble_tracker_t *tracker, uint32_t cutoff_ms) {
    if (!tracker) {
        return 0;
    }
    EXPANSE_LOCK_TAKE(tracker->lock);
    if (tracker->count == 0) {
        EXPANSE_LOCK_GIVE(tracker->lock);
        return 0;
    }

    uint64_t cutoff_mono_ms = unwrap_mono_ms(tracker, cutoff_ms);
    uint32_t cutoff_sec = (uint32_t)(cutoff_mono_ms / 1000ULL);
    uint32_t rel_cutoff_sec = 0;
    if (cutoff_sec >= tracker->base_epoch_sec) {
        uint32_t diff = cutoff_sec - tracker->base_epoch_sec;
        rel_cutoff_sec = (diff > 0x7FFFF) ? 0x7FFFF : diff;
    }
    uint32_t max_time_key = (rel_cutoff_sec << TIME_KEY_SHIFT) | SLAB_IDX_MASK;

    size_t expired_count = 0;
#if EXPANSE_WIDE_SURFACE
    /* 64-bit host build of this component: the batched entry point is
     * 32-bit-only, so keep the per-record loop. */
    while (true) {
        expanse_word_t time_key = 0;
        expanse_word_t idx_word = 0;
        if (!expanse_map_first(tracker->by_time, &time_key, &idx_word)) {
            break;
        }
        if (time_key > max_time_key) {
            break;
        }
        expanse_map_remove(tracker->by_time, time_key, NULL);
        expire_slot(tracker, (uint16_t)idx_word);
        expired_count++;
    }
#else
    /* One descent to the range and one structural fix-up per touched node
     * on by_time, instead of a first()/remove() descent pair per stale
     * record (#578). Buffering the slab indices across a small fixed
     * stack batch (64 entries, 128 bytes) decouples the by_time range walk
     * from the by_mac removals, avoiding L1 D-cache thrashing between the
     * two tries without dynamic allocations or key sorting (#617). */
    expire_batch_ctx_t bctx;
    bctx.tracker = tracker;
    bctx.count = 0;
    expired_count = expanse_map_remove_range(tracker->by_time, 0, max_time_key,
                                             expire_one, &bctx);
    if (bctx.count > 0) {
        flush_expire_batch(&bctx);
    }
#endif

    EXPANSE_LOCK_GIVE(tracker->lock);
    return expired_count;
}

size_t expanse_ble_tracker_count(const expanse_ble_tracker_t *tracker) {
    if (!tracker) {
        return 0;
    }
    expanse_ble_tracker_t *non_const = (expanse_ble_tracker_t *)tracker;
    EXPANSE_LOCK_TAKE(non_const->lock);
    size_t c = tracker->count;
    EXPANSE_LOCK_GIVE(non_const->lock);
    return c;
}

size_t expanse_ble_tracker_mem_used(const expanse_ble_tracker_t *tracker) {
    if (!tracker) {
        return 0;
    }
    expanse_ble_tracker_t *non_const = (expanse_ble_tracker_t *)tracker;
    EXPANSE_LOCK_TAKE(non_const->lock);
    size_t trie_mem = expanse_map_mem_used(tracker->by_mac) + expanse_map_mem_used(tracker->by_time);
    size_t slab_mem = tracker->max_capacity * sizeof(expanse_ble_record_t) +
                      tracker->max_capacity * sizeof(uint32_t) +
                      tracker->max_capacity * sizeof(uint16_t) +
                      ((tracker->max_capacity + 31) / 32) * sizeof(uint32_t);
    EXPANSE_LOCK_GIVE(non_const->lock);
    return trie_mem + slab_mem;
}
