/*
 * test_expanse.c — Unity unit tests for the Expanse ESP-IDF component.
 *
 * The engine cases below exist to make a missing or unlinked libexpanse.a a
 * test failure rather than a silent no-op: every one of them calls through
 * the C ABI, so the component cannot pass while the archive is absent.
 *
 * Judy.h is deliberately not included. A 32-bit libexpanse exports no Judy*
 * symbols at all (docs/COMPAT.md, build-configuration surface matrix).
 */
#include "unity.h"
#include "expanse_esp_idf.h"
#include "expanse.h"

TEST_CASE("Expanse internal SRAM allocation helper", "[expanse]") {
    void *ptr = expanse_esp_alloc_internal(128);
    TEST_ASSERT_NOT_NULL(ptr);
    expanse_esp_free(ptr);
}

TEST_CASE("Expanse SPIRAM allocation helper", "[expanse]") {
    void *ptr = expanse_esp_alloc_spiram(1024);
    // May be NULL if board lacks PSRAM, but should not crash
    if (ptr != NULL) {
        expanse_esp_free(ptr);
    }
}

TEST_CASE("Expanse host allocator backs libexpanse", "[expanse]") {
    void *ptr = expanse_host_malloc(64);
    TEST_ASSERT_NOT_NULL(ptr);
    expanse_host_free(ptr);
}

TEST_CASE("Expanse library identity links", "[expanse]") {
    const char *version = expanse_version();
    TEST_ASSERT_NOT_NULL(version);
    TEST_ASSERT_TRUE(version[0] != '\0');
}

TEST_CASE("Expanse word is one machine word", "[expanse]") {
    TEST_ASSERT_EQUAL_UINT(sizeof(void *), sizeof(expanse_word_t));
    TEST_ASSERT_EQUAL_INT(0, EXPANSE_WIDE_SURFACE);
}

TEST_CASE("Expanse set round-trips keys", "[expanse]") {
    expanse_set_t *set = expanse_set_new();
    TEST_ASSERT_NOT_NULL(set);

    for (expanse_word_t k = 0; k < 512; ++k) {
        TEST_ASSERT_TRUE(expanse_set_insert(set, k * 7));
    }
    TEST_ASSERT_EQUAL_UINT64(512, expanse_set_len(set));
    TEST_ASSERT_TRUE(expanse_set_contains(set, 7));
    TEST_ASSERT_FALSE(expanse_set_contains(set, 8));

    expanse_word_t first = 1, last = 0;
    TEST_ASSERT_TRUE(expanse_set_first(set, &first));
    TEST_ASSERT_TRUE(expanse_set_last(set, &last));
    TEST_ASSERT_EQUAL_UINT32(0, first);
    TEST_ASSERT_EQUAL_UINT32(511 * 7, last);

    TEST_ASSERT_TRUE(expanse_set_remove(set, 7));
    TEST_ASSERT_FALSE(expanse_set_remove(set, 7));
    TEST_ASSERT_EQUAL_UINT64(511, expanse_set_len(set));
    TEST_ASSERT_TRUE(expanse_set_mem_used(set) > 0);

    expanse_set_free(set);
}

TEST_CASE("Expanse map round-trips key/value pairs", "[expanse]") {
    expanse_map_t *map = expanse_map_new();
    TEST_ASSERT_NOT_NULL(map);

    for (expanse_word_t k = 0; k < 256; ++k) {
        TEST_ASSERT_TRUE(expanse_map_insert(map, k, k * 3, NULL));
    }
    TEST_ASSERT_EQUAL_UINT64(256, expanse_map_len(map));

    expanse_word_t value = 0;
    TEST_ASSERT_TRUE(expanse_map_get(map, 100, &value));
    TEST_ASSERT_EQUAL_UINT32(300, value);
    TEST_ASSERT_FALSE(expanse_map_get(map, 999, &value));

    expanse_word_t old = 0;
    TEST_ASSERT_FALSE(expanse_map_insert(map, 100, 7, &old));
    TEST_ASSERT_EQUAL_UINT32(300, old);

    TEST_ASSERT_TRUE(expanse_map_remove(map, 100, &old));
    TEST_ASSERT_EQUAL_UINT32(7, old);
    TEST_ASSERT_EQUAL_UINT64(255, expanse_map_len(map));

    expanse_map_clear(map);
    TEST_ASSERT_EQUAL_UINT64(0, expanse_map_len(map));

    expanse_map_free(map);
}

#include "expanse_memtable.h"
#include "expanse_ble_tracker.h"
#include <string.h>

static bool dummy_flush_cb(uint32_t key, uint32_t value, void *user_data) {
    size_t *count = (size_t *)user_data;
    (*count)++;
    return true;
}

