/*
 * test_expanse.c — Unity unit tests for Expanse ESP-IDF component.
 */
#include "unity.h"
#include "expanse_esp_idf.h"
#include "expanse.h"
#include "Judy.h"

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
