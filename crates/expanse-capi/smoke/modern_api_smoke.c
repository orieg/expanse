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
    assert(expanse_sync_strmap_mem_used_estimate(m) == expanse_sync_strmap_mem_used(m));
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

/* The shared bytes and ordered-bytes maps under one writer and reader
 * threads, as for the string map above. Keys carry an embedded NUL and the
 * ordered map's 0x01 escape byte, so they are passed by length. On the
 * ordered map each reader also walks the map with the container's ordered
 * reads, which exclude writers for each call. */
#define BYTES_STABLE 512
#define BYTES_CHURN 256
#define BYTES_READERS 4

static expanse_sync_bytesmap_t *bytes_shared;
static expanse_sync_ordered_bytesmap_t *obytes_shared;
static atomic_bool bytes_stop;

/* "s\0NNNN" (6 bytes) and "c\x01NNNN\0t" (8 bytes); returns the length. */
static size_t bytes_stable_key(char *buf, int i) {
    buf[0] = 's';
    buf[1] = '\0';
    snprintf(buf + 2, 5, "%04d", i);
    return 6;
}
static size_t bytes_churn_key(char *buf, int i) {
    buf[0] = 'c';
    buf[1] = '\x01';
    snprintf(buf + 2, 5, "%04d", i);
    buf[6] = '\0';
    buf[7] = 't';
    return 8;
}

static void *bytes_writer(void *arg) {
    bool ordered = arg != NULL;
    char key[16];
    for (int round = 0; round < 40; round++) {
        for (int i = 0; i < BYTES_CHURN; i++) {
            size_t n = bytes_churn_key(key, i);
            uint64_t v = 100000u + (uint64_t) i;
            if (round % 2 == 0) {
                if (ordered)
                    expanse_sync_ordered_bytesmap_insert(obytes_shared, key, n, v, NULL);
                else
                    expanse_sync_bytesmap_insert(bytes_shared, key, n, v, NULL);
            } else if (ordered) {
                expanse_sync_ordered_bytesmap_remove(obytes_shared, key, n, NULL);
            } else {
                expanse_sync_bytesmap_remove(bytes_shared, key, n, NULL);
            }
        }
    }
    atomic_store(&bytes_stop, true);
    return NULL;
}

static void *bytes_reader(void *arg) {
    bool ordered = arg != NULL;
    char key[16];
    uint64_t v;
    unsigned long reads = 0;
    expanse_sync_bytesmap_reader_t *r = ordered ? NULL : expanse_sync_bytesmap_reader_new(bytes_shared);
    expanse_sync_ordered_bytesmap_reader_t *orr =
        ordered ? expanse_sync_ordered_bytesmap_reader_new(obytes_shared) : NULL;
    assert(ordered ? orr != NULL : r != NULL);
    do {
        for (int i = 0; i < BYTES_STABLE; i++) {
            size_t n = bytes_stable_key(key, i);
            v = 0;
            if (ordered)
                assert(expanse_sync_ordered_bytesmap_reader_get(orr, key, n, &v) && v == (uint64_t) i
                       && expanse_sync_ordered_bytesmap_reader_contains(orr, key, n));
            else
                assert(expanse_sync_bytesmap_reader_get(r, key, n, &v) && v == (uint64_t) i
                       && expanse_sync_bytesmap_reader_contains(r, key, n));
        }
        for (int i = 0; i < BYTES_CHURN; i++) {
            size_t n = bytes_churn_key(key, i);
            v = 0;
            bool hit = ordered ? expanse_sync_ordered_bytesmap_reader_get(orr, key, n, &v)
                               : expanse_sync_bytesmap_reader_get(r, key, n, &v);
            if (hit)
                assert(v == 100000u + (uint64_t) i);
        }
        if (ordered) {
            /* An ordered walk sees every stable key, in order; the output
             * buffer is also the search key of the next call. */
            char out[16];
            size_t req = 0;
            int seen = 0;
            expanse_ordered_bytes_nav_status st =
                expanse_sync_ordered_bytesmap_first(obytes_shared, out, sizeof out, &req, &v);
            while (st == EXPANSE_ORDERED_BYTES_NAV_OK) {
                if (req == 6 && out[0] == 's') {
                    size_t n = bytes_stable_key(key, seen);
                    assert(memcmp(out, key, n) == 0 && v == (uint64_t) seen);
                    seen++;
                }
                st = expanse_sync_ordered_bytesmap_next_after(obytes_shared, out, req, out, sizeof out, &req, &v);
            }
            assert(st == EXPANSE_ORDERED_BYTES_NAV_NOT_FOUND && seen == BYTES_STABLE);
        }
        reads++;
    } while (!atomic_load(&bytes_stop));
    if (ordered)
        expanse_sync_ordered_bytesmap_reader_free(orr);
    else
        expanse_sync_bytesmap_reader_free(r);
    return (void *) reads;
}

