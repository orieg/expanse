//! Tests of the concurrent YCSB harness (METHODOLOGY §20.12 item 2, §20.5, §20.15).
//!
//! These include `benches/ycsb_concurrent_common/mod.rs`, the module the
//! harness (`examples/ycsb_concurrent.rs`) runs, so the stream generator, the
//! timed loop and the oracle under test are the harness's own code.

#![cfg(not(miri))]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

#[path = "../benches/ycsb_common/mod.rs"]
mod ycsb_common;
#[path = "../benches/ycsb_concurrent_common/mod.rs"]
mod ycsb_concurrent_common;

use ycsb_common::ZIPFIAN_THETA;
use ycsb_concurrent_common::{
    Arm, ArmStore, DEFAULT_SUITE_SEED, Expected, Family, LOG2X16_BUCKETS, Log2x16Histogram,
    LoopTally, OlcStore, Op, RANK_K, STANDARD_OPS_PER_THREAD, STANDARD_POPULATION_N, encode_write,
    generate_initial_population, generate_thread_stream, oracle_verdict, run_cell,
    run_cell_monotonicity, run_monotonicity_pass, run_store, run_window, tally_expected,
    verify_oracle,
};

/// Share of draws on the k lowest ranks under Gray's closed form
/// (`scripts/ycsb_concurrent_bounds.py::gray_top_k_share`, §20.7 (a)).
fn gray_top_k_share(k: usize, n: usize, theta: f64) -> f64 {
    assert!(n >= 3, "closed form needs n >= 3");
    if k == 0 {
        return 0.0;
    }
    let zeta_n: f64 = (1..=n).map(|i| (i as f64).powf(-theta)).sum();
    if k == 1 {
        return 1.0 / zeta_n;
    }
    let zeta_2: f64 = (1..=2).map(|i| (i as f64).powf(-theta)).sum();
    let eta = (1.0 - (2.0 / n as f64).powf(1.0 - theta)) / (1.0 - zeta_2 / zeta_n);
    1.0 - (1.0 - (k as f64 / n as f64).powf(1.0 - theta)) / eta
}

/// Streams for `threads` threads of `family`, with the population they address.
fn streams_for(
    family: Family,
    threads: usize,
    n: usize,
    ops: usize,
) -> (Vec<u64>, Vec<u64>, Vec<Vec<Op>>) {
    let keys = generate_initial_population(n);
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    let streams = (0..threads)
        .map(|t| {
            generate_thread_stream(
                family,
                t,
                threads,
                ops,
                n,
                &keys,
                &sorted,
                DEFAULT_SUITE_SEED,
            )
            .0
        })
        .collect();
    (keys, sorted, streams)
}

/// The harness's stream generator, at the registered N = 2^20, stream length
/// 2^20 and k set {1, 2, 16, 256, 4,096}, held to `gray_top_k_share` within a
/// stated binomial tolerance of 4.5 σ (§20.12 item 2).
#[test]
fn harness_stream_rank_histogram_matches_gray_top_k_share_at_registered_n() {
    let n = STANDARD_POPULATION_N;
    let keys = generate_initial_population(n);
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    let (stream, hist) = generate_thread_stream(
        Family::A,
        0,
        8,
        STANDARD_OPS_PER_THREAD,
        n,
        &keys,
        &sorted,
        DEFAULT_SUITE_SEED,
    );
    assert_eq!(stream.len(), STANDARD_OPS_PER_THREAD);
    assert_eq!(hist.draws, STANDARD_OPS_PER_THREAD as u64);
    assert_eq!(RANK_K, [1, 2, 16, 256, 4_096]);

    // §20.7 (a)'s table, so the Rust closed form cannot drift from the bounds module.
    let pinned = [0.064740, 0.097336, 0.232078, 0.416150, 0.605396];
    for ((&k, observed), want) in RANK_K.iter().zip(hist.shares()).zip(pinned) {
        let expected = gray_top_k_share(k as usize, n, ZIPFIAN_THETA);
        assert!(
            (expected - want).abs() < 1e-6,
            "gray_top_k_share({k}) = {expected}, §20.7 says {want}"
        );
        let sigma = (expected * (1.0 - expected) / hist.draws as f64).sqrt();
        assert!(
            (observed - expected).abs() <= 4.5 * sigma,
            "rank histogram at k={k}: observed {observed:.6}, expected {expected:.6}, sigma {sigma:.6}"
        );
    }

    // The histogram describes the stream it came with: rank 0 is the first key
    // in generator draw order, so its share of the stream's operations is the
    // observed share at k = 1.
    let on_rank0 = stream
        .iter()
        .filter(|op| matches!(op, Op::Read { key } | Op::Update { key, .. } if *key == keys[0]))
        .count() as u64;
    assert_eq!(on_rank0, hist.at_or_below[0]);
}

