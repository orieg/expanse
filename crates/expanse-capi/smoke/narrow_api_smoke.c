/*
 * Smoke test for the 32-bit-only C surface (EXPANSE_WIDE_SURFACE == 0).
 * Compiled with -m32 against the i686 cdylib in CI — the one host-runnable
 * 32-bit libexpanse — so the narrow block of expanse.h is exercised through
 * a real C link, not only through the Rust-side test.
 */
#include <assert.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include "expanse.h"

#if EXPANSE_WIDE_SURFACE
#error "narrow_api_smoke.c targets 32-bit builds (EXPANSE_WIDE_SURFACE == 0)"
#endif

struct acc {
    uint32_t n;
    uint32_t last;
    uint32_t sum;
};

static void on_removed(expanse_word_t key, expanse_word_t value, void *ctx) {
    struct acc *a = ctx;
    assert(a->n == 0 || key > a->last); /* ascending key order */
    a->last = key;
    a->sum += value;
    a->n++;
}

/*
 * expanse_sync32_*: verifies the ABI plumbing and liveness through a real
 * C link with real threads — one writer thread paced to ~100k mutations/s
 * on a churn range, two reader threads on their own handles reading
 * stable keys, which must come back exactly or BUSY, never torn. The
 * deterministic synchronization-boundary tests live on the Rust side.
 */
#define S32_STABLE 256u
#define S32_READERS 2

struct s32_reader_arg {
    expanse_sync32_map_t *map;
    size_t idx;
    volatile int *stop;
    uint32_t ok, busy, max_busy_streak;
};

static bool on_visited(expanse_word_t key, expanse_word_t value, void *ctx) {
    struct acc *a = ctx;
    assert(a->n == 0 || key > a->last); /* ascending key order */
    a->last = key;
    a->sum += value;
    a->n++;
    return a->n < 5u; /* stop after five, to prove the stop is honoured */
}

static void *s32_reader(void *p) {
    struct s32_reader_arg *a = p;
    expanse_sync32_map_reader_t *r = expanse_sync32_map_reader(a->map, a->idx);
    assert(r);
    uint32_t streak = 0, k = 0;
    while (!*a->stop || a->ok < 20000u) {
        expanse_word_t v = 0;
        expanse_sync32_status_t st = expanse_sync32_map_reader_try_get(r, k % S32_STABLE, &v);
        if (st == EXPANSE_SYNC32_OK) {
            assert(v == ((k % S32_STABLE) ^ 0xABCDu)); /* stable key must never tear */
            a->ok++;
            streak = 0;
        } else if (st == EXPANSE_SYNC32_BUSY) {
            a->busy++;
            if (++streak > a->max_busy_streak) a->max_busy_streak = streak;
        } else {
            fprintf(stderr, "reader %zu: unexpected status %s\n", a->idx, expanse_sync32_status_str((int)st));
            assert(0);
        }
        k += 7;
        if (a->ok >= 20000u && *a->stop) break;
    }
    return NULL;
}

static void sync32_smoke(void) {
    assert(expanse_sync32_map_new(expanse_sync32_mutation_headroom() - 1, 1) == NULL);
    expanse_sync32_map_t *m = expanse_sync32_map_new(16384, S32_READERS);
    assert(m);
    expanse_sync32_map_writer_t *w = expanse_sync32_map_writer(m);
    assert(w && expanse_sync32_map_writer(m) == w);
    assert(expanse_sync32_map_reader(m, S32_READERS) == NULL);
    for (uint32_t k = 0; k < S32_STABLE; k++) {
        assert(expanse_sync32_map_writer_try_insert(w, k, k ^ 0xABCDu, NULL, NULL) == EXPANSE_SYNC32_OK);
    }
    expanse_sync32_stats_t st;
    memset(&st, 0, sizeof st);
    assert(expanse_sync32_map_writer_stats(w, &st, sizeof st) == EXPANSE_SYNC32_OK);
    assert(st.len == S32_STABLE && st.free_slots > expanse_sync32_mutation_headroom());

    volatile int stop = 0;
    pthread_t tids[S32_READERS];
    struct s32_reader_arg args[S32_READERS];
    for (size_t i = 0; i < S32_READERS; i++) {
        args[i] = (struct s32_reader_arg){ .map = m, .idx = i, .stop = &stop, .ok = 0, .busy = 0, .max_busy_streak = 0 };
        assert(pthread_create(&tids[i], NULL, s32_reader, &args[i]) == 0);
    }
    /* Writer paced to ~100k mutations/s (10 us period) on a disjoint range. */
    uint32_t refused = 0;
    struct timespec pace = { 0, 10000 };
    for (uint32_t i = 0; i < 20000u; i++) {
        uint32_t k = S32_STABLE + (i * 2654435761u) % 4096u;
        expanse_sync32_status_t s = (i % 3 == 2)
            ? expanse_sync32_map_writer_try_remove(w, k, NULL)
            : expanse_sync32_map_writer_try_insert(w, k, i, NULL, NULL);
        if (s == EXPANSE_SYNC32_ARENA_FULL || s == EXPANSE_SYNC32_RECLAIM_BACKLOG) {
            refused++;
            expanse_sync32_map_writer_try_reclaim(w);
        } else {
            assert(s == EXPANSE_SYNC32_OK || s == EXPANSE_SYNC32_NOT_FOUND);
        }
        nanosleep(&pace, NULL);
    }
    stop = 1;
    for (size_t i = 0; i < S32_READERS; i++) assert(pthread_join(tids[i], NULL) == 0);
    for (size_t i = 0; i < S32_READERS; i++) {
        printf("sync32 reader %zu: ok=%u busy=%u max_busy_streak=%u\n", i, args[i].ok, args[i].busy, args[i].max_busy_streak);
        assert(args[i].ok >= 20000u);
    }
    printf("sync32 writer: refused=%u\n", refused);
    assert(refused < 20000u);
    /* Every handle owner is joined: free is now allowed. */
    expanse_sync32_map_free(m);
    expanse_sync32_map_free(NULL);
}

