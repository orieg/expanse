#include "expanse_memtable.h"
#include "expanse.h"

#if defined(__has_include)
#if __has_include(<stdlib.h>)
#include <stdlib.h>
#else
void *malloc(size_t size);
void *calloc(size_t nmemb, size_t size);
void free(void *ptr);
#endif
#else
#include <stdlib.h>
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
#error "Unsupported platform for threading in expanse_memtable — define EXPANSE_SINGLE_THREADED_BAREMETAL for single-threaded bare-metal."
#endif

struct expanse_memtable {
    expanse_map_t  *map;
    expanse_lock_t  lock;
};

expanse_memtable_t *expanse_memtable_create(void) {
    expanse_memtable_t *mt = (expanse_memtable_t *)calloc(1, sizeof(expanse_memtable_t));
    if (!mt) {
        return NULL;
    }
    mt->map = expanse_map_new();
    if (!mt->map) {
        free(mt);
        return NULL;
    }
    EXPANSE_LOCK_INIT(mt->lock);
    return mt;
}

void expanse_memtable_destroy(expanse_memtable_t *mt) {
    if (!mt) {
        return;
    }
    EXPANSE_LOCK_TAKE(mt->lock);
    if (mt->map) {
        expanse_map_free(mt->map);
        mt->map = NULL;
    }
    EXPANSE_LOCK_GIVE(mt->lock);
    EXPANSE_LOCK_FREE(mt->lock);
    free(mt);
}

bool expanse_memtable_insert(expanse_memtable_t *mt, uint32_t key, uint32_t value, uint32_t *old_value) {
    if (!mt) {
        return false;
    }
    EXPANSE_LOCK_TAKE(mt->lock);
    if (!mt->map) {
        EXPANSE_LOCK_GIVE(mt->lock);
        return false;
    }
    expanse_word_t old_word = 0;
    bool is_new = expanse_map_insert(mt->map, (expanse_word_t)key, (expanse_word_t)value, old_value ? &old_word : NULL);
    if (!is_new && old_value) {
        *old_value = (uint32_t)old_word;
    }
    EXPANSE_LOCK_GIVE(mt->lock);
    return is_new;
}

bool expanse_memtable_get(const expanse_memtable_t *mt, uint32_t key, uint32_t *out_value) {
    if (!mt) {
        return false;
    }
    expanse_memtable_t *non_const_mt = (expanse_memtable_t *)mt;
    EXPANSE_LOCK_TAKE(non_const_mt->lock);
    if (!non_const_mt->map) {
        EXPANSE_LOCK_GIVE(non_const_mt->lock);
        return false;
    }
    expanse_word_t val_word = 0;
    bool found = expanse_map_get(non_const_mt->map, (expanse_word_t)key, out_value ? &val_word : NULL);
    if (found && out_value) {
        *out_value = (uint32_t)val_word;
    }
    EXPANSE_LOCK_GIVE(non_const_mt->lock);
    return found;
}

bool expanse_memtable_remove(expanse_memtable_t *mt, uint32_t key, uint32_t *old_value) {
    if (!mt) {
        return false;
    }
    EXPANSE_LOCK_TAKE(mt->lock);
    if (!mt->map) {
        EXPANSE_LOCK_GIVE(mt->lock);
        return false;
    }
    expanse_word_t old_word = 0;
    bool removed = expanse_map_remove(mt->map, (expanse_word_t)key, old_value ? &old_word : NULL);
    if (removed && old_value) {
        *old_value = (uint32_t)old_word;
    }
    EXPANSE_LOCK_GIVE(mt->lock);
    return removed;
}

/* The running aggregate update, shared by both width paths below so the
 * two cannot drift apart. */
static void aggregate_fold(expanse_memtable_agg_t *agg, uint32_t v) {
    if (v < agg->min_val) {
        agg->min_val = v;
    }
    if (v > agg->max_val) {
        agg->max_val = v;
    }
    agg->sum_val += v;
    agg->count++;
}

/*
 * Folds one entry into the running aggregate. Never stops the walk: the
 * aggregate is over the whole range.
 */
#if !EXPANSE_WIDE_SURFACE
static bool aggregate_visit(expanse_word_t key, expanse_word_t value, void *ctx) {
    (void)key;
    aggregate_fold((expanse_memtable_agg_t *)ctx, (uint32_t)value);
    return true;
}
#endif

bool expanse_memtable_aggregate_range(const expanse_memtable_t *mt, uint32_t start_key, uint32_t end_key, expanse_memtable_agg_t *out_agg) {
    if (!mt || !out_agg || start_key > end_key) {
        return false;
    }
    expanse_memtable_t *non_const_mt = (expanse_memtable_t *)mt;
    EXPANSE_LOCK_TAKE(non_const_mt->lock);
    if (!non_const_mt->map) {
        EXPANSE_LOCK_GIVE(non_const_mt->lock);
        return false;
    }

    out_agg->min_val = UINT32_MAX;
    out_agg->max_val = 0;
    out_agg->sum_val = 0;
    out_agg->count = 0;

#if EXPANSE_WIDE_SURFACE
    /*
     * 64-bit host build of this component: the streaming walk is
     * 32-bit-only, so keep the per-key next_at_or_after / next_after loop.
     * Same ascending contiguous order, same aggregate -- it just pays a
     * fresh O(depth) root descent per key.
     */
    expanse_word_t curr_key = 0;
    expanse_word_t curr_val = 0;
    if (expanse_map_next_at_or_after(non_const_mt->map, (expanse_word_t)start_key, &curr_key, &curr_val)) {
        while (curr_key <= (expanse_word_t)end_key) {
            aggregate_fold(out_agg, (uint32_t)curr_val);
            if (curr_key == UINT32_MAX) {
                break;
            }
            if (!expanse_map_next_after(non_const_mt->map, curr_key, &curr_key, &curr_val)) {
                break;
            }
        }
    }
#else
    /*
     * One descent to start_key, then contiguous streaming through the
     * leaves the range spans. The next_at_or_after / next_after loop this
     * replaced paid a fresh O(depth) root descent per key walked (#614).
     */
    expanse_map_for_each_range(non_const_mt->map, (expanse_word_t)start_key,
                               (expanse_word_t)end_key, aggregate_visit, out_agg);
#endif

    EXPANSE_LOCK_GIVE(non_const_mt->lock);
    return out_agg->count > 0;
}

