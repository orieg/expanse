/*
 * test_expanse.c — Unity unit tests for the Expanse ESP-IDF component.
 *
 * The engine cases below exist to make a missing or unlinked libexpanse.a a
 * test failure rather than a silent no-op: every one of them calls through
 * the C ABI, so the component cannot pass while the archive is absent.
 *
 * Judy.h is deliberately not included. A 32-bit libexpanse exports no Judy*
 * symbols at all (docs/COMPAT.md, build-configuration surface matrix).
 */
#include "unity.h"
#include "expanse_esp_idf.h"
#include "expanse.h"

TEST_CASE("Expanse internal SRAM allocation helper", "[expanse]") {
    void *ptr = expanse_esp_alloc_internal(128);
    TEST_ASSERT_NOT_NULL(ptr);
    expanse_esp_free(ptr);
}

TEST_CASE("Expanse SPIRAM allocation helper", "[expanse]") {
    void *ptr = expanse_esp_alloc_spiram(1024);
    // May be NULL if board lacks PSRAM, but should not crash
    if (ptr != NULL) {
        expanse_esp_free(ptr);
    }
}

TEST_CASE("Expanse host allocator backs libexpanse", "[expanse]") {
    void *ptr = expanse_host_malloc(64);
    TEST_ASSERT_NOT_NULL(ptr);
    expanse_host_free(ptr);
}

TEST_CASE("Expanse library identity links", "[expanse]") {
    const char *version = expanse_version();
    TEST_ASSERT_NOT_NULL(version);
    TEST_ASSERT_TRUE(version[0] != '\0');
}

TEST_CASE("Expanse word is one machine word", "[expanse]") {
    TEST_ASSERT_EQUAL_UINT(sizeof(void *), sizeof(expanse_word_t));
    TEST_ASSERT_EQUAL_INT(0, EXPANSE_WIDE_SURFACE);
}

TEST_CASE("Expanse set round-trips keys", "[expanse]") {
    expanse_set_t *set = expanse_set_new();
    TEST_ASSERT_NOT_NULL(set);

    for (expanse_word_t k = 0; k < 512; ++k) {
        TEST_ASSERT_TRUE(expanse_set_insert(set, k * 7));
    }
    TEST_ASSERT_EQUAL_UINT64(512, expanse_set_len(set));
    TEST_ASSERT_TRUE(expanse_set_contains(set, 7));
    TEST_ASSERT_FALSE(expanse_set_contains(set, 8));

    expanse_word_t first = 1, last = 0;
    TEST_ASSERT_TRUE(expanse_set_first(set, &first));
    TEST_ASSERT_TRUE(expanse_set_last(set, &last));
    TEST_ASSERT_EQUAL_UINT32(0, first);
    TEST_ASSERT_EQUAL_UINT32(511 * 7, last);

    TEST_ASSERT_TRUE(expanse_set_remove(set, 7));
    TEST_ASSERT_FALSE(expanse_set_remove(set, 7));
    TEST_ASSERT_EQUAL_UINT64(511, expanse_set_len(set));
    TEST_ASSERT_TRUE(expanse_set_mem_used(set) > 0);

    expanse_set_free(set);
}

TEST_CASE("Expanse map round-trips key/value pairs", "[expanse]") {
    expanse_map_t *map = expanse_map_new();
    TEST_ASSERT_NOT_NULL(map);

    for (expanse_word_t k = 0; k < 256; ++k) {
        TEST_ASSERT_TRUE(expanse_map_insert(map, k, k * 3, NULL));
    }
    TEST_ASSERT_EQUAL_UINT64(256, expanse_map_len(map));

    expanse_word_t value = 0;
    TEST_ASSERT_TRUE(expanse_map_get(map, 100, &value));
    TEST_ASSERT_EQUAL_UINT32(300, value);
    TEST_ASSERT_FALSE(expanse_map_get(map, 999, &value));

    expanse_word_t old = 0;
    TEST_ASSERT_FALSE(expanse_map_insert(map, 100, 7, &old));
    TEST_ASSERT_EQUAL_UINT32(300, old);

    TEST_ASSERT_TRUE(expanse_map_remove(map, 100, &old));
    TEST_ASSERT_EQUAL_UINT32(7, old);
    TEST_ASSERT_EQUAL_UINT64(255, expanse_map_len(map));

    expanse_map_clear(map);
    TEST_ASSERT_EQUAL_UINT64(0, expanse_map_len(map));

    expanse_map_free(map);
}
