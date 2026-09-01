/*
 * Smoke test for the 32-bit-only C surface (EXPANSE_WIDE_SURFACE == 0).
 * Compiled with -m32 against the i686 cdylib in CI — the one host-runnable
 * 32-bit libexpanse — so the narrow block of expanse.h is exercised through
 * a real C link, not only through the Rust-side test.
 */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
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

int main(void) {
    expanse_map_t *m = expanse_map_new();
    assert(m);
    for (uint32_t i = 0; i < 1000; i++) {
        assert(expanse_map_insert(m, i * 3, i, NULL));
    }
    struct acc a = {0, 0, 0};
    /* Multiples of 3 in [30, 300]: 30, 33, ..., 300 -> 91 entries, values 10..=100. */
    size_t n = expanse_map_remove_range(m, 30, 300, on_removed, &a);
    assert(n == 91 && a.n == 91 && a.last == 300 && a.sum == (10u + 100u) * 91u / 2u);
    assert(expanse_map_len(m) == 1000 - 91);
    assert(expanse_map_remove_range(m, 300, 30, NULL, NULL) == 0);          /* inverted */
    assert(expanse_map_remove_range(NULL, 0, UINT32_MAX, NULL, NULL) == 0); /* null map */
    assert(expanse_map_remove_range(m, 0, UINT32_MAX, NULL, NULL) == 1000 - 91);
    assert(expanse_map_len(m) == 0);
    expanse_map_free(m);
    puts("narrow_api_smoke: OK");
    return 0;
}
