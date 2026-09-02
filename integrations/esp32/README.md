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
   pass its tests is not benchmarked.
3. Runs `components/expanse/test/bench_esp32_memtable.c`, which emits one
   JSON object per measurement for `scripts/esp32_bench_harvest.py`
   (BCa 95% CIs, `--emit-json` for the charts).

## Build, flash, harvest

```bash
. "$IDF_PATH/export.sh"
idf.py -C integrations/esp32 set-target esp32c3        # esp32c6 / esp32h2 / esp32p4 likewise
idf.py -C integrations/esp32 build
idf.py -C integrations/esp32 -p "$ESP_PORT" flash monitor
```

The component builds `libexpanse.a` with cargo for the bare-metal RISC-V
target matching `IDF_TARGET`; the matching rustup target must be installed.
Xtensa parts (`esp32`, `esp32s2`, `esp32s3`) fail at configure by design —
there is no mainline rustc target for them.

Harvest a captured monitor log:

```bash
python3 scripts/esp32_bench_harvest.py --input esp32.log --out report.md --emit-json docs/benchmarks/embedded/esp32.json
```

Raw logs stay out of the repo (§8.5); the harvested JSON commits with its
provenance line. Results reference the target by chip, revision, clock and
IDF version only — no host names or serial ports (§7).

## Status

- `esp32c3` and `esp32c6`: application builds and links (ESP-IDF v6.0-dev,
  `riscv32-esp-elf` GCC 15.2). Nothing has run on hardware yet.
- `esp32p4`: staticlib float ABI asserted in CI; application link not yet
  attempted.
- No CI lane runs `idf.py`; the link is a local check until one exists.
