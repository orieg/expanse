# ESP-IDF harvest application

The ESP-IDF application around the [`expanse` component](../../components/expanse/README.md)
for the on-device run tracked in #579. It exists so the component's link is
exercised end to end (it was not, until this application first failed to link)
and so the benchmark runner already in the component has something to flash.

What `app_main` does, in order:

1. Prints one JSON line of provenance: chip model and revision, core count,
   CPU clock, ESP-IDF version, `expanse_version()`, free and largest internal
   heap block.
2. Runs the component's Unity cases (`components/expanse/test/test_expanse.c`,
   tag `[expanse]`). A failing case stops the run: a library that does not
   pass its tests is not benchmarked. The *count* is asserted too — those
   cases register from constructor helpers that nothing references, so a
   `main` component linked without `WHOLE_ARCHIVE` loses the whole archive
   member and Unity reports `0 Tests ... OK`. That is a green gate that ran
   nothing, and it is how the first ESP32 boot passed with the engine
   entirely unexercised.
3. Runs `components/expanse/test/bench_esp32_memtable.c`, which emits one
   JSON object per measurement for `scripts/esp32_bench_harvest.py`
   (BCa 95% CIs, `--emit-json` for the charts).
4. Prints the main task's stack high-water mark, so the headroom the engine
   needs is a measured number in every run rather than a guess.

## Build, flash, harvest

```bash
. "$IDF_PATH/export.sh"
idf.py -C integrations/esp32 set-target esp32c3        # esp32c6 / esp32h2 / esp32p4 likewise
idf.py -C integrations/esp32 build
idf.py -C integrations/esp32 -p "$ESP_PORT" flash monitor
```

`sdkconfig.defaults` carries the settings the harvest depends on — main-task
stack size, the Unity test runner and its 64-bit comparisons, and `-O2` for
the C glue — so a fresh clone reproduces the harvest build with no
menuconfig pass. Each is commented with what it is for.

The component builds `libexpanse.a` with cargo for the bare-metal target
matching `IDF_TARGET`; the matching rustup target must be installed. For the
Xtensa parts (`esp32`, `esp32s2`, `esp32s3`) that means the esp-rs rustc
fork, which has no mainline target:

```bash
cargo install espup --locked
espup install --targets esp32
```

The configure step names those two commands if the `esp` toolchain is missing.

### Two targets side by side

`set-target` rewrites `build/` and `sdkconfig` in place, so switching between
an Xtensa and a RISC-V board loses the other's tree. Give each its own:

```bash
idf.py -C integrations/esp32 -B integrations/esp32/build.esp32 \
       -D SDKCONFIG=integrations/esp32/sdkconfig.esp32 set-target esp32
```

Both `build.*/` and `sdkconfig.*` are gitignored (`sdkconfig.defaults` is
not — it is an input, not an artifact).

Harvest a captured monitor log:

```bash
python3 scripts/esp32_bench_harvest.py --input esp32.log --out report.md --emit-json docs/benchmarks/embedded/esp32.json
```

Raw logs stay out of the repo (§8.5); the harvested JSON commits with its
provenance line. Results reference the target by chip, revision, clock and
IDF version only — no host names or serial ports (§7).

## Status

- `esp32`: **run on hardware** — ESP32-D0WD-V3 rev v3.1, 160 MHz, ESP-IDF
  v6.0-dev, `xtensa-esp-elf` GCC 15.2, Xtensa Rust 1.97.0.0. 11/11 Unity
  cases pass; the full benchmark suite completes; harvest committed to
  `docs/benchmarks/embedded/esp32.json`.
- `esp32c3` and `esp32c6`: application builds and links (ESP-IDF v6.0-dev,
  `riscv32-esp-elf` GCC 15.2). Neither has run on hardware.
- `esp32s2`, `esp32s3`: configure and target selection only; not built or run.
- `esp32p4`: staticlib float ABI asserted in CI; application link not yet
  attempted.
- No CI lane runs `idf.py`; the link is a local check until one exists.

### What the first on-device run found

Both defects were invisible to every build-time and host-side check, and
both were found by the run rather than by review:

- **ESP-IDF's default main-task stack is too small.** The deepest chain in
  the suite is the trie descent inside `expanse_map_insert`. Xtensa's
  windowed-register ABI spills a register window per call level, so the same
  descent that fits on RV32 does not fit in 3584 bytes here — peak use
  measured **4388 B**. The overflow ran into the adjacent DRAM and corrupted
  the heap's TLSF free-list metadata, so it surfaced as a `StoreProhibited`
  inside `insert_free_block` on some later allocation, with a backtrace
  pointing at the allocator rather than at the overflow. A/B isolated at
  fixed poisoning: 3584 B panicked on all 98 boots observed, 8192 B ran the
  suite to completion.
- **The Unity gate ran nothing.** See step 2 above. Fixed by `WHOLE_ARCHIVE`
  on the `main` component plus a case-count assertion, so the same regression
  fails loudly instead of reporting `OK`.
