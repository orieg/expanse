# Expanse ESP-IDF Component

[![ESP-IDF](https://img.shields.io/badge/ESP--IDF-v5.0%2B-blue.svg)](https://docs.espressif.com/projects/esp-idf/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

**Expanse** is a clean-room, high-performance digital radix trie engine. This
component builds the 32-bit engine as a bare-metal staticlib and links it into
an ESP-IDF project.

## Supported targets

| Chip | ISA (HP core) | Harts (HP / LP) | Rust target built | Status |
|---|---|---|---|---|
| ESP32-C2 (ESP8684), ESP32-C3 | RV32IMC | 1 / 0 | `riscv32imc-unknown-none-elf` | supported |
| ESP32-C6 | RV32IMAC | 1 / 1 | `riscv32imac-unknown-none-elf` | supported |
| ESP32-H2 | RV32IMAC | 1 / 0 | `riscv32imac-unknown-none-elf` | supported |
| ESP32-P4 | RV32IMAFC (+Zc, Zb) | 2 / 1 | `riscv32imafc-unknown-none-elf` (ilp32f, matching ESP-IDF's `-mabi=ilp32f`) | supported, end-to-end link untested — see below |
| ESP32 | Xtensa | 2 / — (reported by the part) | `xtensa-esp32-none-elf` (esp-rs fork) | supported, harvested on hardware — see below |
| ESP32-S2 | Xtensa | not sourced | `xtensa-esp32s2-none-elf` (esp-rs fork) | supported, end-to-end link untested |
| ESP32-S3 | Xtensa | not sourced | `xtensa-esp32s3-none-elf` (esp-rs fork) | supported, end-to-end link untested |

Every ISA and hart count in the **RISC-V** rows above is quoted from the part's
Espressif datasheet or Technical Reference Manual — document revision, section
and page — in
[`docs/HARDWARE.md` §4.3](../../docs/HARDWARE.md#43-espressif-risc-v-per-part-core-inventory--cas-soundness--validated-567).
That section is a RISC-V inventory and does not cover the Xtensa parts, so the
Xtensa rows are not sourced from it: the ESP32 core count is what the part
itself reported over UART during the harvest below, and the S2/S3 counts are
left unstated rather than asserted from memory. The RISC-V section also
records which compare-and-swap mechanism is sound per part;
the short version is that `portable-atomic`'s `unsafe-assume-single-core` is
sound on C2/C3/H2 and **unsound on C6 and P4**, which have a second hart on the
same buses.

**Xtensa.** The Xtensa parts have no mainline rustc target. They are built
against the esp-rs rustc fork, which `espup` installs as the `esp` rustup
toolchain:

```bash
cargo install espup --locked
espup install --targets esp32   # or esp32s2 / esp32s3
```

`CMakeLists.txt` selects the fork toolchain and a from-source sysroot for
those targets — `xtensa-*-none-elf` is tier 3 and publishes no precompiled
`core`/`alloc`, so the Xtensa arm adds `-Z build-std=core,alloc`. If the `esp`
toolchain is absent the configure step fails with the two commands above
rather than registering a component with no engine behind it. Nothing else
differs between the two ISAs: the same `no_std` staticlib, the same
host-allocator pair, the same link order. The RISC-V command line is
unchanged.

`libexpanse.a` is built `no_std` against the *bare-metal* targets, not
`riscv32imc-esp-espidf`. It makes no libc calls: its allocator reaches memory
only through `expanse_host_malloc`/`expanse_host_free`, which `src/expanse_esp_idf.c`
defines over `heap_caps_malloc`. On C2, C3, C6 and H2 the bare-metal ABI (ilp32,
soft float) is the only one available — none of those cores implements the F
extension — and the whole thing builds on stable rustc.

**ESP32-P4.** The P4's HP complex is RV32IMAFC (ESP32-P4 Datasheet
Pre-release v0.7 §4.1.1.1), and ESP-IDF compiles it hard-float — its build
system (`components/soc/project_include.cmake` in esp-idf) sets
`-march=rv32imafc` (`rv32imafcb` on newer silicon revisions, a superset that
runs `imafc` code) and `-mabi=ilp32f` for FPU-bearing cores. This component
therefore builds `esp32p4` against `riscv32imafc-unknown-none-elf`, whose
archive carries `EF_RISCV_FLOAT_ABI_SINGLE` (ilp32f) — a soft-float `imac`
archive would be rejected by the RISC-V linker's float-ABI check (#581). CI
builds the `imafc` staticlib and asserts its ELF float-ABI flag; the ESP-IDF
link itself still has no CI lane, so treat end-to-end P4 linking as unverified
rather than known-good until an on-device build covers it.

**End-to-end link status.** The full ESP-IDF application link is verified
locally for `esp32c3`, `esp32c6` (ESP-IDF v6.0-dev, `riscv32-esp-elf` GCC 15.2)
and `esp32` (same IDF, `xtensa-esp-elf` GCC 15.2, Xtensa Rust 1.97.0.0)
through the wrapper application in
[`integrations/esp32/`](../../integrations/esp32/README.md). The first such
link failed: the archive was listed as a bare interface path, ESP-IDF does
not wrap the final link in `--start-group`, and the two host-allocator
symbols the archive imports came *before* it on the link line and stayed
undefined. `CMakeLists.txt` now registers the archive as an imported library
that links back to the component, which is what orders it correctly. There
is still no CI lane for the ESP-IDF link.

**On-device status.** `esp32` is the only part run on hardware so far — an
ESP32-D0WD-V3 rev v3.1 at 160 MHz. The harvest is
[`docs/benchmarks/embedded/esp32.json`](../../docs/benchmarks/embedded/esp32.json)
and the run found two defects that no build-time check could have caught:

- **The main task needs more stack than ESP-IDF gives it.** The suite's
  deepest chain is the trie descent inside `expanse_map_insert`, and on
  Xtensa the windowed-register ABI spills a register window per call level.
  Peak use measured **4388 B** against ESP-IDF's default 3584 B, so the task
  ran off the end of its stack into the adjacent DRAM. What it corrupted was
  the heap's TLSF free-list metadata, so the failure surfaced later and
  somewhere else — a `StoreProhibited` inside `insert_free_block` on a
  subsequent allocation, backtracing to the allocator rather than to the
  overflow. A/B isolated: 3584 B panics on every boot, 8192 B completes.
  [`integrations/esp32/sdkconfig.defaults`](../../integrations/esp32/sdkconfig.defaults)
  sets it; a consumer calling the engine from their own task needs the same
  headroom, which is why the app publishes its high-water mark every run.
- **The Unity gate was vacuous.** `test_expanse.c` exports nothing anyone
  links against, so without `WHOLE_ARCHIVE` the linker never extracted it
  from `libmain.a`: all 11 cases were compiled, discarded, and reported as
  `0 Tests ... OK`. The first ESP32 boot passed its test gate with the engine
  entirely unexercised. `main.c` now asserts the case count.

`esp32s2`, `esp32s3` and the RISC-V parts have not been run on hardware;
#579 tracks the rest.

## What the 32-bit library exports

A 32-bit `libexpanse` carries **57 symbols** — the width-parametric ordered
core with full bidirectional range navigation:

| Container | Entry points |
|---|---|
| identity | `expanse_version` |
| `expanse_set_t` | `_new`, `_free`, `_len`, `_mem_used`, `_clear`, `_insert`, `_remove`, `_contains`, `_contains_batch`, `_first`, `_last`, `_next_at_or_after`, `_next_after`, `_prev_at_or_before`, `_prev_before` |
| `expanse_map_t` | `_new`, `_free`, `_len`, `_mem_used`, `_clear`, `_insert`, `_get`, `_remove`, `_remove_range` (32-bit-only: batched eviction), `_for_each_range` (32-bit-only: streaming range walk), `_remove_many` (32-bit-only: batched scattered removal), `_first`, `_last`, `_next_at_or_after`, `_next_after`, `_prev_at_or_before`, `_prev_before` |
| `expanse_sync32_map_t` / `expanse_sync32_set_t` | one writer, single-attempt optimistic readers (#573, provisional): `_new`, `_free`, `_writer`, `_reader`, `_writer_try_insert`, `_writer_try_remove`, `_writer_try_reclaim`, `_writer_get` / `_writer_contains`, `_writer_stats`, `_reader_try_get` / `_reader_try_contains`, `_reader_try_len`; `expanse_sync32_mutation_headroom`, `expanse_sync32_status_str` |

**There are no `Judy*` symbols in a 32-bit build.** The legacy drop-in ABI is a
64-bit-only guarantee. So are the byte-string, string, blob and concurrent
containers, rank/select (`_count_below` / `_count_range` / `_by_count`), and
the value-slot accessors (`_slot` / `_ins_slot`). Omitted symbols are
**absent**, not stubbed — you get a link error naming the gap, not different
behaviour.

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