/// Ac and Fc map rank r to the r-th smallest key; A maps it to the r-th key in
/// draw order (§20.4). Reduced population (4,096), which the mapping does not depend on.
#[test]
fn contiguous_families_map_ranks_to_sorted_keys_reduced_4096() {
    let n = 4_096;
    let keys = generate_initial_population(n);
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_ne!(keys[0], sorted[0], "the two layouts must differ at rank 0");

    for (family, rank0) in [
        (Family::A, keys[0]),
        (Family::Ac, sorted[0]),
        (Family::Fc, sorted[0]),
    ] {
        let (stream, hist) =
            generate_thread_stream(family, 0, 1, 8_192, n, &keys, &sorted, DEFAULT_SUITE_SEED);
        let on_rank0 = stream
            .iter()
            .filter(|op| match op {
                Op::Read { key } | Op::Update { key, .. } | Op::ReadModifyWrite { key } => {
                    *key == rank0
                }
                Op::Insert { .. } => false,
            })
            .count() as u64;
        assert_eq!(on_rank0, hist.at_or_below[0], "{family:?}");
    }
}

/// G5 negative control (§20.5): the harness's own `olc` store and operation
/// loop with the striped lock skipped MUST lose updates at T = 8, at the
/// registered N = 2^20 and stream length 2^20. Asserted on the oracle's
/// diagnostic string, not on an exit code (AGENTS.md §5).
#[test]
fn g5_negative_control_harness_olc_without_stripe_lock_loses_updates_at_t8() {
    let (_keys, sorted, streams) =
        streams_for(Family::F, 8, STANDARD_POPULATION_N, STANDARD_OPS_PER_THREAD);
    let expected = tally_expected(&streams);
    assert!(expected.rmw_ops > 0);
    let out = run_store(
        OlcStore::prefilled_without_stripe_lock(&sorted),
        Family::F,
        &sorted,
        streams,
        &expected,
    );
    assert!(
        out.verdict.starts_with("VOID_LOST_UPDATE"),
        "negative control MUST read VOID_LOST_UPDATE, got {}",
        out.verdict
    );
    assert!(out.oracle.lost_updates > 0 && out.oracle.per_key_mismatches > 0);
}

/// G5 positive control: the same cell through the harness's production path
/// (`run_cell`, striped lock held) loses nothing, in every arm.
#[test]
fn g5_holds_in_every_arm_through_the_harness_path_at_t8() {
    for arm in [Arm::Olc, Arm::Mutex, Arm::Skip, Arm::Dash, Arm::RwBTree] {
        // `olc` at the registered size, beside its negative control; the other
        // arms at a reduced 2^16 keys and 2^16 operations per thread.
        let (n, ops) = if arm == Arm::Olc {
            (STANDARD_POPULATION_N, STANDARD_OPS_PER_THREAD)
        } else {
            (1 << 16, 1 << 16)
        };
        let (_keys, sorted, streams) = streams_for(Family::F, 8, n, ops);
        let expected = tally_expected(&streams);
        let out = run_cell(arm, Family::F, &sorted, streams, &expected);
        assert_eq!(out.verdict, "PASS", "{arm:?}");
        assert_eq!(out.oracle.value_sum, expected.rmw_ops, "{arm:?}");
        assert_eq!(out.oracle.per_key_mismatches, 0, "{arm:?}");
    }
}

