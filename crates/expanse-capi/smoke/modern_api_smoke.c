/* Compile-and-run check of include/expanse.h against the built library. */
#include <expanse.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <pthread.h>
#include <stdatomic.h>

/* Shared string map under one writer and reader threads: stable keys are
 * inserted before the threads start and never touched again, so every read of
 * one must return its exact value; churned keys come and go, and a read of one
 * must return either nothing or the value its writer stores for it. */
#define STR_STABLE 512
#define STR_CHURN 256
#define STR_READERS 4

static expanse_sync_strmap_t *str_shared;
static atomic_bool str_stop;

static void str_stable_key(char *buf, size_t len, int i) { snprintf(buf, len, "stable/%04d", i); }
static void str_churn_key(char *buf, size_t len, int i) { snprintf(buf, len, "churn/%04d/tail", i); }

static void *str_writer(void *arg) {
    (void) arg;
    char key[32];
    for (int round = 0; round < 40; round++) {
        for (int i = 0; i < STR_CHURN; i++) {
            str_churn_key(key, sizeof key, i);
            if (round % 2 == 0)
                expanse_sync_strmap_insert(str_shared, key, 100000u + (uint64_t) i, NULL);
            else
                expanse_sync_strmap_remove(str_shared, key, NULL);
        }
    }
    atomic_store(&str_stop, true);
    return NULL;
}

static void *str_reader(void *arg) {
    (void) arg;
    char key[32];
    uint64_t v;
    unsigned long reads = 0;
    expanse_sync_strmap_reader_t *r = expanse_sync_strmap_reader_new(str_shared);
    assert(r != NULL);
    do {
        for (int i = 0; i < STR_STABLE; i++) {
            str_stable_key(key, sizeof key, i);
            v = 0;
            assert(expanse_sync_strmap_reader_get(r, key, &v) && v == (uint64_t) i);
            assert(expanse_sync_strmap_reader_contains(r, key));
        }
        for (int i = 0; i < STR_CHURN; i++) {
            str_churn_key(key, sizeof key, i);
            v = 0;
            if (expanse_sync_strmap_reader_get(r, key, &v))
                assert(v == 100000u + (uint64_t) i);
        }
        {
            /* An ordered walk sees every stable key, in order. */
            char prev[32], out[32];
            size_t req = 0;
            int seen = 0;
            expanse_str_nav_status st = expanse_sync_strmap_reader_first(r, out, sizeof out, &req, &v);
            while (st == EXPANSE_STR_NAV_OK) {
                if (strncmp(out, "stable/", 7) == 0) {
                    str_stable_key(key, sizeof key, seen);
                    assert(strcmp(out, key) == 0 && v == (uint64_t) seen);
                    seen++;
                }
                memcpy(prev, out, req);
                st = expanse_sync_strmap_reader_next_after(r, prev, out, sizeof out, &req, &v);
                assert(st != EXPANSE_STR_NAV_OK || strcmp(prev, out) < 0);
            }
            assert(st == EXPANSE_STR_NAV_NOT_FOUND && seen == STR_STABLE);
        }
        reads++;
    } while (!atomic_load(&str_stop));
    expanse_sync_strmap_reader_free(r);
    return (void *) reads;
}

