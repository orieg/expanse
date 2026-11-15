/*
 * Focused harness over zmap's response-dedup call pattern.
 *
 * Drives zmap's own lib/cachehash.c (compiled unchanged from zmap at the
 * recorded SHA) exactly as src/recv.c:handle_packet does under
 * DEDUP_METHOD_WINDOW:
 *
 *     target_t t = {.ip = src_ip, .port = src_port, .status = 0};
 *     if (cachehash_get(ch, &t, sizeof(target_t))) is_repeat = 1;
 *     else cachehash_put(ch, &t, sizeof(target_t), (void *)1);
 *
 * One measurement per process. Keys are generated before the timed region;
 * cachehash_init before it and cachehash_free after it.
 *
 * Workload shape
 * | field              | value |
 * |--------------------|-------|
 * | workload_id        | zmap_dedup_m1_miss / zmap_dedup_m2_dup50 / zmap_dedup_m3_evict |
 * | group              | zmap cachehash (JudyHS JHSG/JHSI/JHSD, 8-byte keys) |
 * | population         | M1: 1,048,576 targets, window 1,000,000; M2: 1,048,576 targets x2 responses, window 1,000,000; M3: 4,194,304 targets, window 1,000,000 |
 * | insertion_order    | generator (bijective 32-bit mix of a counter: pseudo-random, like zmap's cyclic-group order) |
 * | probes_and_reuse   | M1/M3: each key probed once (get miss + put); M2: get miss + put, then get hit immediately (probes=2 back to back) |
 * | hit_rate           | M1 0%, M2 50% of gets, M3 0% (every put after the first 1,000,000 evicts the LRU entry) |
 * | miss_gen_method    | a miss is a key not yet inserted, drawn from the same generator (never a transform of a present key) |
 * | value_dereference  | returned node pointer dereferenced by cachehash itself (n->data); hits summed into a sink |
 * | measured_region    | the op sequence only; key generation, cachehash_init and cachehash_free outside |
 * | arm_symmetry       | identical source and keys; arms differ only in Judy.h + libJudy at build/link time |
 * | statistics         | n rounds per run, arms interleaved per round, one process per measurement, BCa 95% |
 * | verdict            | see PREREG.md |
 */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>

#include "cachehash.h"

/* Same layout as zmap src/shard.h:49-53. */
typedef struct target {
	uint32_t ip;
	uint16_t port;
	uint8_t status;
} target_t;

static uint32_t mix32(uint32_t x)
{
	/* Bijective on uint32_t (lowbias32): distinct counters give distinct ips. */
	x ^= x >> 16;
	x *= 0x7feb352dU;
	x ^= x >> 15;
	x *= 0x846ca68bU;
	x ^= x >> 16;
	return x;
}

static const uint16_t PORTS[4] = {80, 443, 8080, 8443};

static double now_ns(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return (double)ts.tv_sec * 1e9 + (double)ts.tv_nsec;
}

int main(int argc, char **argv)
{
	if (argc != 3) {
		fprintf(stderr, "usage: %s <m1|m2|m3> <seed>\n", argv[0]);
		return 2;
	}
	const char *wl = argv[1];
	uint32_t seed = (uint32_t)strtoul(argv[2], NULL, 0);
	size_t ntargets, window = 1000000;
	int dup;
	if (!strcmp(wl, "m1")) {
		ntargets = 1u << 20;
		dup = 0;
	} else if (!strcmp(wl, "m2")) {
		ntargets = 1u << 20;
		dup = 1;
	} else if (!strcmp(wl, "m3")) {
		ntargets = 1u << 22;
		dup = 0;
	} else {
		fprintf(stderr, "unknown workload %s\n", wl);
		return 2;
	}

	target_t *keys = malloc(ntargets * sizeof(target_t));
	if (!keys) {
		perror("malloc");
		return 1;
	}
	for (size_t i = 0; i < ntargets; i++) {
		/* ip distinct per target (mix32 is a bijection; seed offsets the counter). */
		memset(&keys[i], 0, sizeof(target_t));
		keys[i].ip = htonl(mix32((uint32_t)i + seed * 0x9e3779b9U));
		keys[i].port = htons(PORTS[i & 3]);
		keys[i].status = 0;
	}

	cachehash *ch = cachehash_init(window, NULL);
	struct rusage ru0, ru1;
	getrusage(RUSAGE_SELF, &ru0);

	uint64_t hits = 0, misses = 0;
	double t0 = now_ns();
	for (size_t i = 0; i < ntargets; i++) {
		int reps = dup ? 2 : 1;
		for (int r = 0; r < reps; r++) {
			if (cachehash_get(ch, &keys[i], sizeof(target_t))) {
				hits++;
			} else {
				misses++;
				cachehash_put(ch, &keys[i], sizeof(target_t),
					      (void *)1);
			}
		}
	}
	double t1 = now_ns();
	getrusage(RUSAGE_SELF, &ru1);

	uint64_t responses = hits + misses;
	uint64_t expect_hits = dup ? ntargets : 0;
	int ok = (hits == expect_hits) && (misses == ntargets);
	printf("{\"workload\":\"%s\",\"seed\":%u,\"responses\":%llu,\"hits\":%llu,"
	       "\"misses\":%llu,\"ok\":%s,\"elapsed_ns\":%.0f,\"ns_per_response\":%.3f,"
	       "\"maxrss_kb_before\":%ld,\"maxrss_kb_after\":%ld}\n",
	       wl, seed, (unsigned long long)responses, (unsigned long long)hits,
	       (unsigned long long)misses, ok ? "true" : "false", t1 - t0,
	       (t1 - t0) / (double)responses, ru0.ru_maxrss, ru1.ru_maxrss);
	cachehash_free(ch, NULL);
	free(keys);
	return ok ? 0 : 3;
}
