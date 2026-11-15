/* memprobe: RSS and JudyLMemUsed per small JudyL array (PREREG A1, H_mem2).
 * usage: memprobe <arrays> <keys_per_array> <key_range>
 * Builds <arrays> JudyL arrays, each with <keys_per_array> distinct keys drawn
 * from [0, key_range) (the stress-ng hashjudy shape is 20 keys of 500), then
 * deletes every key (array emptied, not freed), then JLFA's every array.
 * Prints RSS deltas per array after each phase. Compiled once against a Judy.h,
 * run with LD_LIBRARY_PATH selecting the library. */
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <Judy.h>

static long rss_kb(void)
{
	long pages = 0, rss = 0;
	FILE *f = fopen("/proc/self/statm", "r");
	if (!f || fscanf(f, "%ld %ld", &pages, &rss) != 2) { perror("statm"); exit(1); }
	fclose(f);
	return rss * (sysconf(_SC_PAGESIZE) / 1024);
}

int main(int argc, char **argv)
{
	if (argc != 4) { fprintf(stderr, "usage: memprobe arrays keys range\n"); return 2; }
	size_t na = strtoul(argv[1], 0, 10), nk = strtoul(argv[2], 0, 10), range = strtoul(argv[3], 0, 10);
	Pvoid_t *a = calloc(na, sizeof(*a));
	Word_t *keys = malloc(nk * sizeof(*keys));
	unsigned long long s = 88172645463325252ULL;
	Word_t *pv, mu = 0, bytes;
	int rc;
	long r0, r1, r2, r3;

	if (!a || !keys) return 1;
	r0 = rss_kb();
	for (size_t i = 0; i < na; i++) {
		size_t got = 0;
		while (got < nk) {
			s ^= s << 13; s ^= s >> 7; s ^= s << 17;
			Word_t k = (Word_t)(s % range);
			JLG(pv, a[i], k);
			if (pv) continue;
			JLI(pv, a[i], k);
			if (pv == PJERR || !pv) { fprintf(stderr, "JLI failed\n"); return 1; }
			*pv = k ^ 0x5a5a;
			keys[got++] = k;
		}
		/* verify */
		for (size_t j = 0; j < nk; j++) {
			JLG(pv, a[i], keys[j]);
			if (!pv || *pv != (keys[j] ^ 0x5a5a)) { fprintf(stderr, "verify failed\n"); return 1; }
		}
		if (i + 1 == na) {
			/* keep the last array's keys for the delete phase check below */
		}
	}
	r1 = rss_kb();
	for (size_t i = 0; i < na; i++) {
		JLMU(bytes, a[i]);
		mu += bytes;
	}
	/* empty every array by deleting all of its keys (re-derive via JLF/JLN) */
	for (size_t i = 0; i < na; i++) {
		Word_t idx = 0;
		JLF(pv, a[i], idx);
		while (pv) {
			Word_t cur = idx;
			JLN(pv, a[i], idx);
			JLD(rc, a[i], cur);
			if (rc != 1) { fprintf(stderr, "JLD failed\n"); return 1; }
		}
		if (a[i] != NULL) { fprintf(stderr, "array %zu not NULL after emptying\n", i); }
	}
	r2 = rss_kb();
	for (size_t i = 0; i < na; i++) {
		JLFA(bytes, a[i]);
	}
	r3 = rss_kb();
	printf("arrays=%zu keys=%zu range=%zu rss_full_B_per_array=%.1f memused_B_per_array=%.1f "
	       "rss_after_empty_B_per_array=%.1f rss_after_jlfa_B_per_array=%.1f\n",
	       na, nk, range, (r1 - r0) * 1024.0 / na, (double)mu / na,
	       (r2 - r0) * 1024.0 / na, (r3 - r0) * 1024.0 / na);
	return 0;
}
