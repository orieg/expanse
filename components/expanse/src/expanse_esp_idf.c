#include "expanse_esp_idf.h"

void *expanse_esp_alloc_internal(size_t size) {
    if (size == 0) {
        return NULL;
    }
    return heap_caps_malloc(size, MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
}

void *expanse_esp_alloc_spiram(size_t size) {
    if (size == 0) {
        return NULL;
    }
    return heap_caps_malloc(size, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
}

void expanse_esp_free(void *ptr) {
    if (ptr != NULL) {
        heap_caps_free(ptr);
    }
}
