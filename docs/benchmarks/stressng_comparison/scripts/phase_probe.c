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
 *               for i below 4,194,304; the measured phase is the inserts,
 *               `repeats` times (1 except under SNG_LAYOUT=3), each array
 *               freed outside the phase.
 *   del_1k      judy's delete phase at --judy-size 1024: JLI of gen_index(i)
 *               for i below 1,024, then JLD in reverse order; the measured
 *               phase is the deletes, `repeats` builds each followed by its
 *               counted delete pass, so each array is as warm as stress-ng's.
 *   del_4m      the same at 4,194,304 keys, `repeats` times (1 except
 *               under SNG_LAYOUT=3).
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
 * With SNG_LAYOUT=1 in the environment, the 4 Mi shapes print the process's
 * memory layout outside the counted phase, while the array is populated:
 * `layout anonhuge_kb` (AnonHugePages from /proc/self/smaps_rollup), and one
 * `layout map <start> <end> <name>` per [heap] mapping and per anonymous
 * mapping of at least 1 MiB (/proc/self/maps). Without it nothing changes.
 * With SNG_LAYOUT=2 they also walk the [heap] pages through /proc/self/pagemap
 * and print aggregates of their physical frames, never a frame number:
 * `layout pfn_pages` (present pages), `layout pfn_zero` (present pages whose
 * frame reads 0, which is what an unprivileged reader sees),
 * `layout pfn_runs` and `layout pfn_longest` (maximal runs of virtually
 * consecutive pages on consecutive frames, and the longest), and
 * `layout l2_colour` (32 counts of frame mod 32, the L2 page colour: 2,048
 * sets of 64-byte lines span 128 KiB, 32 pages).
 * With SNG_LAYOUT=3 the 4 Mi shapes run their phase `repeats` times in one
 * process (judy_4m frees the array outside the phase between repetitions;
 * del_4m rebuilds it outside the phase) and, per repetition and outside the
 * phase, print `layout pfn_rep <r> <pages> <zero> <same>`: the [heap]
 * mapping's present pages, those whose frame reads 0, and those on the same
 * frame at the same virtual page as in the previous repetition. Frame numbers stay in
 * this process's memory and are never printed.
 * With SNG_SELFCOUNT=1 the program counts its own phase instead of perf
 * stat: one counter group on the cpu_core PMU (cycles, instructions,
 * mem_load_retired.l1_miss, mem_load_retired.fb_hit, l1d.replacement,
 * l1d.hwpf_miss; user mode only), reset and enabled just before each
 * repetition and disabled just after, and prints
 * `rep <r> cpu <before> <after> enabled <ns> running <ns> counts <6 values>`.
 * Run it with `-` for both fifos.
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
#define _GNU_SOURCE
#include <sched.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <time.h>
#include <unistd.h>
#include <linux/perf_event.h>
#include <sys/ioctl.h>
#include <sys/syscall.h>

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

/* SNG_SELFCOUNT=1: the phase's own counter group (header comment). */
#define SC_N 6
static int sc_fd[SC_N] = {-1, -1, -1, -1, -1, -1};
static int sc_cpu0;

static int sc_open(void)
{
	/* cpu_core raw encodings, from the host's `perf list --details`. */
	static const uint64_t raw[4] = {0x08d1, 0x40d1, 0x0151, 0x2051};
	struct perf_event_attr at;
	unsigned long type;
	FILE *f = fopen("/sys/bus/event_source/devices/cpu_core/type", "r");
	int k;

	if (f == NULL || fscanf(f, "%lu", &type) != 1) {
		fprintf(stderr, "STOP: no cpu_core PMU type\n");
		return -1;
	}
	fclose(f);
	for (k = 0; k < SC_N; k++) {
		memset(&at, 0, sizeof at);
		at.size = sizeof at;
		if (k < 2) {
			/* Hybrid extended type: the generic event on the cpu_core PMU. */
			at.type = PERF_TYPE_HARDWARE;
			at.config = (k == 0 ? PERF_COUNT_HW_CPU_CYCLES : PERF_COUNT_HW_INSTRUCTIONS)
				| ((uint64_t)type << 32);
		} else {
			at.type = (uint32_t)type;
			at.config = raw[k - 2];
		}
		at.disabled = k == 0;
		at.exclude_kernel = 1;
		at.exclude_hv = 1;
		at.read_format = PERF_FORMAT_GROUP | PERF_FORMAT_TOTAL_TIME_ENABLED
			| PERF_FORMAT_TOTAL_TIME_RUNNING;
		sc_fd[k] = (int)syscall(SYS_perf_event_open, &at, 0, -1, k ? sc_fd[0] : -1, 0);
		if (sc_fd[k] < 0) {
			fprintf(stderr, "STOP: perf_event_open failed for counter %d\n", k);
			return -1;
		}
	}
	return 0;
}

