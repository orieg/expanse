/* Compile-and-run check of include/expanse.h against the built library. */
#include <expanse.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>

int main(void) {
    printf("libexpanse %s\n", expanse_version());

    expanse_map_t *m = expanse_map_new();
    for (uint64_t k = 0; k < 1000; k++) {
        uint64_t *slot = expanse_map_ins_slot(m, k * 3);
        *slot = k;
    }
    uint64_t v = 0, key = 0;
    assert(expanse_map_get(m, 300, &v) && v == 100);
    assert(expanse_map_count_range(m, 0, 299) == 100);
    assert(expanse_map_by_count(m, 10, &key, &v) && key == 30 && v == 10);
    assert(expanse_map_next_after(m, 30, &key, NULL) && key == 33);
    printf("map: len=%llu mem=%zu rank/select ok\n",
           (unsigned long long) expanse_map_len(m), expanse_map_mem_used(m));
    expanse_map_free(m);

    expanse_sync_map_t *s = expanse_sync_map_new();
    expanse_sync_map_insert(s, 7, 77, NULL);
    expanse_sync_map_reader_t *r = expanse_sync_map_reader_new(s);
    assert(expanse_sync_map_reader_get(r, 7, &v) && v == 77);
    expanse_sync_map_reader_free(r);

    /* Atomic RMW: compare_exchange / get_or_insert, and "update" as the
     * loop documented in expanse.h. */
    bool seen_present = false;
    uint64_t seen = 0;
    assert(expanse_sync_map_compare_exchange(s, 8, false, 0, true, 1, &seen_present, &seen));
    assert(!seen_present);
    assert(!expanse_sync_map_compare_exchange(s, 8, true, 9, true, 2, &seen_present, &seen));
    assert(seen_present && seen == 1);
    assert(expanse_sync_map_compare_exchange(s, 8, true, 1, false, 0, NULL, NULL));
    assert(!expanse_sync_map_get(s, 8, &v));
    assert(expanse_sync_map_get_or_insert(s, 9, 5, NULL));
    assert(!expanse_sync_map_get_or_insert(s, 9, 6, &v) && v == 5);
    {
        uint64_t cur = 0;
        bool present = expanse_sync_map_get(s, 7, &cur);
        for (;;) {
            uint64_t nv = cur + 1; /* f(current) */
            bool sp;
            uint64_t sv;
            if (expanse_sync_map_compare_exchange(s, 7, present, cur, true, nv, &sp, &sv))
                break;
            present = sp;
            cur = sv;
        }
    }
    assert(expanse_sync_map_get(s, 7, &v) && v == 78);

    expanse_sync_map_free(s);
    printf("sync: concurrent reader, compare_exchange, get_or_insert ok\n");

    expanse_bytesmap_t *b = expanse_bytesmap_new();
    expanse_bytesmap_insert(b, "a\0b", 3, 5, NULL);
    assert(expanse_bytesmap_get(b, "a\0b", 3, &v) && v == 5);
    assert(!expanse_bytesmap_get(b, "a", 1, &v));
    expanse_bytesmap_free(b);
    printf("bytesmap: embedded NUL ok\n");

    expanse_ordered_bytesmap_t *ob = expanse_ordered_bytesmap_new();
    assert(expanse_ordered_bytesmap_len(ob) == 0);
    expanse_ordered_bytesmap_insert(ob, "alpha", 5, 10, NULL);
    expanse_ordered_bytesmap_insert(ob, "beta\0part", 9, 20, NULL);
    expanse_ordered_bytesmap_insert(ob, "beta\x01part", 9, 30, NULL);
    expanse_ordered_bytesmap_insert(ob, "gamma", 5, 40, NULL);
    assert(expanse_ordered_bytesmap_len(ob) == 4);
    assert(expanse_ordered_bytesmap_contains(ob, "beta\0part", 9));
    assert(expanse_ordered_bytesmap_get(ob, "beta\0part", 9, &v) && v == 20);

    /* Test navigation with caller buffer probe and fetch */
    char buf[16];
    size_t req = 0;
    expanse_ordered_bytes_nav_status st;

    /* Probe length with NULL buffer */
    st = expanse_ordered_bytesmap_first(ob, NULL, 0, &req, &v);
    assert(st == EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL && req == 5);

    /* Fetch with sized buffer */
    st = expanse_ordered_bytesmap_first(ob, buf, sizeof(buf), &req, &v);
    assert(st == EXPANSE_ORDERED_BYTES_NAV_OK && req == 5 && v == 10);
    assert(memcmp(buf, "alpha", 5) == 0);

    /* Next after alpha should be beta\0part */
    st = expanse_ordered_bytesmap_next_after(ob, "alpha", 5, buf, sizeof(buf), &req, &v);
    assert(st == EXPANSE_ORDERED_BYTES_NAV_OK && req == 9 && v == 20);
    assert(memcmp(buf, "beta\0part", 9) == 0);

    /* Next after beta\0part should be beta\x01part */
    st = expanse_ordered_bytesmap_next_after(ob, "beta\0part", 9, buf, sizeof(buf), &req, &v);
    assert(st == EXPANSE_ORDERED_BYTES_NAV_OK && req == 9 && v == 30);
    assert(memcmp(buf, "beta\x01part", 9) == 0);

    /* Last should be gamma */
    st = expanse_ordered_bytesmap_last(ob, buf, sizeof(buf), &req, &v);
    assert(st == EXPANSE_ORDERED_BYTES_NAV_OK && req == 5 && v == 40);
    assert(memcmp(buf, "gamma", 5) == 0);

    expanse_ordered_bytesmap_free(ob);
    printf("ordered_bytesmap: caller-buffer navigation & escape integrity ok\nok\n");
    return 0;
}
