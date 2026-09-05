/*
 * twin_containers.h — comparison baselines for the on-device benchmark (§8.3).
 *
 * The first ESP32 harvest published single-arm numbers: they sized the part
 * for a workload but could not say whether Expanse was the right structure
 * for it. These are the twins that make the comparison possible (#579).
 *
 * Every twin here is meant to WIN somewhere. A baseline that cannot beat the
 * primary under any parameter regime makes the comparison definitional
 * rather than measured (§8.3), so:
 *
 *   - twin_ring is the right answer for monotonic append-only telemetry and
 *     should beat Expanse on ingest outright;
 *   - twin_sorted is contiguous, so its range scan should beat a pointer
 *     chase through trie nodes;
 *   - twin_hash is the reserved unordered_map equivalent and should win point
 *     lookup.
 *
 * Expanse's case has to be made against those, not against strawmen.
 *
 * Symmetry obligations these types exist to satisfy:
 *   - §8.16 lock symmetry: every twin takes and releases the SAME FreeRTOS
 *     recursive mutex construct the expanse_memtable wrapper does, inside the
 *     same operations, so no arm is measured below a lock layer another arm
 *     pays for.
 *   - §8.16 payload symmetry: the BLE twins store the identical 28-byte
 *     expanse_ble_record_t, not a reduced stand-in.
 *   - §8.3 reserved capacity: the hash and sorted twins are pre-sized for the
 *     population, which is the realistic production configuration and the
 *     one that flatters them.
 */
#ifndef TWIN_CONTAINERS_H
#define TWIN_CONTAINERS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "expanse.h"
#include "expanse_memtable.h"
#include "expanse_ble_tracker.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- u32 -> u32 telemetry twins ------------------------------------- */

/** Reserved open-addressing hash table, linear probing. unordered_map twin. */
typedef struct twin_hash twin_hash_t;
twin_hash_t *twin_hash_create(size_t expected_entries);
void twin_hash_destroy(twin_hash_t *h);
bool twin_hash_insert(twin_hash_t *h, uint32_t key, uint32_t value);
bool twin_hash_get(twin_hash_t *h, uint32_t key, uint32_t *out_value);
bool twin_hash_remove(twin_hash_t *h, uint32_t key);
/** Unordered: this necessarily scans the whole table. Disclosed, not hidden. */
bool twin_hash_aggregate(twin_hash_t *h, uint32_t lo, uint32_t hi, expanse_memtable_agg_t *out);
size_t twin_hash_len(const twin_hash_t *h);

/** Sorted array with binary search and memmove insert. Ordered-map twin. */
typedef struct twin_sorted twin_sorted_t;
twin_sorted_t *twin_sorted_create(size_t capacity);
void twin_sorted_destroy(twin_sorted_t *s);
bool twin_sorted_insert(twin_sorted_t *s, uint32_t key, uint32_t value);
bool twin_sorted_get(twin_sorted_t *s, uint32_t key, uint32_t *out_value);
bool twin_sorted_remove(twin_sorted_t *s, uint32_t key);
bool twin_sorted_aggregate(twin_sorted_t *s, uint32_t lo, uint32_t hi, expanse_memtable_agg_t *out);

/*
 * The same walk, same fold, reached through a function pointer per key.
 *
 * expanse_memtable_aggregate_range crosses the C ABI into Rust and is called
 * back once per key; twin_sorted_aggregate folds inline. That is a harness
 * asymmetry the ratio between them silently carries (§8.3), and it was
 * disclosed but never sized. This pair sizes it: the two functions are
 * identical except that `agg_add(out, val)` becomes `visit(key, val, out)`,
 * so the difference between them is the per-key dispatch and nothing else.
 *
 * twin_visit_fn is declared to the same parameter and return types as
 * expanse_map_for_each_range_fn -- (expanse_word_t, expanse_word_t, void *)
 * returning bool -- so the indirect call has the calling convention Expanse
 * actually pays. It is declared here rather than reused from expanse.h
 * because that typedef sits in the narrow-only block (AGENTS.md §4): reusing
 * it would gate this pair to !EXPANSE_WIDE_SURFACE and leave it compiled by
 * no CI lane, since the lane that can build these TUs is the 64-bit one.
 * expanse_word_t is defined at both widths, so this compiles at both and is
 * uint32_t on the device, which is where the arm is measured.
 */
typedef bool (*twin_visit_fn)(expanse_word_t key, expanse_word_t value, void *ctx);

bool twin_sorted_aggregate_indirect(twin_sorted_t *s, uint32_t lo, uint32_t hi,
                                    twin_visit_fn visit,
                                    expanse_memtable_agg_t *out);

/*
 * The visitor for the call above. Lives beside agg_add so the fold body is
 * literally the one the inlined arm runs, leaving the indirect call as the
 * only difference.
 */
bool twin_visit_agg(expanse_word_t key, expanse_word_t value, void *ctx);

size_t twin_sorted_len(const twin_sorted_t *s);

/** Fixed ring buffer over monotonic keys. The append-only telemetry twin. */
typedef struct twin_ring twin_ring_t;
twin_ring_t *twin_ring_create(size_t capacity);
void twin_ring_destroy(twin_ring_t *r);
bool twin_ring_insert(twin_ring_t *r, uint32_t key, uint32_t value);
bool twin_ring_get(twin_ring_t *r, uint32_t key, uint32_t *out_value);
bool twin_ring_aggregate(twin_ring_t *r, uint32_t lo, uint32_t hi, expanse_memtable_agg_t *out);
size_t twin_ring_len(const twin_ring_t *r);

/* ---- BLE tracker twins ---------------------------------------------- */

/** Reserved MAC-keyed hash table over the identical 28-byte record. */
typedef struct twin_ble_hash twin_ble_hash_t;
twin_ble_hash_t *twin_ble_hash_create(size_t expected_entries);
void twin_ble_hash_destroy(twin_ble_hash_t *t);
bool twin_ble_hash_record(twin_ble_hash_t *t, const expanse_ble_record_t *rec);
bool twin_ble_hash_get(twin_ble_hash_t *t, const uint8_t mac[6], expanse_ble_record_t *out);
/** No time index: expiry scans every occupied slot. Disclosed, not hidden. */
size_t twin_ble_hash_expire_stale(twin_ble_hash_t *t, uint32_t cutoff_ms);

/** Flat array with linear search — what a small firmware usually ships. */
typedef struct twin_ble_scan twin_ble_scan_t;
twin_ble_scan_t *twin_ble_scan_create(size_t capacity);
void twin_ble_scan_destroy(twin_ble_scan_t *t);
bool twin_ble_scan_record(twin_ble_scan_t *t, const expanse_ble_record_t *rec);
bool twin_ble_scan_get(twin_ble_scan_t *t, const uint8_t mac[6], expanse_ble_record_t *out);
size_t twin_ble_scan_expire_stale(twin_ble_scan_t *t, uint32_t cutoff_ms);

#ifdef __cplusplus
}
#endif

#endif /* TWIN_CONTAINERS_H */
