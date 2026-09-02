# QEMU Cortex-M3 smoke (mps2-an385)

The ARM execution gate that needs no hardware (#598 step 4). CI builds the
narrow-surface C ABI staticlib for `thumbv7m-none-eabi` (soft float), links
`smoke.c` bare-metal for QEMU's `mps2-an385` machine (Cortex-M3), and runs
it under `qemu-system-arm` with semihosting, so the firmware's exit code is
the job's exit code and its transcript is the job log.

What it checks: the ordered map and set through the C ABI (insert / replace
/ get / remove, ascending iteration, first / last / next, `remove_range`
counts), and the `sync32` single-writer / interrupt-reader protocol — a
SysTick interrupt reads random keys while the main loop churns inserts and
removes for at least 200k mutations and at least 2,000 interrupts; any
wrong value, a reader that never succeeds, an arena-full, or a writer view
that ends up corrupted fails the run.

What it does not check: cycles, caches (the M3 has none), or timing — QEMU
is not cycle-accurate. Those stay a hardware measurement in
[`integrations/stm32h747/`](../stm32h747/README.md) and its suite
[`docs/benchmarks/stm32h747/`](../../docs/benchmarks/stm32h747/README.md).

```bash
cargo build --release -p expanse-capi --no-default-features \
  --features embedded-panic-handler --target thumbv7m-none-eabi
sh integrations/qemu-cortex-m3/build.sh   # asserts ARMv7-M, soft float
sh integrations/qemu-cortex-m3/run.sh     # PASS + exit 0, or FAIL <check> + exit 1
```

Needs the Arm GNU toolchain (`arm-none-eabi-gcc`) and `qemu-system-arm`
(Homebrew `qemu`, apt `qemu-system-arm`). Files: `smoke.c`, `startup.c`,
`mps2.ld` (code at 0, 2 MB SRAM at `0x20000000`, heap after `.bss`,
64 KB stack at the top), `build.sh`, `run.sh`.

Superseded when a probe-rs or Renode lane replaces the semihosting flow, or
when the smoke is folded into the STM32 harness as a build variant.