/*
 * State for the flush walk: the caller's callback, the highest key it has
 * accepted so far, and how many it accepted. The walk stops at the first
 * entry the callback rejects, which is neither flushed nor removed --
 * exactly what the per-entry loop this replaced did.
 */
#if !EXPANSE_WIDE_SURFACE
struct flush_walk {
    expanse_memtable_flush_cb cb;
    void *user_data;
    expanse_word_t last_key;
    size_t count;
};

static bool flush_visit(expanse_word_t key, expanse_word_t value, void *ctx) {
    struct flush_walk *w = (struct flush_walk *)ctx;
    if (w->cb && !w->cb((uint32_t)key, (uint32_t)value, w->user_data)) {
        return false;
    }
    w->last_key = key;
    w->count++;
    return true;
}
#endif

size_t expanse_memtable_flush_range(expanse_memtable_t *mt, uint32_t start_key, uint32_t end_key, expanse_memtable_flush_cb cb, void *user_data) {
    if (!mt || start_key > end_key) {
        return 0;
    }
    EXPANSE_LOCK_TAKE(mt->lock);
    if (!mt->map) {
        EXPANSE_LOCK_GIVE(mt->lock);
        return 0;
    }

#if EXPANSE_WIDE_SURFACE
    /*
     * 64-bit host build of this component: both the streaming walk and the
     * batched remove_range are 32-bit-only, so keep the per-entry loop.
     * Contract is identical -- ascending order, and the first entry the
     * callback rejects is neither flushed nor removed -- because each
     * iteration re-seeks to the lowest surviving key at or after
     * start_key, which is exactly the entry the previous one removed.
     */
    size_t flushed = 0;
    while (true) {
        expanse_word_t curr_key = 0;
        expanse_word_t curr_val = 0;
        if (!expanse_map_next_at_or_after(mt->map, (expanse_word_t)start_key, &curr_key, &curr_val)) {
            break;
        }
        if (curr_key > (expanse_word_t)end_key) {
            break;
        }
        if (cb && !cb((uint32_t)curr_key, (uint32_t)curr_val, user_data)) {
            break;
        }
        expanse_map_remove(mt->map, curr_key, NULL);
        flushed++;
    }

    EXPANSE_LOCK_GIVE(mt->lock);
    return flushed;
#else
    /*
     * Two passes over the range, each one descent plus contiguous
     * streaming: the walk hands every entry to the callback in ascending
     * key order and records the last one it accepted, then a single
     * remove_range retires exactly that prefix. The loop this replaced
     * re-seeked from start_key and removed one key at a time, so it paid
     * two full root descents per entry flushed (#614).
     */
    struct flush_walk w;
    w.cb = cb;
    w.user_data = user_data;
    w.last_key = 0;
    w.count = 0;
    expanse_map_for_each_range(mt->map, (expanse_word_t)start_key, (expanse_word_t)end_key,
                               flush_visit, &w);
    if (w.count > 0) {
        expanse_map_remove_range(mt->map, (expanse_word_t)start_key, w.last_key, NULL, NULL);
    }

    EXPANSE_LOCK_GIVE(mt->lock);
    return w.count;
#endif
}

size_t expanse_memtable_len(const expanse_memtable_t *mt) {
    if (!mt) {
        return 0;
    }
    expanse_memtable_t *non_const_mt = (expanse_memtable_t *)mt;
    EXPANSE_LOCK_TAKE(non_const_mt->lock);
    size_t len = non_const_mt->map ? (size_t)expanse_map_len(non_const_mt->map) : 0;
    EXPANSE_LOCK_GIVE(non_const_mt->lock);
    return len;
}

size_t expanse_memtable_mem_used(const expanse_memtable_t *mt) {
    if (!mt) {
        return 0;
    }
    expanse_memtable_t *non_const_mt = (expanse_memtable_t *)mt;
    EXPANSE_LOCK_TAKE(non_const_mt->lock);
    size_t used = non_const_mt->map ? expanse_map_mem_used(non_const_mt->map) : 0;
    EXPANSE_LOCK_GIVE(non_const_mt->lock);
    return used;
}

void expanse_memtable_clear(expanse_memtable_t *mt) {
    if (!mt) {
        return;
    }
    EXPANSE_LOCK_TAKE(mt->lock);
    if (mt->map) {
        expanse_map_clear(mt->map);
    }
    EXPANSE_LOCK_GIVE(mt->lock);
}
