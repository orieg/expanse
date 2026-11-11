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
 *
 * The draws come from splitmix64, not stress-ng's mwc32, so the keys follow
 * the stressor's distribution and not its exact sequence. Every arm draws
 * the same keys. `repeats` repeats the measured phase on the same array, for
 * the sparsematrix shapes only. perf stat runs with `-D -1 --control
 * fifo:<ctl>,<ack>`, so its counters start disabled; this program enables
 * them just before the measured phase and disables them just after. Built
 * against this repository's clean-room include/Judy.h, and linked against
 * whichever libJudy.so.1 the dynamic loader finds.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
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

static int judy_delete(uint64_t n, long repeats)
{
	Pvoid_t a = NULL;
	PWord_t pv;
	uint64_t i;
	int rc;
	long r;

	for (r = 0; r < repeats; r++) {
		for (i = 0; i < n; i++) {
			JLI(pv, a, (Word_t)gen_index(i));
			if (pv == NULL || pv == PJERR)
				return 2;
			*pv = i;
		}
		perf_ctl("enable\n");
		for (i = n; i-- > 0;) {
			JLD(rc, a, (Word_t)gen_index(i));
			if (rc != 1)
				return 2;
		}
		perf_ctl("disable\n");
		if (a != NULL)
			return 2;
	}
	printf("deleted %lu x %ld\n", (unsigned long)n, repeats);
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
	ctl_fd = open(argv[3], O_WRONLY);
	ack_fd = open(argv[4], O_RDONLY);
	if (ctl_fd < 0 || ack_fd < 0 || repeats < 1) {
		fprintf(stderr, "STOP: cannot open perf control fifos or bad repeats\n");
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
	fprintf(stderr, "STOP: unknown shape %s\n", argv[1]);
	return 1;
}
