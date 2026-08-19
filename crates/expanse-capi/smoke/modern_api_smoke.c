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
    expanse_sync_map_free(s);
    printf("sync: concurrent reader ok\n");

    expanse_bytesmap_t *b = expanse_bytesmap_new();
    expanse_bytesmap_insert(b, "a\0b", 3, 5, NULL);
    assert(expanse_bytesmap_get(b, "a\0b", 3, &v) && v == 5);
    assert(!expanse_bytesmap_get(b, "a", 1, &v));
    expanse_bytesmap_free(b);
    printf("bytesmap: embedded NUL ok\nok\n");
    return 0;
}
