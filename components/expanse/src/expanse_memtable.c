#include "expanse_memtable.h"
#include "expanse.h"
#include <stdlib.h>

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

    expanse_word_t curr_key = 0;
    expanse_word_t curr_val = 0;
    if (!expanse_map_next_at_or_after(non_const_mt->map, (expanse_word_t)start_key, &curr_key, &curr_val)) {
        EXPANSE_LOCK_GIVE(non_const_mt->lock);
        return false;
    }

    while (curr_key <= (expanse_word_t)end_key) {
        uint32_t v = (uint32_t)curr_val;
        if (v < out_agg->min_val) {
            out_agg->min_val = v;
        }
        if (v > out_agg->max_val) {
            out_agg->max_val = v;
        }
        out_agg->sum_val += v;
        out_agg->count++;

        if (curr_key == UINT32_MAX) {
            break;
        }
        if (!expanse_map_next_after(non_const_mt->map, curr_key, &curr_key, &curr_val)) {
            break;
        }
    }

    EXPANSE_LOCK_GIVE(non_const_mt->lock);
    return out_agg->count > 0;
}

size_t expanse_memtable_flush_range(expanse_memtable_t *mt, uint32_t start_key, uint32_t end_key, expanse_memtable_flush_cb cb, void *user_data) {
    if (!mt || start_key > end_key) {
        return 0;
    }
    EXPANSE_LOCK_TAKE(mt->lock);
    if (!mt->map) {
        EXPANSE_LOCK_GIVE(mt->lock);
        return 0;
    }

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
