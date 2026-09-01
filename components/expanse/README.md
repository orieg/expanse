# Expanse ESP-IDF Component

[![ESP-IDF](https://img.shields.io/badge/ESP--IDF-v5.0%2B-blue.svg)](https://docs.espressif.com/projects/esp-idf/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

**Expanse** is a clean-room, high-performance digital radix trie engine. This
component builds the 32-bit engine as a bare-metal staticlib and links it into
an ESP-IDF project.

## Supported targets

| Chip | ISA | Rust target | Status |
|---|---|---|---|
| ESP32-C2, ESP32-C3 | RV32IMC | `riscv32imc-unknown-none-elf` | supported |
| ESP32-C6, ESP32-H2, ESP32-P4 | RV32IMAC | `riscv32imac-unknown-none-elf` | supported |
| ESP32, ESP32-S2, ESP32-S3 | Xtensa | — | **not supported** |

The Xtensa parts have no mainline rustc target — building for them needs the
esp-rs toolchain fork, which this component does not use. `CMakeLists.txt`
fails the configure step with a named error rather than registering a
component with no engine behind it.

`libexpanse.a` is built `no_std` against the *bare-metal* targets, not
`riscv32imc-esp-espidf`. It makes no libc calls: its allocator reaches memory
only through `expanse_host_malloc`/`expanse_host_free`, which `src/expanse_esp_idf.c`
defines over `heap_caps_malloc`. The bare-metal ABI (ilp32, soft float) is what
the ESP-IDF RISC-V toolchain emits, and the whole thing builds on stable rustc.

## What the 32-bit library exports

A 32-bit `libexpanse` carries **22 symbols** — the width-parametric ordered
core, and nothing else:

| Container | Entry points |
|---|---|
| identity | `expanse_version` |
| `expanse_set_t` | `_new`, `_free`, `_len`, `_mem_used`, `_clear`, `_insert`, `_remove`, `_contains`, `_contains_batch`, `_first`, `_last` |
| `expanse_map_t` | `_new`, `_free`, `_len`, `_mem_used`, `_clear`, `_insert`, `_get`, `_remove`, `_first`, `_last` |

**There are no `Judy*` symbols in a 32-bit build.** The legacy drop-in ABI is a
64-bit-only guarantee. So are the byte-string, string, blob and concurrent
containers, rank/select (`_count_below` / `_count_range` / `_by_count`), the
value-slot accessors (`_slot` / `_ins_slot`), and the
`_next_after` / `_prev_before` family. Omitted symbols are **absent**, not
stubbed — you get a link error naming the gap, not different behaviour.

The full matrix, and why each omission exists, is in
[docs/COMPAT.md](../../docs/COMPAT.md#build-configuration-surface-matrix).

## Architecture on 32-bit parts

- **Compact 8-byte `Edge32` descriptors** (`[ptr: 4B | aux: 3B | tag: 1B]`),
  packing up to 7 immediate keys with zero heap allocations.
- **32-byte node alignment.** Node geometries are sized for embedded
  microarchitectures. `expanse_host_malloc` returns whatever
  `heap_caps_malloc` gives (4–8 byte alignment), so the Rust-side allocator
  over-allocates and aligns by hand — see
  `crates/expanse-capi/src/alloc_bridge.rs`.
- **SRAM-targeted allocation** through `MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT`,
  selected by `CONFIG_EXPANSE_SRAM_INTERNAL_ONLY`.

No throughput, latency or footprint figure is published for this component:
nothing in CI measures it on an ESP target, and an unmeasured number is not a
claim this project makes.

## Installation

### Via ESP-IDF Component Manager

Add `expanse` to your project's `main/idf_component.yml`:

```yaml
dependencies:
  expanse:
    version: "^0.5.0"
```

### Via local submodule / component copy

Clone or copy `components/expanse` into your ESP-IDF project's `components/`
directory:

```bash
cd my-esp-project/components
git clone https://github.com/orieg/expanse.git
```

The component invokes `cargo build --release` during the ESP-IDF build, so a
stable Rust toolchain and the matching target must be installed:

```bash
rustup target add riscv32imc-unknown-none-elf riscv32imac-unknown-none-elf
```

## Usage in C

Keys and values are `expanse_word_t` — one machine word, so `uint32_t` here.
`expanse.h` typedefs it and defines `EXPANSE_WIDE_SURFACE` (0 on 32-bit) so the
same header serves both libraries.

```c
#include "expanse.h"
#include "expanse_esp_idf.h"
#include "esp_log.h"

static const char *TAG = "app_main";

void app_main(void) {
    expanse_map_t *map = expanse_map_new();

    expanse_word_t can_id = 0x18FF50E5;
    expanse_map_insert(map, can_id, 42, NULL);

    expanse_word_t val = 0;
    if (expanse_map_get(map, can_id, &val)) {
        ESP_LOGI(TAG, "CAN ID 0x%08X -> %u", (unsigned int)can_id, (unsigned int)val);
    }

    ESP_LOGI(TAG, "%u keys, %u heap bytes",
             (unsigned int)expanse_map_len(map),
             (unsigned int)expanse_map_mem_used(map));

    expanse_map_free(map);
}
```

## Configuration (`Kconfig`)

`idf.py menuconfig` → `Component config` → `Expanse Embedded Digital Trie`:

- `EXPANSE_SRAM_INTERNAL_ONLY` (default `y`): draw libexpanse's allocations
  from internal high-speed DRAM (`MALLOC_CAP_INTERNAL`) rather than external
  SPI PSRAM. When disabled, `MALLOC_CAP_DEFAULT` is used.