/// Every arm passes the oracle in every family shape (reduced: 4,096 keys,
/// 8,192 operations per thread, T = 4), with a 100% hit rate.
#[test]
fn every_arm_and_family_passes_the_oracle_reduced_4096() {
    for family in [
        Family::A,
        Family::B,
        Family::D,
        Family::F,
        Family::A0,
        Family::C,
        Family::C0,
        Family::Ac,
        Family::Fc,
    ] {
        for arm in [Arm::Olc, Arm::Mutex, Arm::Skip, Arm::Dash, Arm::RwBTree] {
            let (_keys, sorted, streams) = streams_for(family, 4, 4_096, 8_192);
            let expected = tally_expected(&streams);
            let out = run_cell(arm, family, &sorted, streams, &expected);
            assert_eq!(out.verdict, "PASS", "{family:?} {arm:?}");
            assert_eq!(out.window.tally.read_misses, 0, "{family:?} {arm:?}");
            assert_eq!(out.window.tally.write_misses, 0, "{family:?} {arm:?}");
            assert_eq!(
                out.oracle.final_count,
                4_096 + expected.insert_ops,
                "{family:?} {arm:?}"
            );
            assert_eq!(out.window.thread_elapsed_s.len(), 4);
        }
    }
}

/// A store whose contents the tests can corrupt, and whose per-thread handle
/// records the thread it is dropped on.
struct FakeStore {
    map: Mutex<HashMap<u64, u64>>,
    handle_drops: Arc<Mutex<Vec<ThreadId>>>,
}

struct DropProbe(Arc<Mutex<Vec<ThreadId>>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.lock().unwrap().push(std::thread::current().id());
    }
}

impl FakeStore {
    fn prefilled(keys: &[u64]) -> Self {
        Self {
            map: Mutex::new(keys.iter().map(|&k| (k, 0)).collect()),
            handle_drops: Arc::default(),
        }
    }
}

impl ArmStore for FakeStore {
    type Handle = DropProbe;

    fn handle(this: &Arc<Self>) -> DropProbe {
        DropProbe(Arc::clone(&this.handle_drops))
    }
    fn read(&self, _h: &DropProbe, key: u64) -> Option<u64> {
        self.map.lock().unwrap().get(&key).copied()
    }
    fn update(&self, _h: &DropProbe, key: u64, val: u64) -> Option<u64> {
        self.map
            .lock()
            .unwrap()
            .get_mut(&key)
            .map(|v| std::mem::replace(v, val))
    }
    fn insert(&self, _h: &DropProbe, key: u64, val: u64) -> Option<u64> {
        self.map.lock().unwrap().insert(key, val)
    }
    fn rmw(&self, _h: &DropProbe, key: u64) -> Option<u64> {
        self.map.lock().unwrap().get_mut(&key).map(|v| {
            *v += 1;
            *v - 1
        })
    }
    fn final_value(&self, key: u64) -> Option<u64> {
        self.map.lock().unwrap().get(&key).copied()
    }
    fn final_len(&self) -> u64 {
        self.map.lock().unwrap().len() as u64
    }
}

/// A two-thread family-A history over keys {10, 20, 30}: thread 0 writes key 10
/// twice, thread 1 writes it once, nobody writes 30.
fn small_history() -> (Vec<u64>, Vec<Vec<Op>>, [u64; 3]) {
    let (w0_early, w0_last, w1_last) = (encode_write(0, 0), encode_write(0, 5), encode_write(1, 2));
    let streams = vec![
        vec![
            Op::Update {
                key: 10,
                val: w0_early,
            },
            Op::Read { key: 20 },
            Op::Update {
                key: 10,
                val: w0_last,
            },
        ],
        vec![
            Op::Update {
                key: 10,
                val: w1_last,
            },
            Op::Read { key: 30 },
        ],
    ];
    (vec![10, 20, 30], streams, [w0_early, w0_last, w1_last])
}

