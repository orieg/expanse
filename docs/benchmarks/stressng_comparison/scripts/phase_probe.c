/*
 * One phase of a stress-ng Judy stressor, bracketed for `perf stat`
 * (README.md, "Diagnostic: phase counters"; #1680).
 *
 *   phase_probe <shape> <repeats> <ctl_fifo> <ack_fifo>
 *
 * Shapes:
 *   sm_default  sparsematrix judy: 10,000 draws of (x, y) on [0, 500)^2, key
 *               x << 32 | y, put when absent (JLG then JLI); the measured
 *               phase is both of the stressor's get phases, the same draws
 *               again and then as many fresh draws.
 *   sm_big      the same over 1,000,000 draws on [0, 100000)^2.
 *   judy_4m     judy: JLI of gen_index(i) = (~i & 0xff) << 24 | (i & 0xffffff)
 *               for i below 4,194,304; the measured phase is the inserts.
 *   del_1k      judy's delete phase at --judy-size 1024: JLI of gen_index(i)
 *               for i below 1,024, then JLD in reverse order; the measured
 *               phase is the deletes, `repeats` builds each followed by its
 *               counted delete pass, so each array is as warm as stress-ng's.
 *   del_4m      the same at 4,194,304 keys, once (`repeats` is 1).
 *               Both delete shapes also print `phase_ns`, the CLOCK_MONOTONIC
 *               time summed over the counted delete passes, read inside the
 *               enable/disable bracket so the perf control round trips are
 *               outside it (#1679, the accounting ablation).
 *
 *   osc_1       Judy1: for each of 50,000 keys drawn by the Callgrind bench's
 *               XorShift (seed 0x0DDB1A5E5EED0001), J1S then J1U of that key on
 *               an array that starts and ends empty, `repeats` times; the
 *               shape of the bench's `set_oscillate` arm, which never
 *               allocates (#1679 promotion).
 *   j1s_clustered  Judy1: J1S of the bench's 50,000 `clustered` keys (runs of
 *               256 consecutive keys under a random base) into an empty
 *               array, `repeats` times, each array freed outside the phase.
 *   j1s_sparse  the same with the bench's `sparse` keys, i << 40.
 *   ins_1k      JudyL: JLI of gen_index(i) for i below 1,024 into an empty
 *               array, `repeats` times, each freed outside the phase.
 *               These four print `phase_ns` as the delete shapes do.
 *
 * The draws come from splitmix64, not stress-ng's mwc32, so the keys follow
 * the stressor's distribution and not its exact sequence. Every arm draws
 * the same keys. `repeats` repeats the measured phase on the same array, for
 * the sparsematrix shapes only. perf stat runs with `-D -1 --control
 * fifo:<ctl>,<ack>`, so its counters start disabled; this program enables
 * them just before the measured phase and disables them just after. Built
 * against this repository's clean-room include/Judy.h, and linked against
 * whichever libJudy.so.1 the dynamic loader finds.
 *
 * Passing `-` for both fifos runs without perf control, for a run under
 * Callgrind, which brackets the phase by function instead.
 */
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <time.h>
#include <unistd.h>

#include "Judy.h"

static uint64_t sm_state;

static uint64_t splitmix64(void)
{
	uint64_t z = (sm_state += 0x9E3779B97F4A7C15ULL);
	z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
	z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
	return z ^ (z >> 31);
}

static int ctl_fd = -1, ack_fd = -1;

static void perf_ctl(const char *cmd)
{
	char ack[8];
	if (ctl_fd < 0)
		return;
	if (write(ctl_fd, cmd, strlen(cmd)) < 0 || read(ack_fd, ack, sizeof(ack)) <= 0) {
		fprintf(stderr, "STOP: perf control '%s' failed\n", cmd);
		exit(3);
	}
}

static Word_t sm_key(uint64_t size)
{
	uint64_t x = splitmix64() % size, y = splitmix64() % size;
	return (Word_t)((x << 32) | y);
}

