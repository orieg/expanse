# Expanse vs. HOT (Height Optimized Trie): Empirical Benchmark Suite

Head-to-head evaluation of `ExpanseSet` and `ExpanseMap` — and, for string keys,
`ExpanseStrMap` and `ExpanseBytesMap` (§6) — against **HOT**
([Binna, Zangerle, Pichl, Specht & Leis, SIGMOD 2018](https://dl.acm.org/doi/10.1145/3506692)),
reached through a C++ FFI shim over the reference implementation.

> **Tracking & provenance.** Delivers the HOT arm of
> [#660](https://github.com/orieg/expanse/issues/660); the string-key arms
> (§6) deliver [#693](https://github.com/orieg/expanse/issues/693) and carry
> their own provenance block.
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

### Per-pillar charts

Badges are driven by the cell's **verdict**, not by which bar is shorter: a cell
whose BCa interval spans parity carries a neutral `BOUNDARY` badge and claims no
winner (§8.4).

**Point lookup, 100% hit**

![Point lookup 100% hit](results/chart_lookup_hit.svg)

**Point lookup, 50% hit / 50% miss**

![Point lookup 50/50](results/chart_lookup_miss.svg)

**Insertion into a cold structure**

![Insertion](results/chart_insert.svg)

**Live heap memory, selected occupancies**

![Memory](results/chart_memory.svg)

Read that one against the curve above, not on its own — it samples five
occupancies either side of the cascade, and which side a cell lands on decides
its winner.

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

![Ordered range scan](results/chart_scan.svg)

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
2. **Integer keys in §1–§4.** Arm A is restricted to a 63-bit domain because
   HOT's inline value payload is 63 bits wide (§9.4), and is labelled
   `hot_set_63bit` throughout. String keys are §6, under their own ceiling
   (§10.9): claims attach to HOT's C-string configuration only, never to keys
   longer than 255 bytes, and an `ExpanseBytesMap` cell is a hash-indexed
   structure against a trie, not a trie comparison.
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

## 6. String keys (#693): `ExpanseStrMap` and `ExpanseBytesMap` against HOT's C-string configuration

> *(measured: reference host — Intel Core i9-12900F, 8P+8E/24 threads, 30 MiB L3,
> Ubuntu 22.04 / kernel 6.8; HOT `96bf6fb`; harness commit `d0149469`;
> `docs/benchmarks/hot_comparison/run.sh strings`; benchmark shell pinned to CPUs
> 0-15; both arms `-C target-cpu=haswell` / `-march=haswell -O3 -std=c++17
> -DNDEBUG`; load average 0.04 / 0.04 / 0.37 / 1.02 at start, after the gate,
> after the memory sweep and at the end; 15 rounds per cell, median reported,
> BCa 95% bootstrap ratio intervals over 2,000 resamples;
> `results/baseline_string_latency.json`, `results/baseline_string_memory.json`;
> gate transcript `results/string_validate.log`)*. Pre-registration:
> [`METHODOLOGY.md` §10](METHODOLOGY.md#10-string-key-arms-693-pre-registration).
> Tables in this section are the output of `scripts/string_tables.py` over those
> two files. Internal work; no external peer review.

Three pairings (§10.2): **Arm C** `hot_str_ptr` — HOT's shipped string
configuration, `HOTSingleThreaded<const char*, IdentityKeyExtractor>`, against
`ExpanseStrMap` storing the same key pointer as its value; **Arm D**
`hot_str_map` — HOT through a heap `std::pair<const char*, uint64_t>` against
`ExpanseStrMap` string → `u64`; **Arm E** `hot_bytes_ptr` — the same HOT
configuration against the **unordered, hash-indexed** `ExpanseBytesMap`, which is
not a trie comparison and has no scan pillar. Five key shapes (§10.5): `short`
(8–16 random alphanumerics), `counter` (`k` + 11 digits), `prefixed` (96 shared
bytes + 24 random), `skewed` (Pareto lengths, 4–192), `beyond` (256 shared bytes
+ 16 random, 272 bytes). The strings are the harness's, one heap allocation
each; the census counts them on neither side (§10.3).

### 6.1 The losses first

**Ordered scan is a loss in every cell, and it is the largest loss in this
suite.** All 72 scan cells with a HOT column go to HOT; HOT ÷ Expanse runs from
0.336 [0.303, 0.402] (`short`, Arm D, k=10, N=1M) down to 0.019 [0.019, 0.019]
(`prefixed`, Arm C, k=1000, N=10k). That is the pre-registered high-confidence
loss (§10.7) and it is a statement about the **shipped navigation surface**, not
the trie: `ExpanseStrMap` exposes `next_at_or_after` / `next_after`, each a
fresh root descent returning a heap-allocated key, against HOT's `lower_bound`
plus an incremental iterator (§10.6). A cursor iterator for `ExpanseStrMap` is an
engine change outside this suite and is the obvious follow-up.

| Arm | Shape | N | k=10 | k=100 | k=1000 |
|---|---|---:|---:|---:|---:|
| C · ptr | `counter` | 1,000,000 | 0.228 [0.209, 0.276] | 0.056 [0.054, 0.062] | 0.034 [0.034, 0.035] |
| C · ptr | `prefixed` | 1,000,000 | 0.171 [0.155, 0.204] | 0.049 [0.048, 0.052] | 0.023 [0.023, 0.023] |
| C · ptr | `short` | 1,000,000 | 0.249 [0.227, 0.297] | 0.077 [0.075, 0.082] | 0.041 [0.041, 0.041] |
| C · ptr | `skewed` | 1,000,000 | 0.223 [0.201, 0.267] | 0.062 [0.060, 0.069] | 0.035 [0.035, 0.035] |
| D · map | `counter` | 1,000,000 | 0.265 [0.235, 0.330] | 0.066 [0.062, 0.073] | 0.040 [0.040, 0.040] |
| D · map | `prefixed` | 1,000,000 | 0.182 [0.164, 0.222] | 0.059 [0.057, 0.062] | 0.026 [0.026, 0.026] |
| D · map | `short` | 1,000,000 | 0.336 [0.303, 0.402] | 0.110 [0.107, 0.117] | 0.053 [0.053, 0.053] |
| D · map | `skewed` | 1,000,000 | 0.275 [0.246, 0.336] | 0.086 [0.083, 0.093] | 0.045 [0.045, 0.045] |
| C, D | `beyond` | any | Expanse 249–289 ns/element; HOT column withheld (§10.4) | | |

The 10k and 100k rows are in `results/baseline_string_latency.json`; none is
above 0.25.

**Memory ownership on short and skewed keys goes to HOT, and the
pre-registration had it the other way.** §10.7 registered, at low-medium
confidence, that Expanse would win the `ownership` column on `short` and
`counter`. On `counter` it does; on `short` it does not, by a wide margin, and
`skewed` goes to HOT as well *(workload: `hot_str_ptr`)*:

| Shape (Arm C, N = 1M) | external (exact) | HOT index | Expanse index | **HOT ownership** | **Expanse ownership** | Expanse `mem_used` |
|---|---:|---:|---:|---:|---:|---:|
| `short` | 24.00 (13.00) | 12.23 | 69.16 | **36.23** | 69.16 | 50.77 |
| `skewed` | 29.76 (15.27) | 12.22 | 47.65 | **41.98** | 47.65 | 41.19 |
| `counter` | 24.00 (13.00) | 11.42 | 20.53 | 35.42 | **20.53** | 19.56 |
| `prefixed` | 136.00 (121.00) | 12.23 | 71.83 | 148.23 | **71.83** | 62.77 |
| `beyond` | 280.00 (273.00) | withheld | 71.84 | withheld | 71.84 | 54.78 |

`ExpanseStrMap` holds a 13-byte key in about 69 bytes: the gate's allocation
counts say why — 204,791 allocations for 100,000 `short` keys against HOT's
4,566 (`results/string_validate.log`). Every key that is not resolved inside a
terminal 8-byte chunk costs a `StrSuffix` shell plus a separate byte buffer, two
allocations, and the allocator's rounding on each; the engine's own `mem_used`
(50.77 B/key) undercounts that rounding by a further 18 B/key (§9.3 reason 1,
now on the string path). This is **`REFUTED`** against §10.7 and it is a finding
about `ExpanseStrMap`'s leaf representation, not about HOT.

**The `index` column goes to HOT on every shape, categorically.** HOT holds
11.4–12.2 B/key of index on every representable shape at N = 1M because it
stores an 8-byte pointer plus node bits and nothing else; Expanse holds the key
bytes. Registered in §10.3 as **`PASS_categorical_by_design` in HOT's favour**
and labelled so: the contest is the `ownership` column above, where HOT's
external string table is added back.

**`prefixed` point lookup on Arm C is HOT's, as registered.** 100% hit
0.764 [0.763, 0.764]; 50/50 0.900 [0.898, 0.902] (N = 1M). This is the regime
HOT is designed for — 96 shared bytes that its discriminative-bit selection
skips and `ExpanseStrMap` descends one chunk at a time — and it is the one
place in the string suite where the pre-registered loss on HOT's home ground
landed as predicted. At N = 100,000 the 50/50 cell is a `BOUNDARY_RESULT`
(1.015 [0.987, 1.023]).

**`ExpanseBytesMap` (Arm E) loses insertion everywhere and most 100%-hit
lookups, and its index is the heaviest thing measured here.** Insert:
0.518 [0.517, 0.521] on `counter`, 0.648 [0.646, 0.651] on `short`,
0.665 [0.663, 0.667] on `skewed`, 0.730 [0.727, 0.733] on `prefixed` (N = 1M).
100% hit: HOT on `prefixed` 0.932 [0.931, 0.932], `short` 0.978 [0.975, 0.980],
`skewed` 0.991 [0.990, 0.992]; Expanse only on `counter` 1.048 [1.047, 1.050].
Its index costs 96.6–102.1 B/key on 12–15-byte keys and 192.7 B/key on
`prefixed` — a hash-trie entry, a boxed collision bucket, the bucket's vector,
and a boxed copy of the key. None of this was pre-registered (§10.7 declined to
predict Arm E); it is reported as `not pre-registered` and it is the largest
per-entry footprint in either HOT suite. Arm E does win every 50/50 cell at
N = 1M (1.103–1.188).

**`UNPREDICTED LOSS`: `counter` 100%-hit lookup on Arm C at small N.** §10.7
registered `counter` lookup as a high-confidence Expanse win. It is one at
N = 1M (1.065 [1.062, 1.067]) and a HOT win at N = 10,000
(0.843 [0.834, 0.862]) and N = 100,000 (0.884 [0.861, 0.893]). The
prediction was stated without a population and was wrong below a million keys.

### 6.2 Where the pre-registration was refuted in Expanse's favour

These are reported with the same prominence as the losses (§6 taxonomy). At
N = 1M, HOT ÷ Expanse:

| Cell | Registered (§10.7) | Measured | Label |
|---|---|---:|---|
| `skewed` 100% hit, Arm C | HOT wins (low) | 1.174 [1.172, 1.178] | **`REFUTED`** |
| `skewed` 100% hit, Arm D | HOT wins (low) | 1.737 [1.734, 1.739] | **`REFUTED`** |
| `skewed` 50/50, Arm C | HOT wins (low) | 1.375 [1.372, 1.377] | **`REFUTED`** |
| `skewed` 50/50, Arm D | HOT wins (low) | 1.972 [1.968, 1.975] | **`REFUTED`** |
| `prefixed` 100% hit, Arm D | HOT wins (medium-high) | 1.088 [1.087, 1.089] | **`REFUTED`** |
| `prefixed` 50/50, Arm D | HOT wins (medium-high) | 1.254 [1.253, 1.255] | **`REFUTED`** |
| `prefixed` insert, Arm C | HOT wins (medium) | 1.199 [1.196, 1.203] | **`REFUTED`** |
| `prefixed` insert, Arm D | HOT wins (medium) | 1.285 [1.281, 1.288] | **`REFUTED`** |

The `skewed` row is the one §10.7 flagged as "registered because the issue
expects it; the mechanism reading does not support it". The mechanism reading
was right and the registration was wrong, in every population and on both
`ExpanseStrMap` arms. On `prefixed`, the loss that held on Arm C reverses on
Arm D exactly as the integer arms' uniform-random loss did (§2): HOT's
per-entry heap pair costs more than the descent it saves.

### 6.3 Confirmed wins, and the rest

`counter` at N = 1M is Expanse's on both `ExpanseStrMap` arms: 100% hit
1.065 [1.062, 1.067] (C) and 1.568 [1.560, 1.571] (D); insert
1.456 [1.451, 1.460] (C) and 1.706 [1.700, 1.710] (D) — **`CONFIRMED`**, with
the small-N caveat of §6.1. `short` 100% hit on Arm C is 1.213 [1.207, 1.215],
**`CONFIRMED`**. Arm D's memory is `PASS_categorical_by_design` for Expanse on
every representable shape (HOT ownership 59.4–172.2 B/key against Expanse
20.5–71.8), as §10.7 registered. The cells §10.7 declined to predict — 50/50
lookups on `short` and `skewed`, insertion on `short` and `skewed`, and all of
Arm E — are `not pre-registered`; at N = 1M the `ExpanseStrMap` ones are
Expanse's (1.21–2.81), the Arm E ones split as described above.

![String keys, latency at N = 1M](results/chart_string_latency.svg)

### 6.4 Memory across the population sweep

![String keys, Arm C ownership across N](results/chart_string_memory_sweep.svg)

| N | `counter` HOT / Expanse | `short` HOT / Expanse | `skewed` HOT / Expanse | `prefixed` HOT / Expanse | `beyond` — / Expanse |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 44.7 / 91.1 | 44.8 / 106.9 | 51.2 / 84.8 | 157.5 / 109.5 | — / 109.8 |
| 10,000 | 36.6 / 27.5 | 37.0 / 75.6 | 42.7 / 53.9 | 149.1 / 78.7 | — / 78.4 |
| 100,000 | 35.5 / 21.1 | 36.1 / 64.0 | 41.9 / 42.5 | 148.1 / 66.6 | — / 66.6 |
| 125,000 | 35.5 / 21.0 | 36.4 / 68.0 | 42.2 / 46.6 | 148.4 / 70.8 | — / 70.7 |
| 150,000 | 35.5 / 20.9 | 36.7 / 71.2 | 42.5 / 49.7 | 148.7 / 73.9 | — / 73.8 |
| 200,000 | 35.5 / 20.8 | 36.5 / 71.8 | 42.3 / 50.3 | 148.5 / 74.5 | — / 74.5 |
| 1,000,000 | 35.4 / 20.5 | 36.2 / 69.2 | 42.0 / 47.7 | 148.2 / 71.8 | — / 71.8 |

Arm C ownership, B/key; the 2k, 5k, 20k and 500k rows are in
`results/baseline_string_memory.json`. **HOT is flat again** — 35.4–36.7 B/key
on every 12–15-byte shape from 10k to 1M, the string analogue of the integer
arms' 11.68–12.06 — and `counter` is the only shape where Expanse's line drops
under it.

**The registered chunk-occupancy hypothesis is consistent with the sweep and
not confirmed by it.** §10.5 predicted a `LEAF_CAP` cascade in the
discriminating chunk map near N ≈ 1.23 × 10⁵ for the random-alphanumeric
shapes and none for `counter`. Between 100k and 150k the Expanse line rises
on exactly those shapes — `short` 64.0 → 68.0 → 71.2, `prefixed`
66.6 → 70.8 → 73.9, `skewed` 42.5 → 46.6 → 49.7 — and `counter` does not
move (21.1 → 21.0 → 20.9). The step is about +11%, not the 2.7× swing of the
integer arm, because stored key bytes dominate the per-key cost. The
single-variable test that would confirm the mechanism — changing the alphabet
width and watching the step move — was not run, so the rows stay on a
population axis and the re-expression against `λ_chunk` §10.5 conditionally
promised is **not** made. Hypothesis, partially supported.

### 6.5 The 255-byte window: a capability finding about HOT

HOT discriminates C-string keys on their first 255 bytes (`MAX_STRING_KEY_LENGTH`,
measured at the boundary: 254- and 255-byte keys differing in their last byte
are two entries, 256- and 300-byte keys are one; §10.10). On `beyond`, whose
272-byte keys share a 256-byte prefix, HOT's `insert` reported 1 of 1,000 keys
new, the trie walked 1, and `lookup` found 1 — no false positive, because HOT
confirms every leaf with a full `strcmp`, but **silent population loss under a
build that reports success**, the same class as the integer arms' §3.1. Every
`beyond` cell therefore publishes the Expanse figure alone with the HOT column
withheld (45 latency cells, 36 memory cells), never a HOT number over a smaller
population. The Expanse side is unrestricted: `ExpanseStrMap` holds all 10⁶
272-byte keys at 71.84 B/key and `ExpanseBytesMap` at 352.29 B/key *(workload:
`hot_string_memory`)*; their 100%-hit lookups at N = 1M take 356.05 ns and
327.72 ns respectively *(workload: `hot_string_latency`)*.

### 6.6 Scorecard

225 latency cells, 180 memory cells. Latency cells with a HOT column: 180.

| | Count |
|---|---:|
| HOT wins (CI excludes parity) | 97 — of which 72 are scan cells |
| Expanse wins (CI excludes parity) | 78 — none is a scan cell |
| `BOUNDARY_RESULT` | 5 |
| HOT column withheld (`beyond`, §10.4) | 45 |

Against §10.7:

| Registered | Outcome |
|---|---|
| HOT wins ordered scan, every k, every shape (high) | **CONFIRMED**, 72 of 72 |
| HOT wins `prefixed` point lookup (medium-high) | **CONFIRMED** on Arm C; **REFUTED** on Arm D |
| HOT wins `prefixed` insert (medium) | **REFUTED** on both arms |
| HOT wins the `index` memory column on long and skewed keys (high, categorical) | **CONFIRMED**, `PASS_categorical_by_design` in HOT's favour, on every shape |
| HOT wins `skewed` point lookup (low) | **REFUTED** on both arms, at every population |
| Expanse wins `counter` lookup and insert (high) | **CONFIRMED** at N = 1M; **UNPREDICTED LOSS** on 100%-hit lookup at 10k and 100k |
| Expanse wins `short` 100%-hit lookup, Arm C (medium) | **CONFIRMED** |
| Expanse wins `ownership` memory on `counter` and `short` (low-medium) | **CONFIRMED** on `counter`; **REFUTED** on `short` |
| Expanse wins Arm D memory (high, categorical) | **CONFIRMED**, `PASS_categorical_by_design` |
| `λ_chunk` cascade near N ≈ 1.23 × 10⁵ (hypothesis) | consistent, not confirmed — §6.4 |

---

## 7. Reproducing

Requires an x86-64 host with AVX2 and BMI2.

```bash
git submodule update --init --depth 1 third_party/hot
docs/benchmarks/hot_comparison/run.sh                    # integer arms, full sweep -> results/
docs/benchmarks/hot_comparison/run.sh --quick            # reduced, -> gitignored results/quick/
docs/benchmarks/hot_comparison/run.sh strings            # string arms (#693) -> results/baseline_string_*.json
docs/benchmarks/hot_comparison/run.sh strings --quick    # reduced, -> gitignored results/quick/
python3 docs/benchmarks/hot_comparison/scripts/generate_charts.py   # every chart, from results/
python3 docs/benchmarks/hot_comparison/scripts/string_tables.py     # the §6 tables, from results/
docs/benchmarks/hot_comparison/run.sh            # full sweep -> results/
docs/benchmarks/hot_comparison/run.sh --quick    # reduced, -> gitignored results/quick/

# The concurrent arm (#692, METHODOLOGY.md §10) additionally needs HOT's
# nested TBB submodule; libtbb is built from it into the cargo build dir.
git -C third_party/hot submodule update --init --depth 1 third-party/tbb
docs/benchmarks/hot_comparison/run.sh --only-concurrent          # -> results/baseline_concurrent.json
docs/benchmarks/hot_comparison/run.sh --only-concurrent --quick  # -> results/quick/
```

The runner takes the host-wide benchmark lock, pins to performance cores, drives
every cell in its own process — HOT's node pool is a process-global `static` and
a warm pool undercounts by up to 3.3× (§9.2) — and snapshots load average at
start, between pillars and at the end. The string runner additionally executes
`hot_string_validate` first and records nothing if it fails (§10.8).
