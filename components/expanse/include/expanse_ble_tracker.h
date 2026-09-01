/*
 * expanse_ble_tracker.h — High-Churn BLE Asset-Tracker Registry for ESP-IDF.
 *
 * Tracks rotating 48-bit MAC addresses with a 28-byte tracking record
 * in internal fast DRAM.
 *
 * Provides hash-then-verify collision detection, slab-indexed storage
 * (ValueSlot32 is 4 bytes), and dual-index O(expired) TTL range eviction.
 */
#ifndef EXPANSE_BLE_TRACKER_H
#define EXPANSE_BLE_TRACKER_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EXPANSE_BLE_OK             0
#define EXPANSE_BLE_ERR_NOMEM     -1
#define EXPANSE_BLE_ERR_COLLISION -2
#define EXPANSE_BLE_ERR_INVAL     -3

#define EXPANSE_BLE_MAX_CAPACITY 8192

/**
 * BLE Asset Tracking record (exactly 28 bytes, 4-byte aligned).
 * Symmetric across all benchmark arms (Expanse, std::unordered_map, std::map).
 */
typedef struct {
    uint8_t  mac[6];       /* 6 bytes: 48-bit IEEE MAC address */
    int8_t   rssi;         /* 1 byte: RSSI in dBm */
    uint8_t  flags;        /* 1 byte: Status/presence flags */
    uint32_t last_seen_ms; /* 4 bytes: Millisecond timestamp */
    uint16_t distance_cm;  /* 2 bytes: Estimated distance in cm */
    char     name[14];     /* 14 bytes: Advertised device name (NUL-terminated) */
} expanse_ble_record_t;

typedef struct expanse_ble_tracker expanse_ble_tracker_t;

/**
 * Creates a BLE asset tracker registry with the declared maximum capacity.
 * Enforces max_capacity <= 8192 (the 13-bit slab index limit).
 * Returns NULL if max_capacity > 8192 or allocation fails.
 */
expanse_ble_tracker_t *expanse_ble_tracker_create(size_t max_capacity);

/** Destroys the tracker and frees all trie and slab storage. */
void expanse_ble_tracker_destroy(expanse_ble_tracker_t *tracker);

/**
 * Records or updates a sighting of a BLE device.
 * Computes 32-bit FNV-1a hash over mac[6].
 * Hash-then-verify: if hash exists but mac differs, returns EXPANSE_BLE_ERR_COLLISION.
 * Returns EXPANSE_BLE_OK on success, negative error code on failure.
 */
int expanse_ble_tracker_record(expanse_ble_tracker_t *tracker, const expanse_ble_record_t *record);

/**
 * Looks up a device by 48-bit MAC address.
 * Verifies full 6-byte MAC against slab record.
 * Returns true and writes record to *out_record if found, false otherwise.
 */
bool expanse_ble_tracker_get(const expanse_ble_tracker_t *tracker, const uint8_t mac[6], expanse_ble_record_t *out_record);

/**
 * Removes a device by 48-bit MAC address.
 * Returns true and writes removed record to *out_record (if non-NULL) if found.
 */
bool expanse_ble_tracker_remove(expanse_ble_tracker_t *tracker, const uint8_t mac[6], expanse_ble_record_t *out_record);

/**
 * Expires and evicts all entries whose last_seen_ms <= cutoff_ms in O(expired) time.
 * Scans the time index and removes expired entries from both time and MAC indices.
 * Performs epoch rebase if relative time approaches the 19-bit second field limit.
 * Returns the count of expired devices evicted.
 */
size_t expanse_ble_tracker_expire_stale(expanse_ble_tracker_t *tracker, uint32_t cutoff_ms);

/** Returns the current count of active tracked devices. */
size_t expanse_ble_tracker_count(const expanse_ble_tracker_t *tracker);

/** Returns total heap memory in bytes (dual tries + slab arena). */
size_t expanse_ble_tracker_mem_used(const expanse_ble_tracker_t *tracker);

#ifdef __cplusplus
}
#endif

#endif /* EXPANSE_BLE_TRACKER_H */
