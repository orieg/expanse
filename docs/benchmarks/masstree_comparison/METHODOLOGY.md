# Masstree vs. Expanse: Pre-Registration & Comparative Methodology

**Status: committed before the harness commit (`54dea5ad`, then `82966aae`
forty minutes later — a commit ordering, not a claim that no harness code was
being written); measured on the reference host at harness commit `82966aae`
and published in [`README.md`](README.md).
The text of §1–§9 is the locked pre-registration; outcomes are recorded in the
README and are never reconciled into it (§8.7); measurement constraints found
while building and running are §10.** This document is commit 2 of the
three-commit cadence (AGENTS.md §8.8) for
[#661](https://github.com/orieg/expanse/issues/661): hypothesis, locked
constraint decisions, expected-losses matrix, gate taxonomy and claims ceiling,
committed before the harness and before any measurement. Outcomes are recorded
in [`README.md`](README.md) and are never reconciled into this text (§8.7);
measurement constraints found while building the harness are appended as
amendments in §10.

The bounds this document leans on are functions in
[`scripts/masstree_envelope.py`](../../../scripts/masstree_envelope.py) (commit
1), each with a reference-pinned test; the text names the function rather than
repeating its arithmetic.

---

## 1. Problem Statement

[#387](https://github.com/orieg/expanse/issues/387) filed three SOTA comparison
arms. ART landed in `docs/benchmarks/art_comparison/`; HOT, its C++ FFI
foundation, its string-key arms and its ROWEX concurrent arm landed in
`docs/benchmarks/hot_comparison/` (#660, #693, #692). Masstree is the last, and
it is the arm for the two regimes the repo still cannot speak to with a
measured competitor:

- **Write concurrency.** Masstree ([Mao, Kohler & Morris, EuroSys 2012](https://doi.org/10.1145/2168836.2168855))
  admits concurrent writers with per-node locks and version-validated lock-free
  readers. `SyncExpanseMap` / `SyncExpanseStrMap` serialize every writer on one
  mutex; the protocol is optimistic lock coupling and **blocking by design**
  (AGENTS.md §2.2). #692 measured that loss against HOT-ROWEX; this arm is the
  second, independent route to it, on a structure whose concurrency protocol
  the same authors designed for exactly this workload.
- **Long shared-prefix string keys.** Masstree is a trie of B+-trees: one
  B+-tree per 8-byte key slice, descended a slice at a time. `ExpanseStrMap`
  also keys one 8-byte chunk per level (`hot_comparison/METHODOLOGY.md` §10.1).
  The two therefore descend the *same number of slices* through a shared
  prefix and differ in what each slice costs; that is the contest §6 registers,
  and the issue's stated expectation that Expanse loses it is carried as a
  registered prediction with its confidence stated honestly.

Implementation under test: [`kohler/masstree-beta`](https://github.com/kohler/masstree-beta)
at `1119842` (the tip of `master`, October 2023), pinned as the submodule
`third_party/masstree`, reached through the C++ FFI foundation
`crates/expanse-hot-bench` built for #660. **Not** the pure-Rust `masstree`
crate on crates.io, for the reason #387 exists: a number measured against a
single-author re-implementation would be a number about that crate.

---

## 2. Prior Observations at Lock Time (mandatory disclosure)

This pre-registration is **not** blind. A Step 0 feasibility gate ran before it
was written, exactly as #660's did. It is a standalone C++ program against the
pinned Masstree, compiled with the flags §3.5 locks, not a registered harness,
and nothing in it is a published figure. Every prediction below that leans on
an observation is marked **(informed)**.

*(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
Ubuntu 22.04, kernel 6.8; Masstree `1119842`; `g++ 11.4 -O3 -std=c++17
-march=haswell -DNDEBUG`; Step-0 gate program, not a registered harness;
workload `step0_masstree_gate`; load average 1.22 at start — a co-resident
process held one core at 100%, so every timing in this table is diagnostic
and none is a figure)*:

| Check | Result |
|---|---|
| Builds without autoconf | yes — six translation units (`compiler`, `kvthread`, `str`, `string`, `straccum`, `json`) and a hand-derived `config.h` reproducing configure's x86-64 Linux output |
| `sizeof(leaf<15>)` / `sizeof(internode<15>)` | 312 B / 272 B, both pool-rounded to **320 B** — one size class |
| Correctness, 1M uniform random u64 keys, values full width | 1,000,000/1,000,000 inserted, found with correct value, and walked; 0/100,000 rejection-sampled misses found |
| Key 0 with value 0; key and value with bit 63 set; `u64::MAX` | all inserted, found, value intact — **no payload predicate on integer keys** (values are discriminated from layer pointers by the leaf's key-length byte, not by a tag bit) |
| Ordered scan, k = 10 from a random start | 10 visited, in sorted order |
| `prefixed` shape (96 shared bytes + 24 random), 100k keys | 100,000/100,000 found and walked; `json_stats` places every key at **layer 12** — one one-key twig leaf per shared 8-byte slice (`layers_for_shared_prefix(96)`) |
| 272-byte keys (`beyond` shape) | insert and lookup succeed mechanically (1,000/1,000); scan over such a table writes past a `MASSTREE_MAXKEYLEN`-sized stack buffer with preconditions compiled out — **outside the library's contract** (§3.4) |
| Node census, 1M random u64 (`json_stats`) | 94,381 leaves, 9,144 internodes, 0 suffix bytes: **33.13 B/key** structural, leaf fill 70.6% (`structural_bytes`, `leaf_fill`) |
| Allocator census, build-only, N = 1k and 10k | **0 B counted** — the whole build fit inside the one 2 MiB slab the gate created before arming (§3.3) |
| Allocator census, N = 100k / 1M | 20.97 / 31.46 B/key counted, from 1 / 15 slabs, plus one uncounted slab each — at 1M the 16 slabs are within 1.3% of the structural bytes |
| Rebuild on the same `threadinfo` after `destroy` | 0.50× the cold figure — freed nodes sit in RCU limbo and the pool, so a warm thread slot changes what a build allocates (§3.6) |
| 1M inserts, W = 1 / 8 / 16 writers, own `threadinfo` each | 147.4 / 27.4 / 16.4 ms — **diagnostic, shared host, not a figure**; all 1,000,000 keys verified from a thread that never inserted |
| 8 readers probing a 1M prefill while 8 writers insert 1M fresh keys | 1,364,604 reads, **0 errors**; 2,000,000 walked after join |

No Expanse-side observation was made in the gate. No timing on any registered
harness exists.

---

## 3. Locked Constraint Decisions

#661 lists three constraints as blocking. They are settled here with the
evidence each rests on, plus three the gate surfaced. None is revisited by the
harness; what the harness finds goes in §10.

### 3.1 License — vendoring and linking are permitted

`masstree-beta` ships its own `LICENSE`: an **MIT license plus one clause taken
from the W3C license** — the copyright holders' names and trademarks may not be
used in advertising or publicity pertaining to the Software without written
permission. Copying, modification, distribution and linking are granted without
restriction; the notice must be preserved.

**Locked:** the source enters the tree as a **pinned submodule, unmodified**,
with its `LICENSE` and `AUTHORS` intact; nothing is vendored into this
repository's own directories and no Masstree file is patched. The suite's prose
attributes the design to its paper by bibliographic citation only and does not
use the authors' names in any other way. This is a new category for the
workspace — third-party C++ *source* rather than a system library — and the
`third_party/` convention #660 established for HOT (ISC) holds for it. The §3
clean-room rule is not implicated: Masstree is unrelated to libjudy.

### 3.2 Threading model through FFI — per-thread `threadinfo`, epochs advanced as `mttest` does

Every Masstree operation takes a `threadinfo&`. The library owns its
epoch-based reclamation: each thread records freed nodes in its own limbo list,
`rcu_start` / `rcu_stop` bracket a thread's active period, `rcu_quiesce`
reclaims what every active thread has moved past, and the **global epoch is
advanced by the program, not the library** — `mttest` derives it from the wall
clock (`timestamp() >> 16`, roughly 65 ms granularity) inside its
`rcu_quiesce`, and calls that every 64 operations from its timed loops.

**Locked:**

1. **Thread slots.** The shim owns an array of `threadinfo` slots. A slot is
   created lazily under a mutex — `threadinfo::make` pushes onto an
   unsynchronised global list and is not safe to call concurrently — and is
   never freed; the library has no destructor for it. A Rust thread takes one
   slot for its lifetime; a slot may be reused by a later thread but never by
   two at once.
2. **Bracketing.** Every harness thread calls `rcu_start` before its first
   operation and `rcu_stop` after its last, so a finished thread does not pin
   the active epoch for everyone else (`min_active_epoch` skips only slots
   whose epoch is zero).
3. **Quiescence cadence.** The Masstree side calls the shim's `quiesce` every
   64 operations **inside its timed loops**, reproducing `mttest` exactly: the
   global epoch is set from the clock under a lock when it has moved, then
   `rcu_quiesce` on the calling thread. This is Masstree's cost of operating
   and is billed to Masstree; the Expanse side runs its own epoch mechanism
   inside its API and receives no extra call (§8.16: each system through its
   native protocol).
4. **Validation before any figure.** `masstree_validate` re-runs the gate's
   threaded checks — disjoint concurrent inserts verified by walking from a
   thread that never inserted; readers alongside writers with zero errors —
   and no concurrent cell is recorded if it fails (§8.1).

### 3.3 The allocator, and what the memory census may publish

Masstree's README says it "needs a fast malloc" and its configure links
jemalloc, tcmalloc, mimalloc or its own Flow allocator when one is found.
Beneath that, `threadinfo::pool_allocate` carves nodes from **2 MiB slabs**
obtained by `posix_memalign`, one slab at a time per size class per thread
(`kvthread.cc refill_pool`), and `threadinfo::allocate` uses `malloc` for
external key-suffix bags and limbo groups. The reference host has no jemalloc,
tcmalloc or mimalloc installed and no package is installed for a benchmark.

**Locked:**

1. **Both arms allocate from glibc 2.35 `malloc`**, as every arm in the HOT
   suite does. This is disclosed as a risk against Masstree, not against
   Expanse: its authors expect a faster allocator under it. No jemalloc or
   tcmalloc cell is run and no claim about Masstree under either is made (§8).
2. **The census is published, on two instruments, and says which is which.**
   The issue made this pillar optional pending a defensible method; the method
   is:
   - **Allocator column** — bytes held from the C allocator after a
     build-only population, one link-time interposition for both arms exactly
     as the HOT suite (§9.1 there), with the table and its thread slot created
     **inside the armed window**. On Masstree this figure is **quantized to
     the 2 MiB slab**: every cell also reports its measured **slack** — the
     allocator figure minus Masstree's structural figure below — and
     `census_quantum_dominated` flags every cell whose slack exceeds 25%
     of the structural bytes. A flagged cell is published with its
     flag and is never read as a per-key cost of the index.
     `slab_slack_bound(classes)` is the a-priori ceiling the slack must
     respect. The gate measured the quantum's two faces: 0 B counted at
     N ≤ 10k, 1.3% slack at N = 1M.
   - **Engine-instrument column** — Masstree's own `json_stats` node census
     turned into bytes by `structural_bytes` (leaves and internodes at their
     pool-rounded 320 B, plus external suffix-bag capacity and the extra a
     leaf was allocated for an internal bag), beside Expanse's own
     `mem_used()`. These are each system's deterministic accounting of its
     own nodes, the same pairing the HOT suite publishes as its "engine
     instrument" column, and they are **never mixed with the allocator
     column in one comparison**.
   - **Falsifier.** A Masstree cell whose allocator figure is *below* its
     structural figure is void — the census is not seeing the arm. A cell
     whose allocator figure exceeds structural by more than
     `slab_slack_bound(20) + 16 B × malloc count` is void — something outside
     the model is being counted.
3. **Superpages stay on.** `configure` enables `HAVE_SUPERPAGE` by default on
   Linux; the pool then requests 2 MiB-aligned slabs and `madvise`s them
   `MADV_HUGEPAGE`, and the reference host's transparent-hugepage policy is
   `madvise`, so Masstree's nodes are huge-page backed while Expanse's `malloc`
   arena is not. Turning it off would configure the competitor below its
   shipped default; it is left on and **disclosed as favouring Masstree** on
   TLB behaviour, unmeasured — no counter is taken for it here (§8.9).
4. **The census never runs inside a throughput cell**, for the reason §11.3
   of the HOT methodology records: armed counters under 16 writers distorted
   the build 4.6×. Throughput cells run disarmed; census cells are their own
   processes with no timing claim.

### 3.4 The 255-byte key-length predicate

`MASSTREE_MAXKEYLEN` is 255 by default. The gate showed that insert and lookup
of a 272-byte key *succeed mechanically* while scan over such a table writes
past a `MASSTREE_MAXKEYLEN`-sized stack buffer once preconditions are compiled
out. The limit is therefore the library's declared contract, not a silent
truncation like HOT's — but the standing rule from `hot_comparison` §9.4 and
§10.4 applies unchanged: **a twin's limitation is a predicate on the twin,
evaluated against the workload, reported as a finding; the workload is never
edited for it.**

**Locked:** `masstree_can_key(len) := len ≤ 255`, read from the header through
the shim. The shim **refuses** a longer key at the call (returns a distinct
code and stores nothing), the harness evaluates the predicate over every
population before building, and a cell whose population contains any key the
predicate rejects publishes the Expanse figure with the finding
(`not representable: n keys > 255 B`) in Masstree's column — never a Masstree
number over a smaller population. The `beyond` shape exists to exercise this.
Raising the constant is a compile-time reconfiguration of the competitor and
is not done; the shipped default is measured and the fact that it is
configurable is recorded here.

### 3.5 Build-flag symmetry, and Masstree's own defaults

Masstree's authors specify no ISA target (`-g -W -Wall -O3`) and
`--disable-assertions` for measurement. `hot_comparison` §3.3 binds both arms
of every cell to one ISA target so no cell compares an AVX2 C++ arm against a
baseline Rust arm.

**Locked:** Masstree is built `-O3 -std=c++17 -march=haswell -DNDEBUG` with
assertions, preconditions and invariants off (the `config.h` the crate ships
reproduces `--disable-assertions`), and Expanse with `-C target-cpu=haswell`,
exactly the flags every HOT cell used; the same `cc` toolchain compiles both
shims. The flags are recorded in every result artifact.

### 3.6 One cell, one process, one fresh thread slot

The gate showed that a rebuild on a `threadinfo` that has destroyed a table
reports a different figure (0.50×): freed nodes sit in that thread's limbo
list and pool free lists and are reused without an allocator call. This is the
same hazard class as HOT's process-global pool (§9.2 there), scoped to a
thread slot rather than the process.

**Locked:** every census cell is its own process invocation, driven by the
runner, and builds on a **fresh thread slot** created inside the armed window;
no census process destroys a table. Latency cells likewise run one cell per
process, so that no pillar's leftovers change another's node shapes.

---

## 4. Pairings

Masstree stores one value word per key inside the leaf (`leafvalue`, 8 bytes);
it has no value-less configuration. Rather than charge it for a word a set does
not carry, the arm pairs it with the Expanse types that carry the same payload,
and states what is not measured.

| Pairing | Masstree side | Expanse side | Workload ID |
|---|---|---|---|
| **M1 — integer map** | `basic_table<P>`, `value_type = uint64_t`, 8-byte big-endian keys (so byte order is numeric order) | `ExpanseMap` (`u64 → u64`) | `masstree_map_64bit` |
| **M2 — string map** | the same table type with byte-string keys; keys longer than 8 bytes hold their suffix in the leaf's suffix bag or descend a layer | `ExpanseStrMap` (`&[u8] → u64`) | `masstree_str_map` |
| **MC1 — concurrent integer map** | M1's table, W writer slots and R reader slots | `SyncExpanseMap` | `masstree_conc_map_64bit` |
| **MC2 — concurrent string map** | M2's table, W and R slots | `SyncExpanseStrMap` | `masstree_conc_str` |

**Ownership symmetry (M2, MC2).** Both sides *copy* key bytes into their own
nodes — Masstree into its ikey slots and suffix bags, Expanse into its chunk
levels and `StrSuffix` leaves. Unlike the HOT string arms, where HOT stored a
pointer into harness-owned strings and the census had to publish an
`external` column (`hot_comparison` §10.3), here the index column *is* the
ownership column on both sides and no external-storage column exists. The
harness still allocates every key individually and probes from separate
allocations, as the string generator already does, so the Masstree side's
suffix compares read from its own nodes rather than from a probe it was just
handed.

**Not measured, and why.** No `ExpanseSet` pairing: Masstree would carry an
8-byte value word per key that a set does not, and the comparison would be
decided by the payload rather than the index (§C-b). No `ExpanseBytesMap`
pairing: a hash-indexed structure against a trie of B+-trees was measured once
against HOT (Arm E) and described as not a trie comparison; it is not repeated
here.

---

## 5. Workload

Shared with the HOT suite through `expanse-hot-bench`'s `workload.rs` and
`strings.rs` — the same XorShift64 at the same seed, the same rejection-sampled
misses (§8.6), the same shuffled probe streams (§9.8 there), extended rather
than duplicated (§8.3 symmetry by construction; #693's scaffolding reused as
#661 requires).

**Integer cells (M1).** Distributions `sequential`, `clustered`, `sparse`,
`random` over the full 64-bit domain — no keyspace restriction, since §2 found
no payload predicate. Populations N ∈ {10⁴, 10⁵, 10⁶} for the latency pillars;
the memory pillar sweeps expanse occupancy λ on `random` at the HOT suite's ten
targets (1 … 61), so the `ExpanseMap` column is the *same cells* as
`hot_comparison` §1 and the two suites' Expanse columns are relatable by
workload ID; `sequential`, `clustered` and `sparse` are censused at N = 10⁶
only, because their occupancy is construction-fixed (`hot_comparison` §9.5).

**String cells (M2).** The five shapes of `hot_comparison` §10.5 unchanged —
`short` (8–16 random alphanumerics), `counter` (`k` + 11 digits), `prefixed`
(96 shared bytes + 24 random), `skewed` (Pareto lengths, 4–192), `beyond`
(256 shared + 16 random, 272 bytes, fails the §3.4 predicate for its whole
population). Latency at N ∈ {10⁴, 10⁵, 10⁶}; memory as the same population
sweep {1k … 1M}, dense around 1.23 × 10⁵ for the reason recorded there.

**Pillars, per pairing.** Point lookup at 100% hit; point lookup at 50% hit /
50% same-generator miss; insertion into a cold structure; ordered range scan
with k ∈ {10, 100, 1000} from 1,000 starts drawn from the probe stream; memory
census per §3.3. Both sides fetch the stored value and fold it into a sink; on
M2 the sinks must be equal at the end of every lookup round (identical values
stored on identical keys) and a divergence voids the cell. The Expanse scan
drives the shipped surfaces — `range()` on `ExpanseMap`, `next_at_or_after` /
`next_after` on `ExpanseStrMap` — against Masstree's `scan` from a start key
with a visitor that stops after k; the string-scan disclosure of
`hot_comparison` §10.6 applies verbatim.

**Concurrent cells (MC1, MC2).** The exact cells of `hot_comparison` §11.4,
so the two routes to the write-concurrency loss are read side by side: prefill
N₀ = 2²⁰ inserted single-threaded outside the window, M = 2²⁰ fresh keys
rejection-sampled against the prefill, split into W contiguous slices; fixed
work, timed from barrier release to the last writer's join; R readers probing
a 50/50 stream against the prefill until the writers finish, values checked on
every hit. **C1:** W ∈ {1, 2, 4, 8, 16}, R = 0. **C2:** R = 8,
W ∈ {0, 1, 2, 4, 8}. **H:** the C2 cells with writers, Expanse side only, on
the `occ-stats` build — restart share, fallback share, `sample_spins`; event
ratios, never a timing (decision 5 there). **M:** build-only single-writer
census at the λ targets, `Masstree` against `SyncExpanseMap`. MC2 uses the
`short` shape for its prefill and fresh keys and mirrors C1 and C2 only.
Writers + readers ≤ 16 inside the P-core pin, `Cpus_allowed_list` recorded per
row (decision 3 there). Both arms below any external lock (decision 4 there):
Masstree through its own per-node locks and version validation, Expanse through
`SyncExpanseMap` / `SyncExpanseStrMap`.

**Rounds and statistics.** 15 rounds per wall-clock cell, arms interleaved per
round, structures rebuilt outside the window; verdicts on the BCa 95% bootstrap
interval of the ratio over ≥ 2,000 resamples (§8.4). **Ratios are Masstree ÷
Expanse for latency (above 1.000 means Expanse is faster) and Expanse ÷
Masstree for throughput (above 1.000 means Expanse is faster)** — the same
orientation as every table in the HOT suite. Memory is deterministic and
carries no interval.

---

## 6. Expected-Losses Matrix

Registered before measurement. Confidence is the pre-registration's own.
Losses first, because two of them are the reason the arm exists. Where a row
leans on a Step 0 observation it says **(informed)**; where it leans on a
published HOT-suite figure it names the workload.

### 6.1 Where Expanse is expected to LOSE

| Pairing | Cell | Prediction | Confidence | Reasoning |
|---|---|---|---|---|
| MC1, MC2 | C1, W ≥ 2 | **Masstree wins** | High | One writer mutex bounds Expanse's aggregate insert rate at its single-writer rate and #692 measured it falling below that as writers were added; Masstree's per-node locking admits concurrent writers. **(informed)** — the gate's 1M build ran 5.4× faster at W = 8 and 9.0× at W = 16 than at W = 1 on a loaded host, which says Masstree scales, not by how much. |
| MC1, MC2 | C1, W = 1 | **Masstree wins or `BOUNDARY_RESULT`** | Medium-low | Registered as *Expanse does not win the single-writer cell*, the opposite of the ROWEX arm's registration. **(informed)** — the gate's single-threaded cold build of 1M random keys took 147 ms under load, a rate in the same band as the `SyncExpanseMap` single writer measured in `hot_comparison` §7.1 *(workload `hot_rowex_map_64bit`; different workload — prefill insert, not a cold build — and a loaded diagnostic against a quiet measurement: not comparable, disclosed as the reason for the direction only)*. An Expanse win here is a `REFUTED` in Expanse's favour and is reported as such. |
| MC1, MC2 | C2, W ≥ 1, R = 8 — reader throughput | **Masstree wins** | Medium-high | Masstree readers validate a node version and retry at that node; Expanse readers restart from the root when the tree version moves and fall back to the writer mutex after 64 restarts — the mechanism #692 measured taking eight readers to 0.10× of their reader-only rate under one writer *(workload `hot_rowex_set_63bit`)*. |
| M1 | Ordered scan, k = 10 and k = 100, every distribution | **Masstree wins** | Medium-high | Masstree leaves are linked; a scan descends once and walks. Expanse pays a descent and cursor setup per scan that short scans cannot amortize — the loss the ART and HOT suites both recorded (`hot_comparison` README §3, workload `hot_map_64bit`). k = 1000 is not predicted. |
| M2 | Ordered scan, every k, every representable shape | **Masstree wins** | High | API-surface shape: `ExpanseStrMap` re-descends from the root and allocates a key per visited element; Masstree's scan is one descent and a leaf walk. The largest loss in the HOT string arms (`hot_comparison` §6.1, workload `hot_str_ptr`) and nothing about Masstree removes it. |
| M2 | Point lookup (hit and miss) and insert, `prefixed` | **Masstree wins** | Low | The issue's stated expectation, registered as such. The mechanism reading does not strongly support it: both structures descend twelve 8-byte slices through the 96 shared bytes (`layers_for_shared_prefix(96)`); Masstree's are one-key 320-byte twig leaves, Expanse's are its chunk-level nodes, and which dependent hop chain is cheaper is unmeasured. `BOUNDARY_RESULT` is a live outcome. |
| M2 | Memory (index), `short` and `skewed` | **Masstree wins** | Medium | **(informed)** `ExpanseStrMap` holds a 13-byte key in about 69 B and a `skewed` key in about 48 B (`hot_comparison` §6.1, workload `hot_str_ptr`): two allocations per key not resolved in a terminal chunk. Masstree holds the first 8 bytes in a 320-byte leaf at ~70% fill and the rest in a shared per-leaf suffix bag; `leaf_only_bpk(0.7)` is 30.5 B/key before the bag. |

### 6.2 Where Expanse is expected to WIN

| Pairing | Cell | Prediction | Confidence | Reasoning |
|---|---|---|---|---|
| M1, MC1-M | Memory, `random`, λ ∈ [8, 23] | **Expanse wins** | High | **(informed)** `structural_bytes` at the gate's node counts is 33.13 B/key, and `projected_random_bpk_at_fill` at the measured fill reproduces it; `ExpanseMap` spans 16.26–24.71 B/key across the whole λ sweep (`hot_comparison` §1, workload `hot_map_64bit`). Masstree's per-key cost is a leaf slot regardless of key density, so it is expected flat across λ as HOT was. |
| M1, MC1-M | Memory, `random`, λ < 8 and λ ≥ 30 | **Expanse wins** | Medium | The same 33 B/key against the top of the `ExpanseMap` band; at λ ≈ 46 the set arm reached 21.93 B/key and the map sits a value word above it, so `BOUNDARY_RESULT`-by-magnitude (a few B/key) is possible even without an interval. |
| M1 | Memory, `sequential` and `clustered`, N = 10⁶ | **Expanse wins, large margin** | High | Bitmap leaves hold 256 keys per descriptor (`ExpanseSet` 0.07 B/key sequential, `hot_comparison` §5.2 reasoning); Masstree allocates a leaf slot per key regardless. Low-information as a contest; registered so a win is a confirmation. |
| M1 | Memory, `sparse`, N = 10⁶ | **Expanse wins** | Medium | One 16-byte edge plus a value word per isolated key against a 320-byte leaf holding at most 15. |
| M1 | Point lookup, `sequential` and `sparse` | **Expanse wins** | Medium-high | Expanse's strongest lookup regimes (17.79 and 14.81 ns at 1M, `hot_comparison` §5.1 reasoning, workload `art_lookup_hit`), where the trie skips empty expanses; Masstree descends a B+-tree of the same height whatever the key distribution. |
| M1 | Point lookup, `random` | **Expanse wins** | Medium | At N = 10⁶ Masstree's tree is 94k leaves under ~9k internodes: a five-level descent ending in a 30 MB leaf set that does not fit the 30 MiB L3, every hop a dependent load. Expanse's random 1M lookup is 43.32 ns *(workload `art_lookup_hit`)*. Hypothesis about depth, unmeasured. |
| M1 | Insert, every distribution | **Expanse wins** | Medium | Expanse won insertion against HOT everywhere, 2.03×–12.26× (`hot_comparison` README §2, workload `hot_map_64bit`). Masstree's insert takes a node lock, shifts a permutation and splits on overflow; registered as a medium win rather than carried over at HOT's margin. |
| M2 | Point lookup and insert, `counter`, N = 10⁶ | **Expanse wins** | High | The string analogue of `sequential`: the terminal chunk lands in a dense digit expanse. Registered **at N = 10⁶ only** — the HOT arm registered this without a population and recorded an `UNPREDICTED LOSS` at 10k and 100k (`hot_comparison` §6.1). Below 10⁶ this cell is `not pre-registered`. |
| M2 | Point lookup (hit), `short` | **Expanse wins** | Low-medium | One chunk descent plus one suffix compare against a B+-tree descent plus a suffix-bag compare; the HOT arm's analogous registration was confirmed, at a different competitor. |
| M2 | Memory (index), `counter` and `prefixed` | **Expanse wins** | Medium | **(informed)** the gate's `prefixed` table held 84.0 B/key structurally (twelve twig layers of one-key leaves are paid once, but every key's 24 discriminating bytes sit in a suffix bag) against `ExpanseStrMap`'s 71.83 B/key on the same shape (`hot_comparison` §6.1, workload `hot_str_ptr`); `counter` keys resolve in a dense digit expanse on the Expanse side (20.53 B/key there) against a leaf slot plus a 4-byte suffix on Masstree's. |
| MC1, MC2 | C2, W = 0, R = 8 — reader-only | **Expanse wins** | Medium | The single-threaded lookup predictions above carried into the concurrent wrappers, plus a `MemoryGuard`-free pin per lookup on the Expanse side against an `unlocked_tcursor` per lookup on Masstree's. |

### 6.3 Protocol health (H) — a replication, with the same falsifier

The H cells measure the Expanse side alone (Masstree has no counterpart
counter) on the same workload construction as `hot_comparison` §7.3. They are
registered as a **replication**: restart share expected to rise with W, and
fallback share expected to stay **below 1%** at every W ≤ 8. A fallback share
of 1% or more is reader starvation and is reported as a protocol-health
finding whatever the throughput cells say. `sample_spins` is reported without a
prediction.

### 6.4 Explicitly not predicted

The 50% miss pillar on `clustered`, `random`, `short` and `skewed`; point
lookup and insert on `clustered`; every `skewed` latency cell (a random-content
key of any length is one chunk descent for Expanse and one B+-tree descent
plus a suffix compare for Masstree — no mechanism argument separates them);
insert on `short` and `skewed`; scan at k = 1000 on M1; the W = 16 cells
(SMT sharing); `counter` below N = 10⁶. Reported with their numbers as
`not pre-registered`.

---

## 7. Gate Taxonomy

The labels of `hot_comparison/METHODOLOGY.md` §6 apply unchanged and every
published cell carries exactly one: `CONFIRMED`, `REFUTED`, `BOUNDARY_RESULT`,
`PASS_categorical_by_design`, `not pre-registered`, `UNPREDICTED LOSS`. Two
labels are added for this arm:

| Label | Condition |
|---|---|
| `NOT_REPRESENTABLE_MASSTREE` | The cell's population contains keys the §3.4 predicate rejects; the Expanse figure is published alone with the count. |
| `QUANTUM_DOMINATED` | A Masstree allocator-column cell where `census_quantum_dominated` is true; published with its flag, never read as a per-key index cost (§3.3). |

Wall-clock cells pass on the **BCa 95% bootstrap CI lower bound**, ≥ 2,000
resamples, never a point estimate (§8.4). Memory and event ratios are
deterministic and carry no interval.

---

## 8. Claims Ceiling

What this suite may claim when it lands, stated before the numbers exist:

1. **One Masstree implementation at one commit**, `kohler/masstree-beta`
   `1119842`, built as §3.5 documents, on **glibc 2.35 `malloc` with
   superpages on**. No claim about Masstree under jemalloc, tcmalloc, mimalloc
   or Flow, and none about the figures in the EuroSys paper, which were
   measured on different hardware with a different harness and allocator.
2. **x86-64 with AVX2 and BMI2.** Masstree itself does not need them; the
   ISA target is bound for symmetry with the Expanse arm and with the HOT
   suite, and no aarch64 cell exists.
3. **Integer keys over the full 64-bit domain; string keys of at most 255
   bytes.** Cells beyond the predicate publish Expanse alone (§3.4).
4. **Insert and point-lookup concurrency only, up to 16 threads on 8 physical
   P-cores with SMT.** No deletion under concurrency is measured — a scope
   choice for cell-by-cell symmetry with #692, not a Masstree limitation
   (Masstree supports removal, HOT-ROWEX does not) — no contended-key claim,
   no scan-under-concurrency claim, nothing about larger machines.
5. **The health ratios describe Expanse's protocol** and are not a
   comparison.
6. **No cross-suite ratio** (`hot_comparison` §7 item 5): a Masstree ÷ Expanse
   ratio is never placed beside a HOT ÷ Expanse or ROWEX ratio as though the
   competitors were commensurable. The shared Expanse column is the only
   bridge, and only where the workload construction is identical — which the
   concurrent cells and the `random` memory cells deliberately are, so the
   Expanse-side numbers of this arm are also a **replication** of #692's and
   #660's Expanse columns and are reported as such.
7. **No peer review.** This is internal work; every verdict carries that
   qualifier.

---

## 9. What Would Void a Cell

Recorded so the harness cannot quietly fail into a flattering result:

- Either arm's population after a build differs from the intended population,
  Masstree's counted by walking, never inferred from insert return values.
- Any lookup round where the two arms' folded sinks differ, or any concurrent
  round where a reader observed a hit with a wrong value.
- A census control that does not move the counter by its known size or return
  to zero; a Masstree allocator figure below its structural figure; a cell
  that did not create its table and thread slot inside the armed window; a
  census process in which any Masstree table was destroyed or any slot reused.
- W + R > 16, or a recorded `Cpus_allowed_list` that is not the pin.
- Any throughput figure from a binary built with `occ-stats`, or any health
  ratio quoted as a timing.
- Any cell whose two arms were built for different ISA targets (§3.5).
- Any string cell where a key the §3.4 predicate rejects reached the Masstree
  side rather than withholding its column.

---

## 10. Amendments after Pre-Registration

Recorded as amendments rather than edited into §3–§9, so the locked text stays
readable as what was locked (§8.7). Both entries below are measurement
constraints found while building the harness and running its validation gate;
neither is a result, and no suite cell had been run when they were written.

### 10.1 Page-aligned allocations are counted at their requested size

The census of §3.3 counts `malloc_usable_size` on every allocation, as the
HOT suite's does. The validation gate's first census pass counted **two** 2 MiB
Masstree slabs as 8.4 MB. A probe on the reference host explains it: for a
2 MiB request at 2 MiB alignment glibc's `_int_memalign` over-allocates by the
alignment, takes an mmapped chunk, and reports everything beyond the aligned
pointer as usable — eight consecutive requests read back 4,194,304 down to
4,165,632 bytes, and the Step 0 gate had read 2,097,160 for the identical
request because the kernel happened to place that mapping on a 2 MiB
boundary. The reported figure depends on where the mapping landed, and the
padding is address space the program never touches *(measured: reference
host, glibc 2.35, `masstree_validate` "glibc memalign probe" line and
`crates/expanse-hot-bench/cpp/hot_shim.cpp`)*.

**Amended:** requests at **page alignment or above** are recorded in a side
table at their **requested size** and subtracted at that size when freed.
Requests below page alignment — every HOT node (8- or 64-byte aligned), every
Expanse node, every `malloc` — keep the `malloc_usable_size` path unchanged,
so no figure the HOT suite published moves: `hot_memory_curve map 65536`
reproduces its committed `baseline_memory_curve.json` cell to the byte before
and after the change (35.8815 B/key HOT, 23.8768 B/key Expanse, 68,695 and
6,550 allocations; `hot_map_64bit`). The validation gate measures the probe
on every run and prints the padding it would have counted.

### 10.2 Insertion order moves Masstree's footprint 1.45×, and is a registered dimension

**This is a defect in the pre-registration, not a measurement constraint:**
the Step 0 gate measured one regime and §5 locked the generators of another.
The shared generators sort the population after drawing it, and every suite in
this repository builds in that order. Expanse partitions by key expanse and its
own node census (`mem_used`) does not depend on the order keys arrive — the
allocator figure and its insertion cost do, as the sensitivity set later
measured (README §6). A B+-tree's leaf fill does too: sorted
insertion fills every leaf, random insertion leaves them at the random-fill
occupancy. The Step 0 gate inserted **shuffled** and measured 94,381 leaves at
70.6% fill for 1M uniform random keys; the harness, inserting **sorted**,
measured 66,667 leaves at 100% fill on the same keys — 33.13 against 22.76
B/key structural *(measured: reference host, `step0_masstree_gate` and a
development run of `masstree_memory map random 1000000`; neither a suite cell;
`structural_bytes`, `leaf_fill`)*. The §6 memory predictions were **informed by
the shuffled figure**, and the sorted-order floor —
`projected_random_bpk_at_fill(1.0, ·)` ≈ 22.8 B/key — sits at the low edge of
the published `ExpanseMap` band, so the "Expanse wins memory at λ < 8" row of
§6.2 may land `REFUTED`. That is recorded here, before any suite cell exists,
and the row is not edited.

**Amended:** every M1 and M2 cell runs in the shared **sorted** order, exactly
as every other suite, and that is the order the §6 predictions are evaluated
on. In addition, an **insertion-order sensitivity set** re-runs both arms on a
Fisher–Yates permutation of the same population from the suite PRNG
(`workload::shuffle_in_place`) at N = 10⁶ — `random` for M1; `short` and
`prefixed` for M2; memory, 100%-hit lookup and insert — and is published as
its own table (`results/baseline_order_sensitivity.json`), never merged with
the sorted cells or given a verdict against §6. Every result row now carries an
`order` field. The concurrent cells are unaffected: their prefill is sorted and
their fresh keys arrive in generator order on both arms, as in #692.

### 10.3 The single-threaded pairings use Masstree's single-threaded configuration

`ExpanseMap` and `ExpanseStrMap` carry no concurrency protocol; the
`Sync*` wrappers do. Masstree's template carries the same split:
`nodeparams::concurrent` selects between the fenced, spin-locked
`nodeversion` and a `singlethreaded_nodeversion` whose lock, stable-read and
version-check operations compile to nothing (`nodeversion.hh`,
`masstree_struct.hh`). A development run of the harness with the concurrent
configuration on both pairings would have measured Masstree paying for a
protocol its twin does not carry — the asymmetry §8.16 forbids, and the one the
HOT suite avoided by construction because HOT ships `HOTSingleThreaded` and
`HOTRowex` as separate types.

**Amended:** pairings **M1 and M2 use `concurrent = false`**; **MC1 and MC2
use `concurrent = true`**. The shim exports both configurations of one
template (`exp_mts_*` and `exp_mt_*`), every result row carries a `table`
field, and the validation gate runs its fidelity and census checks in both.
The protocol's own single-threaded cost is published as a disclosed
sensitivity row — M1 `random` and M2 `short` at N = 10⁶, 100%-hit lookup and
insert, with the concurrent table — in the same table as §10.2's
insertion-order rows, and is never given a verdict against §6.

### 10.4 The census settles RCU-deferred frees before it reads

Masstree frees superseded structures — a suffix bag that was reallocated
larger, a node replaced by a split — through its epoch-based reclamation:
`deallocate_rcu` records the pointer in the thread's limbo list, and the memory
returns to the allocator only once the global epoch has advanced past the
recording epoch and the thread quiesces. The global epoch is the wall clock at
65 ms granularity (§3.2), so a build that finishes inside one tick has freed
nothing, and a `prefixed` build of 100,000 keys held every superseded bag —
113.7 B/key above its structural figure, in a development run of
`masstree_memory` — not because the index needs them but because reclamation
had not yet run.

**Amended:** after the build and before reading the figure, the memory pillar
advances the global epoch by one and quiesces the building slot, repeating
until the census sees no further frees (`hard_rcu_quiesce` frees at most 128
entries per call). The published allocator figure is the **settled** one;
the figure **before** settling is published beside it as
`masstree_unsettled_bytes_per_key`, with the number of frees reclaimed, so
nothing is hidden. Nodes returned by settling go to the slot's pool free list,
not to the allocator, and so stay inside the slab-quantized figure exactly as
HOT's retained free-list nodes stayed inside its (§3.2 there). The Expanse side
is untouched: `ExpanseMap` and `ExpanseStrMap` free immediately, and the
`Sync*` wrapper on the M pillar runs its own epoch collector as it always
does.

### 10.5 The string wrapper's reader does not count attempts, so MC2 has no restart share

The H cells read the engine's `occ_stats` counters (§5). `MapReader::get`
bumps `ReadOps` on every lookup and `ReadAttempts` on every walk, which is
what the restart share is computed from. `StrReader::get` (and the bytes and
blob readers) bump `ReadFallbacks` only — the string path was never
instrumented for attempts (`crates/expanse/src/sync.rs`). On the MC2 health
cells `read_ops` and `read_attempts` are therefore zero by construction, and a
restart share of 0% would be a number about the counters, not the protocol.

**Amended:** MC2's H cells publish `read_fallbacks` as an **absolute count**
and nothing else; the restart share and the fallback share are marked
`NOT_INSTRUMENTED` in every table and never rendered as 0%. §6.3's hypothesis
is evaluated on MC1 alone. Instrumenting the string reader is an engine change
outside this suite, tracked in [#721](https://github.com/orieg/expanse/issues/721).

### 10.6 Harness amendments before the pre-merge re-run

Review of the first measurement (harness commit `82966aae`) found five things
in the harness, none in the predictions. Each is an amendment to how the cells
are taken, so the suite is re-run at the amended commit and every published
number carries the new commit; nothing is patched in place (§8.10).

- **Arm order.** Every round timed Masstree first and Expanse second, so
  whatever a timed loop leaves behind — a warmed cache, a raised clock — was
  inherited by Expanse alone. The arm timed first now alternates per round,
  and each raw row records `first_arm`.
- **Scan starts.** One thousand starts at every k meant a k = 10 round
  visited 10⁴ elements and a k = 1000 round 10⁶; the smallest cells were the
  shortest timed windows in the suite. Starts are now `max(1000, 10⁶ / k)`,
  cycled from the probe stream when it is shorter, so every k visits about
  10⁶ elements per round (`workload::scan_starts`, pinned by a unit test).
- **Raw rounds.** Every cell carries `rounds_raw`, the per-round samples
  verbatim. The ratio column is `mean(Masstree rounds) / mean(Expanse
  rounds)` with a two-sample BCa interval; the per-arm columns beside it are
  medians of the same rounds, so the ratio is not the quotient of the two
  columns, and the artifact now states so in `provenance.estimators`.
- **Provenance.** The load series adds the host's busy CPU between snapshots
  from `/proc/stat` jiffies, in core-equivalents — exact where the load
  average lags — and the header records the CPU model, the frequency driver
  and governor, the transparent-huge-page mode and the P-core / E-core / SMT
  topology.
- **Census.** The allocator shim's free path skips the page-aligned side
  table when the table holds nothing, which is every free on a build that
  never page-aligns. The figures it publishes are unchanged, checked against
  the HOT arm's reference cells before the re-run.

The predictions of §6 are untouched; the verdicts are re-evaluated against
the re-run, and README §8 says where they moved.