static void sync_strmap_smoke(void) {
    uint64_t v = 0;
    char key[32];

    /* Every symbol once, single-threaded. */
    expanse_sync_strmap_t *m = expanse_sync_strmap_new();
    assert(m != NULL && expanse_sync_strmap_len(m) == 0);
    assert(expanse_sync_strmap_insert(m, "apple", 1, NULL));
    assert(!expanse_sync_strmap_insert(m, "apple", 2, &v) && v == 1);
    assert(expanse_sync_strmap_insert(m, "", 3, NULL)); /* the empty key */
    assert(expanse_sync_strmap_get(m, "apple", &v) && v == 2);
    assert(expanse_sync_strmap_get(m, "", &v) && v == 3);
    assert(!expanse_sync_strmap_get(m, "app", &v));
    assert(expanse_sync_strmap_contains(m, "apple"));
    assert(!expanse_sync_strmap_contains(m, "apples"));
    assert(!expanse_sync_strmap_insert(m, NULL, 9, NULL)); /* NULL key: false */
    assert(!expanse_sync_strmap_get(m, NULL, &v));
    assert(expanse_sync_strmap_len(m) == 2);
    assert(expanse_sync_strmap_mem_used(m) > 0);
    assert(expanse_sync_strmap_mem_held(m) >= expanse_sync_strmap_mem_used(m));
    expanse_sync_strmap_reader_t *r = expanse_sync_strmap_reader_new(m);
    assert(expanse_sync_strmap_reader_get(r, "apple", &v) && v == 2);
    assert(expanse_sync_strmap_reader_contains(r, ""));
    assert(!expanse_sync_strmap_reader_contains(r, "banana"));
    {
        /* Ordered reads: keys "" -> 3, "apple" -> 2. */
        char out[8];
        size_t req = 0;
        assert(expanse_sync_strmap_reader_first(r, out, sizeof out, &req, &v) == EXPANSE_STR_NAV_OK
               && req == 1 && out[0] == '\0' && v == 3);
        assert(expanse_sync_strmap_reader_last(r, NULL, 0, &req, NULL) == EXPANSE_STR_NAV_BUFFER_TOO_SMALL
               && req == 6); /* length probe */
        assert(expanse_sync_strmap_reader_last(r, out, sizeof out, &req, &v) == EXPANSE_STR_NAV_OK
               && strcmp(out, "apple") == 0 && v == 2);
        assert(expanse_sync_strmap_reader_next_at_or_after(r, "a", out, sizeof out, &req, &v) == EXPANSE_STR_NAV_OK
               && strcmp(out, "apple") == 0);
        assert(expanse_sync_strmap_reader_next_after(r, "apple", out, sizeof out, &req, &v)
               == EXPANSE_STR_NAV_NOT_FOUND);
        assert(expanse_sync_strmap_reader_prev_at_or_before(r, "b", out, sizeof out, &req, &v) == EXPANSE_STR_NAV_OK
               && strcmp(out, "apple") == 0);
        assert(expanse_sync_strmap_reader_prev_before(r, "apple", out, 5, &req, &v) == EXPANSE_STR_NAV_OK
               && req == 1 && out[0] == '\0');
        assert(expanse_sync_strmap_reader_prev_before(r, "", out, sizeof out, &req, &v)
               == EXPANSE_STR_NAV_NOT_FOUND);
    }
    expanse_sync_strmap_reader_free(r);
    assert(expanse_sync_strmap_remove(m, "apple", &v) && v == 2);
    assert(!expanse_sync_strmap_remove(m, "apple", &v));
    (void) expanse_sync_strmap_shrink_to_fit(m);
    assert(expanse_sync_strmap_len(m) == 1);
    expanse_sync_strmap_free(m);
    assert(expanse_sync_strmap_len(NULL) == 0 && expanse_sync_strmap_reader_new(NULL) == NULL);

    /* One writer, STR_READERS reader threads. */
    str_shared = expanse_sync_strmap_new();
    for (int i = 0; i < STR_STABLE; i++) {
        str_stable_key(key, sizeof key, i);
        assert(expanse_sync_strmap_insert(str_shared, key, (uint64_t) i, NULL));
    }
    atomic_store(&str_stop, false);
    pthread_t w, rd[STR_READERS];
    for (int t = 0; t < STR_READERS; t++)
        assert(pthread_create(&rd[t], NULL, str_reader, NULL) == 0);
    assert(pthread_create(&w, NULL, str_writer, NULL) == 0);
    assert(pthread_join(w, NULL) == 0);
    for (int t = 0; t < STR_READERS; t++) {
        void *reads = NULL;
        assert(pthread_join(rd[t], &reads) == 0);
        assert((unsigned long) reads > 0);
    }
    assert(expanse_sync_strmap_len(str_shared) == STR_STABLE); /* 40 rounds end on a remove */
    expanse_sync_strmap_free(str_shared);
    printf("sync_strmap: point ops, reader, ordered reads, %d threaded readers ok\n", STR_READERS);
}

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

    sync_strmap_smoke();

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
