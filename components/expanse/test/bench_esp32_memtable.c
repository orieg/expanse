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
#include "twin_containers.h"

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
/* size_t, not int: both feed `%zu` in report_json / report_skipped, and the
 * ESP_PLATFORM branch above yields size_t. */
#define GET_FREE_HEAP() ((size_t)(1024 * 1024))
#define GET_LARGEST_BLOCK() ((size_t)(1024 * 1024))
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
/* A twin or subject that could not be constructed used to fall out of the
 * suite silently -- the arm simply never printed, and the published table
 * acquired a hole with nothing to say why. That is a fail-quiet path
 * (AGENTS.md 8.1): the harvester now sees a record naming the arm, the
 * population it was asked for, and the heap state that refused it. */
static void report_skipped(const char *arm, size_t n) {
    printf("{\"skipped\": true, \"arm\": \"%s\", \"n\": %zu, \"reason\": \"create returned NULL\", "
           "\"free_heap\": %zu, \"largest_block\": %zu}\n",
           arm, n, GET_FREE_HEAP(), GET_LARGEST_BLOCK());
}

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

/*
 * Sink for every arm's aggregate, so no arm's timed work can be elided while
 * another's survives (§8.6). Volatile, written by every arm, read never.
 */
static volatile uint64_t g_sink;

/*
 * The telemetry arms are generated from one macro so their loop structure is
 * identical by construction rather than by inspection. Each arm builds
 * outside the timed window, times exactly the insert loop, then times exactly
 * the aggregation, and reports through the same report_json (§8.3, §8.6).
 *
 * Keys, values, range and repetition count are fixed here and shared by every
 * arm: symmetric workload parameters are the whole point of the comparison.
 */
/*
 * Key order is a parameter, not a constant, because the twins' standing
 * depends on it. Monotonic timestamps are the sorted array's best case --
 * every insert appends and it never memmoves -- so measuring only that
 * regime would report the twin at its strongest and call it the comparison.
 * The shuffled order below is the same key SET in a different sequence, so
 * the arms still store identical data and differ only in arrival order.
 *
 * The shuffle is built once, outside every timed region, and shared verbatim
 * by all four arms (§8.3 symmetric workload).
 */
#define TSDB_MAX_N 2000
static uint32_t g_tsdb_keys[TSDB_MAX_N];

static void tsdb_fill_keys(size_t n, bool shuffled) {
    for (size_t i = 0; i < n; ++i) g_tsdb_keys[i] = (uint32_t)(1700000000u + i);
    if (!shuffled) return;
    /* Fisher-Yates with a fixed xorshift32 seed: same permutation every run
     * and every arm, so the comparison is reproducible. */
    uint32_t rng = 0x9E3779B9u;
    for (size_t i = n - 1; i > 0; --i) {
        rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
        size_t j = (size_t)(rng % (uint32_t)(i + 1));
        uint32_t t = g_tsdb_keys[i]; g_tsdb_keys[i] = g_tsdb_keys[j]; g_tsdb_keys[j] = t;
    }
}

#define TSDB_KEY(i)   ((uint32_t)(1700000000u + (i)))
#define TSDB_VAL(i)   ((uint32_t)((i) % 1000))
#define TSDB_AGG_LO   (1700000000u + 100u)
#define TSDB_AGG_HI   (1700000000u + 600u)

#define DEFINE_TSDB_ARM(fn_name, arm_str, ctype, create_expr, insert_stmt,      \
                        agg_stmt, destroy_stmt)                                 \
static void fn_name(size_t n, const char *ingest_bench, const char *agg_bench) { \
    size_t heap_before = GET_FREE_HEAP();                                       \
    ctype *c = (create_expr);                                                   \
    if (!c) { report_skipped(arm_str, n); return; }                              \
                                                                                \
    /* Warmup on keys the timed loop does not use, then discard the             \
     * container so no arm enters its measurement warm and another cold. */     \
    for (size_t i = 0; i < 100; ++i) { size_t idx = i; (void)idx; insert_stmt; }\
    destroy_stmt;                                                               \
    c = (create_expr);                                                          \
    if (!c) { report_skipped(arm_str, n); return; }                              \
                                                                                \
    uint32_t start = GET_CYCLES();                                              \
    for (size_t idx = 0; idx < n; ++idx) { insert_stmt; }                       \
    uint32_t elapsed = GET_CYCLES() - start;                                    \
    double cycles_per_op = (double)elapsed / (double)n;                         \
                                                                                \
    size_t heap_after = GET_FREE_HEAP();                                        \
    size_t heap_used = (heap_before > heap_after) ? (heap_before - heap_after) : 0; \
    double frag = heap_frag_ratio();                                            \
    report_json(ingest_bench, arm_str, n, n, cycles_per_op, heap_used, frag);   \
                                                                                \
    expanse_memtable_agg_t agg;                                                 \
    start = GET_CYCLES();                                                       \
    agg_stmt;                                                                   \
    elapsed = GET_CYCLES() - start;                                             \
    g_sink += agg.sum_val + agg.count;                                          \
    /* Divide by the keys actually aggregated, never a literal. The range       \
     * [+100,+600] holds 400 keys at pop=500 and 501 at pop=2000, so a          \
     * hardcoded 500 understated pop=500 by 25% and overstated pop=2000 by      \
     * 0.2%. All four arms fold the same key set, so `agg.count` is identical   \
     * across them and the comparison stays symmetric (§8.2 derived outputs).   \
     * Ratios within one population were unaffected -- the divisor cancelled -- \
     * but absolute cycles/key and every cross-population reading were not. */  \
    size_t agg_ops = (agg.count > 0) ? agg.count : 1;                         \
    report_json(agg_bench, arm_str, agg_ops, n,                                 \
                (double)elapsed / (double)agg_ops, heap_used, frag);            \
                                                                                \
    destroy_stmt;                                                               \
}