static int sparsematrix(uint64_t items, uint64_t size, long repeats)
{
	Pvoid_t a = NULL;
	PWord_t pv;
	uint64_t i, seed = 0x2545F4914F6CDD1DULL;
	Word_t sink = 0, hits = 0;
	long r;

	sm_state = seed;
	for (i = 0; i < items; i++) {
		Word_t k = sm_key(size);
		JLG(pv, a, k);
		if (pv == NULL) {
			JLI(pv, a, k);
			if (pv == NULL || pv == PJERR)
				return 2;
			*pv = (k & 0xffffffff) | 1;
		}
	}
	perf_ctl("enable\n");
	for (r = 0; r < repeats; r++) {
		sm_state = seed;
		for (i = 0; i < items; i++) {
			JLG(pv, a, sm_key(size));
			if (pv) {
				sink ^= *pv;
				hits++;
			}
		}
		for (i = 0; i < items; i++) {
			JLG(pv, a, sm_key(size));
			if (pv) {
				sink ^= *pv;
				hits++;
			}
		}
	}
	perf_ctl("disable\n");
	printf("hits %lu sink %lu\n", (unsigned long)hits, (unsigned long)sink);
	{
		Word_t freed;
		JLFA(freed, a);
		(void)freed;
	}
	return hits >= items * (uint64_t)repeats ? 0 : 2;
}

static int judy_4m(void)
{
	Pvoid_t a = NULL;
	PWord_t pv;
	uint64_t i, n = 4194304;
	Word_t freed;

	perf_ctl("enable\n");
	for (i = 0; i < n; i++) {
		JLI(pv, a, (Word_t)(((~i & 0xff) << 24) | (i & 0xffffff)));
		if (pv == NULL || pv == PJERR)
			return 2;
		*pv = i;
	}
	perf_ctl("disable\n");
	JLFA(freed, a);
	printf("inserted %lu freed %lu\n", (unsigned long)n, (unsigned long)freed);
	return 0;
}

static uint64_t gen_index(uint64_t i)
{
	return ((~i & 0xff) << 24) | (i & 0xffffff);
}

#define BENCH_POP 50000

/* The Callgrind bench's XorShift (`crates/expanse/benches/instructions.rs`). */
static uint64_t xs_state;

static uint64_t xorshift(void)
{
	uint64_t x = xs_state;
	x ^= x << 13;
	x ^= x >> 7;
	x ^= x << 17;
	xs_state = x;
	return x;
}

static void bench_keys(const char *dist, Word_t *out)
{
	uint64_t i, base = 0;

	xs_state = 0x0DDB1A5E5EED0001ULL;
	for (i = 0; i < BENCH_POP; i++) {
		if (strcmp(dist, "random") == 0)
			out[i] = (Word_t)xorshift();
		else if (strcmp(dist, "clustered") == 0) {
			if (i % 256 == 0)
				base = xorshift() & ~(uint64_t)0xff;
			out[i] = (Word_t)(base + (i % 256));
		} else
			out[i] = (Word_t)(i << 40);
	}
}

static uint64_t ns_between(const struct timespec *t0, const struct timespec *t1)
{
	return (uint64_t)(t1->tv_sec - t0->tv_sec) * 1000000000ULL
		+ (uint64_t)t1->tv_nsec - (uint64_t)t0->tv_nsec;
}

static int judy1_oscillate(long repeats)
{
	static Word_t keys[BENCH_POP];
	Pvoid_t a = NULL;
	struct timespec t0, t1;
	uint64_t phase_ns = 0, sink = 0;
	long r;
	int rc, i;

	bench_keys("random", keys);
	for (r = 0; r < repeats; r++) {
		perf_ctl("enable\n");
		clock_gettime(CLOCK_MONOTONIC, &t0);
		for (i = 0; i < BENCH_POP; i++) {
			J1S(rc, a, keys[i]);
			if (rc == JERR)
				return 2;
			J1U(rc, a, keys[i]);
			sink += (uint64_t)rc;
		}
		clock_gettime(CLOCK_MONOTONIC, &t1);
		perf_ctl("disable\n");
		phase_ns += ns_between(&t0, &t1);
		if (a != NULL)
			return 2;
	}
	printf("oscillated %d x %ld sink %llu\n", BENCH_POP, repeats, (unsigned long long)sink);
	printf("phase_ns %llu\n", (unsigned long long)phase_ns);
	return sink == (uint64_t)BENCH_POP * (uint64_t)repeats ? 0 : 2;
}

static int judy1_insert(const char *dist, long repeats)
{
	static Word_t keys[BENCH_POP];
	Pvoid_t a = NULL;
	struct timespec t0, t1;
	uint64_t phase_ns = 0;
	Word_t freed, count = 0;
	long r;
	int rc, i;

	bench_keys(dist, keys);
	for (r = 0; r < repeats; r++) {
		perf_ctl("enable\n");
		clock_gettime(CLOCK_MONOTONIC, &t0);
		for (i = 0; i < BENCH_POP; i++) {
			J1S(rc, a, keys[i]);
			if (rc == JERR)
				return 2;
		}
		clock_gettime(CLOCK_MONOTONIC, &t1);
		perf_ctl("disable\n");
		phase_ns += ns_between(&t0, &t1);
		J1C(count, a, 0, -1);
		J1FA(freed, a);
		(void)freed;
	}
	printf("inserted %s %lu x %ld\n", dist, (unsigned long)count, repeats);
	printf("phase_ns %llu\n", (unsigned long long)phase_ns);
	return 0;
}

