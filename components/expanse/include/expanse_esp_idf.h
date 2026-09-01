/*
 * expanse_esp_idf.h — ESP-IDF integration helpers for Expanse.
 *
 * Provides internal SRAM capability allocation helpers and FreeRTOS /
 * ESP-IDF heap hooks for Expanse 32-bit digital tries.
 */
#ifndef EXPANSE_ESP_IDF_H
#define EXPANSE_ESP_IDF_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "esp_heap_caps.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Allocate memory from internal fast DRAM (MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT).
 *
 * Sized and aligned to embedded cache lines to maximize throughput on
 * ESP32-C3 / ESP32-S3 microcontroller architectures.
 */
void *expanse_esp_alloc_internal(size_t size);

/**
 * Allocate memory from external PSRAM / SPIRAM (MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT).
 */
void *expanse_esp_alloc_spiram(size_t size);

/**
 * Free memory previously allocated via expanse_esp_alloc_*.
 */
void expanse_esp_free(void *ptr);

/**
 * Backing heap for libexpanse itself.
 *
 * `libexpanse.a` is built `no_std`; its global allocator imports exactly
 * these two symbols, so the component must define them or the link fails.
 * Honours CONFIG_EXPANSE_SRAM_INTERNAL_ONLY: internal DRAM when set, the
 * default heap otherwise.
 *
 * Alignment is handled on the Rust side (trie nodes are 32-byte aligned,
 * heap_caps_malloc promises 4-8), so these are plain wrappers.
 */
void *expanse_host_malloc(size_t size);

/** Frees a pointer returned by expanse_host_malloc(). */
void expanse_host_free(void *ptr);

#ifdef __cplusplus
}
#endif

#endif /* EXPANSE_ESP_IDF_H */
