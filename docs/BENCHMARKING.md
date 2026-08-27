# Benchmarking Guidelines

> Canonical benchmarking doc. Design targets: [ARCHITECTURE.md](ARCHITECTURE.md) §6 · Testing: [TESTING.md](TESTING.md) · CI: [CI.md](CI.md)

Performance claims are this project's reason to exist, so they follow the strictest discipline in the repo: **no claim without a measurement, no measurement without methodology.**

## Harness and metrics

- Criterion benches live in `crates/expanse/benches/`; C-comparison benches drive both sides through the C surface once `expanse-capi` exports it.
- Key distributions: the same matrix as [TESTING.md](TESTING.md) (sequential, random, clustered, sparse/pathological, boundary) — performance work that only measures random keys is rejected.

| Metric | Definition |
|---|---|
| Lookup latency | ns/op, hit and miss measured separately, per distribution × population size |
| Insert/delete throughput | Mops/s, cold-build and steady-state (interleaved ins/del) measured separately |
| Memory footprint | bytes/key at population checkpoints, via allocator instrumentation (counting `GlobalAlloc` wrapper), cross-checked against `MemUsed` |
| Scan throughput | keys/s for full-order iteration |

## Comparison targets

1. **C libjudy** — the headline comparison ("faster than the original, or explain why").
2. `std::collections::BTreeMap` / `HashMap` — the "why not just use std" baseline ([#122](https://github.com/orieg/expanse/issues/122)).
3. **Adaptive Radix Tree (ART)** (`art-tree` / `art-rs`) — modern trie baseline ([#122](https://github.com/orieg/expanse/issues/122)).
4. **Roaring Bitmaps** (`roaring` / `croaring`) — integer set and posting list baseline ([#122](https://github.com/orieg/expanse/issues/122)).
5. **Swiss Tables** (`hashbrown::HashMap`) — flat SIMD hash map baseline ([#122](https://github.com/orieg/expanse/issues/122)).
6. **Concurrent Maps** (`crossbeam-skiplist`, `dashmap`, `parking_lot::RwLock<BTreeMap>`) — multithreaded scalability baseline ([#123](https://github.com/orieg/expanse/issues/123)).

## Methodology rules (binding)

0. **State the measured region, and verify it with an instrument.** Every
   benchmark's doc comment says exactly what is inside the timed window and
   what is in `setup`/`teardown`. Every other rule below governs how a number
   is interpreted; this one governs whether it measures what its name says.

   Four violations have been found and fixed across both harness files: the
   tree build inside the vs-stock lookup arms; the teardown inside every
   vs-stock arm; key generation inside the insert arms; and the same teardown
   bug again in `benches/instructions.rs`, the file that produces the vs-base
   column on every PR, where the arms take the container by value so `Drop`
   ran inside the timed window.

   Two lessons:

   - **Symmetric contamination is not harmless contamination.** Key generation
     was charged to both libraries equally, which is why it survived review. It
     still corrupts the comparison, by pulling every ratio toward 1.00.
   - **All four were found by an instrument, never by a reviewer.** Treat "a
     reviewer checked it" as the weakest evidence available and get
     per-function output instead: `free_subtree` inside a benchmark named
     `map_get` is unmissable, while the same fact stated in prose was missed
     four times.

   The same discipline applies to **provenance**: a mislabelled commit misleads
   exactly as much as a mismeasured region. Both failure modes, and the figures
   they cost, are listed in [Corrections record](#corrections-record).

0b. **Both arms must be the same shape.** A comparison is only valid between
   binaries built and reached the same way. An LTO'd rlib called directly
   against a PIC shared object reached through `dlopen` grants cross-object
   inlining and direct calls that stock structurally cannot have. Every
   vs-stock arm now has an `*_expanse_dl` twin that loads `libexpanse.so`
   exactly as stock is loaded; the PR comment reports the `.so` ratio as the
   headline and the rlib−`.so` difference as an explicit correction factor.

0c. **The estimated-cycles column is a regression alarm, never an
   adjudicator across inlining changes.** Callgrind's model
   (`cycles = L1hits + 5·LLhits + 35·RAMhits`) charges every instruction
   fetch one cycle with zero overlap. It over-punishes outlining (fetch
   *volume* rises even when misses are flat) and cannot see latency wins
   at all (replacing a 12-instruction serially dependent SWAR chain with
   one 3-cycle `popcnt` barely moves it). PR #19 measured this concretely:
   +9% "cycles" on arms whose misses were flat, from outlining alone. Use
   est-cycles to compare same-shape code; the moment a change moves code
   across an inlining boundary, decide on instruction counts plus real
   hardware counters, not on this model.

1. **Interleaved A/B arms.** Any A-vs-B comparison (regression check, libjudy comparison, before/after a change) alternates arms per benchmark group over several rounds — never suite-A-then-suite-B. Runner/thermal drift then hits both arms and cancels in the paired ratio. (Learned the hard way in php-judy — back-to-back suites reported false regressions; see php-judy issue #87 and its `bench-compare` harness.)
2. **System-load hygiene.** Before the first run and between comparison runs, snapshot load (`ps -A -o %cpu,%mem,command | sort -rn | head`; load average vs core count). A non-target process above ~100% CPU, or a load-average shift > 2 between arms, contaminates the run: discard it, don't reinterpret it. Laptops running concurrent sessions are shared infrastructure.
3. **CI benches detect changes, not truths.** CI runners produce paired ratios good for regression alarms. Publishable absolute numbers (README/claims) come from a dedicated quiet host with the hardware named.
4. **Tag every number.** Performance figures in docs are tagged `(measured: <machine, commit>)` or `(target)`. Untagged numbers are lint errors in review. The two architecture KPIs (<15 ns random point lookup; <9.5 B/key dense) are `(target)` until measured.
5. **Report distributions, not just means.** Criterion medians + CIs; a claim that A beats B requires the CI bounds to separate, not the point estimates.
6. **Fixed seeds, recorded populations.** Every bench names its RNG seed and population sizes so runs are reproducible bit-for-bit.
7. **No time estimates, no projections-as-results** — per repo-wide discipline (CLAUDE.md).

8. **One suite per machine at a time — enforced by a host-wide lock, not by
   coordination.** Every `docs/benchmarks/*/run.sh` takes an atomic `mkdir`
   lock at `${EXPANSE_BENCH_LOCK:-${TMPDIR:-/tmp}/expanse-bench.lock}`
   (shared across all checkouts on the host), records `suite`/`pid`/`start`
   in `owner`, refuses to start (exit 75) while another run holds it, and
   removes it on exit. Scheduling by coordination is not enough: two runs
   arranged that way nearly overlapped twice in one morning. Ad-hoc `cargo bench`
   invocations outside `run.sh` must check the lock manually (`cat
   "$TMPDIR/expanse-bench.lock/owner"`) and are otherwise subject to rule 2's
   load-snapshot requirement.

9. **Fail-loud harness execution (zero silent fallbacks).** If a benchmark
   dependency, runtime, or native extension (`libexpanse.so`, Node addon, Python
   wheel) is missing or fails to build, the benchmark harness MUST exit non-zero
   immediately (`panic!`, `sys.exit(1)`, `b.Fatalf`). Never catch a compilation
   or runtime failure to substitute placeholder numbers or fake loops. In CI
   workflows where a failure is intentionally non-fatal (e.g. historical
   unbuildable base commit), the degradation must surface prominently (`⚠️ NO
   BASELINE — regression gate did not run`, matching `perf_report.py`) and
   must never render as success.
10. **Dynamic report derivation (zero hardcoded findings).** Reporting scripts
    (`bench_report.py`, `perf_report.py`, `generate_charts.py`) must never
    stamp hardcoded summary constants (e.g. `"4× to 10× faster"`). All published
    tables, speedup ratios, badges, and findings statements must be computed
    dynamically from the parsed results of that specific run.
11. **Symmetric substitution-twin discipline.** Baselines must represent
    production-grade configurations (e.g. InlineSkipList-equivalent
    variable-height node allocation — not a strawman embedding all 16 tower
    pointers statically at 146.7 B/entry, the audited anti-example; the fair
    baseline's actual footprint is established by measurement, #372). Workload predicates
    must be identical across arms (e.g. matching 50% filter selectivity in YCSB
    Workload E), PRNG algorithms and seeds must match across all languages (e.g.
    XorShift64), and memory accounting must measure live heap via allocator
    hooks (`TrackingAlloc`/`GlobalAlloc`) symmetrically across arms.
12. **Metric-scoped statistical gating (CI lower bound $\ge$ floor).** Sharpens
    rule 5: Continuous
    and wall-clock sampling distributions pass iff the **BCa 95% bootstrap CI
    lower bound $\ge$ floor** (≥1,000 resamples), NOT iff point estimate $\ge$
    floor. Point estimates and CIs must use identical definitions (macro-mean
    with macro-CI). Overlapping intervals must be labeled `BOUNDARY_RESULT` or
    `INTERMEDIATE_floor_within_ci`. Exact Callgrind instruction counts (zero
    sampling variance) are evaluated against the deterministic threshold
    contract.
13. **In-repo scratch path isolation.** Quick or developmental sweeps
    (`--quick`) must write strictly to gitignored scratch paths (e.g.
    `results/quick/` or `scratch/`) and must never overwrite canonical committed
    `results/baseline_*.json` or committed SVGs.
14. **Workload realism & DCE prevention.** Inner loops must consume every
    returned value via `std::hint::black_box`, `b.Fatalf`, or an accumulator
    sink to prevent Dead-Code Elimination. Read workloads must test realistic
    hit rates (e.g. 50% hit / 50% miss), never probing unbounded 64-bit random
    keys against sparse structures (~0% hit rate) unless explicitly evaluating
    the miss path.
15. **Provenance and pre-registration integrity.** Extends rule 4: every
    published number must carry a provenance tag `(measured: host, commit)`
    that additionally resolves to a committed JSON artifact or cited CI run. Pre-registration sections, hypotheses,
    claims ceilings, and expected loss matrices must never be backfilled or
    reconciled in place with observed results; unexpected outcomes must be
    published honestly under their strict pre-registered verdict label.

## Bench matrix

| Bench | Status | Notes |
|---|---|---|
| Lookup latency grid (hit/miss × distribution × population) | landed (`benches/compare.rs`) | vs `BTreeSet`/`BTreeMap`, `HashSet`/`HashMap`; measured on the reference host — see "vs stdlib" section below |
| Insert throughput (cold build per distribution) | landed (`benches/compare.rs`) | measured on the reference host — see "vs stdlib" section below |
| bytes/key | landed (`examples/bytes_per_key.rs`) | deterministic allocator accounting — load-immune, results below; **gates CI** via the `memory-budget` job |
| Instruction/cache counts | landed (`benches/instructions.rs`, iai-callgrind) | deterministic via callgrind — load-immune and resolves ~1% changes; **posted as a PR comment with head-vs-base deltas** by the `instruction-counts` job |
| Lookup attribution | landed (`examples/lookup_profile.rs`) | sampling profile of a `get`-only loop — *where* time goes, not how long; sample distribution inside one process is far less load-sensitive than a cross-binary ratio |
| Concurrent read scaling (1..N threads) | landed (`benches/concurrency.rs`) | Read-only and write-churn mixes; the per-node-OCC go/no-go instrument |
| JudySL/JudyHS instruction cells | landed (`benches/instructions.rs`, `benches/smoke_instructions.rs`) | Route-shaped string keys through `ExpanseStrMap`/`ExpanseBytesMap` insert/get/churn (#364); the smoke cells gate every PR automatically |
| Comparative benchmarks vs 3rd-party | landed (`benches/comparative.rs`) | `RoaringBitmap`, `hashbrown::HashMap` across lookups, insertions, ranges, and sparse/clustered/dense distributions |
| Automated Comparative Report Tool | landed (`scripts/bench_report.py`, `examples/bench_lookup_compare.rs`) | Standalone fast head-to-head comparison generator vs `hashbrown`, `BTreeMap`, and `libjudy` with GFM output |
| Standardized YCSB Suite (Workloads A-F) | landed (`benches/ycsb.rs`) | vs `BTreeMap`, `crossbeam_skiplist::SkipMap` (RocksDB MemTable); Zipfian $\theta=0.99, N=100\text{k}$, 128B blobs |
| Full libjudy + ART comparison | Phase 8 remainder | Headline table, dedicated-host runs, driven through the capi surface |
| Domain comparative suites (search, sorted-set, hash-map) | landed (`docs/benchmarks/*`) | self-contained reproducible suites with pre-registered hypotheses — see "Comparative benchmark suites" below |

## Comparative benchmark suites (`docs/benchmarks/`)

Each domain suite is a self-contained directory with the same shape: `README.md`
(results), `METHODOLOGY.md` (Step-0 pre-registered hypotheses, claims ceiling,
expected losses), `run.sh` (one-command reproduction on the reference host),
`scripts/` (JSON → tables/SVG), and `results/` (raw criterion/JSON output and
dual-theme charts). The criterion benches live under `crates/expanse/benches/`
and are registered in `crates/expanse/Cargo.toml`. All methodology rules above
apply; every losing cell is published, and a suite whose Step-0 hypothesis was
refuted says so in its README.

| Suite | Directory | Benches | Competitor(s) | Outcome at last measurement |
|---|---|---|---|---|
| Hash-map comparison | [`hashbrown_comparison/`](benchmarks/hashbrown_comparison/README.md) | `hashbrown_native_suite`, `hashbrown_ycsb`, `hashbrown_tail_latency`, `hashbrown_container_dists`, `hashbrown_memory_alloc` | `hashbrown::HashMap` (SwissTable), `std::BTreeMap` | see suite README (point/insert/YCSB/tail-latency/memory cells) |
| Search inverted index | [`search_inverted_index/`](benchmarks/search_inverted_index/README.md) | `search_boolean`, `search_wand`, `search_memory`, `search_instructions` | `roaring` 0.10 (`RoaringTreemap`) | Boolean pillar mixed (native kernels #339/#347, materialization #348, re-measured at commit `9c0026c8`): native cardinality within 3.84× of Roaring and **faster on 15 of 48 symmetric cells**; materialization v2 is 7×–225× faster than the v1 insert path but still loses the dense/clustered/sparse symmetric cells to Roaring; WAND: the #340 stateful cursor (re-measured at commit `5bd7bdda`) **beats or ties Roaring on all 6 dense cells** and clustered shallow/medium (down to 0.53× — 1.9× faster), within 1.2× in 14/18 cells, only sparse deep-skip trails (1.40×–1.64×); memory: Expanse wins shard-clustered 1e5, ties dense 1e6 |
| Redis ZSET engine | [`redis_zset_engine/`](benchmarks/redis_zset_engine/README.md) | `zset_zadd`, `zset_range`, `zset_rank`, `zset_memory` | `crossbeam_skiplist` + hash dict (Redis/Valkey ZSET design) | Expanse dual-trie wins 9 of 13 cells at the pre-#341 baseline (forward range ~5.5×); the #341 `range_rev` reverse iterator (re-measured at commit `ad540acc`) **flipped both pre-registered reverse-range losses into wins** (2.70× / 1.63× over the skip list; reverse now within 1.2× of forward); remaining loss: a rank-select dead heat |
| LLM inference & speculative decoding | [`llm_inference/`](benchmarks/llm_inference/README.md) | `bench_draft_quality`, `bench_llm_datastore`, `bench_grammar_masks`, `bench_prefix_lru` | HuggingFace PromptLookup (adaptive/fixed), Static Sorted Window Index, Dense Bitmask (`[u64]`), `roaring::Bitmap`, `collections.OrderedDict` | Draft quality: Expanse LSM yields modest +2.6% tok/s gain on Code (below 5% gate; lookup drafting accepts ~1 tok/step), dead heat on Summary, +8.8% on small-N JSON; Dynamic datastore: Expanse wins streaming updates whenever batch B < 73k; Grammar masks: Roaring wins 2.66x lower RAM; Prefix LRU: 9.5x lower RAM (cross-accounting; see suite caveat) + 1.98M-2.16M/s rank-eviction (baseline twin pending) |

Feature work that a suite gates (#339, #340, #341) re-runs the suite's `run.sh`
on the reference host at the feature commit and refreshes that suite's
`results/` and README in the same PR; the "Outcome" column here is updated
only from those re-measured tables.

## Automated Benchmark Comparison Report Tool (`scripts/bench_report.py`)

To generate instant head-to-head benchmark comparison tables ready for PR descriptions and documentation, use `scripts/bench_report.py`. The tool executes a fast standalone Rust harness (`crates/expanse/examples/bench_lookup_compare.rs`) comparing `ExpanseMap`, `hashbrown::HashMap`, `std::collections::BTreeMap`, and stock `libjudy` across key distributions.

### Usage

```bash
# Fast smoke run (< 2s, N = 10,000 keys)
python3 scripts/bench_report.py --quick

# Extended multi-population scaling sweep (N = 10k, 100k, 1M keys)
python3 scripts/bench_report.py --extended

# Microarchitecture target CPU scaling sweep (baseline generic vs x86-64-v2 vs x86-64-v3 vs native)
python3 scripts/bench_report.py --arch-sweep

# Target-specific microarchitecture compilation
python3 scripts/bench_report.py --target-cpu x86-64-v3 --quick

# Full population sweep (N = 1,000,000 keys, all distributions)
python3 scripts/bench_report.py --pop 1000000 --dist all --format markdown

# Single distribution in terminal table format
python3 scripts/bench_report.py --dist random --format table

# Export raw JSON for artifact logging
python3 scripts/bench_report.py --pop 100000 --format json --output bench_results.json
```

### CLI Flags

| Flag | Description | Default |
|---|---|---|
| `--quick` | Fast smoke mode ($N = 10,000$ keys) | `false` |
| `--extended` | Multi-population sweep ($N \in [10\text{k}, 100\text{k}, 1\text{M}]$); combine with `--arch-sweep` for the microarchitecture matrix (the `/bench extended` CI trigger passes both) | `false` |
| `--arch-sweep` | Target CPU microarchitecture sweep (baseline, v2, v3, native) | `false` |
| `--target-cpu <cpu>` | Specific target CPU architecture (e.g. `x86-64-v3`, `native`) | `None` (generic baseline) |
| `--pop <N>` | Target key population | `1,000,000` |
| `--pop-sweep <list>` | Comma-separated populations to sweep (e.g. `10000,100000,1000000`) | `None` |
| `--dist <dist>` | Key distribution (`sequential`, `random`, `clustered`, `sparse`, `all`) | `all` |
| `--format <fmt>` | Output format (`markdown`, `json`, `table`) | `markdown` |
| `--output <file>` | Output destination file path | `stdout` |
| `--rounds <N>` | Interleaved benchmark rounds (median reported) | `3` |
| `--input <file>` | Render tables from precomputed JSON artifact | `None` |
| `--self-test` | Run unit-style checks on the rendering helpers (parity band vs legend, derived summary) and exit | `false` |


## Reading perf results in a PR

Every pull request gets a single updating comment from the `instruction-counts` job, featuring three distinct comparisons:

1. **Head-to-Head Standard Baselines** (generated via `scripts/bench_report.py`):
   Instant comparative breakdown of point lookup hit latency (ns/op), lookup throughput (Mops/s), and full iteration (Mops/s) against `hashbrown::HashMap` and `std::collections::BTreeMap` across sequential, random, and clustered distributions.
2. **vs stock libjudy** (leads the instruction report): identical C ABI calls and
   key streams through libexpanse and through a `dlopen`'d stock
   libjudy, in instructions retired (`crates/expanse-capi/benches/vs_stock.rs`).
   This is the drop-in question — is the replacement competitive — and
   it is the project's headline claim. Ratio below 1.00 means libexpanse
   does less work.
3. **vs the merge base**: the same engine measured against its own
   merge-base commit on the base branch (`crates/expanse/benches/instructions.rs` via `--save-baseline=main_base` and `--baseline=main_base`). This is
   the regression question — did this change make the engine do more
   work — reported at Callgrind's deterministic `0.1%` measurement
   resolution. The **automated gate** (`scripts/perf_report.py
   --fail-on-regression`) fails the job at a `>5%` single-worst regression
   or ≥2 arms regressed above the `0.5%` noise floor; anything above
   `0.1%` remains a *review* blocker per AGENTS.md §6, and the only
   automated override is a literal `allow-regression: <reason>` line in
   the PR body.

All three tables are paired with collapsed sections for cache line / RAM traffic breakdowns and the deterministic bytes/key allocator memory budget tables.

---

## Callgrind Benchmark Suite Catalog (`crates/expanse/benches/instructions.rs`)

The deterministic Callgrind matrix evaluates instructions retired and cache line traffic across all engine operations and key distributions (50,000 keys per benchmark):

### 1. Operations Evaluated

| Benchmark Arm | Operation Description | Measured Scope & Invariants |
|---|---|---|
| `map_insert/*` | Cold insertion into `ExpanseMap` | Evaluates allocation, compression ladder climbs, and leaf building. Key generation is isolated in `setup`. |
| `set_insert/*` | Cold insertion into `ExpanseSet` | Evaluates bitset transitions, immediate-edge packaging, and `FullExpanse` upgrades. |
| `map_ins_slot/*` | Single-walk `JudyLIns` slot insertion | Measures the fused `locate_or_insert` path returning a writable value pointer. |
| `map_get/*` | Point lookup on prebuilt map | Evaluates pointer descent and leaf probe without measuring structure teardown (`core::mem::forget`). |
| `set_contains/*` | Point membership on prebuilt set | Evaluates bitset testing and immediate matching. |
| `map_churn/*` | Steady-state upsert/insert/delete mix | Crosses capacity-class boundaries in steady state (`k ^ 1` fresh neighbor insertion and removal). |
| `map_remove/*` | Key removal and tree condensation | Evaluates 1-index hysteresis and node compaction back down the compression ladder. |
| `map_iterate/*` | Full-order traversal | Measures iterator state machine and stackless trie traversal. |
| `map_nav/*` | Ordered navigation (`next_at_or_after`) | Evaluates cursor bounding and successor searching across multi-level branches. |

### 2. Key Distributions & Targeted Node Forms

| Distribution Name | Pattern Generation | Targeted Compression Forms & Algorithms Exercised |
|---|---|---|
| `sequential` | Contiguous integers: `0..50000` | Exercises monotonic append fast paths, `LeafB1` (256-bit bitmaps), and level 1 `FullExpanse` transitions. |
| `random` | 64-bit uniform pseudorandom keys | Exercises 6-byte immediates (~66% of terminals), linear `Leaf6` leaves, and multi-level `BranchL3`/`BranchL7` fanouts. |
| `clustered` | Runs of 256 keys with shared 56-bit prefixes | Exercises narrow-pointer skip decoding and branch placement at divergence levels. |
| `small` | Modular keys: `(i % 12) \| ((i / 12) << 32)` | Exercises root leaves ($\text{pop} \le 31$) and compact immediates. |
| `dense_leaf` | Runs of 32 keys sharing prefixes | Crosses `LEAF_CAP` (25) to exercise `LeafB1` (256-bit bitmap leaves at level 1). |
| `linear_leaf` | Runs of 15 keys sharing prefixes | Locks population into the 16-slot capacity class ($13 \le \text{pop} \le 16$) to exercise 128-bit AVX2/SSE2 vector search and `lower_bound` scans in `Leaf1`. |

---

## Bare-Metal Hardware Benchmarks

To ensure consistent performance measurements unaffected by shared cloud runner noise, Expanse supports automated bare-metal benchmarking on a dedicated bare-metal **reference host**.

### 1. Automated Execution via Self-Hosted GitHub Actions Runner
For dedicated benchmark rigs residing on private LANs (without inbound WAN access), a self-hosted GitHub Actions runner daemon (`runs-on: [self-hosted, linux]`) connects to GitHub via outbound-only HTTPS polling.

#### Setting Up the Runner on the Benchmark Machine
```bash
# Create directory and download the runner package
mkdir -p ~/actions-runner && cd ~/actions-runner
curl -o actions-runner-linux-x64-2.322.0.tar.gz -L https://github.com/actions/runner/releases/download/v2.322.0/actions-runner-linux-x64-2.322.0.tar.gz
tar xzf ./actions-runner-linux-x64-2.322.0.tar.gz

# Configure runner with repository registration token
./config.sh --url https://github.com/orieg/expanse --token <RUNNER_REGISTRATION_TOKEN> --labels baremetal,reference-host --unattended

# Run the worker (or install as systemd service: sudo ./svc.sh install && sudo ./svc.sh start)
./run.sh
```

### 2. Dual-Pass Baseline Drift Reporting Pipeline
To eliminate `N/A` comparison columns and guarantee accurate, side-by-side regression detection on bare metal, the runner executes a **two-pass evaluation workflow**:

1. **Pass 1 (Base Branch / Merge Base Baseline)**:
   - Identifies the target base commit (via explicit `base_ref` or by calculating `git merge-base origin/main "$REF"`).
   - Checks out the base ref and builds optimized release artifacts (`cargo build --release -p expanse-capi`).
   - Executes instruction and vs-stock benchmark suites under Callgrind, saving baselines:
     - `cargo bench --bench instructions -p expanse-trie -- --save-baseline=baremetal_base`
     - `cargo bench --bench vs_stock -p expanse-capi -- --save-baseline=baremetal_base_vs_stock`
2. **Pass 2 (Head Branch / Candidate Evaluation)**:
   - Returns to the candidate commit (`git checkout "$REF"`).
   - Re-compiles release artifacts and runs candidate benchmarks directly against the saved base baselines:
     - `cargo bench --bench instructions -p expanse-trie -- --baseline=baremetal_base`
     - `cargo bench --bench vs_stock -p expanse-capi -- --baseline=baremetal_base_vs_stock`
   - Executes deterministic allocator accounting (`bytes_per_key` and `bytes_per_key_32`).
   - Runs comparative baseline benchmarks against `hashbrown::HashMap` and `BTreeMap` (`scripts/bench_report.py`).
3. **Drift Aggregation & Sticky PR Reporting**:
   - `scripts/perf_report.py` synthesizes the dual-pass measurements into a structured GitHub Flavored Markdown report.
   - Posts or updates **that suite's** sticky comment on the PR thread — addressed by the stable marker `<!-- expanse-bench:<suite> -->`, so requesting several suites on one PR leaves several comments rather than one that overwrites itself — tagged with an anonymized host hardware description captured from the runner itself (`lscpu` / `nproc` / `uname` — no hostname), plus the system-load snapshot (uptime + top processes) recorded at run start, and a `Run` link so the published numbers keep resolving to a cited run (§8.7).
   - Prints a `Base Ref` line only when the base pass actually produced comparable benchmark output; a comparison that never ran is reported as such, and an empty base parse renders the report's prominent `⚠️ NO BASELINE` section instead of a quiet chip.
   - Emits the formatted report directly to `$GITHUB_STEP_SUMMARY`.

### 3. Triggering via PR Comment
Maintainers and collaborators can trigger benchmarks directly on any pull request by commenting:
- `/bench` (runs standard dual-pass suites: C ABI vs stock, instruction counters, and fast comparative sweep)
- `/bench extended` (or `/benchmark extended`): runs full multi-population sweeps ($N \in [10\text{k}, 100\text{k}, 1\text{M}]$) + microarchitecture target matrix (`baseline`, `x86-64-v2`, `x86-64-v3`, `native`) + Callgrind
- `/benchmark vs_stock` (runs only the `vs_stock` suite)
- `/benchmark instructions` (runs only the `instructions` suite)
- `/benchmark comparative` (runs only the `comparative` suite)
- `/benchmark ycsb` (runs only the `ycsb` suite)
- `/benchmark concurrency` (runs only the `concurrency` suite — `benches/concurrency.rs`, the dedicated `Sync*` wall-clock scaling instrument: `SyncExpanseMap`/`Set`/`BlobMap`/`StrMap`/`BytesMap` plus their `Mutex`/`RwLock`/`SkipMap`/`DashMap` baselines)

The self-hosted runner executes the suite natively on bare metal and posts/updates **that suite's own** sticky comment on the PR thread. Before benchmarking it takes the host-wide benchmark lock (methodology rule 8 above — refuses to start with exit 75 while another suite holds the host, releasing on exit, on interrupt, and on termination), records the system-load hygiene snapshot (uptime + top processes, non-gating) into the report, and fails fast with a clear error if a Callgrind suite (`all`, `extended`, `instructions`, `vs_stock`) is requested on a host missing `valgrind` or `iai-callgrind-runner`. Benchmark steps run under `pipefail`, so a crashed `cargo bench … | tee` fails the step instead of silently producing an empty report.

**Every trigger reaches a terminal comment.** Comments are addressed by the marker `<!-- expanse-bench:<suite> -->`, never by the heading text, so each suite owns one comment and suites never clobber one another. The reporting step is `if: always()`: a run that produces no numbers replaces its own `⏳` with the reason — the bench lock holder (`suite`/`pid`/`start`, read straight out of the lock's `owner` file), the missing `valgrind` / `iai-callgrind-runner`, a build or benchmark failure, or cancellation — plus the run link. A pending marker is never the final state, and a run with no numbers keeps whatever that suite last published in a collapsed block rather than erasing it (§8.1 forbids a degradation that renders as still-working; §8.7 wants a published figure to keep resolving to a cited run). A `concurrency:` group keyed on PR + suite means re-triggering the same suite supersedes its own in-flight run instead of queueing behind the single runner, while a *different* suite proceeds and is arbitrated by the host lock; the superseded run stands down rather than overwriting the newer run's comment. `timeout-minutes: 180` bounds a wedged run: comfortably above the longest observed run (~15 min for `all`) with headroom for `extended`'s population and microarchitecture sweeps, and half of GitHub's 6-hour default.

**Concurrency suite specifics.** Unlike the Callgrind suites, the `concurrency` suite is wall-clock and single-pass (report-only, no dual-pass baseline arm — thread-scheduling noise makes a tight per-PR threshold dishonest). `benches/concurrency.rs` accepts two env knobs, parsed in its `main()`:

- `EXPANSE_BENCH_THREADS` — comma-separated thread counts (default: `1,2,4,8,16`, clamped to available parallelism).
- `EXPANSE_BENCH_WORKLOADS` — comma-separated read percentages defining the read/write mixes (default: `100,95,50`, i.e. 100%/0%, 95%/5%, 50%/50%).

With the variables unset, local behavior is the unchanged full sweep. CI runs a reduced sweep to bound runtime: `/benchmark concurrency` uses `EXPANSE_BENCH_THREADS=1,4,16` + `EXPANSE_BENCH_WORKLOADS=100,50`; the nightly `bench-report` job uses `1,4` + `95,50` (the hosted runner has ~4 vCPUs).

**Nightly scaling-ratio gate (warn-only).** The nightly job tees the reduced-sweep tables into the `bench-report` artifact and runs `scripts/bench_concurrency_check.py`, which parses the tables into per-(engine, workload, threads) ops/s and gates on **scaling ratios** (total ops/s at max threads ÷ 1 thread, per engine/workload) against the previous nightly's `concurrency-baseline` artifact — the same artifact round-trip as the bindings baseline. Ratios are robust to host-load drift and catch exactly the collapse class the deterministic instruction gates cannot (a change that serializes readers or drops a lock-free path into the mutex fallback), with a generous default threshold of a 30% relative ratio drop (`--max-ratio-drop-pct`). The check is currently **warn-only** (`--fail-on-regression` unset); it will be promoted to failing once the baseline proves stable across several consecutive unmodified nightlies (issue #360). The script's parser and ratio math are covered by `python3 scripts/bench_concurrency_check.py --self-test`, which nightly runs before the real check.

### 4. Triggering via `workflow_dispatch`
The `Bare-Metal Benchmarks` workflow can also be triggered manually via GitHub Actions UI (*Actions* tab $\rightarrow$ *Bare-Metal Benchmarks* $\rightarrow$ *Run workflow*). It accepts `ref`, `base_ref`, `pr_number`, and `benchmark_suite`.

### 5. Running Locally over LAN
Developers can also execute the exact same sync, build, and benchmark suite from their local development machine across their LAN using `scripts/run_remote_bench.sh`:

```bash
export BENCH_HOST="user@bare-metal-host"
export BENCH_REPO="/path/to/remote/dir"
./scripts/run_remote_bench.sh all
```

**Privacy Reminder:** Per `AGENTS.md`, never commit private hostnames, LAN IPs, or personal paths. Always use environment variables like `$BENCH_HOST` and `$BENCH_REPO`.

---

## Measured results

### bytes/key (measured: deterministic allocation accounting via `NodeAlloc`, commit with this section; machine-independent)

> Produced by `cargo run --release -p expanse-trie --example bytes_per_key`. `test_memory_budget_matches_engine` recomputes every cell in-process and asserts [`docs/visualizer_data.json`](visualizer_data.json) matches, so that artifact is the gated copy and this table must agree with it. Regenerate both from the example rather than editing either by hand.

| dist | pop 1k | pop 100k | pop 1M | |
|---|---|---|---|---|
| sequential (set) | 0.32 | 0.07 | **0.07** | full-expanse + bitmap-leaf compression |
| clustered 256-run (set) | 0.38 | 0.37 | **0.36** | was 1.34 before leaf-targeted narrow pointers — a 3.7× improvement |
| clustered 4096-run (set) | 0.32 | 0.12 | **0.12** | was 0.64 / 0.20 / 0.19 before **branch-targeted** narrow pointers (divergence-level branch placement + `split_skip`) |
| random (set) | 13.50 | 14.78 | 7.92 | not part of the dense/clustered target |
| sparse `i << 40` (set) | 16.83 | 16.32 | **16.31** | one 16-byte edge per isolated key — the structural floor, not a chain cost (immediates absorb the remainders) |

Map-flavor figures run ~8 B/key above the set figures (the stored value word). The `< 9.5 B/key dense+clustered` architecture target is **met** on the distributions it names. Unit-level anchor for the branch-targeted work: two 512-key clusters cost 192 structural bytes vs 960 under per-level chains (`branch_skip_clusters` tests, both flavors).

### Instruction counts: issue #1 items 1-3 (measured: callgrind via the `instruction-counts` CI job; deterministic, so these are exact — commit with this section)

Controlled A/B against a branch with all three items reverted, same harness and job. Instructions retired, 50k keys per benchmark; negative is less work.

| benchmark | instructions | est. cycles |
|---|---:|---:|
| `set_insert/random` | **−16.95%** | −17.62% |
| `map_iterate/random` | −14.47% | −15.42% |
| `set_contains/random` | −14.11% | −14.95% |
| `map_ins_slot/random` | −12.28% | −12.62% |
| `map_insert/random` | −12.06% | −12.49% |
| `set_insert/sequential` | −10.73% | −11.42% |
| `map_get/random` | −10.43% | −10.98% |
| `map_insert/small` | −8.90% | −9.95% |
| `map_insert/sequential` | −8.07% | −8.18% |
| `map_remove/random` | −7.44% | −8.36% |
| `set_insert/clustered` | −6.88% | −7.83% |
| `map_get/sequential` | −6.02% | −6.28% |
| `map_insert/clustered` | −2.98% | −3.95% |
| `map_get/clustered` | −2.26% | −3.08% |

All 14 benchmarks improved; nothing regressed. The three changes were: the per-level OCC check hoisted into a const generic (removing ~10 atomic loads per mutation), immediate-edge key handling moved from `Vec` to a stack buffer (removing a malloc/free per insert into an immediate), and width-monomorphized packed key access (`read_packed`/`write_packed`/`lower_bound`).

Two things this table is **not**. It is not a wall-clock claim: instructions are cost, and how much of it a machine hides needs a quiet host to answer. It is also not comparable to the vs-stock ratios, which are wall-clock on different hardware. What it is: reproducible evidence that the engine does measurably less work, at a resolution (0.1%) neither available environment can reach with a timer. Contrast the `memcmp` episode below, where wall-clock at n=1 suggested a 7-11% lookup win that a second run erased.

### Attribution findings (measured: M1 MacBook Pro under load — attribution only, no timing claim; commit with this section)

Profiling a 1M-key random `get` loop (`examples/lookup_profile.rs`, macOS `sample`) attributed **~6% of lookup samples to `memcmp` reached through a dynamic-linker stub**: the linear-leaf key comparison used a runtime-width slice compare, which lowers to a libc call rather than inline code. Monomorphizing the scan over the key width (`leaf::search_fixed::<KB>`) removed the call — the re-profiled binary contains **zero** `memcmp` references on that path. This is a *structural* claim (the call no longer exists), independent of machine load; the resulting ns/op change is unclaimed until a quiet-host run.

Also visible: `EdgeTag::from_u8` not fully inlined (~1.7% of samples), which is the measured part of the "tag-dispatch overhead" hypothesis — small next to the leaf comparison, and worth revisiting only with the vs-stock harness on a quiet host.

**Did removing the `memcmp` call speed lookups up? Not measurably — verdict: NO DETECTABLE EFFECT.** A controlled A/B was run through the `bench-report` job: identical code and job on both arms, with only `leaf::search` reverted to the runtime-width compare on a throwaway branch. The first pair looked like a 7–11% get-ratio win in all three distributions; a second pair of runs refuted it.

| get ratio @1M | with monomorphized search | with `memcmp` |
|---|---|---|
| sequential | 1.60, 1.75 | 1.77, 1.62 |
| random | 1.47, 1.47 | 1.66, **1.34** |
| clustered | 1.55, 1.73 | 1.65, 1.65 |

The arms overlap; the pre-change branch produced both the worst and the best random-key ratio observed. Within-arm spread (up to 0.32 on a ~1.5 ratio) is as large as the between-arm difference, so any real effect is buried. The change is kept because it strictly removes work (a PLT-indirected libc call from the innermost loop) and cannot be slower, but **no speedup is claimed**.

**The CI runner's noise floor.** Across four runs the same ratio varied by up to ±10%, and absolute ns by ±40%, on freshly booted runners at load < 1.7. That puts the minimum detectable effect for this setup around **15–20% at n=2**: fine for catching a structural regression, useless for validating incremental optimization. Adding runs to CI buys little, because the variance is between runner instances rather than within a run. Incremental perf work needs a dedicated quiet host with many rounds and reported confidence intervals; absent that, prefer changes justified *structurally* (work removed, allocations avoided, cache lines not touched) over changes justified by a measured delta this environment cannot resolve.

Method note: attribution is done inside a single process, so co-resident load shifts *how many* samples land, not *where* they land. A/B wall-clock ratios do not share that property and stay deferred (rule 2).

### Concurrent read scaling (`benches/concurrency.rs`, all `Sync*` arms)

*(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu
22.04 / kernel 6.8, run
[33030152085](https://github.com/orieg/expanse/actions/runs/33030152085), ref
`main` @ `5fb03aa3`; `EXPANSE_BENCH_THREADS="1,4,16"`,
`EXPANSE_BENCH_WORKLOADS="100,50"`, 500 ms windows, load average 0.00 at start)*

Every arm uses a bounded keyspace (`2 × POP`, ~50% hit rate) since
[#375](https://github.com/orieg/expanse/issues/375). A dense bounded keyspace
builds a compact, cache-resident trie, so these arms measure descent through a
populated tree rather than a miss walk.

**100% read:**

| arm | 1 thread | 4 threads | 16 threads | scale @ 16T |
|---|---:|---:|---:|---:|
| `SyncExpanseSet` | 77.5 M ops/s | 295.2 M | **884.5 M ops/s** | **11.42×** |
| `SyncExpanseMap` | 37.2 M | 138.9 M | **424.1 M ops/s** | **11.40×** |
| `SyncExpanseBlobMap` | 31.4 M | 120.5 M | **332.4 M** | **10.60×** |
| `SyncExpanseBytesMap` | 11.7 M | 40.9 M | **133.8 M** | **11.48×** |
| `SyncExpanseStrMap` | 7.0 M | 26.6 M | **82.9 M** | **11.84×** |
| `DashMap<Vec<u8>, u64>` | 15.6 M | 50.4 M | 132.1 M | 8.47× |
| `SkipMap<u64, Vec<u8>>` | 3.4 M | 13.1 M | 38.3 M | 11.13× |
| `RwLock<BTreeMap<u64, …>>` | 10.6 M | 20.0 M | 18.6 M | 1.75× |
| `Mutex<ExpanseBlobMap>` | 40.0 M | 10.4 M | 5.6 M | 0.14× |
| `Mutex<ExpanseBytesMap>` | 12.2 M | 6.0 M | 3.5 M | 0.29× |
| `Mutex<ExpanseStrMap>` | 9.2 M | 5.7 M | 4.1 M | 0.45× |

Reproducibility: the string/bytes arms measured 82.7 M / 11.76× and 135.1 M /
11.52× in the independent earlier run
[33016450539](https://github.com/orieg/expanse/actions/runs/33016450539)
(commit `698bf70c`) — within ~1% of this run's 82.9 M / 133.8 M.

**50% read / 50% write (read ops/s; the honest worst case):**

| arm | 1 thread | 4 threads | 16 threads | scale @ 16T |
|---|---:|---:|---:|---:|
| `DashMap<Vec<u8>, u64>` | 5.6 M | 17.3 M | **44.5 M** | **7.93×** |
| `SkipMap<u64, Vec<u8>>` | 1.1 M | 3.4 M | 7.7 M | 6.94× |
| `SyncExpanseStrMap` | 3.1 M | 2.6 M | 1.7 M | 0.55× |
| `SyncExpanseBlobMap` | 7.3 M | 3.1 M | 2.1 M | 0.28× |
| `SyncExpanseMap` | 10.9 M | 3.0 M | 2.0 M | 0.19× |
| `SyncExpanseBytesMap` | 3.1 M | 2.0 M | 0.5 M | 0.17× |
| `SyncExpanseSet` | 16.6 M | 3.3 M | 2.1 M | 0.12× |
| `RwLock<BTreeMap<u64, …>>` | 4.2 M | 2.1 M | 1.5 M | 0.34× |

**The architectural trade-off.** Under a 50/50 mix every single-writer arm
*loses throughput as threads are added* (0.12×–0.55×). Sharded and
lock-free-write structures win this regime outright: `DashMap` scales 7.93× and
`SkipMap` 6.94×, both far above every Expanse arm. Expanse targets read-mostly
workloads, and at 100% read it leads `DashMap` in both absolute throughput
(884.5 M `SyncExpanseSet`, 424.1 M `SyncExpanseMap`, 133.8 M
`SyncExpanseBytesMap` vs 132.1 M) and scaling (10.6×–11.8× vs 8.47×), while
retaining ordered iteration and range/rank queries a sharded hash map cannot
serve.

**Mechanism.** Read-only scaling is near-linear (10.6×–11.8× at 16 threads)
while the coarse-mutex baselines fall below their own single-thread throughput
(0.14×–0.45×). The write-mixed collapse has a single cause: a tree-level
seqlock brackets whole operations for the root snapshot, so under an active
writer the version changes faster than a walk completes and readers retry or
fall back to the mutex. The per-node version refinement (writers bracket each
node's in-place mutations; readers validate hand-over-hand) keeps churn in the
millions rather than collapsing to zero, but closing the write-mixed gap needs
multi-writer support — sharding or per-node write locks — not finer validation
(`docs/ARCHITECTURE.md` §6). Single-threaded trees skip the version brackets
entirely (`NodeAlloc::occ_enabled`), so the classic engine pays nothing.

### vs stdlib & 3rd-party collections (measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit 695b98d; `benches/compare.rs` + `benches/comparative.rs`, criterion medians)

**Point lookup, hit, 1,000,000 keys (ns/op)** — the engine's strongest arm:

| dist | `ExpanseSet` | `BTreeSet` | `HashSet` | `ExpanseMap` | `BTreeMap` | `HashMap` |
|---|---:|---:|---:|---:|---:|---:|
| sequential | **6.75** | 97.8 | 12.0 | **11.9** | 108.9 | 12.1 |
| random | 36.5 | 104.5 | **11.9** | 38.6 | 111.9 | **12.4** |
| clustered | **8.19** | 101.6 | 12.0 | **12.9** | 110.2 | 12.4 |
| sparse | **7.47** | 103.5 | 12.6 | **8.53** | 111.9 | 12.3 |

- **vs `BTreeMap`/`BTreeSet`: Expanse wins every cell** — 2.9× (random) to ~14.5× (sequential/sparse) faster.
- **vs `HashMap`/`HashSet` (Swiss table)**: near parity on sequential/clustered; **~3× slower on uniform-random 1M** (hash O(1) probe beats trie descent under cache misses); faster on sparse. Expanse buys ordering + prefix search for that.
- **Random lookup is a working-set-vs-cache crossover** — `benches/compare.rs` measures it at both 10k (cache-resident: ~1.1× hashbrown, 10.0 ns vs 8.9 ns) and 1M (out-of-cache: ~2.9×, cache-miss-bound trie descent vs single hash probe) *(measured: reference host, commit 4a12f046)*. Never quote a single random-key ratio without its population: the widening from ~1.1× to ~2.9× is a scale/cache effect (verified stable, not a regression), not a fixed weakness.

**Cold-build insert, 100,000 keys (`set_insert_build`)**:

| dist | `ExpanseSet` | `BTreeSet` | `HashSet` |
|---|---:|---:|---:|
| sequential | **484 µs** | 4.46 ms | 1.81 ms |
| random | 4.17 ms | 7.45 ms | **1.81 ms** |
| clustered | **1.04 ms** | 4.59 ms | 1.81 ms |
| sparse | 2.88 ms | 4.47 ms | **1.81 ms** |

Expanse beats `BTreeSet` on every distribution (1.6×–9.2×); beats `HashSet` on sequential/clustered but is ~1.6×–2.3× slower on random/sparse cold builds.

**`ExpanseSet` vs `RoaringBitmap` (`comparative.rs`, 100k)**:

| op | sparse | clustered | dense |
|---|---|---|---|
| `contains` (ns) | 8.5 vs 19.0 → **2.2× faster** | 7.3 vs 17.9 → **2.4× faster** | 6.8 vs 3.5 → 1.9× slower |
| `rank` (ns) | 336 vs 108 → 3.1× slower | 198 vs 51 → 3.9× slower | 218 vs 196 → ~parity |
| `select` (ns) | 343 vs 235 → 1.5× slower | 217 vs 66 → 3.3× slower | 208 vs 376 → 1.8× faster |

Expanse wins membership on sparse/clustered; Roaring's specialized rank index wins `rank`/`select`.

**Full ordered iteration — now faster than `BTreeMap` for dense keys (post-#245).** [#245](https://github.com/orieg/expanse/pull/245) replaced the per-step allocating descent with a stack-based zero-allocation iterator, flipping the earlier result. Full ordered `map.iter()` vs `BTreeMap::iter()` over 1M keys, as a ratio of Expanse's time to `BTreeMap::iter()`'s (< 1 means Expanse is faster):

| key distribution | pre-#245 | post-#245 | post-#270 |
|---|---|---|---|
| sequential | 6.8× slower | **0.7× (faster)** | **0.7× (faster)** |
| clustered | 6.4× slower | **0.8× (faster)** | **0.8× (faster)** |
| random | 2.1× slower | **0.5× (2× faster)** | **0.5× (2× faster)** |
| sparse | 10.4× slower | 4.7× slower | **2.4× slower** |

*(post-#245 measured: reference host — Intel i9-12900F, 24 threads, commit 46529f19; post-#270 measured: same host, commit 1feefadf; `benches/compare.rs`, criterion mean of the 1M-key `map_iter` grid; ratios are Expanse's `map.iter()` time over `BTreeMap::iter()`'s, so < 1 means Expanse is faster)*. Across the four distributions #245 delivered a **2.2×–9.4× speedup**. Dense (sequential/clustered/random) iteration beats `BTreeMap::iter()` and is unchanged by #270.

**Sparse-key iteration ([#270](https://github.com/orieg/expanse/issues/270)):** the `sparse` distribution `keys(i) = i << 40` puts all variation in bytes 5–7 with bytes 0–4 always zero, so at 1M keys the trie is a three-level dense `BranchU` spine over **1,000,000 single-key immediate leaves** (one 16-byte edge per isolated key — the structural floor; immediates absorb the zero remainders, so there is no separate leaf allocation or pointer chain to remove). [#270](https://github.com/orieg/expanse/issues/270) added a single-key-immediate fast path to the stack iterator: the post-#245 iterator decoded every immediate through the general multi-key path, zeroing two `[u64; 15]` staging arrays and copying a 240-byte cursor to carry one 16-byte key/value. Decoding single-key immediates directly cut the Expanse arm from **14.34 ms → 7.09 ms** (~2.0× faster; criterion CIs [14.18, 14.42] vs [7.09, 7.10] separate cleanly), narrowing the gap from ~4.7× to ~2.4× slower than `BTreeMap`. The residual ~2.4× is the remaining structural floor: the trie still visits ≈3,900 branch nodes (~16 MB of `BranchU` pages) and re-descends per key, where a B-tree walks contiguous 11-wide leaf arrays. Point lookups and prefix seeks, where the trie skips empty expanses, remain the engine's other advantage.

### Checkpoint B0 — first vs-stock baseline on the corrected harness (measured: GitHub `ubuntu-latest` runner, callgrind via the `instruction-counts` job, **the issue #1 items-1-3 branch, not `af21e02`**; deterministic)

The first vs-stock instruction baseline taken after the harness stopped
measuring its own setup: both the tree build and the teardown sat inside the
timed region of every arm before `af21e02`, so the lookup arms were reporting
insert work.

This table is an rlib-only measurement of the issue #1 items-1-3 branch, i.e.
`main` *plus* that engine work — not of `af21e02`, the commit it was originally
labelled with. B2 below carries the true `main` baseline alongside it.

Ratio = ours ÷ stock, instructions retired through the identical C ABI on
identical key streams. 30k keys except `random_big`, which is 1.5M.

| operation | ours | stock | ratio | est. cycles ratio |
|---|---:|---:|---:|---:|
| `judy1_set/clustered` | 12,561,871 | 10,910,533 | **1.15×** | 1.20× |
| `judy1_set/random` | 32,633,176 | 16,350,887 | 2.00× | 2.02× |
| `judy1_test/random` | 7,160,539 | 3,970,257 | 1.80× | 1.69× |
| `judyl_get/clustered` | 6,730,957 | 4,238,479 | 1.59× | 1.52× |
| `judyl_get/random` | 7,576,697 | 5,095,457 | 1.49× | 1.45× |
| `judyl_get/random_big` (1.5M) | 441,381,294 | 403,155,522 | **1.09×** | **1.04×** |
| `judyl_get/sequential` | 9,300,109 | 5,072,393 | 1.83× | 1.68× |
| `judyl_insert/clustered` | 20,710,296 | 12,532,068 | 1.65× | 1.69× |
| `judyl_insert/random` | 40,152,643 | 18,377,126 | 2.18× | 2.20× |
| `judyl_insert/sequential` | 23,217,209 | 13,089,262 | 1.77× | 1.77× |

**The gap is population-dependent.** The same random-key lookup is 1.49× at 30k
keys and 1.09× at 1.5M, and its estimated-cycles ratio (1.04×) is *below* its
instruction ratio — the only arm where that happens, and what a denser
structure taking proportionally fewer misses looks like.

Read that as a hypothesis, not a result: it is one arm at one population, and
the estimated-cycles figure is callgrind's `Ir + 10·L1m + 100·LLm` model, which
assumes zero mispredicts and zero dependent-load latency. Confirming the
narrowing needs wall-clock at 1.5M with bootstrap CIs (checkpoint B5). What the
table does establish is that small-population arms are not representative of
the whole curve, so no single ratio should be quoted as "the gap" without its
population.

**Every ratio above is optimistic**, uncorrected. This arm is an LTO'd rlib
reached by direct calls; stock is a PIC shared object reached through
`dlopen`/`dlsym`, with its `dlopen` still inside the measured region. Both
biases favour libexpanse, so these are a floor on the gap, not an estimate. The
drop-in number is B2 below, measured through `libexpanse.so`.

Operational note: `instruction-counts` runs in **~2 minutes against its
45-minute timeout**, including the 1.5M arms, so the big arms stay in the
required check. `miri` is the only expensive job at ~25 minutes, and it is
conditional on the diff touching Rust sources.

### Checkpoint B2 — the same-shape comparison, and the shared-library correction factor (measured: GitHub `ubuntu-latest` runner, callgrind via `instruction-counts`, commit with this section; deterministic)

B0 above measured the rlib against stock's shared object. This measures
`libexpanse.so`, `dlopen`'d and called through resolved symbols exactly as
stock is — the shape a drop-in consumer gets. **These are the numbers to
quote.**

Taken on `main` at `6587e9f`, via a pull request that changes only CI
configuration, so the measured code is `main`'s exactly.

| operation | `.so` ratio | rlib ratio | est. cycles (`.so`) |
|---|---:|---:|---:|
| `judyl_get/random_big` (1.5M) | **1.15×** | 1.09× | **1.09×** |
| `judy1_set/clustered` | **1.25×** | 1.16× | 1.25× |
| `judyl_get/random` | 1.58× | 1.49× | 1.53× |
| `judyl_insert/sequential` | 1.71× | 1.78× | 1.70× |
| `judyl_get/clustered` | 1.71× | 1.59× | 1.64× |
| `judyl_insert/clustered` | 1.78× | 1.68× | 1.77× |
| `judy1_test/random` | 1.88× | 1.80× | 1.79× |
| `judy1_set/random` | 1.93× | 2.02× | 1.95× |
| `judyl_get/sequential` | 1.94× | 1.83× | 1.79× |
| `judyl_insert/random` | 2.16× | 2.21× | 2.18× |

**Correction factor: 1.06× median, range 0.95–1.08×** (1.05×, 0.96–1.07×
measured on the same harness without items 1-3 — it barely moves between two
code states, which is what a ratio between two builds of the *same* code should
do, and makes it the most robust number in this section).

**Best and worst, both worth naming.** The 1.5M lookup at **1.15×** (1.09× in
estimated cycles) and clustered `Judy1` inserts at **1.25×** are the strongest
arms; random `JudyL` insert at **2.16×** is the weakest, and per-function
profiling puts ~16% of it inside the allocator. The 1.5M arm remains the only
one whose cycles ratio falls *below* its instruction ratio.

**The shared-object cost is not uniform.** It is **confined to the lookup
arms** (1.05–1.07×), where a PLT-mediated entry is a meaningful fraction of a
short operation. On the insert arms the `.so` is *cheaper* than the LTO'd rlib
(0.96–0.98×). Both builds get `lto = "thin"` and `codegen-units = 1`; what the
rlib arm additionally gets is cross-crate inlining with the harness, and on a
long insert that costs more instructions than it saves. Cross-object inlining
is worth ~6–8% on lookups and nothing on inserts.

**Two arms are byte-identical with and without items 1-3.**
`judyl_get/clustered` at 7,227,854 and `judyl_get/sequential` at 9,840,110 did
not move by a single instruction while every other arm did. The terminal-form
census below predicts exactly that: those arms build only bitmap leaves, so
neither the immediate scan nor the linear-leaf population read is on their
path. That also makes the +1.50% those items landed on `map_get/sequential` a
codegen side-effect rather than a change in work done — that arm executes none
of the changed code.

Both arms bind their symbols in `setup`, so neither measures its own dynamic
linking, and key generation has moved out of the insert arms' measured region —
see rule 0 for why that mattered even though it was symmetric.

### Tag decode inlined — the first arm below 1.00 (measured: GitHub `ubuntu-latest` runner, callgrind via `instruction-counts`, commit with this section; deterministic)

The first optimization measured against B2, and the largest single change in
the project so far. Three `#[inline(always)]` attributes on `EdgeType::from_u8`,
`ImmedType::from_u8` and `EdgeTag::from_u8`.

**`judyl_get/random_big` (1.5M keys) is now 0.95×** — libexpanse retires ~5%
**fewer** instructions than stock libjudy through the identical C ABI, in the
`.so` shape a drop-in consumer gets. Estimated cycles 0.94×. That is the first
arm to go below 1.00.

| operation | before | after |
|---|---:|---:|
| `judyl_get/random_big` (1.5M) | 1.15× | **0.95×** |
| `judy1_set/clustered` | 1.25× | **1.14×** |
| `judyl_get/random` | 1.58× | **1.23×** |
| `judyl_get/clustered` | 1.71× | 1.43× |
| `judy1_test/random` | 1.88× | 1.55× |
| `judyl_get/sequential` | 1.94× | 1.65× |
| `judyl_insert/sequential` | 1.71× | 1.61× |
| `judyl_insert/random` | 2.16× | 2.08× |

All 14 self-comparison benchmarks improved: lookups −13.8% to −21.2%, inserts
−4.0% to −10.9%, iteration −13.7%, remove −1.8%.

**Instructions are cost, not time.** A ratio below 1.00 says the engine does
less work, not that it is faster; wall-clock confirmation needs a quiet host
and belongs to checkpoint B5. It is also one arm at one population — the 30k
arms remain 1.14–2.08×.

**Plain `#[inline]` changed nothing** — all 14 benchmarks byte-identical, the
symbol still present at 17.05% of `set_contains/random`. Within a single crate
built with `codegen-units = 1`, `#[inline]` is close to advisory: LLVM already
has full visibility into the body, so the hint carries no information it lacks
and does not override its cost model. It earns its keep across crate
boundaries. Only `#[inline(always)]` moved this.

The saving slightly exceeds the symbol's own cost: `set_contains/random` fell
2,155,565 against a `from_u8` self-cost of 2,022,260, and `ExpanseSet::contains`
*shrank* rather than absorbing the work. The caller's match fused with the
decode once LLVM could see through the call, which is most of what raw-tag
dispatch was meant to achieve; re-justify that follow-up against fresh profiles
before spending a PR on it.

### First per-function attribution (measured: GitHub `ubuntu-latest` runner, `callgrind_annotate` over the `instruction-counts` job, commit with this section; deterministic)

Per-benchmark totals show a delta but cannot attribute it. The first run with
per-function output found three things in one pass.

**1. `EdgeTag::from_u8` is not inlined, and it is 8-19% of every lookup.**
*(Resolved — see the tag-decode section above. Shares below predate the
harness teardown fix, so part of each is `free_subtree` decoding tags inside
what was then the measured region; on the corrected harness it was 17.05% of
`set_contains/random`.)*

| arm | `from_u8` | share |
|---|---:|---:|
| `set_contains/random` | 2,948,168 | **19.4%** |
| `map_get/random` | 2,413,688 | 14.2% |
| `map_get/sequential` | 1,602,072 | 9.6% |
| `map_get/clustered` | 1,008,776 | 8.1% |

It appears as its own symbol, a real call, at ~8 instructions each, once per
level descended. The call count cross-checks exactly against the terminal-form
census below: sequential lookups touch 4 edges (`BranchL3`@8, `BranchL3`@7,
`BranchU`@2, `LeafB1`@1) and the profile shows 200,259 calls over 50,000
probes, i.e. 4.005 per lookup. The earlier macOS figure of ~1.7% was a sampling
profile of a different distribution.

**2. ~16% of `map_insert/random` is inside the allocator** — `_int_malloc`
7,685,357 (11.2%) plus `_mid_memalign` 3,226,000 (4.7%). The `memalign` path
is what 64-byte alignment costs: it takes glibc off its fast `malloc` route.
This is the measured case for alignment classes.

**3. Rule 0 violated a fourth time, in the other harness file.**
`free_subtree` appeared *inside the lookup benchmarks*: 1,654,204 (9.7%) of
`map_get/random`, 1,264,288 (8.3%) of `set_contains/random`. The arms take the
container by value, so `Drop` ran inside the measured region. `vs_stock.rs` had
been fixed; `instructions.rs`, the file that produces the vs-base column on
every PR, had not. Fixed by leaking, matching `vs_stock.rs`, along with
`keys()` moving into `setup` for the insert arms.

### What the bench matrix structurally covers (measured: terminal-form census over the tree each benchmark builds, 50k keys, map flavor; commit with this section; machine-independent)

A benchmark named after a distribution does not say which node *forms* it
exercises. The coverage is narrower than the names suggest:

| distribution | terminal forms |
|---|---|
| `sequential` | 196 × `LeafB1` at level 1 — no linear leaves, no immediates |
| `clustered` | 196 × `LeafB1` at levels 6-7, behind narrow pointers |
| `random` | 23,290 immediates (6-byte) + 11,675 × `Leaf6` at level 6 |

Two consequences, both load-bearing:

- **No benchmark exercises a linear leaf on a dense distribution.** Linear-leaf
  search is reachable only through `random`, and only at width 6. Any claim
  about leaf-width monomorphization or leaf search is currently measured on one
  arm at one width — the `dense_leaf` arm is a known coverage hole, not a
  precaution.
- **Immediates are ~2/3 of random-key terminals** (23,290 of 34,965),
  confirming the assumption behind prioritizing `immed_find`.

The census also attributed a +1.50% regression that per-benchmark totals could
not: the two arms that regressed reach *none* of the code paths the change
touched. Re-run it whenever a distribution or population changes; promoting it
out of a throwaway probe is tracked in issue #1.

### libexpanse vs stock libjudy, JudyL surface (measured: GitHub `ubuntu-latest` runner, 2 cores, load 0.42 at start — the standing reference environment; commit with this section; interleaved A/B medians of 5 rounds; harness: `crates/expanse-capi/examples/bench_vs_libjudy.rs`, nightly `bench-report` job)

| dist | pop | get ratio (ours/stock) | insert ratio | B/key ratio |
|---|---|---|---|---|
| sequential | 1M | 1.60× slower | 2.88× slower | 1.03 |
| random | 1M | 1.47× slower | 2.38× slower | **0.93 (smaller)** |
| clustered | 1M | 1.55× slower | 2.48× slower | **0.92 (smaller)** |

**This CI table is the reproducible instruction-baseline reference** (`x86-64-v1`, no runtime SIMD). The **quiet-host table below is the wall-clock reference** for absolute numbers. A CI runner is not a quiet host, but it is freshly booted, unshared with a desktop session, and reproducible. Only ratios transfer between machines — the paired interleaved arms normalize away machine speed (rule 3).

Cross-machine sanity check: bytes/key columns are byte-identical to the local run (deterministic accounting), which is what makes the timing columns' disagreement attributable to hardware rather than to the build.

### libexpanse vs stock libjudy, JudyL surface — dedicated quiet host *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit `43b46f38`; harness `crates/expanse-capi/examples/bench_vs_libjudy.rs`, interleaved A/B median of 5 rounds, 6 repetitions, load < 0.5; **native runtime feature detection — AVX2/BMI2 paths active**; stock libjudy 1.0.5 built from the SourceForge release tarball and `dlopen`'d)*

Wall-clock ns per operation, 1,000,000-key populations (the stable rows; `< 1.00` = libexpanse faster / smaller):

| dist | insert ours / stock (ratio) | get ours / stock (ratio) | B/key ours / stock (ratio) |
|---|---|---|---|
| **sequential 1M** | **13.4 / 23.6 ns (0.57×)** | **7.5 / 11.1 ns (0.68×)** | 8.56 / 8.32 (1.03×) |
| **random 1M** | **56.2 / 71.7 ns (0.78×)** | 35.8 / 32.3 ns (**1.11×**) | 16.70 / 17.67 (**0.95×**) |
| **clustered 1M** | **19.9 / 21.6 ns (0.92×)** | **8.5 / 10.4 ns (0.82×)** | 8.61 / 9.32 (**0.92×**) |

Honest reading: with the **native SIMD paths active** libexpanse wins insert across all three distributions (0.57×–0.92×) and wins sequential/clustered lookup (0.68× / 0.82×). It is **~11% slower on random 1M lookup** (1.11×). Random point access is memory-latency-bound, where the trie's extra indirection costs and SIMD does not help; this agrees with the M-series NEON laptop reading below (random lookup 1.55× slower) and is the engine's weak arm. On the `x86-64-v1` CI runner above, without the SIMD leaf/branch kernels, the same operations are slower than stock. The swing between the two rows is the microarchitecture tier, not a code change (per-routine v1→v3 instruction deltas span −1.9% to −42.6% — [`docs/visualizer_data.json`](visualizer_data.json)). The 100k-population rows are cache-warmup-noisy on this desktop part and are omitted. bytes/key is deterministic allocator accounting (`JudyLMemUsed`) and reproduces byte-for-byte across runs.

### Earlier: same harness on the development laptop (measured: M1 MacBook Pro under load — a VM at ~226% CPU co-resident — commit with this section; interleaved A/B medians of 5 rounds, so the *ratios* are meaningful while absolute ns are contaminated; harness: `crates/expanse-capi/examples/bench_vs_libjudy.rs`)

| dist | pop | get ratio (ours/stock) | insert ratio | B/key ratio |
|---|---|---|---|---|
| sequential | 1M | 2.3–4× slower | 3.4× slower | 1.03 |
| random | 1M | 1.55× slower | **1.35× slower** | **0.93 (smaller)** |
| clustered | 1M | **~parity (0.94–1.2×)** | **1.7× slower** | **0.93 (smaller)** |

The insert column reached **1.7×** from 13.5× at v1, through narrow-pointer synthesis and then insert-path optimization (capacity-classed allocations with in-place shifts across leaves, bitmap-branch subarrays and bitmap-leaf value arrays, plus the fused single-walk `JudyLIns`).

**Reproducibility correction:** re-measuring three historical commits back-to-back on a quieter host put the random-insert ratio in a **1.8–2.3 band at every one of them**, including the commit the table's 1.35 was recorded at. The 1.35 reading was distorted by the co-resident VM load of its session — interleaving cancels drift, not cache-pressure asymmetry. Treat the band, not the point, as the working baseline until a quiet-host run. Clustered insert (~1.7–1.8×) and all bytes/key columns reproduce exactly.

Memory cost of the capacity classes is bounded by keeping ≤2-entry allocations exact (random map bytes/key briefly regressed 21→27 with naive rounding; 22.9 after the refinement, vs stock's 24.8). The remaining insert gap is profile-driven follow-up (immediate rebuilds, per-level dispatch).

Honest reading (v1 correctness-first, zero optimization passes yet):

- **Memory is already competitive** — smaller than stock on random keys.
- **Clustered lookup is solved**: narrow-pointer synthesis removed the chain walk (5.31× slower → parity). The remaining lookup gap is on sequential/random — next profile targets: per-level dispatch overhead and the root-leaf/tree split.
- **Insert gap** has three known, documented v1 costs to burn down: full leaf rebuild per insert (no capacity classes), `Vec` materialization in the mutation path, and capi `JudyLIns` walking the tree three times (contains + insert + slot). Each is an isolated follow-up with this table as baseline.

Timing numbers here are working baselines, not publishable claims; headline numbers still require a quiet-host run under the system-load protocol above.

---

## Microarchitecture Scaling: x86-64-v1 vs v2 vs v3 vs v4

Per-tier **instruction** data lives in
[`docs/visualizer_data.json`](visualizer_data.json): deterministic Callgrind
counts for the portable baseline (`x86-64-v1`) and `x86-64-v3` for every
instruction-benchmark routine (v1→v3 deltas span −1.9% to −42.6%, the largest
on `map_remove/random`). No AVX-512 kernel is implemented and no CI job
compiles `x86-64-v4`.

**Wall-clock sweep ([#382](https://github.com/orieg/expanse/issues/382))**
*(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 /
kernel 6.8, run
[33030463060](https://github.com/orieg/expanse/actions/runs/33030463060), ref
`main` @ `5fb03aa3`; `scripts/bench_report.py --extended --arch-sweep`,
**wall-clock**, N = 10,000, 3 interleaved rounds, median, load average 0.02)*:

| Distribution | Target CPU | Lookup (ns) | Lookup (Mops) | Insert (Mops) | vs `baseline` lookup |
|---|---|---:|---:|---:|---:|
| `clustered` | `baseline` | 5.39 | 185.4 | 44.0 | — |
| `clustered` | `x86-64-v2` | 4.89 | 204.4 | 53.3 | **1.10×** |
| `clustered` | `x86-64-v3` | 5.00 | 200.1 | 51.4 | **1.08×** |
| `clustered` | `native` | 4.74 | 210.9 | 53.8 | **1.14×** |
| `random` | `baseline` | 9.96 | 100.4 | 13.3 | — |
| `random` | `x86-64-v2` | 11.43 | 87.5 | 13.7 | 0.87× |
| `random` | `x86-64-v3` | 10.45 | 95.7 | 12.8 | 0.95× |
| `random` | `native` | 11.35 | 88.1 | 10.2 | 0.88× |
| `sequential` | `baseline` | 5.34 | 187.3 | 57.9 | — |
| `sequential` | `x86-64-v2` | 15.74 | 63.5 | 15.8 | 0.34× ⚠️ |
| `sequential` | `x86-64-v3` | 9.42 | 106.1 | 28.1 | 0.57× ⚠️ |
| `sequential` | `native` | 7.72 | 129.5 | 37.5 | 0.69× ⚠️ |

**What this establishes, and what it does not.** The sweep measures
**wall-clock, not instruction counts**; a per-tier instruction claim needs a
Callgrind-under-`target-cpu` harness, which does not exist yet. What it settles
is that **higher ISA tiers do not uniformly help.** Clustered lookups gain
modestly (1.08×–1.14×), random is flat to slightly worse (0.87×–0.95×), and
sequential regresses.

⚠️ **The sequential rows are published as measurement, not as a finding.** A 3×
slowdown from enabling POPCNT (`x86-64-v2`) has no plausible mechanism; at
N = 10,000 the sequential structure is L2-resident with ~5 ns lookups, the
regime where code layout, branch alignment and inlining decisions dominate and
rule 0 says to distrust a wall-clock reading. Read that column as evidence that
this harness is layout-sensitive at small N — a real per-tier claim needs the
deterministic instrument.

---

## Visualizer Benchmark Dataset & CI Synchronization

All 22 deterministic Callgrind instruction benchmarks (`benches/instructions.rs`), memory budget distributions (`examples/bytes_per_key.rs`), and ladder compression thresholds are published in machine-readable format to [`docs/visualizer_data.json`](visualizer_data.json) and rendered in [`docs/architecture_visualizer.html`](architecture_visualizer.html).

### Data Synchronization Protocol
1. **Source of Truth**: The Rust benchmark suite in `crates/expanse/benches/instructions.rs` is canonical.
2. **Deterministic Linux CI**: The `instruction-counts` CI job executes Valgrind/Callgrind on clean `ubuntu-latest` instances with zero wall-clock noise.
3. **Machine-Readable Export**: Benchmark instruction counts, memory overheads, and `x86-64-v3` deltas are mirrored in `docs/visualizer_data.json`.
4. **CI Drift Gate**: `cargo test --test test_visualizer_sync` validates on every pull request that all 22 benchmark routines exist and match between code, JSON, and the HTML visualizer.

---

## Standardized YCSB Workload Suite (`crates/expanse/benches/ycsb.rs`)

The Yahoo! Cloud Serving Benchmark (YCSB) standardizes real-world database engine access patterns across analytical caches, session stores, time-series appends, and read-modify-write transactional workloads.

### 1. Workload Specifications & Key Parameters
- **Population Size**: $N = 100,000$ 64-bit keys.
- **Request Distribution**: Zipfian distribution with skew parameter $\theta = 0.99$ (Gray et al. model), yielding highly skewed access to hot keys.
- **Payload Size**: 128-byte structured byte records with 32-bit hot metadata (modeling modern database MemTables and row storage).

| Workload | Access Pattern & Ratio | Database Subsystem Analogue | Target Operational Profile |
|---|---|---|---|
| **Workload A** | 50% Read, 50% Update | Session Store / Heavy Write Churn | Stresses leaf update in-place shifts and arena slab churn. |
| **Workload B** | 95% Read, 5% Update | Analytical Read Cache with Background Updates | Tests L1/L2 cache residency under optimistic OCC readers. |
| **Workload C** | 100% Read | User Profile / Catalog Point Lookups | Evaluates pure $O(\text{depth})$ trie traversal latency. |
| **Workload D** | 95% Read Latest, 5% Insert | Event Log / User Status Timeline Append | Evaluates append skew at top of key range + monotonic branch extension. |
| **Workload E** | 95% Short Range Scan (10..100), 5% Insert | Threaded Conversations / Secondary Index Scans | Exercises `scan_filtered` predicate pushdown without cold payload dereference. |
| **Workload F** | 50% Read, 50% Read-Modify-Write | User Balance / Counter Mutation | Tests atomic lookup-modify-upsert cycles. |

---

### 2. Measured Comparative Results (`N = 100,000`, `θ = 0.99`, 128B Blobs)

*(Measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, run [33037221608](https://github.com/orieg/expanse/actions/runs/33037221608), ref `main` post-[#385](https://github.com/orieg/expanse/pull/385), load average 0.07; `benches/ycsb.rs`, seed `0x1234_5678_9ABC`, criterion median throughput)*

Throughput (Mops/s) per workload × engine:

| Workload | `ExpanseMap (u64)` | `ExpanseBlobMap (128B)` | `BTreeMap (128B)` | `SkipMap (128B)` (RocksDB) |
|---|---:|---:|---:|---:|
| **A** (50R / 50U) | **20.66** | **20.38** | 4.26 | 1.90 |
| **B** (95R / 5U) | **23.27** | **21.21** | 4.42 | 2.54 |
| **C** (100% Read) | **23.61** | **21.26** | 4.44 | 1.98 |
| **D** (95R-Latest / 5I) | **23.02** | **21.50** | 4.28 | 1.93 |
| **E** (95% Scan / 5I) | 0.830 | 0.657 | **1.289** | 0.305 |
| **F** (50R / 50 RMW) | **18.82** | **14.73** | 4.35 | 1.81 |

> **Workload E is a loss, and the previously published E numbers were
> measuring the wrong thing.** Until [#385](https://github.com/orieg/expanse/pull/385)
> the scan bound was a *key-width* window (`start_k ..= start_k + len*1000`)
> against uniform-random `u64` keys, so the expected number of keys in
> range was ~3e-10: every arm traversed **1.0000 records per scan** against
> a mean requested length of **54.9**, and 0 of 19,054 scans ever reached
> their target length. With scans actually traversing ~55 records,
> `BTreeMap` leads `ExpanseMap` **1.55×**. The earlier 4.33× and 3.08×
> figures — both in Expanse's favour — are withdrawn.
>
> The cause is structural, not a tuning gap. On uniform-random 64-bit keys
> the trie holds **~1.43 records per leaf** (a level-7 subexpanse holds
> ~1.5 keys, a level-8 one ~390, with nothing in between), so a 55-record
> scan costs ~27.6 leaf transitions where a B-tree walks contiguous leaf
> arrays. `LEAF_CAP` is 32 and is not the binding constraint; no
> in-invariant tuning raises occupancy. Scan cost is dominated by the
> body, not the seek: the initial descent is 3 level-steps against ~27.6
> leaf transitions. Dense distributions behave completely differently —
> sequential and clustered keys yield ~322 records/leaf and 0.003 leaf
> transitions per record — so this is a sparse-key result, not a general
> range-scan result.

#### Per-operation latency percentiles *(measured: reference host — Intel i9-12900F, 24 threads, 30 MiB L3, Ubuntu 22.04 / kernel 6.8, commit `43b46f38`; `benches/ycsb.rs` run with `YCSB_LATENCY_REPORT=1`, seed `0x1234_5678_9ABC`, 200,000 recorded ops/workload)*

The criterion groups above time whole op batches (`record_latencies = false`). The opt-in report path (`YCSB_LATENCY_REPORT=1`) instead times each op individually with `record_latencies = true` and emits the percentiles below plus resident memory.

> **Read these as tail-shape, not absolute op cost.** Bracketing every op with `Instant::now()`/`elapsed()` adds a **calibrated ~26 ns/op** on this host (the report prints the figure each run), and that overhead is included in every number below. For the fast engines the true op latency is roughly the column minus ~26 ns — e.g. `ExpanseMap` read-only p50 of 28 ns is a sub-few-ns lookup plus the bracket, consistent with the "sub-15 ns point lookup" criterion figure elsewhere. The overhead is ~constant and additive, so cross-engine ordering and the *tail* percentiles (p99/p99.9), where real work dominates, are the trustworthy signal.

| Workload | Engine | p50 | p95 | p99 | p99.9 | Mem (MB) |
|---|---|---:|---:|---:|---:|---:|
| **A** (50R / 50U) | `ExpanseMap (u64)` | 33 ns | 56 ns | 66 ns | 77 ns | 2.35 |
| | `ExpanseBlobMap (128B)` | 140 ns | **37,755 ns** | **39,205 ns** | 40,788 ns | 18.35 |
| | `BTreeMap (128B)` | 72 ns | 126 ns | 141 ns | 177 ns | 18.31 |
| | `SkipMap (128B)` | 311 ns | 641 ns | 1,148 ns | 2,228 ns | 19.84 |
| **B** (95R / 5U) | `ExpanseMap (u64)` | 28 ns | 51 ns | 62 ns | 74 ns | 2.35 |
| | `ExpanseBlobMap (128B)` | 47 ns | 84 ns | 118 ns | 797 ns | 18.35 |
| | `BTreeMap (128B)` | 58 ns | 104 ns | 124 ns | 155 ns | 18.31 |
| | `SkipMap (128B)` | 209 ns | 413 ns | 594 ns | 1,733 ns | 19.84 |
| **C** (100% Read) | `ExpanseMap (u64)` | 28 ns | 49 ns | 59 ns | 69 ns | 2.35 |
| | `ExpanseBlobMap (128B)` | 41 ns | 78 ns | 94 ns | 132 ns | 16.35 |
| | `BTreeMap (128B)` | 57 ns | 100 ns | 111 ns | 134 ns | 18.31 |
| | `SkipMap (128B)` | 197 ns | 357 ns | 436 ns | 549 ns | 19.84 |
| **D** (95R-Latest / 5I) | `ExpanseMap (u64)` | 27 ns | 50 ns | 61 ns | 125 ns | 2.43 |
| | `ExpanseBlobMap (128B)` | 43 ns | 94 ns | 133 ns | 772 ns | 18.43 |
| | `BTreeMap (128B)` | 70 ns | 104 ns | 116 ns | 148 ns | 20.13 |
| | `SkipMap (128B)` | 193 ns | 359 ns | 448 ns | 1,163 ns | 21.80 |
| **E** (95% Scan / 5I) | `ExpanseMap (u64)` | 50 ns | 83 ns | 101 ns | 127 ns | 2.43 |
| | `ExpanseBlobMap (128B)` | 62 ns | 103 ns | 129 ns | 761 ns | 18.43 |
| | `BTreeMap (128B)` | 82 ns | 122 ns | 135 ns | 169 ns | 20.13 |
| | `SkipMap (128B)` | 234 ns | 406 ns | 498 ns | 1,136 ns | 21.80 |
| **F** (50R / 50 RMW) | `ExpanseMap (u64)` | 37 ns | 65 ns | 80 ns | 99 ns | 2.35 |
| | `ExpanseBlobMap (128B)` | 134 ns | **38,224 ns** | **39,688 ns** | 40,651 ns | 18.35 |
| | `BTreeMap (128B)` | 67 ns | 118 ns | 132 ns | 160 ns | 18.31 |
| | `SkipMap (128B)` | 331 ns | 752 ns | 1,433 ns | 2,129 ns | 19.84 |

**Tail finding — `ExpanseBlobMap` under write-heavy workloads (A, F):** p50 stays low (~140 ns) but p95–p99.9 spike to **~38–41 µs**, ~300× the p50 and ~250× `BTreeMap`'s p99. The spikes are arena slab-chunk allocation / growth stalls on the 50%-write mixes (A, F); the read-mostly and read-latest mixes (B, C, D — ≤5% writes) do not show them (p99.9 ≤ 800 ns). `SkipMap` carries the highest steady-state latency across the board (p50 ~200–330 ns). This tail is the load-bearing signal in this table; the sub-100 ns figures for the fast engines are near the measurement floor described in the caveat above. Mem (MB) is `mem_used()` after the 200k-op stream, so the insert-bearing workloads (D, E) read slightly higher than the read-only steady state.

---

### 3. Concurrent scaling under write churn: `SyncExpanseMap`

Measured in the concurrent-read-scaling section above (run
[33030152085](https://github.com/orieg/expanse/actions/runs/33030152085)). The
bare-metal sweep runs `EXPANSE_BENCH_WORKLOADS="100,50"`, so 95/5 is not a
published cell; the env knob accepts it for local runs.

Under a 50/50 mix `SyncExpanseMap` goes 10.9 M → 3.0 M → 2.0 M read-ops/s
across 1→4→16 threads (**0.19×**): throughput *falls* as threads are added.
Every other single-writer arm behaves the same way (0.12×–0.55×). Writes
serialize on the wrapper mutex and invalidate concurrent readers' snapshots, so
added threads mostly contend and retry. `DashMap` (7.93×) and `SkipMap`
(6.94×) scale in this regime because they admit concurrent writers; the
single-writer OCC design does not, and validation refinement does not change
that. Multi-writer support is the standing follow-up
(`docs/ARCHITECTURE.md` §6).

**Architectural Takeaways (measured, reference host):**
1. **`ExpanseBlobMap` vs RocksDB `SkipMap`**: **~7.6×–11.0× higher throughput** across Workloads B, C, D, F on this host (host- and payload-dependent — the boxed-blob path is heavier for the skiplist here than on the earlier Apple-silicon run).
2. **`ExpanseBlobMap` vs `BTreeMap` on Read-Latest (Workload D)**: **~5.7× higher throughput** (21.04 vs 3.67 Mops/s) — digital-trie appends avoid B-tree page splits.
3. **Pure Word Trie (`ExpanseMap`)**: sustained **>23 Mops/s** on read-heavy workloads (B & C) with a compact ~24.6 B/key footprint.
4. **Range-heavy workloads (E) are a measured loss on sparse keys** — `BTreeMap` leads 1.55×, for the same structural reason as the full-`iter()` gap (`docs/DATABASE.md` §7.1): ~1.43 records per leaf on uniform-random keys means a scan pays a leaf transition roughly every 1.4 records. Dense and clustered key distributions do not share this behaviour (~322 records/leaf).




---

## Corrections record

- **YCSB Workload E (#385).** Published as 4.33×, then 3.08×, in Expanse's favour. Both were measured with a key-width scan bound that made every arm traverse 1 record instead of ~55. With the bound fixed to record count, `BTreeMap` leads 1.55× — E is a loss. The #375 note attributing the 15.26 → 12.70 move to predicate asymmetry was also wrong: both predicates traversed one record, so selectivity could not have changed traversal volume.

Published figures that were withdrawn, and what replaced them. Kept because each
one names a failure mode the methodology rules now guard against.

- **vs-stock lookup ratios** (`judyl_get/random 2.09×`, `judy1_test/random 2.11×`, and the clustered/sequential lookup rows): the arms built their 30k-key array inside the measured region, so "lookup" was a blend dominated by insert. Fixed with `setup =`; checkpoints B0/B2 are the current tables. (rule 0)
- **Checkpoint B0's commit label**: published as `af21e02`, actually measured on the issue #1 items-1-3 branch. Table retained with corrected provenance. (rule 0)
- **"2.1×–3.4× faster range scans"**: retracted; the post-#245 / post-#270 iteration table is the current reading. ([#245](https://github.com/orieg/expanse/pull/245), [#270](https://github.com/orieg/expanse/issues/270))
- **Laptop random-insert 1.35×** (2026-08-18): distorted by a co-resident VM; a quieter host puts the ratio in a 1.8–2.3 band at every historical commit re-measured.
- **Microarchitectural Capability Matrix, the v1–v4 instruction-scaling table, and the derived tier percentages**: retracted in full — cited benchmark arms and populations the harness does not produce, and an `x86-64-v4` column for an unimplemented AVX-512 kernel. Replaced by [`docs/visualizer_data.json`](visualizer_data.json) and the #382 wall-clock sweep. ([#372](https://github.com/orieg/expanse/issues/372))
- **Concurrency word-key arms** (including the 95/5 `SyncExpanseMap` "peak at ~4 threads" table, 19.63 → 35.62 → 28.84 Mops/s): probed unbounded random `u64` keys, so they measured near-100%-miss descent walks. Retracted; every arm has used a bounded keyspace since [#375](https://github.com/orieg/expanse/issues/375).
- **YCSB Workload E**: the figure published under #375 is superseded; the row is pending re-measurement in [#385](https://github.com/orieg/expanse/pull/385).
