#include "expanse_esp_idf.h"

#include "sdkconfig.h"

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

/*
 * The two symbols libexpanse.a's #[global_allocator] imports. Without them
 * the staticlib does not link — which is the intended failure: a bare-metal
 * libexpanse has no libc heap of its own and must be told where memory comes
 * from. See crates/expanse-capi/src/alloc_bridge.rs.
 *
 * The Rust side over-allocates and aligns by hand (trie nodes are
 * align(32); heap_caps_malloc promises 4-8), so these are plain
 * size-in/pointer-out wrappers and must not adjust alignment themselves.
 */
void *expanse_host_malloc(size_t size) {
#ifdef CONFIG_EXPANSE_SRAM_INTERNAL_ONLY
    return expanse_esp_alloc_internal(size);
#else
    return heap_caps_malloc(size, MALLOC_CAP_DEFAULT);
#endif
}

void expanse_host_free(void *ptr) {
    expanse_esp_free(ptr);
}
