//! Standardized YCSB (Yahoo! Cloud Serving Benchmark) suite — **dense clustered keys**.
//!
//! The twin of `ycsb.rs` on the other key shape: runs of 256 consecutive keys
//! at uniform-random bases (the `clustered` class of `benches/compare.rs`).
//! Workload E on uniform-random keys is a measured loss to `BTreeMap`, and the
//! published reading of it is "a sparse-key result, not a general range-scan
//! result"; this harness is the cell that statement needs, for E and for
//! A–D and F alike. Same engines, same operation-stream generator, same
//! runners, same modes — everything lives in `ycsb_common/mod.rs`. A distinct
//! shape gets its own harness file and its own table (AGENTS.md §8.15).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `workload_ycsb_dense` |
//! | `group` | 4 |
//! | `population` | 100k by default; 1M and 10M opt-in through `YCSB_POPULATIONS`, recorded in every bench id and result row |
//! | `insertion_order` | both — every cell is built once sorted ascending and once Fisher–Yates shuffled from the suite PRNG, the order recorded in every bench id and result row; the operation stream is generated from the canonical draw order and is identical across the two |
//! | `probes_and_reuse` | 20,000 ops per criterion iteration; 200,000 ops per cell in the rounds/latency report (`YCSB_OPS` overrides); one deterministic stream per workload, reused across engines, orders and rounds |
//! | `hit_rate` | 100% on reads — every read targets a present key, Zipfian θ = 0.99 over the canonical draw order (workload D: Zipfian over recency, in-run inserts first) |
//! | `miss_gen_method` | n/a — no miss probes; every read key is drawn from the population or from the stream's own earlier inserts |
//! | `value_dereference` | blob arms read byte 0 of the 128 B payload on every read and scanned record; the `ExpanseMap` arm holds `u64` values and has no payload to dereference |
//! | `measured_region` | Op loop only. Criterion routines return the structure so its drop is outside the timed region; the report builds and drops outside the runner's timer. Latency percentiles are window means (64 ops per `Instant` pair), never a per-op bracket |
//! | `arm_symmetry` | One op stream for every arm; key-parity scan predicate (every other key of a dense run, identical across arms); a per-cell work checksum (`consumed`) asserted equal across arms |
//! | `statistics` | Criterion for local iteration; published cells come from per-round samples with BCa 95% intervals and paired per-round ratios (`scripts/ycsb_bench.py`) |
//! | `verdict` | **UNMEASURED (#1005)** `[verified: CODE READ]`: added so workload E has a dense cell beside the sparse one; no figure from it is published yet. |

#[path = "ycsb_common/mod.rs"]
mod ycsb_common;

use criterion::{Criterion, criterion_group};
use ycsb_common::{KeyShape, Suite};

/// This harness file's suite: the id above, on dense clustered keys.
const SUITE: Suite = Suite {
    id: "workload_ycsb_dense",
    shape: KeyShape::DenseClustered,
};

fn bench_ycsb_dense(c: &mut Criterion) {
    ycsb_common::bench_ycsb_workloads(c, &SUITE);
}

criterion_group!(benches, bench_ycsb_dense);

fn main() {
    ycsb_common::harness_entry(&SUITE, benches);
}