static void bytes_threads(bool ordered) {
    char key[16];
    for (int i = 0; i < BYTES_STABLE; i++) {
        size_t n = bytes_stable_key(key, i);
        assert(ordered ? expanse_sync_ordered_bytesmap_insert(obytes_shared, key, n, (uint64_t) i, NULL)
                       : expanse_sync_bytesmap_insert(bytes_shared, key, n, (uint64_t) i, NULL));
    }
    void *mode = ordered ? (void *) 1 : NULL;
    atomic_store(&bytes_stop, false);
    pthread_t w, rd[BYTES_READERS];
    for (int t = 0; t < BYTES_READERS; t++)
        assert(pthread_create(&rd[t], NULL, bytes_reader, mode) == 0);
    assert(pthread_create(&w, NULL, bytes_writer, mode) == 0);
    assert(pthread_join(w, NULL) == 0);
    for (int t = 0; t < BYTES_READERS; t++) {
        void *reads = NULL;
        assert(pthread_join(rd[t], &reads) == 0);
        assert((unsigned long) reads > 0);
    }
    /* 40 rounds end on a remove. */
    assert((ordered ? expanse_sync_ordered_bytesmap_len(obytes_shared) : expanse_sync_bytesmap_len(bytes_shared))
           == BYTES_STABLE);
}