DEFINE_TSDB_ARM(bench_tsdb_expanse, "expanse_memtable", expanse_memtable_t,
                expanse_memtable_create(),
                expanse_memtable_insert(c, g_tsdb_keys[idx], TSDB_VAL(idx), NULL),
                expanse_memtable_aggregate_range(c, TSDB_AGG_LO, TSDB_AGG_HI, &agg),
                expanse_memtable_destroy(c))

DEFINE_TSDB_ARM(bench_tsdb_hash, "hash_open_addressing", twin_hash_t,
                twin_hash_create(n),
                twin_hash_insert(c, g_tsdb_keys[idx], TSDB_VAL(idx)),
                twin_hash_aggregate(c, TSDB_AGG_LO, TSDB_AGG_HI, &agg),
                twin_hash_destroy(c))

DEFINE_TSDB_ARM(bench_tsdb_sorted, "sorted_array", twin_sorted_t,
                twin_sorted_create(n),
                twin_sorted_insert(c, g_tsdb_keys[idx], TSDB_VAL(idx)),
                twin_sorted_aggregate(c, TSDB_AGG_LO, TSDB_AGG_HI, &agg),
                twin_sorted_destroy(c))

DEFINE_TSDB_ARM(bench_tsdb_ring, "ring_buffer", twin_ring_t,
                twin_ring_create(n),
                twin_ring_insert(c, g_tsdb_keys[idx], TSDB_VAL(idx)),
                twin_ring_aggregate(c, TSDB_AGG_LO, TSDB_AGG_HI, &agg),
                twin_ring_destroy(c))

/* 1. Sensor TSDB Ingestion & Range Aggregation, in both key orders. */
void bench_tsdb_hardware(size_t n) {
    if (n > TSDB_MAX_N) return;

    tsdb_fill_keys(n, false);
    bench_tsdb_expanse(n, "esp32_tsdb_ingest", "esp32_tsdb_aggregate_500");
    bench_tsdb_hash(n, "esp32_tsdb_ingest", "esp32_tsdb_aggregate_500");
    bench_tsdb_sorted(n, "esp32_tsdb_ingest", "esp32_tsdb_aggregate_500");
    bench_tsdb_ring(n, "esp32_tsdb_ingest", "esp32_tsdb_aggregate_500");

    /* The ring buffer is deliberately absent from the shuffled arms: it has
     * no key order to violate, so re-running it would republish the same
     * append under a name implying it handled out-of-order arrival. */
    tsdb_fill_keys(n, true);
    bench_tsdb_expanse(n, "esp32_tsdb_ingest_shuffled", "esp32_tsdb_aggregate_500_shuffled");
    bench_tsdb_hash(n, "esp32_tsdb_ingest_shuffled", "esp32_tsdb_aggregate_500_shuffled");
    bench_tsdb_sorted(n, "esp32_tsdb_ingest_shuffled", "esp32_tsdb_aggregate_500_shuffled");
}

/* 2. BLE Asset Tracker Sighting & TTL Eviction */
/*
 * The BLE arms, same treatment: one macro, three containers, identical
 * sighting stream and identical 28-byte payload (§8.16). The eviction cutoff
 * retires the older half of the population in every arm, so all three do the
 * same amount of logical work and differ only in how they find it.
 */
#define BLE_FILL_MAC(r, i)                                    \
    do {                                                      \
        (r).mac[0] = 0x00; (r).mac[1] = 0x1A; (r).mac[2] = 0x2B; \
        (r).mac[3] = (uint8_t)(((i) >> 16) & 0xFF);           \
        (r).mac[4] = (uint8_t)(((i) >> 8) & 0xFF);            \
        (r).mac[5] = (uint8_t)((i) & 0xFF);                   \
        (r).last_seen_ms = (uint32_t)(1000 + (i) * 10);       \
    } while (0)

/*
 * How much of the population the cutoff retires is the parameter that decides
 * this comparison. Expanse prunes through a by_time index, so its cost should
 * follow the number of EXPIRED entries; a table or array with no time index
 * must visit every slot, so its cost follows the TRACKED count. Measuring
 * only a cutoff that retires half the population reports the regime where the
 * sweep is cheapest per entry and would bury the reason the index exists.
 * Both regimes are run: `stale_frac` of 2 retires about half, 100 about one
 * percent.
 */