fn verdict_with(
    store: &FakeStore,
    family: Family,
    pop: &[u64],
    exp: &Expected,
    tally: &LoopTally,
) -> String {
    let rep = verify_oracle(store, family, pop, exp, tally.successful_inserts);
    oracle_verdict(family, &rep, tally, exp)
}

/// §20.15, one negative control per clause of the final-value check.
#[test]
fn oracle_accepts_only_a_per_thread_last_write() {
    let (pop, streams, [w0_early, w0_last, w1_last]) = small_history();
    let exp = tally_expected(&streams);
    assert_eq!(
        exp.last_writes[&10].len(),
        2,
        "one candidate per writing thread"
    );
    let tally = LoopTally::default();

    for last in [w0_last, w1_last] {
        let store = FakeStore::prefilled(&pop);
        store.map.lock().unwrap().insert(10, last);
        assert_eq!(verdict_with(&store, Family::A, &pop, &exp, &tally), "PASS");
    }

    // Clause: a write a thread made and later overwrote is not a candidate.
    let store = FakeStore::prefilled(&pop);
    store.map.lock().unwrap().insert(10, w0_early);
    let v = verdict_with(&store, Family::A, &pop, &exp, &tally);
    assert!(
        v.starts_with("VOID_ORACLE") && v.contains("1 per-key mismatches"),
        "{v}"
    );

    // Clause: the prefill value 0 is not accepted for a key a stream wrote.
    let store = FakeStore::prefilled(&pop);
    let v = verdict_with(&store, Family::A, &pop, &exp, &tally);
    assert!(
        v.starts_with("VOID_ORACLE") && v.contains("1 per-key mismatches"),
        "{v}"
    );

    // Clause: a key no stream wrote must still hold the prefill.
    let store = FakeStore::prefilled(&pop);
    store.map.lock().unwrap().insert(10, w0_last);
    store.map.lock().unwrap().insert(30, w1_last);
    let v = verdict_with(&store, Family::A, &pop, &exp, &tally);
    assert!(
        v.starts_with("VOID_ORACLE") && v.contains("1 per-key mismatches"),
        "{v}"
    );
}

#[test]
fn oracle_checks_presence_count_inserts_and_hit_rate() {
    let (pop, streams, [_, w0_last, _]) = small_history();
    let exp = tally_expected(&streams);
    let good = || {
        let s = FakeStore::prefilled(&pop);
        s.map.lock().unwrap().insert(10, w0_last);
        s
    };
    let tally = LoopTally::default();
    assert_eq!(verdict_with(&good(), Family::A, &pop, &exp, &tally), "PASS");

    // Clause: every population key present.
    let store = good();
    store.map.lock().unwrap().remove(&20);
    let v = verdict_with(&store, Family::A, &pop, &exp, &tally);
    assert!(v.contains("1 population keys absent"), "{v}");

    // Clause: final count == N + successful inserts.
    let store = good();
    store.map.lock().unwrap().insert(99, 1);
    let v = verdict_with(&store, Family::A, &pop, &exp, &tally);
    assert!(
        v.contains("final count 4 against population plus successful inserts 3"),
        "{v}"
    );

    // Clause: the registered 100% hit rate — a read miss voids.
    let missed = LoopTally {
        read_misses: 1,
        ..LoopTally::default()
    };
    let v = verdict_with(&good(), Family::A, &pop, &exp, &missed);
    assert!(v.contains("1 read misses"), "{v}");
    let missed = LoopTally {
        write_misses: 2,
        ..LoopTally::default()
    };
    let v = verdict_with(&good(), Family::A, &pop, &exp, &missed);
    assert!(v.contains("2 write misses"), "{v}");

    // Clause: C and C0 check values too — a read-only cell must leave the prefill.
    let c_streams = vec![vec![Op::Read { key: 10 }]];
    let c_exp = tally_expected(&c_streams);
    let store = FakeStore::prefilled(&pop);
    assert_eq!(
        verdict_with(&store, Family::C, &pop, &c_exp, &tally),
        "PASS"
    );
    store.map.lock().unwrap().insert(20, 7);
    let v = verdict_with(&store, Family::C, &pop, &c_exp, &tally);
    assert!(v.starts_with("VOID_ORACLE"), "{v}");

    // Clause: every D insert present with exactly its own value, and counted.
    let d_val = encode_write(0, 0);
    let d_streams = vec![vec![Op::Insert {
        key: 500,
        val: d_val,
    }]];
    let d_exp = tally_expected(&d_streams);
    let one_insert = LoopTally {
        successful_inserts: 1,
        ..LoopTally::default()
    };
    let store = FakeStore::prefilled(&pop);
    store.map.lock().unwrap().insert(500, d_val);
    assert_eq!(
        verdict_with(&store, Family::D, &pop, &d_exp, &one_insert),
        "PASS"
    );
    store.map.lock().unwrap().insert(500, d_val + 1);
    let v = verdict_with(&store, Family::D, &pop, &d_exp, &one_insert);
    assert!(v.contains("1 per-key mismatches"), "{v}");
    let store = FakeStore::prefilled(&pop);
    store.map.lock().unwrap().insert(500, d_val);
    let v = verdict_with(&store, Family::D, &pop, &d_exp, &tally);
    assert!(v.contains("0 successful inserts of 1"), "{v}");
}