static void sync_bytesmaps_smoke(void) {
    uint64_t v = 0;

    /* Every bytes-map symbol once, single-threaded. */
    expanse_sync_bytesmap_t *b = expanse_sync_bytesmap_new();
    assert(b != NULL && expanse_sync_bytesmap_len(b) == 0);
    assert(expanse_sync_bytesmap_insert(b, "a\0b", 3, 1, NULL));
    assert(!expanse_sync_bytesmap_insert(b, "a\0b", 3, 2, &v) && v == 1);
    assert(expanse_sync_bytesmap_insert(b, NULL, 0, 3, NULL)); /* the empty key */
    assert(!expanse_sync_bytesmap_insert(b, NULL, 1, 9, NULL)); /* NULL key, len 1: false */
    assert(expanse_sync_bytesmap_get(b, "a\0b", 3, &v) && v == 2);
    assert(!expanse_sync_bytesmap_get(b, "a", 1, &v));
    assert(expanse_sync_bytesmap_contains(b, "", 0));
    assert(expanse_sync_bytesmap_len(b) == 2);
    assert(expanse_sync_bytesmap_mem_used(b) > 0);
    assert(expanse_sync_bytesmap_mem_held(b) >= expanse_sync_bytesmap_mem_used(b));
    expanse_sync_bytesmap_reader_t *br = expanse_sync_bytesmap_reader_new(b);
    assert(expanse_sync_bytesmap_reader_get(br, "a\0b", 3, &v) && v == 2);
    assert(expanse_sync_bytesmap_reader_contains(br, NULL, 0));
    assert(!expanse_sync_bytesmap_reader_contains(br, "a\0", 2));
    expanse_sync_bytesmap_reader_free(br);
    assert(expanse_sync_bytesmap_remove(b, "a\0b", 3, &v) && v == 2);
    assert(!expanse_sync_bytesmap_remove(b, "a\0b", 3, &v));
    (void) expanse_sync_bytesmap_shrink_to_fit(b);
    assert(expanse_sync_bytesmap_len(b) == 1);
    expanse_sync_bytesmap_free(b);
    assert(expanse_sync_bytesmap_len(NULL) == 0 && expanse_sync_bytesmap_reader_new(NULL) == NULL);

    /* Every ordered-bytes-map symbol once. */
    expanse_sync_ordered_bytesmap_t *o = expanse_sync_ordered_bytesmap_new();
    assert(o != NULL && expanse_sync_ordered_bytesmap_len(o) == 0);
    assert(expanse_sync_ordered_bytesmap_insert(o, "beta\0x", 6, 20, NULL));
    assert(!expanse_sync_ordered_bytesmap_insert(o, "beta\0x", 6, 21, &v) && v == 20);
    assert(expanse_sync_ordered_bytesmap_insert(o, "beta\x01x", 6, 30, NULL));
    assert(expanse_sync_ordered_bytesmap_insert(o, "alpha", 5, 10, NULL));
    assert(expanse_sync_ordered_bytesmap_get(o, "beta\0x", 6, &v) && v == 21);
    assert(expanse_sync_ordered_bytesmap_contains(o, "beta\x01x", 6));
    assert(!expanse_sync_ordered_bytesmap_contains(o, "beta", 4));
    assert(expanse_sync_ordered_bytesmap_len(o) == 3);
    assert(expanse_sync_ordered_bytesmap_mem_used(o) > 0);
    assert(expanse_sync_ordered_bytesmap_mem_held(o) >= expanse_sync_ordered_bytesmap_mem_used(o));
    expanse_sync_ordered_bytesmap_reader_t *orr = expanse_sync_ordered_bytesmap_reader_new(o);
    assert(expanse_sync_ordered_bytesmap_reader_get(orr, "alpha", 5, &v) && v == 10);
    assert(expanse_sync_ordered_bytesmap_reader_contains(orr, "beta\0x", 6));
    expanse_sync_ordered_bytesmap_reader_free(orr);
    {
        /* Ordered reads: "alpha" -> 10 < "beta\0x" -> 21 < "beta\x01x" -> 30. */
        char out[8];
        size_t req = 0;
        assert(expanse_sync_ordered_bytesmap_first(o, NULL, 0, &req, NULL) == EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL
               && req == 5); /* length probe */
        assert(expanse_sync_ordered_bytesmap_first(o, out, sizeof out, &req, &v) == EXPANSE_ORDERED_BYTES_NAV_OK
               && req == 5 && memcmp(out, "alpha", 5) == 0 && v == 10);
        assert(expanse_sync_ordered_bytesmap_last(o, out, sizeof out, &req, &v) == EXPANSE_ORDERED_BYTES_NAV_OK
               && req == 6 && memcmp(out, "beta\x01x", 6) == 0 && v == 30);
        assert(expanse_sync_ordered_bytesmap_next_after(o, "alpha", 5, out, 5, &req, &v)
               == EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL && req == 6); /* one byte short */
        assert(expanse_sync_ordered_bytesmap_next_after(o, "alpha", 5, out, sizeof out, &req, &v)
               == EXPANSE_ORDERED_BYTES_NAV_OK && memcmp(out, "beta\0x", 6) == 0 && v == 21);
        assert(expanse_sync_ordered_bytesmap_next_at_or_after(o, "beta", 4, out, sizeof out, &req, &v)
               == EXPANSE_ORDERED_BYTES_NAV_OK && memcmp(out, "beta\0x", 6) == 0);
        assert(expanse_sync_ordered_bytesmap_prev_at_or_before(o, "beta\x01", 5, out, sizeof out, &req, &v)
               == EXPANSE_ORDERED_BYTES_NAV_OK && memcmp(out, "beta\0x", 6) == 0);
        assert(expanse_sync_ordered_bytesmap_prev_before(o, "alpha", 5, out, sizeof out, &req, &v)
               == EXPANSE_ORDERED_BYTES_NAV_NOT_FOUND);
    }
    assert(expanse_sync_ordered_bytesmap_remove(o, "alpha", 5, &v) && v == 10);
    assert(!expanse_sync_ordered_bytesmap_remove(o, "alpha", 5, &v));
    (void) expanse_sync_ordered_bytesmap_shrink_to_fit(o);
    assert(expanse_sync_ordered_bytesmap_len(o) == 2);
    expanse_sync_ordered_bytesmap_free(o);
    assert(expanse_sync_ordered_bytesmap_len(NULL) == 0 && expanse_sync_ordered_bytesmap_reader_new(NULL) == NULL);

    /* One writer, BYTES_READERS reader threads, on each map. */
    bytes_shared = expanse_sync_bytesmap_new();
    bytes_threads(false);
    expanse_sync_bytesmap_free(bytes_shared);
    obytes_shared = expanse_sync_ordered_bytesmap_new();
    bytes_threads(true);
    expanse_sync_ordered_bytesmap_free(obytes_shared);
    printf("sync_bytesmap, sync_ordered_bytesmap: point ops, readers, ordered reads, %d threaded readers ok\n",
           BYTES_READERS);
}