#define DEFINE_BLE_ARM(fn_name, arm_str, ctype, create_expr, record_stmt,       \
                       get_stmt, expire_expr, destroy_stmt)                     \
static void fn_name(size_t n, size_t stale_frac, const char *evict_bench,        \
                    bool report_rw) {                                            \
    size_t heap_before = GET_FREE_HEAP();                                       \
    ctype *c = (create_expr);                                                   \
    if (!c) { report_skipped(arm_str, n); return; }                              \
                                                                                \
    expanse_ble_record_t rec;                                                   \
    memset(&rec, 0, sizeof(rec));                                               \
    strcpy(rec.name, "Beacon_Node");                                            \
    rec.rssi = -68;                                                             \
    rec.distance_cm = 120;                                                      \
                                                                                \
    uint32_t start = GET_CYCLES();                                              \
    for (size_t idx = 0; idx < n; ++idx) {                                      \
        BLE_FILL_MAC(rec, idx);                                                 \
        record_stmt;                                                            \
    }                                                                           \
    uint32_t elapsed = GET_CYCLES() - start;                                    \
    double cycles_per_op = (double)elapsed / (double)n;                         \
                                                                                \
    size_t heap_after = GET_FREE_HEAP();                                        \
    size_t heap_used = (heap_before > heap_after) ? (heap_before - heap_after) : 0; \
    double frag = heap_frag_ratio();                                            \
    if (report_rw)                                                              \
        report_json("esp32_ble_sighting_record", arm_str, n, n, cycles_per_op,  \
                    heap_used, frag);                                           \
                                                                                \
    start = GET_CYCLES();                                                       \
    for (size_t idx = 0; idx < n; ++idx) {                                      \
        uint8_t mac[6] = {0x00, 0x1A, 0x2B, (uint8_t)((idx >> 16) & 0xFF),      \
                          (uint8_t)((idx >> 8) & 0xFF), (uint8_t)(idx & 0xFF)}; \
        expanse_ble_record_t out_rec;                                           \
        get_stmt;                                                               \
        g_sink += out_rec.last_seen_ms;                                         \
    }                                                                           \
    elapsed = GET_CYCLES() - start;                                             \
    if (report_rw)                                                              \
        report_json("esp32_ble_point_lookup", arm_str, n, n,                    \
                    (double)elapsed / (double)n, heap_used, frag);              \
                                                                                \
    uint32_t cutoff_ms = (uint32_t)(1000 + (n / stale_frac) * 10);              \
    start = GET_CYCLES();                                                       \
    size_t expired = (expire_expr);                                             \
    elapsed = GET_CYCLES() - start;                                             \
    g_sink += expired;                                                          \
    report_json(evict_bench, arm_str, expired, n,                               \
                (double)elapsed / (double)(expired > 0 ? expired : 1),          \
                heap_used, frag);                                               \
                                                                                \
    destroy_stmt;                                                               \
}

DEFINE_BLE_ARM(bench_ble_expanse, "expanse_slab", expanse_ble_tracker_t,
               expanse_ble_tracker_create(n),
               expanse_ble_tracker_record(c, &rec),
               expanse_ble_tracker_get(c, mac, &out_rec),
               expanse_ble_tracker_expire_stale(c, cutoff_ms),
               expanse_ble_tracker_destroy(c))

DEFINE_BLE_ARM(bench_ble_hash, "hash_open_addressing", twin_ble_hash_t,
               twin_ble_hash_create(n),
               twin_ble_hash_record(c, &rec),
               twin_ble_hash_get(c, mac, &out_rec),
               twin_ble_hash_expire_stale(c, cutoff_ms),
               twin_ble_hash_destroy(c))

DEFINE_BLE_ARM(bench_ble_scan, "linear_scan", twin_ble_scan_t,
               twin_ble_scan_create(n),
               twin_ble_scan_record(c, &rec),
               twin_ble_scan_get(c, mac, &out_rec),
               twin_ble_scan_expire_stale(c, cutoff_ms),
               twin_ble_scan_destroy(c))

/* 2. BLE Asset Tracker Sighting & TTL Eviction, in both expiry regimes. */
void bench_ble_tracker_hardware(size_t n) {
    /* Dense expiry: about half the population goes. Record and lookup are
     * reported from this pass only -- they do not depend on the cutoff, and
     * publishing them twice would double-count them in the interval. */
    bench_ble_expanse(n, 2, "esp32_ble_ttl_eviction", true);
    bench_ble_hash(n, 2, "esp32_ble_ttl_eviction", true);
    bench_ble_scan(n, 2, "esp32_ble_ttl_eviction", true);

    /* Sparse expiry: about one percent goes, which is what an always-on
     * tracker actually does on each sweep. */
    bench_ble_expanse(n, 100, "esp32_ble_ttl_eviction_sparse", false);
    bench_ble_hash(n, 100, "esp32_ble_ttl_eviction_sparse", false);
    bench_ble_scan(n, 100, "esp32_ble_ttl_eviction_sparse", false);
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