int main(void) {
    expanse_map_t *m = expanse_map_new();
    assert(m);
    for (uint32_t i = 0; i < 1000; i++) {
        assert(expanse_map_insert(m, i * 3, i, NULL));
    }
    /*
     * Read-only ordered walk: one descent, then streaming. Stops when the
     * callback asks, leaves the map untouched, and reports whether it ran
     * the range out.
     */
    struct acc w = {0, 0, 0};
    assert(!expanse_map_for_each_range(m, 30, 300, on_visited, &w)); /* stopped */
    assert(w.n == 5 && w.last == 42 && w.sum == (10u + 14u) * 5u / 2u);
    assert(expanse_map_len(m) == 1000); /* the walk removed nothing */
    assert(expanse_map_for_each_range(m, 300, 30, on_visited, &w));  /* inverted */
    assert(expanse_map_for_each_range(m, 0, UINT32_MAX, NULL, NULL)); /* null callback */
    assert(expanse_map_for_each_range(NULL, 0, UINT32_MAX, on_visited, &w)); /* null map */
    assert(w.n == 5); /* no no-op form visited an entry */

    struct acc a = {0, 0, 0};
    /* Multiples of 3 in [30, 300]: 30, 33, ..., 300 -> 91 entries, values 10..=100. */
    size_t n = expanse_map_remove_range(m, 30, 300, on_removed, &a);
    assert(n == 91 && a.n == 91 && a.last == 300 && a.sum == (10u + 100u) * 91u / 2u);
    assert(expanse_map_len(m) == 1000 - 91);
    assert(expanse_map_remove_range(m, 300, 30, NULL, NULL) == 0);          /* inverted */
    assert(expanse_map_remove_range(NULL, 0, UINT32_MAX, NULL, NULL) == 0); /* null map */
    /* Scattered removal: a sorted set sharing no interval, half of it absent
     * from the map, so the return must count removals and not requests. */
    struct acc b = {0, 0, 0};
    expanse_word_t victims[60];
    size_t present = 0;
    for (size_t i = 0; i < 60; ++i) {
        victims[i] = (expanse_word_t)(i * 7); /* only multiples of 3 are in the map */
        if (victims[i] % 3 == 0 && victims[i] >= 30 && victims[i] <= 300) {
            continue; /* already retired by the remove_range above */
        }
        if (victims[i] % 3 == 0) {
            present++;
        }
    }
    size_t rm = expanse_map_remove_many(m, victims, 60, on_removed, &b);
    assert(rm == present && b.n == present);
    assert(expanse_map_len(m) == 1000 - 91 - present);
    assert(expanse_map_remove_many(m, victims, 0, NULL, NULL) == 0);    /* zero len */
    assert(expanse_map_remove_many(m, NULL, 60, NULL, NULL) == 0);      /* null keys */
    assert(expanse_map_remove_many(NULL, victims, 60, NULL, NULL) == 0); /* null map */

    assert(expanse_map_remove_range(m, 0, UINT32_MAX, NULL, NULL) == 1000 - 91 - present);
    assert(expanse_map_len(m) == 0);
    expanse_map_free(m);

    sync32_smoke();
    puts("narrow_api_smoke: OK");
    return 0;
}