/* The shared blob map under one writer and reader threads. Stable keys hold
 * arena values with metadata and are never touched again, so every read of
 * one must return its exact bytes and metadata. Churned keys cycle through an
 * inline value (7 bytes or fewer), an arena value with metadata, and an
 * absence, and the writer compacts the arena between rounds; a read of one
 * into a buffer too small for the arena value must be OK with the inline
 * value, NOT_FOUND, or BUFFER_TOO_SMALL with the arena value's length and
 * metadata and the buffer untouched. */
#define BLOB_STABLE 256
#define BLOB_CHURN 128
#define BLOB_READERS 4
#define BLOB_LONG 200
#define BLOB_META 77u

static expanse_sync_blob_map_t *blob_shared;
static atomic_bool blob_stop;

static size_t blob_stable_value(uint8_t *buf, int i) {
    size_t n = 8 + (size_t) (i % 64);
    memset(buf, 'a' + i % 26, n);
    return n;
}

static void *blob_writer(void *arg) {
    (void) arg;
    uint8_t long_value[BLOB_LONG];
    memset(long_value, 'L', sizeof long_value);
    for (int round = 0; round < 60; round++) {
        for (int i = 0; i < BLOB_CHURN; i++) {
            uint64_t k = BLOB_STABLE + (uint64_t) i;
            switch ((round + i) % 3) {
            case 0:
                assert(expanse_sync_blob_map_insert(blob_shared, k, (const uint8_t *) "short", 5, 0)
                       == EXPANSE_BLOB_OK);
                break;
            case 1:
                assert(expanse_sync_blob_map_insert(blob_shared, k, long_value, sizeof long_value, BLOB_META)
                       == EXPANSE_BLOB_OK);
                break;
            default:
                expanse_sync_blob_map_remove(blob_shared, k);
            }
        }
        if (round % 10 == 9)
            assert(expanse_sync_blob_map_compact(blob_shared) == EXPANSE_BLOB_OK);
    }
    atomic_store(&blob_stop, true);
    return NULL;
}

static void *blob_reader(void *arg) {
    (void) arg;
    uint8_t want[128], buf[128];
    unsigned long reads = 0;
    expanse_sync_blob_map_reader_t *r = expanse_sync_blob_map_reader_new(blob_shared);
    assert(r != NULL);
    do {
        for (int i = 0; i < BLOB_STABLE; i++) {
            size_t n = blob_stable_value(want, i), len = 0;
            uint32_t meta = 0;
            assert(expanse_sync_blob_map_reader_get_into(r, (uint64_t) i, buf, sizeof buf, &len, &meta)
                   == EXPANSE_BLOB_OK);
            assert(len == n && memcmp(buf, want, n) == 0 && meta == (uint32_t) i + 1);
        }
        for (int i = 0; i < BLOB_CHURN; i++) {
            uint64_t k = BLOB_STABLE + (uint64_t) i;
            uint8_t small[16];
            size_t len = 0;
            uint32_t meta = 0;
            memset(small, 0xA5, sizeof small);
            expanse_blob_status_t st =
                expanse_sync_blob_map_reader_get_into(r, k, small, sizeof small, &len, &meta);
            if (st == EXPANSE_BLOB_OK) {
                assert(len == 5 && memcmp(small, "short", 5) == 0 && meta == 0);
            } else if (st == EXPANSE_BLOB_BUFFER_TOO_SMALL) {
                assert(len == BLOB_LONG && meta == BLOB_META);
                for (size_t j = 0; j < sizeof small; j++)
                    assert(small[j] == 0xA5); /* nothing copied */
            } else {
                assert(st == EXPANSE_BLOB_NOT_FOUND);
            }
        }
        reads++;
    } while (!atomic_load(&blob_stop));
    expanse_sync_blob_map_reader_free(r);
    return (void *) reads;
}