static void sc_begin(void)
{
	if (sc_fd[0] < 0)
		return;
	sc_cpu0 = sched_getcpu();
	ioctl(sc_fd[0], PERF_EVENT_IOC_RESET, PERF_IOC_FLAG_GROUP);
	ioctl(sc_fd[0], PERF_EVENT_IOC_ENABLE, PERF_IOC_FLAG_GROUP);
}

static int sc_end(long r)
{
	uint64_t v[3 + SC_N];
	int k;

	if (sc_fd[0] < 0)
		return 0;
	ioctl(sc_fd[0], PERF_EVENT_IOC_DISABLE, PERF_IOC_FLAG_GROUP);
	if (read(sc_fd[0], v, sizeof v) != (ssize_t)sizeof v || v[0] != SC_N) {
		fprintf(stderr, "STOP: counter group read failed\n");
		return -1;
	}
	printf("rep %ld cpu %d %d enabled %llu running %llu counts", r, sc_cpu0, sched_getcpu(),
	       (unsigned long long)v[1], (unsigned long long)v[2]);
	for (k = 0; k < SC_N; k++)
		printf(" %llu", (unsigned long long)v[3 + k]);
	printf("\n");
	return 0;
}

/* SNG_LAYOUT=3: the previous repetition's [heap] frames, kept in memory only. */
static uint64_t *prev_pfn, *cur_pfn;
static unsigned long prev_lo, prev_n;

static void frames_rep(long r)
{
	char line[512];
	FILE *f = fopen("/proc/self/maps", "r");
	unsigned long lo = 0, hi = 0, va, n, pages = 0, zero = 0, same = 0, i;
	uint64_t e;
	int fd;

	if (f == NULL)
		return;
	while (fgets(line, sizeof line, f) != NULL) {
		unsigned long a, b;
		char name[256] = "";
		if (sscanf(line, "%lx-%lx %*s %*s %*s %*s %255s", &a, &b, name) >= 2
		    && strcmp(name, "[heap]") == 0) {
			lo = a;
			hi = b;
		}
	}
	fclose(f);
	fd = open("/proc/self/pagemap", O_RDONLY);
	n = hi > lo ? (hi - lo) / 4096 : 0;
	cur_pfn = n ? calloc(n, sizeof *cur_pfn) : NULL;
	if (fd < 0 || cur_pfn == NULL) {
		printf("layout pfn_error rep\n");
		if (fd >= 0)
			close(fd);
		return;
	}
	for (va = lo, i = 0; va < hi; va += 4096, i++) {
		if (pread(fd, &e, sizeof e, (off_t)(va / 4096 * 8)) != (ssize_t)sizeof e)
			break;
		if (!(e >> 63))
			continue;
		e &= (1ULL << 55) - 1;
		pages++;
		if (e == 0) {
			zero++;
			continue;
		}
		cur_pfn[i] = e;
		if (prev_pfn != NULL && va >= prev_lo && (va - prev_lo) / 4096 < prev_n
		    && prev_pfn[(va - prev_lo) / 4096] == e)
			same++;
	}
	close(fd);
	free(prev_pfn);
	prev_pfn = cur_pfn;
	prev_lo = lo;
	prev_n = n;
	/* `same`: present pages on the same frame at the same virtual page as in
	 * the previous repetition (all of them for repetition 0). */
	printf("layout pfn_rep %ld %lu %lu %lu\n", r, pages, zero, r == 0 ? pages - zero : same);
}