/// G5's two halves on a corruptible store: the sum and the per-key count.
#[test]
fn oracle_rmw_checks_the_sum_and_each_key() {
    let pop = vec![10, 20];
    let streams = vec![
        vec![
            Op::ReadModifyWrite { key: 10 },
            Op::ReadModifyWrite { key: 10 },
        ],
        vec![Op::ReadModifyWrite { key: 20 }],
    ];
    let exp = tally_expected(&streams);
    let tally = LoopTally::default();
    let store = FakeStore::prefilled(&pop);
    store.map.lock().unwrap().extend([(10, 2), (20, 1)]);
    assert_eq!(verdict_with(&store, Family::F, &pop, &exp, &tally), "PASS");

    // A lost update: the sum falls short.
    store.map.lock().unwrap().insert(10, 1);
    let v = verdict_with(&store, Family::F, &pop, &exp, &tally);
    assert!(
        v.starts_with("VOID_LOST_UPDATE") && v.contains("(1 lost)"),
        "{v}"
    );

    // A misplaced update: the sum is right and two keys are wrong.
    store.map.lock().unwrap().extend([(10, 1), (20, 2)]);
    let v = verdict_with(&store, Family::F, &pop, &exp, &tally);
    assert!(
        v.starts_with("VOID_LOST_UPDATE") && v.contains("(0 lost), 2 per-key"),
        "{v}"
    );
}

/// AGENTS.md §8.6: a thread's reader handle leaves through the join value, so
/// its deregistration happens on the joining thread after the window closes
/// and never inside a worker's timer.
#[test]
fn per_thread_handles_drop_on_the_joining_thread_after_the_window() {
    let (pop, streams, _) = small_history();
    let store = Arc::new(FakeStore::prefilled(&pop));
    let out = run_window(&store, streams);
    assert_eq!(out.thread_elapsed_s.len(), 2);
    let drops = store.handle_drops.lock().unwrap();
    assert_eq!(drops.len(), 2);
    let me = std::thread::current().id();
    assert!(
        drops.iter().all(|&id| id == me),
        "a handle was dropped on a worker thread, inside its timed region"
    );
}