static void sync_blob_map_smoke(void) {
    uint8_t buf[64];
    size_t len = 0;
    uint32_t meta = 0;

    /* Every blob-map symbol once, single-threaded. */
    expanse_sync_blob_map_t *m = expanse_sync_blob_map_new(0);
    assert(m != NULL && expanse_sync_blob_map_len(m) == 0);
    assert(expanse_sync_blob_map_insert(m, 1, (const uint8_t *) "inline", 6, 0) == EXPANSE_BLOB_OK);
    assert(expanse_sync_blob_map_insert(m, 2, (const uint8_t *) "an arena-stored payload", 23, 9)
           == EXPANSE_BLOB_OK);
    assert(expanse_sync_blob_map_insert(m, 3, NULL, 0, 0) == EXPANSE_BLOB_OK); /* empty payload */
    assert(expanse_sync_blob_map_insert(m, 4, NULL, 1, 0) == EXPANSE_BLOB_INVALID_ARGUMENT);
    assert(expanse_sync_blob_map_insert(m, 4, buf, 32, 1u << 24) == EXPANSE_BLOB_META_OVERFLOW);
    assert(expanse_sync_blob_map_len(m) == 3);
    assert(expanse_sync_blob_map_contains(m, 2) && !expanse_sync_blob_map_contains(m, 4));
    assert(expanse_sync_blob_map_get_meta(m, 2, &meta) && meta == 9);
    assert(expanse_sync_blob_map_get_meta(m, 1, &meta) && meta == 0);
    assert(expanse_sync_blob_map_get_into(m, 2, NULL, 0, &len, &meta) == EXPANSE_BLOB_BUFFER_TOO_SMALL
           && len == 23 && meta == 9); /* length probe */
    memset(buf, 0xA5, sizeof buf);
    assert(expanse_sync_blob_map_get_into(m, 2, buf, 22, &len, &meta) == EXPANSE_BLOB_BUFFER_TOO_SMALL
           && len == 23 && buf[0] == 0xA5); /* one byte short: nothing copied */
    assert(expanse_sync_blob_map_get_into(m, 2, buf, 23, &len, &meta) == EXPANSE_BLOB_OK && len == 23
           && memcmp(buf, "an arena-stored payload", 23) == 0 && buf[23] == 0xA5);
    assert(expanse_sync_blob_map_get_into(m, 4, buf, sizeof buf, &len, &meta) == EXPANSE_BLOB_NOT_FOUND);
    assert(expanse_sync_blob_map_get_into(m, 3, NULL, 0, &len, NULL) == EXPANSE_BLOB_OK && len == 0);
    assert(expanse_sync_blob_map_mem_used(m) > 0);
    assert(expanse_sync_blob_map_mem_used_estimate(m) == expanse_sync_blob_map_mem_used(m));
    assert(expanse_sync_blob_map_mem_held(m) >= expanse_sync_blob_map_mem_used(m));
    expanse_sync_blob_map_reader_t *r = expanse_sync_blob_map_reader_new(m);
    assert(expanse_sync_blob_map_reader_get_into(r, 1, buf, sizeof buf, &len, &meta) == EXPANSE_BLOB_OK
           && len == 6 && memcmp(buf, "inline", 6) == 0 && meta == 0);
    assert(expanse_sync_blob_map_reader_get_meta(r, 2, &meta) && meta == 9);
    assert(expanse_sync_blob_map_reader_contains(r, 3) && !expanse_sync_blob_map_reader_contains(r, 4));
    expanse_sync_blob_map_reader_free(r);
    assert(expanse_sync_blob_map_remove(m, 1) && !expanse_sync_blob_map_remove(m, 1));
    expanse_sync_blob_map_set_reclaim_at_cap(m, false);
    assert(expanse_sync_blob_map_compact(m) == EXPANSE_BLOB_OK);
    (void) expanse_sync_blob_map_shrink_to_fit(m);
    assert(expanse_sync_blob_map_reader_get_into(NULL, 2, buf, sizeof buf, &len, &meta)
           == EXPANSE_BLOB_INVALID_ARGUMENT);
    assert(expanse_sync_blob_map_len(m) == 2);
    expanse_sync_blob_map_free(m);
    assert(expanse_sync_blob_map_len(NULL) == 0 && expanse_sync_blob_map_reader_new(NULL) == NULL);

    /* The capacity cap: one 4 KiB chunk under a 6000-byte cap, reclaim off. */
    expanse_sync_blob_map_t *capped = expanse_sync_blob_map_new_with_capacity(4096, 6000);
    expanse_sync_blob_map_set_reclaim_at_cap(capped, false);
    uint8_t rec[3100];
    memset(rec, 'r', sizeof rec);
    for (uint64_t k = 0; k < 4; k++)
        assert(expanse_sync_blob_map_insert(capped, k, rec, 1000, 1) == EXPANSE_BLOB_OK);
    assert(expanse_sync_blob_map_insert(capped, 9, rec, sizeof rec, 1) == EXPANSE_BLOB_CAP_REFUSED);
    expanse_sync_blob_map_free(capped);

    /* One writer, BLOB_READERS reader threads. */
    blob_shared = expanse_sync_blob_map_new(64 * 1024);
    for (int i = 0; i < BLOB_STABLE; i++) {
        uint8_t v[128];
        size_t n = blob_stable_value(v, i);
        assert(expanse_sync_blob_map_insert(blob_shared, (uint64_t) i, v, n, (uint32_t) i + 1) == EXPANSE_BLOB_OK);
    }
    atomic_store(&blob_stop, false);
    pthread_t w, rd[BLOB_READERS];
    for (int t = 0; t < BLOB_READERS; t++)
        assert(pthread_create(&rd[t], NULL, blob_reader, NULL) == 0);
    assert(pthread_create(&w, NULL, blob_writer, NULL) == 0);
    assert(pthread_join(w, NULL) == 0);
    for (int t = 0; t < BLOB_READERS; t++) {
        void *reads = NULL;
        assert(pthread_join(rd[t], &reads) == 0);
        assert((unsigned long) reads > 0);
    }
    assert(expanse_sync_blob_map_len(blob_shared) >= BLOB_STABLE);
    expanse_sync_blob_map_free(blob_shared);
    printf("sync_blob_map: point ops, reader, caller buffers, %d threaded readers ok\n", BLOB_READERS);
}

