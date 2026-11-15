/* Differential over zmap's lib/cachehash.c with churn: keys drawn at random
 * from a pool larger than the window, so keys are hit, evicted (JHSD), and
 * re-inserted after eviction. Prints a hash of the hit/miss sequence. */
#include <arpa/inet.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "cachehash.h"
typedef struct target { uint32_t ip; uint16_t port; uint8_t status; } target_t;
int main(void)
{
	const uint32_t pool = 200000, window = 65536, ops = 4000000;
	uint64_t x = 0x9e3779b97f4a7c15ULL, h = 1469598103934665603ULL, hits = 0;
	cachehash *ch = cachehash_init(window, NULL);
	for (uint32_t i = 0; i < ops; i++) {
		x ^= x << 13; x ^= x >> 7; x ^= x << 17;
		uint32_t k = (uint32_t)(x % pool);
		target_t t; memset(&t, 0, sizeof t);
		t.ip = htonl(0xC6120000u + k / 4); static const uint16_t P[4] = {80, 443, 8080, 8443};
		t.port = htons(P[k & 3]);
		int hit = cachehash_get(ch, &t, sizeof t) != NULL;
		if (!hit) cachehash_put(ch, &t, sizeof t, (void *)1);
		hits += hit; h = (h ^ (uint64_t)hit) * 1099511628211ULL;
	}
	cachehash_free(ch, NULL);
	printf("ops=%u hits=%llu seqhash=%016llx\n", ops, (unsigned long long)hits, (unsigned long long)h);
	return 0;
}
