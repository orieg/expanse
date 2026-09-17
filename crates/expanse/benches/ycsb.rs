//! Standardized YCSB (Yahoo! Cloud Serving Benchmark) suite — **uniform-random keys**.
//!
//! Evaluates Workloads A through F against `ExpanseMap`, `ExpanseBlobMap`,
//! `std::collections::BTreeMap` and `crossbeam_skiplist::SkipMap` (the RocksDB
//! in-memory MemTable model). The generators, operation streams, engine
//! runners, criterion groups and the rounds/latency report live in
//! `ycsb_common/mod.rs`, shared with the dense-key twin `ycsb_dense.rs`; this
//! file declares the shape it measures and nothing else (AGENTS.md §8.15).
//!
//! Modes (see `ycsb_common/mod.rs` for the environment variables):
//! - default: criterion groups, 20,000 ops per iteration.
//! - `YCSB_ROUNDS_JSON=1`: one round, one JSON object per cell, 200,000 ops per
//!   cell — the input of `scripts/ycsb_bench.py`, which owns rounds and intervals.
//! - `YCSB_LATENCY_REPORT=1`: the same cells as a table.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `workload_ycsb` |
//! | `group` | 4 |
//! | `population` | 100k by default; 1M and 10M opt-in through `YCSB_POPULATIONS`, recorded in every bench id and result row |
//! | `insertion_order` | both — every cell is built once sorted ascending and once Fisher–Yates shuffled from the suite PRNG, the order recorded in every bench id and result row; the operation stream is generated from the canonical draw order and is identical across the two |
//! | `probes_and_reuse` | 20,000 ops per criterion iteration; 200,000 ops per cell in the rounds/latency report (`YCSB_OPS` overrides); one deterministic stream per workload, reused across engines, orders and rounds |
//! | `hit_rate` | 100% on reads — every read targets a present key, Zipfian θ = 0.99 over the canonical draw order (workload D: Zipfian over recency, in-run inserts first) |
//! | `miss_gen_method` | n/a — no miss probes; every read key is drawn from the population or from the stream's own earlier inserts |
//! | `value_dereference` | blob arms read byte 0 of the 128 B payload on every read and scanned record; the `ExpanseMap` arm holds `u64` values and has no payload to dereference |
//! | `measured_region` | Op loop only. Criterion routines return the structure so its drop is outside the timed region; the report builds and drops outside the runner's timer. Latency percentiles are window means (64 ops per `Instant` pair), never a per-op bracket |
//! | `arm_symmetry` | One op stream for every arm; key-parity scan predicate (selectivity identical by construction); a per-cell work checksum (`consumed`) asserted equal across arms |
//! | `statistics` | Criterion for local iteration; published cells come from per-round samples with BCa 95% intervals and paired per-round ratios (`scripts/ycsb_bench.py`) |
//! | `verdict` | **RE-MEASURE PENDING (#1005)** `[verified: CODE READ]`: until #1005 the `ExpanseBlobMap` arm discarded a `MetaOverflow` on 255 of every 256 writes in workloads A, B, D and E, the criterion routines dropped the structure inside the timed region, the latency report bracketed every op, and workload D never read a key the run had inserted. |

#[path = "ycsb_common/mod.rs"]
mod ycsb_common;

use criterion::{Criterion, criterion_group};
use ycsb_common::{KeyShape, Suite};

/// This harness file's suite: the id above, on uniform-random keys.
const SUITE: Suite = Suite {
    id: "workload_ycsb",
    shape: KeyShape::UniformRandom,
};

fn bench_ycsb_uniform(c: &mut Criterion) {
    ycsb_common::bench_ycsb_workloads(c, &SUITE);
}

// The former `bench_ycsb_concurrency` criterion group was removed (#375).
// Thread-scaling throughput for the sync structures is owned by the
// `/benchmark concurrency` suite (`benches/concurrency.rs`).
criterion_group!(benches, bench_ycsb_uniform);

fn main() {
    ycsb_common::harness_entry(&SUITE, benches);
}