TEST_CASE("Expanse telemetry memtable aggregation and flush", "[expanse]") {
    expanse_memtable_t *mt = expanse_memtable_create();
    TEST_ASSERT_NOT_NULL(mt);

    /* Insert timestamps 1000..1099 with sensor readings */
    for (uint32_t t = 1000; t < 1100; ++t) {
        TEST_ASSERT_TRUE(expanse_memtable_insert(mt, t, (t - 1000) * 10, NULL));
    }
    TEST_ASSERT_EQUAL_UINT(100, expanse_memtable_len(mt));

    /* Point lookup */
    uint32_t val = 0;
    TEST_ASSERT_TRUE(expanse_memtable_get(mt, 1050, &val));
    TEST_ASSERT_EQUAL_UINT32(500, val);

    /* Range aggregation over [1010, 1020] (11 items: 100, 110, ..., 200) */
    expanse_memtable_agg_t agg;
    TEST_ASSERT_TRUE(expanse_memtable_aggregate_range(mt, 1010, 1020, &agg));
    TEST_ASSERT_EQUAL_UINT(11, agg.count);
    TEST_ASSERT_EQUAL_UINT32(100, agg.min_val);
    TEST_ASSERT_EQUAL_UINT32(200, agg.max_val);
    TEST_ASSERT_EQUAL_UINT64(1650, agg.sum_val);

    /* Flush range [1000, 1049] (50 items) */
    size_t flushed_count = 0;
    size_t flushed = expanse_memtable_flush_range(mt, 1000, 1049, dummy_flush_cb, &flushed_count);
    TEST_ASSERT_EQUAL_UINT(50, flushed);
    TEST_ASSERT_EQUAL_UINT(50, flushed_count);
    TEST_ASSERT_EQUAL_UINT(50, expanse_memtable_len(mt));

    /* Verify flushed items removed, remaining items present */
    TEST_ASSERT_FALSE(expanse_memtable_get(mt, 1025, &val));
    TEST_ASSERT_TRUE(expanse_memtable_get(mt, 1075, &val));

    expanse_memtable_destroy(mt);
}

TEST_CASE("Expanse BLE tracker record, collision handling and TTL eviction", "[expanse]") {
    /* Capacity invariant: max_capacity > 8192 must be rejected */
    TEST_ASSERT_NULL(expanse_ble_tracker_create(8193));

    expanse_ble_tracker_t *tracker = expanse_ble_tracker_create(100);
    TEST_ASSERT_NOT_NULL(tracker);

    expanse_ble_record_t rec1 = {
        .mac = {0x11, 0x22, 0x33, 0x44, 0x55, 0x66},
        .rssi = -65,
        .flags = 0x01,
        .last_seen_ms = 10000,
        .distance_cm = 150,
        .name = "Beacon_01"
    };

    TEST_ASSERT_EQUAL_INT(EXPANSE_BLE_OK, expanse_ble_tracker_record(tracker, &rec1));
    TEST_ASSERT_EQUAL_UINT(1, expanse_ble_tracker_count(tracker));

    /* Get by MAC */
    expanse_ble_record_t out_rec;
    memset(&out_rec, 0, sizeof(out_rec));
    TEST_ASSERT_TRUE(expanse_ble_tracker_get(tracker, rec1.mac, &out_rec));
    TEST_ASSERT_EQUAL_INT8(-65, out_rec.rssi);
    TEST_ASSERT_EQUAL_STRING("Beacon_01", out_rec.name);

    /* Update sighting */
    rec1.last_seen_ms = 25000;
    rec1.rssi = -55;
    TEST_ASSERT_EQUAL_INT(EXPANSE_BLE_OK, expanse_ble_tracker_record(tracker, &rec1));
    TEST_ASSERT_EQUAL_UINT(1, expanse_ble_tracker_count(tracker));

    /* Insert second device */
    expanse_ble_record_t rec2 = {
        .mac = {0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF},
        .rssi = -80,
        .flags = 0x00,
        .last_seen_ms = 12000,
        .distance_cm = 450,
        .name = "Tag_02"
    };
    TEST_ASSERT_EQUAL_INT(EXPANSE_BLE_OK, expanse_ble_tracker_record(tracker, &rec2));
    TEST_ASSERT_EQUAL_UINT(2, expanse_ble_tracker_count(tracker));

    /* TTL Eviction: cutoff at 15000ms should evict rec2 (12000ms) but keep rec1 (25000ms) */
    size_t expired = expanse_ble_tracker_expire_stale(tracker, 15000);
    TEST_ASSERT_EQUAL_UINT(1, expired);
    TEST_ASSERT_EQUAL_UINT(1, expanse_ble_tracker_count(tracker));
    TEST_ASSERT_FALSE(expanse_ble_tracker_get(tracker, rec2.mac, &out_rec));
    TEST_ASSERT_TRUE(expanse_ble_tracker_get(tracker, rec1.mac, &out_rec));

    expanse_ble_tracker_destroy(tracker);
}

