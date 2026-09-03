/*
 * expanse_memtable.h — Embedded Telemetry MemTable for ESP-IDF / FreeRTOS.
 *
 * Backed by Expanse 32-bit digital trie (ExpanseMap32) allocated in internal
 * fast DRAM (MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT).
 *
 * Provides thread-safe time-series sensor ingestion, 29-bit CAN dispatch,
 * sliding-window range aggregations, and periodic batch flushes.
 */
#ifndef EXPANSE_MEMTABLE_H
#define EXPANSE_MEMTABLE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct expanse_memtable expanse_memtable_t;

/** Running aggregation statistics over a range of keys. */
typedef struct {
    uint32_t min_val;
    uint32_t max_val;
    uint64_t sum_val;
    size_t   count;
} expanse_memtable_agg_t;

/**
 * Callback invoked for each key-value pair during a range flush, in
 * ascending key order. Return false to stop the flush before this entry.
 *
 * The callback must not mutate the memtable: the flush walks the range
 * first and retires the flushed prefix in one batched removal afterwards,
 * so a callback that inserts or removes would mutate the map the walk is
 * reading.
 */
typedef bool (*expanse_memtable_flush_cb)(uint32_t key, uint32_t value, void *user_data);

/**
 * Creates an empty embedded telemetry memtable.
 * Allocates all underlying trie nodes in internal fast DRAM.
 * Thread-safe via an internal FreeRTOS recursive mutex.
 */
expanse_memtable_t *expanse_memtable_create(void);

/** Destroys the memtable and frees all allocated node storage. */
void expanse_memtable_destroy(expanse_memtable_t *mt);

/**
 * Inserts or updates a key-value entry (e.g. timestamp_ms -> reading).
 * If the key already exists, writes the previous value to *old_value (if non-NULL)
 * and returns false; returns true if the key was newly inserted.
 */
bool expanse_memtable_insert(expanse_memtable_t *mt, uint32_t key, uint32_t value, uint32_t *old_value);

/**
 * Looks up a key in the memtable.
 * Returns true and writes value to *out_value if found, false otherwise.
 */
bool expanse_memtable_get(const expanse_memtable_t *mt, uint32_t key, uint32_t *out_value);

/**
 * Removes a key from the memtable.
 * Returns true and writes the removed value to *old_value (if non-NULL) if present.
 */
bool expanse_memtable_remove(expanse_memtable_t *mt, uint32_t key, uint32_t *old_value);

/**
 * Computes running min, max, sum, and count over the inclusive range [start_key, end_key].
 * Returns true if at least one entry was found in the range.
 */
bool expanse_memtable_aggregate_range(const expanse_memtable_t *mt, uint32_t start_key, uint32_t end_key, expanse_memtable_agg_t *out_agg);

/**
 * Flushes and removes entries in the range [start_key, end_key] by invoking cb
 * for each entry in ascending key order. Stops if cb returns false; the entry
 * it rejected is neither flushed nor removed.
 * Returns the count of entries successfully flushed and removed.
 *
 * One ordered walk of the range followed by one batched removal of the
 * flushed prefix, so the cost is a descent plus the entries touched rather
 * than two full root descents per entry. Entries are removed after the last
 * callback returns, so cb must not mutate the memtable and must not assume
 * an entry it already saw is gone.
 */
size_t expanse_memtable_flush_range(expanse_memtable_t *mt, uint32_t start_key, uint32_t end_key, expanse_memtable_flush_cb cb, void *user_data);

/** Returns the current number of entries in the memtable. */
size_t expanse_memtable_len(const expanse_memtable_t *mt);

/** Returns the exact heap memory used by the underlying trie in bytes. */
size_t expanse_memtable_mem_used(const expanse_memtable_t *mt);

/** Clears all entries from the memtable. */
void expanse_memtable_clear(expanse_memtable_t *mt);

#ifdef __cplusplus
}
#endif

#endif /* EXPANSE_MEMTABLE_H */
