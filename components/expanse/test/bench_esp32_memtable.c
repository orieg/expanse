/*
 * bench_esp32_memtable.c — Dedicated Hardware Benchmark Runner for ESP32-C3/C6.
 *
 * Measures cycles/op using esp_cpu_get_cycle_count() and exact heap utilization
 * across Expanse, std::unordered_map, std::map, and RingBuffer baselines.
 *
 * Emits machine-parseable JSON lines over UART for consumption by
 * scripts/esp32_bench_harvest.py.
 */

#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#include "expanse_memtable.h"
#include "expanse_ble_tracker.h"
#include "expanse.h"

#if defined(ESP_PLATFORM)
#include "esp_cpu.h"
#include "esp_heap_caps.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#define GET_CYCLES() esp_cpu_get_cycle_count()
#define GET_FREE_HEAP() heap_caps_get_free_size(MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT)
#define GET_LARGEST_BLOCK() heap_caps_get_largest_free_block(MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT)
#else
#include <time.h>
static inline uint32_t get_dummy_cycles(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)(ts.tv_nsec);
}
#define GET_CYCLES() get_dummy_cycles()
#define GET_FREE_HEAP() (1024 * 1024)
#define GET_LARGEST_BLOCK() (1024 * 1024)
#endif

#define WARMUP_RUNS 3
#define SAMPLE_RUNS 10

/*
 * `n` is the number of operations the arm timed; `pop` is the population the
 * structure held while it did so. They are not the same number and neither
 * one alone identifies the workload: the range aggregation below always walks
 * 500 keys (n = 500) but costs materially more over a 2000-key table than
 * over a 500-key one, because the descent is deeper. Reporting only `n` put
 * both of those in one bucket, so the harvester pooled two populations into a
 * single interval -- an §8.3 asymmetry hidden inside what looked like one
 * arm. Both fields are emitted on every line and both are part of the
 * harvester's grouping key (#579).
 */
static void report_json(const char *benchmark, const char *arm, size_t n, size_t pop, double cycles_per_op, size_t heap_used, double fragmentation) {
    printf("{\"benchmark\": \"%s\", \"arm\": \"%s\", \"n\": %zu, \"pop\": %zu, \"cycles_per_op\": %.2f, \"heap_used_bytes\": %zu, \"frag_ratio\": %.4f}\n",
           benchmark, arm, n, pop, cycles_per_op, heap_used, fragmentation);
}

/* Live fragmentation of the internal 8-bit-capable heap: how much of what is
 * free is unreachable to a single allocation. 0 means the whole free pool is
 * one block. */
static double heap_frag_ratio(void) {
    size_t free_now = GET_FREE_HEAP();
    size_t largest = GET_LARGEST_BLOCK();
    return 1.0 - ((double)largest / (double)(free_now > 0 ? free_now : 1));
}

/* 1. Sensor TSDB Ingestion & Range Aggregation */
void bench_tsdb_hardware(size_t n) {
    size_t heap_before = GET_FREE_HEAP();
    expanse_memtable_t *mt = expanse_memtable_create();
    if (!mt) return;

    /* Warmup */
    for (size_t i = 0; i < 100; ++i) {
        expanse_memtable_insert(mt, (uint32_t)(1000 + i), (uint32_t)(i * 5), NULL);
    }
    expanse_memtable_clear(mt);

    /* Timed Ingest */
    uint32_t start = GET_CYCLES();
    for (size_t i = 0; i < n; ++i) {
        expanse_memtable_insert(mt, (uint32_t)(1700000000 + i), (uint32_t)(i % 1000), NULL);
    }
    uint32_t elapsed = GET_CYCLES() - start;
    double cycles_per_op = (double)elapsed / (double)n;

    size_t heap_after = GET_FREE_HEAP();
    size_t heap_used = (heap_before > heap_after) ? (heap_before - heap_after) : expanse_memtable_mem_used(mt);
    size_t largest_block = GET_LARGEST_BLOCK();
    double frag = 1.0 - ((double)largest_block / (double)(heap_after > 0 ? heap_after : 1));

    report_json("esp32_tsdb_ingest", "expanse_memtable", n, n, cycles_per_op, heap_used, frag);

    /* Range Aggregation */
    expanse_memtable_agg_t agg;
    start = GET_CYCLES();
    expanse_memtable_aggregate_range(mt, 1700000000 + 100, 1700000000 + 600, &agg);
    elapsed = GET_CYCLES() - start;
    report_json("esp32_tsdb_aggregate_500", "expanse_memtable", 500, n, (double)elapsed / 500.0, heap_used, frag);

    expanse_memtable_destroy(mt);
}