/// Unit test for Log2x16Histogram: geometry, linear & log-linear regions,
/// invertibility across all 976 buckets, percentiles, and merge associativity (§20.14 (a)).
#[test]
fn test_log2x16_histogram_geometry_and_percentiles() {
    assert_eq!(LOG2X16_BUCKETS, 976);

    // Linear region: values < 16 map directly to their value.
    for v in 0..16 {
        assert_eq!(Log2x16Histogram::bucket_index(v), v as usize);
        let (lo, mid, hi) = Log2x16Histogram::bucket_range(v as usize);
        assert_eq!(lo, v);
        assert_eq!(mid, v);
        assert_eq!(hi, v + 1);
    }

    // Log-linear boundary checks:
    assert_eq!(Log2x16Histogram::bucket_index(16), 16);
    assert_eq!(Log2x16Histogram::bucket_index(31), 31);
    assert_eq!(Log2x16Histogram::bucket_index(32), 32);
    assert_eq!(Log2x16Histogram::bucket_index(47), 39);
    assert_eq!(Log2x16Histogram::bucket_index(48), 40);
    assert_eq!(Log2x16Histogram::bucket_index(63), 47);
    assert_eq!(Log2x16Histogram::bucket_index(64), 48);

    // Maximum bucket check:
    assert_eq!(
        Log2x16Histogram::bucket_index(u64::MAX),
        LOG2X16_BUCKETS - 1
    );

    // Invertibility and monotonic coverage across all buckets:
    for idx in 0..LOG2X16_BUCKETS {
        let (lo, mid, hi) = Log2x16Histogram::bucket_range(idx);
        assert!(lo <= mid, "idx {idx}: lo {lo} <= mid {mid}");
        assert!(
            mid < hi || (idx == LOG2X16_BUCKETS - 1 && hi == u64::MAX),
            "idx {idx}: mid {mid} < hi {hi}"
        );
        assert_eq!(Log2x16Histogram::bucket_index(lo), idx, "lo at idx {idx}");
        assert_eq!(Log2x16Histogram::bucket_index(mid), idx, "mid at idx {idx}");
        if hi < u64::MAX {
            assert_eq!(
                Log2x16Histogram::bucket_index(hi - 1),
                idx,
                "hi - 1 at idx {idx}"
            );
            if idx + 1 < LOG2X16_BUCKETS {
                assert_eq!(
                    Log2x16Histogram::bucket_index(hi),
                    idx + 1,
                    "hi at idx {idx}"
                );
            }
        }
    }

    // Recording and percentile accuracy
    let mut hist = Log2x16Histogram::new();
    for v in 1..=1000 {
        hist.record(v);
    }
    assert_eq!(hist.count, 1000);
    assert_eq!(hist.min_val, 1);
    assert_eq!(hist.max_val, 1000);

    let p50 = hist.percentile_cycles(0.50);
    let p99 = hist.percentile_cycles(0.99);
    let p999 = hist.percentile_cycles(0.999);
    assert!((p50 as i64 - 500).abs() <= 16, "p50={p50}");
    assert!((p99 as i64 - 990).abs() <= 32, "p99={p99}");
    assert!((p999 as i64 - 999).abs() <= 32, "p999={p999}");

    // Merge associativity
    let mut h1 = Log2x16Histogram::new();
    let mut h2 = Log2x16Histogram::new();
    for v in 1..=500 {
        h1.record(v);
    }
    for v in 501..=1000 {
        h2.record(v);
    }
    h1.merge(&h2);
    assert_eq!(h1.count, 1000);
    assert_eq!(h1.min_val, 1);
    assert_eq!(h1.max_val, 1000);
    assert_eq!(h1.buckets, hist.buckets);
}