static int judyl_insert_1k(long repeats)
{
	Pvoid_t a = NULL;
	PWord_t pv;
	struct timespec t0, t1;
	uint64_t phase_ns = 0, i;
	Word_t freed;
	long r;

	for (r = 0; r < repeats; r++) {
		perf_ctl("enable\n");
		clock_gettime(CLOCK_MONOTONIC, &t0);
		for (i = 0; i < 1024; i++) {
			JLI(pv, a, (Word_t)gen_index(i));
			if (pv == NULL || pv == PJERR)
				return 2;
			*pv = i;
		}
		clock_gettime(CLOCK_MONOTONIC, &t1);
		perf_ctl("disable\n");
		phase_ns += ns_between(&t0, &t1);
		JLFA(freed, a);
		(void)freed;
	}
	printf("inserted 1024 x %ld\n", repeats);
	printf("phase_ns %llu\n", (unsigned long long)phase_ns);
	return 0;
}

static int judy_delete(uint64_t n, long repeats)
{
	Pvoid_t a = NULL;
	PWord_t pv;
	uint64_t i;
	int rc;
	long r;
	struct timespec t0, t1;
	uint64_t phase_ns = 0;

	for (r = 0; r < repeats; r++) {
		for (i = 0; i < n; i++) {
			JLI(pv, a, (Word_t)gen_index(i));
			if (pv == NULL || pv == PJERR)
				return 2;
			*pv = i;
		}
		perf_ctl("enable\n");
		clock_gettime(CLOCK_MONOTONIC, &t0);
		for (i = n; i-- > 0;) {
			JLD(rc, a, (Word_t)gen_index(i));
			if (rc != 1)
				return 2;
		}
		clock_gettime(CLOCK_MONOTONIC, &t1);
		perf_ctl("disable\n");
		phase_ns += (uint64_t)(t1.tv_sec - t0.tv_sec) * 1000000000ULL
			+ (uint64_t)t1.tv_nsec - (uint64_t)t0.tv_nsec;
		if (a != NULL)
			return 2;
	}
	printf("deleted %lu x %ld\n", (unsigned long)n, repeats);
	printf("phase_ns %llu\n", (unsigned long long)phase_ns);
	return 0;
}

int main(int argc, char **argv)
{
	long repeats;

	if (argc != 5) {
		fprintf(stderr, "usage: phase_probe <shape> <repeats> <ctl_fifo> <ack_fifo>\n");
		return 1;
	}
	repeats = strtol(argv[2], NULL, 10);
	if (strcmp(argv[3], "-") != 0 || strcmp(argv[4], "-") != 0) {
		ctl_fd = open(argv[3], O_WRONLY);
		ack_fd = open(argv[4], O_RDONLY);
		if (ctl_fd < 0 || ack_fd < 0) {
			fprintf(stderr, "STOP: cannot open perf control fifos\n");
			return 1;
		}
	}
	if (repeats < 1) {
		fprintf(stderr, "STOP: bad repeats\n");
		return 1;
	}
	if (strcmp(argv[1], "sm_default") == 0)
		return sparsematrix(10000, 500, repeats);
	if (strcmp(argv[1], "sm_big") == 0)
		return sparsematrix(1000000, 100000, repeats);
	if (strcmp(argv[1], "judy_4m") == 0)
		return judy_4m();
	if (strcmp(argv[1], "del_1k") == 0)
		return judy_delete(1024, repeats);
	if (strcmp(argv[1], "del_4m") == 0)
		return judy_delete(4194304, 1);
	if (strcmp(argv[1], "osc_1") == 0)
		return judy1_oscillate(repeats);
	if (strcmp(argv[1], "j1s_clustered") == 0)
		return judy1_insert("clustered", repeats);
	if (strcmp(argv[1], "j1s_sparse") == 0)
		return judy1_insert("sparse", repeats);
	if (strcmp(argv[1], "ins_1k") == 0)
		return judyl_insert_1k(repeats);
	fprintf(stderr, "STOP: unknown shape %s\n", argv[1]);
	return 1;
}
