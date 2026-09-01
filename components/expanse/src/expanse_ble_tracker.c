#include "expanse_ble_tracker.h"
#include "expanse.h"
#include <stdlib.h>
#include <string.h>

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
#else
typedef int expanse_lock_t;
#define EXPANSE_LOCK_INIT(lock)   do { (lock) = 0; } while (0)
#define EXPANSE_LOCK_TAKE(lock)   do { } while (0)
#define EXPANSE_LOCK_GIVE(lock)   do { } while (0)
#define EXPANSE_LOCK_FREE(lock)   do { } while (0)
#endif

#define TIME_KEY_SHIFT 13
#define SLAB_IDX_MASK  0x1FFF

struct expanse_ble_tracker {
    expanse_map_t         *by_mac;        /* mac_hash -> slab_idx */
    expanse_map_t         *by_time;       /* (rel_sec << 13) | slab_idx -> slab_idx */
    expanse_ble_record_t  *entries;       /* contiguous slab */
    uint16_t              *free_indices;  /* freelist stack */
    size_t                 free_count;
    size_t                 max_capacity;
    size_t                 count;
    uint32_t               base_epoch_sec;
    expanse_lock_t         lock;
};

static inline uint32_t fnv1a_32(const uint8_t *data, size_t len) {
    uint32_t hash = 2166136261u;
    for (size_t i = 0; i < len; ++i) {
        hash ^= data[i];
        hash *= 16777619u;
    }
    return hash;
}

static inline uint32_t make_time_key(uint32_t last_seen_ms, uint32_t base_epoch_sec, uint16_t slab_idx) {
    uint32_t sec = last_seen_ms / 1000;
    uint32_t rel_sec = (sec >= base_epoch_sec) ? (sec - base_epoch_sec) : 0;
    if (rel_sec > 0x7FFFF) { /* 19-bit limit: 524,287 s (~6 days) */
        rel_sec = 0x7FFFF;
    }
    return (rel_sec << TIME_KEY_SHIFT) | (slab_idx & SLAB_IDX_MASK);
}

static void rebase_epoch_if_needed(expanse_ble_tracker_t *tracker, uint32_t current_sec) {
    if (current_sec < tracker->base_epoch_sec) {
        tracker->base_epoch_sec = current_sec;
    }
    uint32_t span = current_sec - tracker->base_epoch_sec;
    if (span < 400000) { /* ~4.6 days, well under 6.06 day limit */
        return;
    }

    uint32_t shift_delta = span - 100000;
    tracker->base_epoch_sec += shift_delta;

    /* Rebuild by_time with updated base_epoch_sec */
    expanse_map_clear(tracker->by_time);
    for (size_t i = 0; i < tracker->max_capacity; ++i) {
        /* Check if slab slot i is active by checking by_mac */
        uint32_t mac_hash = fnv1a_32(tracker->entries[i].mac, 6);
        expanse_word_t live_idx = 0;
        if (expanse_map_get(tracker->by_mac, mac_hash, &live_idx) && (size_t)live_idx == i) {
            uint32_t tk = make_time_key(tracker->entries[i].last_seen_ms, tracker->base_epoch_sec, (uint16_t)i);
            expanse_map_insert(tracker->by_time, tk, (expanse_word_t)i, NULL);
        }
    }
}