TEST_CASE("Expanse BLE tracker epoch rebase across 4.6-day boundary", "[expanse]") {
    expanse_ble_tracker_t *tracker = expanse_ble_tracker_create(50);
    TEST_ASSERT_NOT_NULL(tracker);

    /* Device 1 at t = 10 seconds */
    expanse_ble_record_t d1 = {
        .mac = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06},
        .rssi = -70,
        .flags = 0,
        .last_seen_ms = 10000,
        .distance_cm = 200,
        .name = "D1_Early"
    };
    TEST_ASSERT_EQUAL_INT(EXPANSE_BLE_OK, expanse_ble_tracker_record(tracker, &d1));

    /* Device 2 at t = 450,000 seconds (~5.2 days): triggers epoch rebase (>400,000s) */
    expanse_ble_record_t d2 = {
        .mac = {0x06, 0x05, 0x04, 0x03, 0x02, 0x01},
        .rssi = -60,
        .flags = 1,
        .last_seen_ms = 450000000,
        .distance_cm = 100,
        .name = "D2_Late"
    };
    TEST_ASSERT_EQUAL_INT(EXPANSE_BLE_OK, expanse_ble_tracker_record(tracker, &d2));
    TEST_ASSERT_EQUAL_UINT(2, expanse_ble_tracker_count(tracker));

    /* Verify both devices exist and are readable post-rebase */
    expanse_ble_record_t out;
    TEST_ASSERT_TRUE(expanse_ble_tracker_get(tracker, d1.mac, &out));
    TEST_ASSERT_EQUAL_STRING("D1_Early", out.name);

    TEST_ASSERT_TRUE(expanse_ble_tracker_get(tracker, d2.mac, &out));
    TEST_ASSERT_EQUAL_STRING("D2_Late", out.name);

    expanse_ble_tracker_destroy(tracker);
}

TEST_CASE("Expanse BLE tracker 49.7-day ms timestamp wrap handling", "[expanse]") {
    expanse_ble_tracker_t *tracker = expanse_ble_tracker_create(50);
    TEST_ASSERT_NOT_NULL(tracker);

    /* Pre-wrap device sighting at t = UINT32_MAX - 5000ms (~49.71 days of uptime) */
    expanse_ble_record_t d_pre = {
        .mac = {0xAA, 0x11, 0x22, 0x33, 0x44, 0x55},
        .rssi = -75,
        .flags = 0,
        .last_seen_ms = 0xFFFFF000u,
        .distance_cm = 300,
        .name = "D_PreWrap"
    };
    TEST_ASSERT_EQUAL_INT(EXPANSE_BLE_OK, expanse_ble_tracker_record(tracker, &d_pre));

    /* Post-wrap device sighting at t = 1000ms (after 49.7-day 32-bit hardware timer wrap) */
    expanse_ble_record_t d_post = {
        .mac = {0xBB, 0x11, 0x22, 0x33, 0x44, 0x55},
        .rssi = -65,
        .flags = 1,
        .last_seen_ms = 1000u,
        .distance_cm = 120,
        .name = "D_PostWrap"
    };
    TEST_ASSERT_EQUAL_INT(EXPANSE_BLE_OK, expanse_ble_tracker_record(tracker, &d_post));
    TEST_ASSERT_EQUAL_UINT(2, expanse_ble_tracker_count(tracker));

    /* Verify both devices are accessible */
    expanse_ble_record_t out;
    TEST_ASSERT_TRUE(expanse_ble_tracker_get(tracker, d_pre.mac, &out));
    TEST_ASSERT_EQUAL_STRING("D_PreWrap", out.name);

    TEST_ASSERT_TRUE(expanse_ble_tracker_get(tracker, d_post.mac, &out));
    TEST_ASSERT_EQUAL_STRING("D_PostWrap", out.name);

    /* Evict at t = 500ms post-wrap: pre-wrap device must be evicted, post-wrap kept */
    size_t expired = expanse_ble_tracker_expire_stale(tracker, 500u);
    TEST_ASSERT_EQUAL_UINT(1, expired);
    TEST_ASSERT_EQUAL_UINT(1, expanse_ble_tracker_count(tracker));
    TEST_ASSERT_FALSE(expanse_ble_tracker_get(tracker, d_pre.mac, &out));
    TEST_ASSERT_TRUE(expanse_ble_tracker_get(tracker, d_post.mac, &out));

    expanse_ble_tracker_destroy(tracker);
}



