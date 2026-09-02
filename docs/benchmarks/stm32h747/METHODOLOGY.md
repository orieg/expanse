# STM32H747I-DISCO on-target suite: pre-registration and measurement discipline

Point-in-time gate record for the Cortex-M7 / Cortex-M4 lane (#598). Frozen: the
hypotheses below are quoted from where they were written *before* the runs
(#598, filed 2026-09-01, and the PR discussions), and outcomes are appended
alongside them, never reconciled in place (AGENTS.md §8.7). Results and their
reading live in [`README.md`](README.md); the harness in
[`integrations/stm32h747/`](../../../integrations/stm32h747/README.md).

## 1. What the suite asks

1. Does the 32-bit engine run on ARM Cortex-M through the C ABI, and does the
   32-byte-line node geometry (`docs/design/32-bit-embedded.md` §2.1.4) pay
   off on a core that actually has a data cache?
2. What does the `sync32` interrupt-handler contract buy on real interrupts,
   against what bare-metal firmware does instead?
3. How does Expanse fare against the structures firmware usually reaches for,
   on the same core and the same fixtures?
4. What happens across two cores — the configuration the `sync32` header
   marks unsupported?

## 2. Pre-registered hypotheses and their outcomes

| # | pre-registered (source, before the run) | outcome | verdict |
|---|---|---|---|
| H1 | The Cortex-M7 D-cache line is 32 bytes and `BranchL2_32` fits one line; the cache-on/off ratio measures the geometry claim (#598 "Finding"; design doc §2.1.4) | CCSIDR reads 4-way × 128 sets × 32 B; cache-on cycle counts flat from 160 to 400 MHz while cache-off grows; ratio 1.6–1.9× at 2:1 core:bus | confirmed |
| H2 | "The critical section may win on total throughput; what the optimistic surface is expected to buy is bounded worst-case interrupt latency" (#598 step 2) | critical section 22% cheaper per mutation at full duty and never BUSY; `sync32` ISR entry-latency ceiling 49–83 cycles vs 1,551–1,728 (M7, 400 MHz); 91–98 vs 4,879–4,897 on the M4 | confirmed as written, both halves |
| H3 | BUSY rate falls with writer duty (stated in the #599 discussion after the first full-duty cell) | 71 / 15 / 4 / 0.4% at full / 40k / 10k / 1k mutations/s on the M7 | confirmed |
| H4 | Alternatives (stated in the #600 discussion before the run): the hash table wins point lookups and unordered ingest; the sorted array wins sequential-append ingest and bytes/key on random keys; Expanse wins steady-state ordered eviction and bytes/key on dense keys | all four as stated; **two losses not pre-registered**: the sorted array also wins point lookup (2.6×), and the ingest gap to the hash table is 17× on target vs 14× on the host | confirmed, with two unregistered losses reported |
| H5 | Dual core: "expect failures without cache maintenance; document what the header's restriction would need to lift" (#598 step 3) | with the M7 heap cacheable: 100% BUSY, 0 corrupted at every duty (fails safe, no hits); with the heap non-cacheable: correct, BUSY 100 / 62 / 23 / 2.7% by duty because an M4 read of the M7 heap costs ~9 µs | confirmed; requirement documented (uncached map heap or cache maintenance around the version bracket) |

Verdict labels are deterministic-instrument labels: DWT cycle counts vary
≤ 0.5% pass to pass, so a hypothesis is confirmed or refuted by the min-of-5
cell, not by an interval (AGENTS.md §8.4, deterministic branch). No
hypothesis was refuted. H4's two unregistered losses are recorded here and in
the README's scorecard rather than folded into the hypothesis.

## 3. Measurement discipline

- **Instrument**: DWT `CYCCNT` on each core; every fixture cell is the min of
  5 passes (median and max are in `results.json`); every ISR/dual cell is a
  fixed 200M-cycle budget with 10,000 interrupts (ISR arm) or as many reads as
  fit (dual cells).
- **Clocks verified from the host**: each clock change spins 320M cycles
  between `TICK`/`TOCK` lines that `capture.py` timestamps; `harvest.py`
  derives Hz from that and uses it for every nanosecond figure. The M4's spin
  is relayed by the M7 through the mailbox. Measured: 64.1 / 160.2 / 400.5 MHz
  (M7), 200.3 MHz (M4).
- **Writer pacing**: jittered gaps uniform over [P/2, 3P/2) with mean P. A
  fixed-period writer aliased with the fixed SysTick period and reported
  0 BUSY at 10k and 1k mutations/s; that run was discarded (#600).
- **Symmetric twins**: the alternatives run behind the same vtable and the
  same fixture code; bytes/key uses newlib `mallinfo` for all four (allocator
  overhead included symmetrically) plus requested bytes. The sorted array and
  the hash table are pre-sized to capacity, like the host suite's
  `HashMap::with_capacity`; Expanse and `tsearch` grow through `malloc`. The
  cross-core twin is hardware semaphore 0 around every access.
- **Discarded runs, stated**: (a) the aliased-pacing sweep above; (b) the first
  dual-core design placed the map in an SRAM4 bump arena and read zero hits —
  the `sync32` arena holds node handles, node bodies are heap `Box`es, so the
  M4 was reading the M7's cached heap (#601); (c) a capture whose banner trim
  matched the M4's dumped banner and dropped the M7 half of the transcript.
  None of these is in the committed artifact.
- **Provenance**: one board, one run per PR; `results.json` is produced from
  `transcript.txt` by `harvest.py` with the staticlib commit passed on the
  command line; the charts and the README tables are rendered from that file
  (`scripts/generate_stm32_svg.py`), never typed in.
- **Clock ceiling**: the DISCO is wired for direct SMPS supply, which caps the
  part at VOS1 / 400 MHz; VOS0 / 480 MHz needs the LDO path and a board
  modification, and was not attempted.

## 4. Not covered

VOS0 / 480 MHz; a balanced-tree twin (the toolchain ships none); the M4 as
writer; a QEMU `mps2-an385` execution lane (#598 step 4); external-reviewer
replication.

Superseded when a second board or a QEMU lane changes the protocol, in which
case a new dated record replaces this one and this file is marked superseded
in place.