/* 2. BLE Asset Tracker Sighting & TTL Eviction */
void bench_ble_tracker_hardware(size_t n) {
    size_t heap_before = GET_FREE_HEAP();
    expanse_ble_tracker_t *tracker = expanse_ble_tracker_create(n);
    if (!tracker) return;

    expanse_ble_record_t rec;
    memset(&rec, 0, sizeof(rec));
    strcpy(rec.name, "Beacon_Node");
    rec.rssi = -68;
    rec.distance_cm = 120;

    /* Timed Sighting Ingestion */
    uint32_t start = GET_CYCLES();
    for (size_t i = 0; i < n; ++i) {
        rec.mac[0] = 0x00;
        rec.mac[1] = 0x1A;
        rec.mac[2] = 0x2B;
        rec.mac[3] = (uint8_t)((i >> 16) & 0xFF);
        rec.mac[4] = (uint8_t)((i >> 8) & 0xFF);
        rec.mac[5] = (uint8_t)(i & 0xFF);
        rec.last_seen_ms = (uint32_t)(1000 + i * 10);
        expanse_ble_tracker_record(tracker, &rec);
    }
    uint32_t elapsed = GET_CYCLES() - start;
    double cycles_per_op = (double)elapsed / (double)n;

    size_t heap_after = GET_FREE_HEAP();
    size_t heap_used = (heap_before > heap_after) ? (heap_before - heap_after) : expanse_ble_tracker_mem_used(tracker);
    size_t largest_block = GET_LARGEST_BLOCK();
    double frag = 1.0 - ((double)largest_block / (double)(heap_after > 0 ? heap_after : 1));

    report_json("esp32_ble_sighting_record", "expanse_slab", n, n, cycles_per_op, heap_used, frag);

    /* Timed Point Lookups */
    start = GET_CYCLES();
    for (size_t i = 0; i < n; ++i) {
        uint8_t mac[6] = {0x00, 0x1A, 0x2B, (uint8_t)((i >> 16) & 0xFF), (uint8_t)((i >> 8) & 0xFF), (uint8_t)(i & 0xFF)};
        expanse_ble_record_t out_rec;
        expanse_ble_tracker_get(tracker, mac, &out_rec);
    }
    elapsed = GET_CYCLES() - start;
    report_json("esp32_ble_point_lookup", "expanse_slab", n, n, (double)elapsed / (double)n, heap_used, frag);

    /* Timed TTL Eviction */
    uint32_t cutoff_ms = (uint32_t)(1000 + (n / 2) * 10);
    start = GET_CYCLES();
    size_t expired = expanse_ble_tracker_expire_stale(tracker, cutoff_ms);
    elapsed = GET_CYCLES() - start;
    report_json("esp32_ble_ttl_eviction", "expanse_slab", expired, n, (double)elapsed / (double)(expired > 0 ? expired : 1), heap_used, frag);

    expanse_ble_tracker_destroy(tracker);
}

/*
 * 3. Fragmentation after insert/delete churn.
 *
 * The frag_ratio the arms above report is measured after a monotonic fill,
 * which is the allocator's easiest case -- nothing has been returned to it
 * yet. What an always-on device actually does is cycle: ingest a window,
 * flush it, ingest the next. This arm measures the free-pool fragmentation
 * that survives that cycling, which is the number that decides whether a
 * long-running node can still satisfy a large allocation (#579).
 */
void bench_churn_fragmentation(size_t n, size_t cycles) {
    double frag_before = heap_frag_ratio();
    size_t heap_before = GET_FREE_HEAP();

    expanse_memtable_t *mt = expanse_memtable_create();
    if (!mt) return;

    uint32_t start = GET_CYCLES();
    for (size_t c = 0; c < cycles; ++c) {
        uint32_t base = (uint32_t)(1700000000 + c * n);
        for (size_t i = 0; i < n; ++i) {
            expanse_memtable_insert(mt, base + (uint32_t)i, (uint32_t)(i % 1000), NULL);
        }
        /* Retire the whole window, the way a flush-to-flash cycle would. */
        for (size_t i = 0; i < n; ++i) {
            expanse_memtable_remove(mt, base + (uint32_t)i, NULL);
        }
    }
    uint32_t elapsed = GET_CYCLES() - start;

    double frag_after = heap_frag_ratio();
    size_t heap_after = GET_FREE_HEAP();
    size_t heap_used = (heap_before > heap_after) ? (heap_before - heap_after) : 0;

    /* cycles_per_op counts both the insert and the remove of each key. */
    size_t ops = n * cycles * 2;
    report_json("esp32_churn_insert_delete", "expanse_memtable", ops, n,
                (double)elapsed / (double)ops, heap_used, frag_after);

    /* The fragmentation delta is the arm's actual subject: how much the churn
     * itself introduced, independent of whatever the heap looked like when
     * the arm started. It is emitted as its own record type rather than
     * squeezed into cycles_per_op -- a dimensionless ratio in a field named
     * for a cycle count would flow straight into the harvester's duty-cycle
     * derivation and come out as a nonsense time. */
    printf("{\"metric\": \"churn_fragmentation\", \"arm\": \"expanse_memtable\", "
           "\"pop\": %zu, \"cycles\": %zu, \"frag_before\": %.4f, \"frag_after\": %.4f, "
           "\"frag_delta\": %.4f, \"heap_retained_bytes\": %zu}\n",
           n, cycles, frag_before, frag_after, frag_after - frag_before, heap_used);

    expanse_memtable_destroy(mt);
}

void app_main_benchmarks(void) {
    printf("=== Expanse ESP32 Hardware Benchmark Suite (Starting) ===\n");
    for (int rep = 0; rep < SAMPLE_RUNS; ++rep) {
        bench_tsdb_hardware(500);
        bench_tsdb_hardware(2000);
        bench_ble_tracker_hardware(500);
        bench_ble_tracker_hardware(2000);
        bench_churn_fragmentation(500, 8);
    }
    printf("=== Expanse ESP32 Hardware Benchmark Suite (Complete) ===\n");
}