/// Untimed monotonicity pass & node census test (§20.15).
/// Verifies 0 monotonicity violations and that node_bytes.total() == mem_used.
#[test]
fn test_monotonicity_pass_and_node_census() {
    let n = 4_096;
    let ops = 4_096;
    for arm in [Arm::Olc, Arm::Mutex] {
        let (keys, sorted, streams) = streams_for(Family::A, 8, n, ops);
        let expected = tally_expected(&streams);

        let out = run_cell_monotonicity(arm, Family::A, &keys, &sorted, streams, &expected);

        assert_eq!(out.monotonicity_violations, 0, "{arm:?}");
        assert_eq!(out.tracked_keys, 4096, "{arm:?}");
        assert_eq!(out.verdict, "PASS", "{arm:?}");
        assert!(out.node_census.is_some(), "{arm:?}");

        let census = out.node_census.as_ref().unwrap();
        assert_eq!(census.node_bytes.total(), out.mem_used.unwrap(), "{arm:?}");
        assert!(
            (census.node_counts.leaf_linear + census.node_counts.leaf_bitmap) > 0,
            "{arm:?}"
        );
    }
}

/// Test store that can inject sequence regressions to test monotonicity violation detection.
struct RegressingStore {
    key: u64,
    reads: AtomicU64,
    regress: bool,
}

impl RegressingStore {
    fn new(key: u64, regress: bool) -> Self {
        Self {
            key,
            reads: AtomicU64::new(0),
            regress,
        }
    }
}

impl ArmStore for RegressingStore {
    type Handle = ();

    fn handle(_this: &Arc<Self>) -> Self::Handle {}
    fn read(&self, _h: &Self::Handle, key: u64) -> Option<u64> {
        if key == self.key {
            let r = self.reads.fetch_add(1, Ordering::SeqCst);
            if self.regress {
                // Inverted sequence: first read observes seq 10, second read observes seq 5.
                if r == 0 {
                    Some(encode_write(0, 10))
                } else {
                    Some(encode_write(0, 5))
                }
            } else {
                // Monotonic sequence: first read observes seq 5, second read observes seq 10.
                if r == 0 {
                    Some(encode_write(0, 5))
                } else {
                    Some(encode_write(0, 10))
                }
            }
        } else {
            Some(0)
        }
    }
    fn update(&self, _h: &Self::Handle, _key: u64, _val: u64) -> Option<u64> {
        Some(0)
    }
    fn insert(&self, _h: &Self::Handle, _key: u64, _val: u64) -> Option<u64> {
        None
    }
    fn rmw(&self, _h: &Self::Handle, _key: u64) -> Option<u64> {
        Some(0)
    }
    fn final_value(&self, _key: u64) -> Option<u64> {
        Some(0)
    }
    fn final_len(&self) -> u64 {
        1
    }
}

/// Mutation test (AGENTS.md §2.3): demonstrates that `run_monotonicity_pass`
/// detects non-monotonic sequence reads from a writer and transitions the verdict
/// to VOID_ORACLE when broken intentionally.
#[test]
fn test_monotonicity_mutation_discriminates_backward_sequences() {
    let key = 100u64;
    let pop = vec![key];
    let sorted = vec![key];
    // One reader thread reading `key` twice.
    let streams = vec![vec![Op::Read { key }, Op::Read { key }]];
    let exp = tally_expected(&streams);

    // Positive case: monotonic sequence (seq 5 then seq 10).
    let store_ok = Arc::new(RegressingStore::new(key, false));
    let out_ok = run_monotonicity_pass(&store_ok, Family::C, &pop, &sorted, streams.clone(), &exp);
    assert_eq!(out_ok.monotonicity_violations, 0);
    assert_eq!(out_ok.verdict, "PASS");

    // Mutation (AGENTS.md §2.3): backward sequence (seq 10 then seq 5).
    // The test MUST fail the monotonicity invariant and produce VOID_ORACLE.
    let store_mutated = Arc::new(RegressingStore::new(key, true));
    let out_mutated =
        run_monotonicity_pass(&store_mutated, Family::C, &pop, &sorted, streams, &exp);
    assert_eq!(out_mutated.monotonicity_violations, 1);
    assert!(
        out_mutated.verdict.starts_with("VOID_ORACLE")
            && out_mutated.verdict.contains("1 monotonicity violations"),
        "verdict must be VOID_ORACLE with violations, got: {}",
        out_mutated.verdict
    );
}