/* Read-modify-write on the shared string, bytes, ordered-bytes and blob maps:
 * every symbol once, then RMW_THREADS threads each running the retry loops of
 * expanse.h RMW_ROUNDS times on one counter per map; no increment may be lost. */
#define RMW_THREADS 4
#define RMW_ROUNDS 500

static expanse_sync_strmap_t *rmw_str;
static expanse_sync_bytesmap_t *rmw_bytes;
static expanse_sync_ordered_bytesmap_t *rmw_obytes;
static expanse_sync_blob_map_t *rmw_blob;

/* A 16-byte blob counter: the count in the first 8 bytes, 8 bytes no codec
 * compresses, so it is stored in the arena with its metadata. */
static void rmw_blob_counter(uint8_t *buf, uint64_t n) {
    memcpy(buf, &n, sizeof n);
    memset(buf + 8, 0xFF, 8);
}

static void *rmw_worker(void *arg) {
    (void) arg;
    for (int i = 0; i < RMW_ROUNDS; i++) {
        uint64_t cur = 0, seen = 0;
        bool present = expanse_sync_strmap_get(rmw_str, "counter", &cur), sp = false;
        while (!expanse_sync_strmap_compare_exchange(rmw_str, "counter", present, cur, true, cur + 1, &sp, &seen)) {
            present = sp;
            cur = seen;
        }
        present = expanse_sync_bytesmap_get(rmw_bytes, "c\0n", 3, &cur);
        while (!expanse_sync_bytesmap_compare_exchange(rmw_bytes, "c\0n", 3, present, cur, true, cur + 1, &sp,
                                                       &seen)) {
            present = sp;
            cur = seen;
        }
        present = expanse_sync_ordered_bytesmap_get(rmw_obytes, "c\x01n", 3, &cur);
        while (!expanse_sync_ordered_bytesmap_compare_exchange(rmw_obytes, "c\x01n", 3, present, cur, true, cur + 1,
                                                               &sp, &seen)) {
            present = sp;
            cur = seen;
        }
        /* The blob loop reuses one buffer for the observed and the expected value. */
        uint8_t buf[16], next[16];
        size_t len = 0;
        uint32_t meta = 0;
        expanse_blob_status_t st = expanse_sync_blob_map_get_into(rmw_blob, 1, buf, sizeof buf, &len, &meta);
        assert(st == EXPANSE_BLOB_OK || st == EXPANSE_BLOB_NOT_FOUND);
        present = st == EXPANSE_BLOB_OK;
        for (;;) {
            uint64_t n = 0;
            if (present)
                memcpy(&n, buf, sizeof n);
            rmw_blob_counter(next, n + 1);
            st = expanse_sync_blob_map_compare_exchange(rmw_blob, 1, present, buf, len, meta, true, next,
                                                        sizeof next, (uint32_t) (n + 1), &present, buf,
                                                        sizeof buf, &len, &meta);
            if (st == EXPANSE_BLOB_OK)
                break;
            assert(st == EXPANSE_BLOB_MISMATCH);
        }
        /* And the metadata-only loop on key 2. */
        meta = 0;
        present = expanse_sync_blob_map_get_meta(rmw_blob, 2, &meta);
        assert(present);
        for (;;) {
            uint32_t seen_meta = 0;
            st = expanse_sync_blob_map_compare_exchange_meta(rmw_blob, 2, present, meta, true, meta + 1, &present,
                                                            &seen_meta);
            if (st == EXPANSE_BLOB_OK)
                break;
            assert(st == EXPANSE_BLOB_MISMATCH);
            meta = seen_meta;
        }
    }
    return NULL;
}

