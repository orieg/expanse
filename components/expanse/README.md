# Expanse ESP-IDF Component

[![ESP-IDF](https://img.shields.io/badge/ESP--IDF-v5.0%2B-blue.svg)](https://docs.espressif.com/projects/esp-idf/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

**Expanse** is a clean-room, high-performance Judy array and digital radix trie engine optimized for 32-bit embedded microprocessors (ESP32, ESP32-C3, ESP32-C6, ESP32-S3).

## Key Architectural Advantages on ESP32

- **50% Structural Memory Reduction**: Uses compact 8-byte `Edge32` descriptors (`[ptr: 4B | aux: 3B | tag: 1B]`), packing up to 7 immediate keys with zero heap allocations.
- **32-Byte Cache Alignment**: Node geometries match the internal SRAM cache line size on ESP32 / Cortex-M microarchitectures.
- **SRAM-Optimized Allocation**: Direct support for internal high-speed DRAM allocation (`MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT`) via `expanse_esp_alloc_internal`.
- **Drop-In Judy Compatibility**: Provides both the modern `expanse_*` C API and the legacy `Judy1*` / `JudyL*` C ABI.

---

## Installation

### Via ESP-IDF Component Manager

Add `expanse` to your project's `main/idf_component.yml`:

```yaml
dependencies:
  expanse:
    version: "^0.5.0"
```

### Via Local Submodule / Component Copy

Clone or copy `components/expanse` into your ESP-IDF project's `components/` directory:

```bash
cd my-esp-project/components
git clone https://github.com/orieg/expanse.git
# The component is automatically discovered by idf.py
```

---

## Usage in C

### Modern `expanse_*` API

```c
#include "expanse.h"
#include "expanse_esp_idf.h"
#include "esp_log.h"

static const char *TAG = "app_main";

void app_main(void) {
    ESP_LOGI(TAG, "Initializing ExpanseMap...");

    // Create a 32-bit digital map
    expanse_map_t *map = expanse_map_new();

    // Insert key-value pairs (CAN ID -> sensor payload)
    uint32_t can_id = 0x18FF50E5;
    uint32_t sensor_val = 42;
    expanse_map_insert(map, can_id, sensor_val);

    // Point lookup (deterministic O(depth) trie traversal; no latency figure is
// published for this component — nothing measures it on an ESP target yet)
    uint32_t val = 0;
    if (expanse_map_get(map, can_id, &val)) {
        ESP_LOGI(TAG, "Lookup match: CAN ID 0x%08X -> Value %u", can_id, (unsigned int)val);
    }

    expanse_map_free(map);
}
```

### Legacy `JudyL` Drop-In C ABI

```c
#include "Judy.h"
#include "esp_log.h"

void demo_judyl(void) {
    Pvoid_t array = (Pvoid_t)NULL;
    Word_t index = 1000;
    Word_t value = 999;
    JError_t jerr;

    // Insert
    PWord_t slot = JudyLIns(&array, index, &jerr);
    if (slot != PJERR) {
        *slot = value;
    }

    // Lookup
    slot = JudyLGet(array, index, &jerr);
    if (slot != (PWord_t)NULL) {
        ESP_LOGI("judyl", "Found index %lu -> value %lu", index, *slot);
    }

    // Free
    JudyLFreeArray(&array, &jerr);
}
```

---

## Configuration (`Kconfig`)

Run `idf.py menuconfig` and navigate to `Component config -> Expanse Embedded Digital Trie`:

- `EXPANSE_SRAM_INTERNAL_ONLY` (default: `y`): Forces node allocations into internal high-speed DRAM (`MALLOC_CAP_INTERNAL`) rather than external SPI PSRAM.
- `EXPANSE_DEFAULT_CHUNK_SIZE` (default: `2048`): Slab chunk size for variable-length payloads.
