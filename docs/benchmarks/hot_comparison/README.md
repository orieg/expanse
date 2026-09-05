# Expanse vs. HOT (Height Optimized Trie): Empirical Benchmark Suite

Head-to-head evaluation of `ExpanseSet` and `ExpanseMap` against **HOT**
([Binna, Zangerle, Pichl, Specht & Leis, SIGMOD 2018](https://dl.acm.org/doi/10.1145/3506692)),
reached through a C++ FFI shim over the reference implementation.

> **Tracking & provenance.** Delivers the HOT arm of
> [#660](https://github.com/orieg/expanse/issues/660).
> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
> Ubuntu 22.04; HOT [`speedskater/hot`](https://github.com/speedskater/hot) `96bf6fb`,
> ISC; `docs/benchmarks/hot_comparison/run.sh`; benchmark shell pinned to CPUs 0-15;
> both arms built for one ISA target — `-C target-cpu=haswell` and
> `-march=haswell -O3 -std=c++17 -DNDEBUG`; load average 0.97 / 0.97 / 1.04 across the
> run; 15 rounds per cell, median reported, BCa 95% bootstrap ratio intervals over
> 2,000 resamples in `results/`)*.
>
> Pre-registration, locked constraints and every amendment: [`METHODOLOGY.md`](METHODOLOGY.md).
> This is internal work; no external peer review has been performed on any claim here.

---

## 1. The headline: there is no single memory answer

Per-key cost for an expanse-partitioned trie is governed by **expanse occupancy**
`λ = N / (populated 2-byte-prefix expanses)`, not by population, and it is a
sawtooth rather than a curve (`METHODOLOGY.md` §9.4). So this suite publishes
memory as a **curve across λ** and refuses to publish a single cell (§9.6).

The reason is visible in the result. **Arm A's winner changes three times.**

![Memory across expanse occupancy](results/chart_memory_curve.svg)

HOT's two lines are flat; Expanse's dip into the shaded band and climb out of it
past the cascade is the whole finding. The shaded region is derived by comparing
the two arms cell by cell, not drawn by eye.


| λ | N | HOT B/key | `ExpanseSet` B/key | winner |
|---:|---:|---:|---:|---|
| 1 | 32,768 | 12.06 | 16.18 | HOT 1.34× |
| 2 | 65,536 | 11.90 | 14.88 | HOT 1.25× |
| 4 | 131,072 | 11.82 | 12.50 | HOT 1.06× |
| 8 | 262,144 | 11.77 | **9.98** | Expanse 1.18× |
| 15 | 491,520 | 11.71 | **8.27** | Expanse 1.42× |
| 23 | 753,664 | 11.69 | **8.11** | Expanse 1.44× |
| 30 | 983,040 | 11.70 | 13.37 | HOT 1.14× |
| 38 | 1,245,184 | 11.77 | 20.70 | HOT 1.76× |
| 46 | 1,507,328 | 11.68 | 21.93 | HOT 1.88× |
| 61 | 1,998,848 | 11.70 | 20.29 | HOT 1.73× |

**Expanse wins only in the band λ ∈ [8, 23].** Outside it, HOT wins — below,
because Expanse has not yet amortized its branch structure; above, because the
`LEAF_CAP = 32` overflow cascade has fired and each key costs its own 16-byte
edge (§9.4).

A single cell would have been true and misleading in either direction: at λ=15
this suite could have published *"Expanse uses 1.42× less memory than HOT"*, and
at λ=46 *"HOT uses 1.88× less memory than Expanse"*. Both are measurements of
the same two systems on the same instrument.

**HOT is flat.** 11.68–12.06 B/key across the entire swept range — a 3% spread
against Expanse's 2.7×. Holding fanout roughly constant by varying discriminative
bits per node is exactly the property its authors claim for it, and on this
instrument it delivers.

### Arm B — the value model decides it

| λ | N | HOT B/key | `ExpanseMap` B/key | Expanse advantage |
|---:|---:|---:|---:|---:|
| 1 | 65,536 | 35.88 | 23.88 | 1.50× |
| 8 | 524,288 | 35.74 | 18.99 | 1.88× |
| 23 | 1,507,328 | 35.68 | 16.26 | **2.19×** |
| 46 | 3,014,656 | 35.67 | 24.71 | 1.44× |
| 61 | 3,997,696 | 35.69 | 23.83 | 1.50× |

Expanse wins at every occupancy, and this cell is labelled
**`PASS_categorical_by_design`** rather than a win: HOT reaches its value through
a heap-allocated `std::pair` per entry, ~24 B/key of allocator-visible overhead
before any index structure, while Expanse packs the value into a `ValueSlot`.
That is a property of the value model, not evidence of an architectural
advantage, and §5.2 pre-registered it as such.

---

## 2. Latency at N = 1,000,000

Ratios are HOT ÷ Expanse, so **above 1.000 means Expanse is faster**. Every cell
is gated on the BCa 95% interval, never the point estimate (§8.4); a cell whose
interval spans parity is `BOUNDARY_RESULT` and claims no winner.

![Latency at N=1M](results/chart_latency_1m.svg)

Both exceptions sit against the parity line: `lookup_hit · set · random` is the
`BOUNDARY_RESULT` at 0.998, and `lookup_miss · set · random` is the one non-scan
HOT win at 0.940.


### Point lookup, 100% hit

| Distribution | Arm | HOT ns | Expanse ns | Ratio | Verdict |
|---|---|---:|---:|---:|---|
| sequential | set | 18.89 | **4.54** | 4.194 | Expanse |
| clustered | set | 24.65 | **8.22** | 3.011 | Expanse |
| sparse | set | 22.74 | **10.93** | 1.917 | Expanse |
| **random** | **set** | 36.50 | 36.39 | **0.998** | **`BOUNDARY_RESULT`** |
| sequential | map | 43.43 | **15.44** | 2.815 | Expanse |
| clustered | map | 50.32 | **23.34** | 2.159 | Expanse |
| sparse | map | 44.24 | **11.22** | 3.897 | Expanse |
| random | map | 60.22 | **43.23** | 1.399 | Expanse |

### Point lookup, 50% hit / 50% rejection-sampled miss

| Distribution | Arm | HOT ns | Expanse ns | Ratio | Verdict |
|---|---|---:|---:|---:|---|
| sequential | set | 21.17 | **8.84** | 2.394 | Expanse |
| clustered | set | 24.21 | **11.43** | 2.134 | Expanse |
| sparse | set | 20.60 | **8.18** | 2.515 | Expanse |
| **random** | **set** | **36.16** | 37.89 | **0.940** | **HOT** |
| sequential | map | 53.23 | **11.72** | 4.587 | Expanse |
| clustered | map | 59.80 | **16.87** | 3.556 | Expanse |
| sparse | map | 53.51 | **9.05** | 5.915 | Expanse |
| random | map | 74.30 | **52.97** | 1.393 | Expanse |

**The pre-registered uniform-random loss is confirmed on Arm A and refuted on
Arm B.** §5.1 registered HOT winning uniform-random point lookup at medium-high
confidence, reasoning that random keys discriminate late and force a deep descent
at a fixed 8-bit span while HOT's variable bit selection bounds height. On the
set arm that is what happened — parity on hit (0.998) and a HOT win on miss
(0.940). On the map arm it did not, because HOT's pointer chase to its heap pair
costs more than the descent it saves.

### Insertion into a cold structure

| Distribution | Arm | HOT ns | Expanse ns | Ratio |
|---|---|---:|---:|---:|
| sequential | set | 59.45 | **4.84** | **12.263** |
| clustered | set | 63.52 | **11.96** | 5.346 |
| sparse | set | 59.12 | **29.08** | 2.027 |
| random | set | 77.46 | **30.55** | 2.520 |
| sequential | map | 74.80 | **12.90** | 5.773 |
| clustered | map | 78.38 | **21.33** | 3.679 |
| sparse | map | 73.97 | **30.70** | 2.393 |
| random | map | 95.74 | **26.80** | 3.553 |

Expanse wins every insertion cell, 2.03×–12.26×. §5.2 registered this as a *weak*
prediction; it landed stronger than registered.

---

## 3. Ordered scan is a systematic loss, wider than predicted

**27 of this suite's 30 HOT wins are scan cells.** §5.1 registered HOT winning
short range scans at k=10 and k=100, carried forward from the loss
`art_comparison/` found unpredicted. The measurement is broader than that on two
axes, and both are recorded as **`UNPREDICTED LOSS`**:

- **k=1000 loses too**, which was not registered — `set`/`random`/100k is
  0.504 [0.497, 0.511], and `map`/`random`/100k is 0.408 [0.404, 0.418].
- **`sparse` loses as well as `random`**, also not registered.

| Arm | Dist | N | k=10 | k=100 | k=1000 |
|---|---|---:|---:|---:|---:|
| set | random | 10,000 | 0.578 | 0.417 | 0.434 |
| set | random | 100,000 | 0.773 | 0.555 | 0.504 |
| set | random | 1,000,000 | — | 0.756 | 0.715 |
| map | random | 10,000 | 0.651 | 0.469 | 0.470 |
| map | random | 100,000 | 0.912 | 0.472 | 0.408 |
| map | random | 1,000,000 | **1.414** | **1.402** | **1.517** |

The `map`/`random`/1M row is the exception and it reverses cleanly: Expanse wins
every scan width there. Scan outcome therefore depends on population as well as
on `k`, which is a second reason this suite does not publish single-population
cells.

Scan on `sequential` and `clustered` is an Expanse win throughout and does not
appear in the loss list.

---

## 4. Scorecard

144 latency cells, 20 memory cells.

| | Count |
|---|---:|
| Expanse wins (CI excludes parity) | 109 |
| HOT wins (CI excludes parity) | 30 |
| `BOUNDARY_RESULT` (interval spans parity) | 5 |

Against the pre-registration:

| Registered | Outcome |
|---|---|
| HOT wins uniform-random point lookup (§5.1, medium-high) | **CONFIRMED** on Arm A; **REFUTED** on Arm B |
| HOT wins short range scans k=10, k=100 (§5.1, medium-high) | **CONFIRMED**, and wider — see §3 |
| HOT wins sparse-stride memory (§5.1, downgraded to low in §9.5) | **CONFIRMED** as part of the λ story: HOT wins above the cascade |
| Expanse wins Arm B memory (§5.2, high) | **CONFIRMED**, labelled `PASS_categorical_by_design` |
| Expanse wins insertion (§5.2, weak) | **CONFIRMED**, stronger than registered |
| Expanse wins sequential and sparse point lookup (§5.2, medium) | **CONFIRMED** |
| Scan losing at k=1000 and on `sparse` | **UNPREDICTED LOSS** |
| Arm A memory winner flipping three times across λ | **not pre-registered** |

---

## 5. What this suite does not claim

Stated before the numbers existed (§7) and unchanged by them:

1. **Single-threaded only.** No concurrency claim follows from this suite; HOT's
   ROWEX variant is separate scope.
2. **Integer keys.** Arm A is restricted to a 63-bit domain because HOT's inline
   value payload is 63 bits wide (§9.4), and is labelled `hot_set_63bit`
   throughout. No string-key claim.
3. **x86-64 with AVX2 and BMI2 only.** HOT does not build on aarch64.
4. **One HOT implementation at one commit** — `speedskater/hot` `96bf6fb` built as
   documented, not "HOT" as a design, and not the SIGMOD paper's figures, which
   were measured on different hardware with a different harness.
5. **No cross-suite ratio.** A HOT-vs-Expanse ratio here is never set beside an
   ART-vs-Expanse ratio from `art_comparison/` (§8.12).
6. **The memory instrument is bytes held from the C allocator**, not the engine's
   `mem_used()`, because it is the only definition both arms satisfy. The two are
   not the same quantity and the gap is not a constant factor (§9.3).

---

## 6. Reproducing

Requires an x86-64 host with AVX2 and BMI2.

```bash
git submodule update --init --depth 1 third_party/hot
docs/benchmarks/hot_comparison/run.sh            # full sweep -> results/
docs/benchmarks/hot_comparison/run.sh --quick    # reduced, -> gitignored results/quick/
```

The runner takes the host-wide benchmark lock, pins to performance cores, drives
every cell in its own process — HOT's node pool is a process-global `static` and
a warm pool undercounts by up to 3.3× (§9.2) — and snapshots load average at
start, between pillars and at the end.