static void sync_rmw_smoke(void) {
    uint64_t v = 0;
    bool present = false;

    /* Every RMW symbol once, single-threaded. */
    expanse_sync_strmap_t *s = expanse_sync_strmap_new();
    assert(expanse_sync_strmap_compare_exchange(s, "k", false, 0, true, 1, &present, &v) && !present);
    assert(!expanse_sync_strmap_compare_exchange(s, "k", true, 9, true, 2, &present, &v) && present && v == 1);
    assert(expanse_sync_strmap_get_or_insert(s, "g", 5, NULL));
    assert(!expanse_sync_strmap_get_or_insert(s, "g", 6, &v) && v == 5);
    assert(!expanse_sync_strmap_get_or_insert(s, NULL, 6, &v)); /* NULL key: false */
    expanse_sync_strmap_free(s);

    expanse_sync_bytesmap_t *b = expanse_sync_bytesmap_new();
    assert(expanse_sync_bytesmap_compare_exchange(b, "a\0b", 3, false, 0, true, 1, NULL, NULL));
    assert(expanse_sync_bytesmap_compare_exchange(b, "a\0b", 3, true, 1, false, 0, &present, &v) && present
           && v == 1); /* removed */
    assert(expanse_sync_bytesmap_get_or_insert(b, NULL, 0, 7, NULL)); /* the empty key */
    assert(!expanse_sync_bytesmap_get_or_insert(b, NULL, 0, 8, &v) && v == 7);
    assert(expanse_sync_bytesmap_len(b) == 1);
    expanse_sync_bytesmap_free(b);

    expanse_sync_ordered_bytesmap_t *o = expanse_sync_ordered_bytesmap_new();
    assert(expanse_sync_ordered_bytesmap_get_or_insert(o, "x\x01", 2, 3, NULL));
    assert(!expanse_sync_ordered_bytesmap_get_or_insert(o, "x\x01", 2, 4, &v) && v == 3);
    assert(!expanse_sync_ordered_bytesmap_compare_exchange(o, "x\x01", 2, false, 0, true, 9, &present, &v)
           && present && v == 3);
    expanse_sync_ordered_bytesmap_free(o);

    expanse_sync_blob_map_t *m = expanse_sync_blob_map_new(0);
    const uint8_t *a = (const uint8_t *) "an arena-stored payload";
    uint8_t buf[32];
    size_t len = 0;
    uint32_t meta = 0;
    assert(expanse_sync_blob_map_get_or_insert(m, 1, a, 23, 9, buf, sizeof buf, &len, &meta) == EXPANSE_BLOB_OK);
    memset(buf, 0xA5, sizeof buf);
    assert(expanse_sync_blob_map_get_or_insert(m, 1, a, 5, 0, buf, 22, &len, &meta) == EXPANSE_BLOB_BUFFER_TOO_SMALL
           && len == 23 && meta == 9 && buf[0] == 0xA5); /* nothing copied */
    assert(expanse_sync_blob_map_compare_exchange(m, 1, false, NULL, 0, 0, true, a, 5, 0, &present, buf, sizeof buf,
                                                  &len, &meta)
               == EXPANSE_BLOB_MISMATCH
           && present && len == 23 && meta == 9 && memcmp(buf, a, 23) == 0);
    assert(expanse_sync_blob_map_compare_exchange(m, 1, true, a, 23, 9, true, a, 23, 1u << 24, NULL, NULL, 0, NULL,
                                                  NULL)
           == EXPANSE_BLOB_META_OVERFLOW); /* the comparison held: a refusal, not a mismatch */
    assert(expanse_sync_blob_map_compare_exchange_meta(m, 1, true, 9, true, 10, NULL, NULL) == EXPANSE_BLOB_OK);
    assert(expanse_sync_blob_map_compare_exchange_meta(m, 5, false, 0, true, 1, NULL, NULL)
           == EXPANSE_BLOB_NO_META_FIELD); /* metadata alone cannot create a key */
    assert(expanse_sync_blob_map_compare_exchange(m, 1, true, a, 23, 10, false, NULL, 0, 0, NULL, NULL, 0, NULL,
                                                  NULL)
           == EXPANSE_BLOB_OK); /* removed */
    assert(expanse_sync_blob_map_len(m) == 0);
    expanse_sync_blob_map_free(m);

    /* RMW_THREADS threads incrementing one counter per map. */
    rmw_str = expanse_sync_strmap_new();
    rmw_bytes = expanse_sync_bytesmap_new();
    rmw_obytes = expanse_sync_ordered_bytesmap_new();
    rmw_blob = expanse_sync_blob_map_new(0);
    uint8_t zero[16];
    rmw_blob_counter(zero, 0);
    assert(expanse_sync_blob_map_insert(rmw_blob, 2, zero, sizeof zero, 0) == EXPANSE_BLOB_OK);
    pthread_t t[RMW_THREADS];
    for (int i = 0; i < RMW_THREADS; i++)
        assert(pthread_create(&t[i], NULL, rmw_worker, NULL) == 0);
    for (int i = 0; i < RMW_THREADS; i++)
        assert(pthread_join(t[i], NULL) == 0);
    const uint64_t total = (uint64_t) RMW_THREADS * RMW_ROUNDS;
    assert(expanse_sync_strmap_get(rmw_str, "counter", &v) && v == total);
    assert(expanse_sync_bytesmap_get(rmw_bytes, "c\0n", 3, &v) && v == total);
    assert(expanse_sync_ordered_bytesmap_get(rmw_obytes, "c\x01n", 3, &v) && v == total);
    assert(expanse_sync_blob_map_get_into(rmw_blob, 1, buf, sizeof buf, &len, &meta) == EXPANSE_BLOB_OK
           && len == 16 && meta == (uint32_t) total);
    memcpy(&v, buf, sizeof v);
    assert(v == total);
    assert(expanse_sync_blob_map_get_meta(rmw_blob, 2, &meta) && meta == (uint32_t) total);
    expanse_sync_strmap_free(rmw_str);
    expanse_sync_bytesmap_free(rmw_bytes);
    expanse_sync_ordered_bytesmap_free(rmw_obytes);
    expanse_sync_blob_map_free(rmw_blob);
    printf("sync RMW: string, bytes, ordered-bytes and blob compare_exchange / get_or_insert, %d threads ok\n",
           RMW_THREADS);
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
    sync_bytesmaps_smoke();
    sync_blob_map_smoke();
    sync_rmw_smoke();

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