expanse_ble_tracker_t *expanse_ble_tracker_create(size_t max_capacity) {
    if (max_capacity == 0 || max_capacity > EXPANSE_BLE_MAX_CAPACITY) {
        return NULL;
    }

    expanse_ble_tracker_t *tracker = (expanse_ble_tracker_t *)malloc(sizeof(expanse_ble_tracker_t));
    if (!tracker) {
        return NULL;
    }

    tracker->by_mac = expanse_map_new();
    tracker->by_time = expanse_map_new();
    tracker->entries = (expanse_ble_record_t *)malloc(max_capacity * sizeof(expanse_ble_record_t));
    tracker->free_indices = (uint16_t *)malloc(max_capacity * sizeof(uint16_t));

    if (!tracker->by_mac || !tracker->by_time || !tracker->entries || !tracker->free_indices) {
        if (tracker->by_mac) expanse_map_free(tracker->by_mac);
        if (tracker->by_time) expanse_map_free(tracker->by_time);
        if (tracker->entries) free(tracker->entries);
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
    rebase_epoch_if_needed(tracker, record->last_seen_ms / 1000);

    uint32_t mac_hash = fnv1a_32(record->mac, 6);
    expanse_word_t existing_idx = 0;
    if (expanse_map_get(tracker->by_mac, mac_hash, &existing_idx)) {
        uint16_t idx = (uint16_t)existing_idx;
        if (memcmp(tracker->entries[idx].mac, record->mac, 6) != 0) {
            /* Pinned collision policy: deterministic error */
            EXPANSE_LOCK_GIVE(tracker->lock);
            return EXPANSE_BLE_ERR_COLLISION;
        }

        /* Re-sighting: remove old time key, update record, insert new time key */
        uint32_t old_tk = make_time_key(tracker->entries[idx].last_seen_ms, tracker->base_epoch_sec, idx);
        expanse_map_remove(tracker->by_time, old_tk, NULL);

        tracker->entries[idx] = *record;

        uint32_t new_tk = make_time_key(record->last_seen_ms, tracker->base_epoch_sec, idx);
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

    uint32_t tk = make_time_key(record->last_seen_ms, tracker->base_epoch_sec, idx);
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
    if (memcmp(tracker->entries[idx].mac, mac, 6) != 0) {
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
    if (memcmp(tracker->entries[idx].mac, mac, 6) != 0) {
        EXPANSE_LOCK_GIVE(tracker->lock);
        return false;
    }

    if (out_record) {
        *out_record = tracker->entries[idx];
    }

    uint32_t tk = make_time_key(tracker->entries[idx].last_seen_ms, tracker->base_epoch_sec, idx);
    expanse_map_remove(tracker->by_mac, mac_hash, NULL);
    expanse_map_remove(tracker->by_time, tk, NULL);

    tracker->free_indices[tracker->free_count++] = idx;
    tracker->count--;

    EXPANSE_LOCK_GIVE(tracker->lock);
    return true;
}

size_t expanse_ble_tracker_expire_stale(expanse_ble_tracker_t *tracker, uint32_t cutoff_ms) {
    if (!tracker || tracker->count == 0) {
        return 0;
    }
    EXPANSE_LOCK_TAKE(tracker->lock);

    uint32_t cutoff_sec = cutoff_ms / 1000;
    uint32_t rel_cutoff_sec = (cutoff_sec >= tracker->base_epoch_sec) ? (cutoff_sec - tracker->base_epoch_sec) : 0;
    if (rel_cutoff_sec > 0x7FFFF) {
        rel_cutoff_sec = 0x7FFFF;
    }
    uint32_t max_time_key = (rel_cutoff_sec << TIME_KEY_SHIFT) | SLAB_IDX_MASK;

    size_t expired_count = 0;
    while (true) {
        expanse_word_t time_key = 0;
        expanse_word_t idx_word = 0;
        if (!expanse_map_first(tracker->by_time, &time_key, &idx_word)) {
            break;
        }
        if (time_key > max_time_key) {
            break;
        }

        uint16_t idx = (uint16_t)idx_word;
        uint32_t mac_hash = fnv1a_32(tracker->entries[idx].mac, 6);

        expanse_map_remove(tracker->by_time, time_key, NULL);
        expanse_map_remove(tracker->by_mac, mac_hash, NULL);

        tracker->free_indices[tracker->free_count++] = idx;
        tracker->count--;
        expired_count++;
    }

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
    size_t slab_mem = tracker->max_capacity * sizeof(expanse_ble_record_t) + tracker->max_capacity * sizeof(uint16_t);
    EXPANSE_LOCK_GIVE(non_const->lock);
    return trie_mem + slab_mem;
}