static int layout3(void)
{
	const char *mode = getenv("SNG_LAYOUT");
	return mode != NULL && strcmp(mode, "3") == 0;
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

static void print_frames(unsigned long lo, unsigned long hi)
{
	unsigned long va, pages = 0, zero = 0, runs = 0, longest = 0, cur = 0;
	unsigned long colour[32] = {0};
	uint64_t prev = 0, e;
	int have_prev = 0, fd, c;

	fd = open("/proc/self/pagemap", O_RDONLY);
	if (fd < 0) {
		printf("layout pfn_error open\n");
		return;
	}
	for (va = lo; va < hi; va += 4096) {
		if (pread(fd, &e, sizeof e, (off_t)(va / 4096 * 8)) != (ssize_t)sizeof e) {
			printf("layout pfn_error read\n");
			close(fd);
			return;
		}
		if (!(e >> 63)) {
			have_prev = 0;
			continue;
		}
		e &= (1ULL << 55) - 1;
		pages++;
		if (e == 0) {
			zero++;
			have_prev = 0;
			continue;
		}
		colour[e & 31]++;
		if (have_prev && e == prev + 1) {
			cur++;
		} else {
			runs++;
			cur = 1;
		}
		if (cur > longest)
			longest = cur;
		prev = e;
		have_prev = 1;
	}
	close(fd);
	printf("layout pfn_pages %lu\nlayout pfn_zero %lu\nlayout pfn_runs %lu\nlayout pfn_longest %lu\n",
	       pages, zero, runs, longest);
	printf("layout l2_colour");
	for (c = 0; c < 32; c++)
		printf("%c%lu", c ? ',' : ' ', colour[c]);
	printf("\n");
}

static void print_layout(void)
{
	char line[512];
	FILE *f;
	const char *mode = getenv("SNG_LAYOUT");

	if (mode == NULL || (strcmp(mode, "1") != 0 && strcmp(mode, "2") != 0))
		return;
	f = fopen("/proc/self/smaps_rollup", "r");
	if (f != NULL) {
		while (fgets(line, sizeof line, f) != NULL) {
			unsigned long kb;
			if (sscanf(line, "AnonHugePages: %lu kB", &kb) == 1)
				printf("layout anonhuge_kb %lu\n", kb);
		}
		fclose(f);
	}
	f = fopen("/proc/self/maps", "r");
	if (f == NULL)
		return;
	while (fgets(line, sizeof line, f) != NULL) {
		unsigned long lo, hi;
		char name[256] = "";
		if (sscanf(line, "%lx-%lx %*s %*s %*s %*s %255s", &lo, &hi, name) < 2)
			continue;
		if (strcmp(name, "[heap]") == 0 || (name[0] == '\0' && hi - lo >= (1UL << 20)))
			printf("layout map %lx %lx %s\n", lo, hi, name[0] ? name : "anon");
		if (strcmp(mode, "2") == 0 && strcmp(name, "[heap]") == 0)
			print_frames(lo, hi);
	}
	fclose(f);
}

static int judy_4m(long repeats)
{
	Pvoid_t a = NULL;
	PWord_t pv;
	uint64_t i, n = 4194304;
	Word_t freed = 0;
	long r;

	for (r = 0; r < repeats; r++) {
		perf_ctl("enable\n");
		sc_begin();
		for (i = 0; i < n; i++) {
			JLI(pv, a, (Word_t)(((~i & 0xff) << 24) | (i & 0xffffff)));
			if (pv == NULL || pv == PJERR)
				return 2;
			*pv = i;
		}
		if (sc_end(r) != 0)
			return 2;
		perf_ctl("disable\n");
		if (layout3())
			frames_rep(r);
		else if (r == 0)
			print_layout();
		JLFA(freed, a);
	}
	printf("inserted %lu x %ld freed %lu\n", (unsigned long)n, repeats, (unsigned long)freed);
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
		if (sc_end(r) != 0)
			return 2;
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
		if (n == 4194304 && layout3())
			frames_rep(r);
		else if (n == 4194304)
			print_layout();
		perf_ctl("enable\n");
		sc_begin();
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
	if (getenv("SNG_SELFCOUNT") != NULL && strcmp(getenv("SNG_SELFCOUNT"), "1") == 0
	    && sc_open() != 0)
		return 1;
	if (strcmp(argv[1], "sm_default") == 0)
		return sparsematrix(10000, 500, repeats);
	if (strcmp(argv[1], "sm_big") == 0)
		return sparsematrix(1000000, 100000, repeats);
	if (strcmp(argv[1], "judy_4m") == 0)
		return judy_4m(repeats);
	if (strcmp(argv[1], "del_1k") == 0)
		return judy_delete(1024, repeats);
	if (strcmp(argv[1], "del_4m") == 0)
		return judy_delete(4194304, repeats);
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
