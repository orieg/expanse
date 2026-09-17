//! Unit and integration tests for YCSB benchmark generation and workload execution.
#![cfg(not(miri))]

use crossbeam_skiplist::SkipMap;
use expanse_trie::blobmap::ExpanseBlobMap;
use expanse_trie::map::ExpanseMap;
use std::collections::BTreeMap;
use std::time::Duration;

#[path = "../benches/ycsb_common/mod.rs"]
mod ycsb;

use ycsb::{
    BLOB_META_MASK, BLOB_PAYLOAD_SIZE, CLUSTER_RUN, Engine, INSERT_SEQ_BASE, InsertionOrder,
    KeyShape, LatencyStats, POPULATION_N, Sampling, Workload, XorShift64, YcsbOp, ZipfianGenerator,
    build_order, generate_initial_keys, generate_keys, generate_operations, generate_payload,
    parse_population, run_cell, run_concurrent_ycsb, run_workload_btreemap,
    run_workload_expanse_blobmap, run_workload_expanse_map, run_workload_skipmap,
};

#[test]
fn test_zipfian_generator_bounds_and_skew() {
    let n = 100_000u64;
    let theta = 0.99;
    let zipf = ZipfianGenerator::new(n, theta);
    let mut rng = XorShift64::new(0xABCD_EF01_2345_6789);

    let num_samples = 100_000;
    let mut counts = vec![0u32; 100]; // Track top 100 items
    let mut max_val = 0u64;

    for _ in 0..num_samples {
        let u = rng.next_f64();
        let val = zipf.next(u);
        assert!(val < n, "Generated value {val} exceeds n ({n})");
        if val > max_val {
            max_val = val;
        }
        if (val as usize) < counts.len() {
            counts[val as usize] += 1;
        }
    }

    // Rank 0 should be the most frequent element
    assert!(
        counts[0] > counts[1],
        "Rank 0 count ({}) should exceed Rank 1 count ({})",
        counts[0],
        counts[1]
    );
    assert!(
        counts[1] > counts[10],
        "Rank 1 count ({}) should exceed Rank 10 count ({})",
        counts[1],
        counts[10]
    );
    assert!(
        counts[0] > 1000,
        "Rank 0 should have substantial concentration under theta=0.99 (got {})",
        counts[0]
    );
}

#[test]
fn test_ycsb_workload_generation_ratios() {
    let initial_keys = generate_initial_keys(10_000);
    let op_count = 10_000;

    for &wl in &[
        Workload::A,
        Workload::B,
        Workload::C,
        Workload::D,
        Workload::E,
        Workload::F,
    ] {
        let ops = generate_operations(wl, &initial_keys, op_count, 0x5CA1_AB1E);
        assert_eq!(ops.len(), op_count);

        let mut reads = 0;
        let mut updates = 0;
        let mut inserts = 0;
        let mut scans = 0;
        let mut rmws = 0;

        for op in &ops {
            match op {
                YcsbOp::Read(_) => reads += 1,
                YcsbOp::Update(_, _) => updates += 1,
                YcsbOp::Insert(_, _) => inserts += 1,
                YcsbOp::Scan(_, _) => scans += 1,
                YcsbOp::ReadModifyWrite(_) => rmws += 1,
            }
        }

        match wl {
            Workload::A => {
                // 50% Read, 50% Update (approx +-5%)
                assert!((4500..=5500).contains(&reads), "Workload A reads: {reads}");
                assert!(
                    (4500..=5500).contains(&updates),
                    "Workload A updates: {updates}"
                );
            }
            Workload::B => {
                // 95% Read, 5% Update
                assert!((9300..=9700).contains(&reads), "Workload B reads: {reads}");
                assert!(
                    (300..=700).contains(&updates),
                    "Workload B updates: {updates}"
                );
            }
            Workload::C => {
                // 100% Read
                assert_eq!(reads, op_count, "Workload C must be 100% reads");
            }
            Workload::D => {
                // 95% Read, 5% Insert
                assert!((9300..=9700).contains(&reads), "Workload D reads: {reads}");
                assert!(
                    (300..=700).contains(&inserts),
                    "Workload D inserts: {inserts}"
                );
            }
            Workload::E => {
                // 95% Scan, 5% Insert
                assert!((9300..=9700).contains(&scans), "Workload E scans: {scans}");
                assert!(
                    (300..=700).contains(&inserts),
                    "Workload E inserts: {inserts}"
                );
            }
            Workload::F => {
                // 50% Read, 50% Read-Modify-Write
                assert!((4500..=5500).contains(&reads), "Workload F reads: {reads}");
                assert!((4500..=5500).contains(&rmws), "Workload F rmws: {rmws}");
            }
        }
    }
}

#[test]
fn test_ycsb_execution_across_all_targets() {
    let initial_keys = generate_initial_keys(1000);
    let payload = generate_payload(0x1234_5678);
    assert_eq!(payload.len(), BLOB_PAYLOAD_SIZE);

    let ops = generate_operations(Workload::A, &initial_keys, 500, 0x9999);

    // 1. ExpanseMap
    let mut expanse_map = ExpanseMap::new();
    for &k in &initial_keys {
        expanse_map.insert(k, k ^ 0x5CA1_AB1E);
    }
    let out_map = run_workload_expanse_map(&mut expanse_map, &ops, Sampling::Windows(64));
    let (stats_map, mem_map) = (out_map.stats.clone(), out_map.mem_bytes);
    assert_eq!(stats_map.count, 500);
    assert!(stats_map.ops_per_sec > 0.0);
    assert!(mem_map > 0);

    // 2. ExpanseBlobMap
    let mut blobmap = ExpanseBlobMap::new();
    for &k in &initial_keys {
        let _ = blobmap.insert(k, &payload, (k & 0xFF) as u32);
    }
    let out_blob =
        run_workload_expanse_blobmap(&mut blobmap, &ops, &payload, Sampling::Windows(64));
    let (stats_blob, mem_blob) = (out_blob.stats.clone(), out_blob.mem_bytes);
    assert_eq!(stats_blob.count, 500);
    assert!(stats_blob.ops_per_sec > 0.0);
    assert!(mem_blob > 0);

    // 3. BTreeMap
    let mut btree = BTreeMap::new();
    for &k in &initial_keys {
        btree.insert(k, payload.to_vec().into_boxed_slice());
    }
    let out_btree = run_workload_btreemap(&mut btree, &ops, &payload, Sampling::Windows(64));
    let (stats_btree, mem_btree) = (out_btree.stats.clone(), out_btree.mem_bytes);
    assert_eq!(stats_btree.count, 500);
    assert!(stats_btree.ops_per_sec > 0.0);
    assert!(mem_btree > 0);

    // 4. SkipMap
    let skipmap = SkipMap::new();
    for &k in &initial_keys {
        skipmap.insert(k, payload.to_vec().into_boxed_slice());
    }
    let out_skip = run_workload_skipmap(&skipmap, &ops, &payload, Sampling::Windows(64));
    let (stats_skip, mem_skip) = (out_skip.stats.clone(), out_skip.mem_bytes);
    assert_eq!(stats_skip.count, 500);
    assert!(stats_skip.ops_per_sec > 0.0);
    assert!(mem_skip > 0);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns concurrent threads over SyncExpanseMap; deliberate seqlock racy reads (see sync.rs docs)"
)]
fn test_concurrent_ycsb_execution() {
    let (r_ops, w_ops) = run_concurrent_ycsb(2, Workload::B, Duration::from_millis(100));
    assert!(
        r_ops > 0.0,
        "Concurrent reads per sec should be > 0 (got {r_ops})"
    );
    assert!(
        w_ops > 0.0,
        "Concurrent writes per sec should be > 0 (got {w_ops})"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "runs the multithreaded SyncExpanseMap concurrency-scaling section; deliberate seqlock racy reads (see sync.rs docs)"
)]
fn test_ycsb_full_workload_suite_report() {
    let initial_keys = generate_initial_keys(POPULATION_N);
    let payload = generate_payload(0xFEED_FACE_CAFE_BEEF);
    let workloads = [
        Workload::A,
        Workload::B,
        Workload::C,
        Workload::D,
        Workload::E,
        Workload::F,
    ];

    println!(
        "\n===================================================================================================="
    );
    println!(" Standardized YCSB Workload Suite (N = 100,000, θ = 0.99, Payload = 128B)");
    println!(
        "===================================================================================================="
    );

    for &wl in &workloads {
        println!("\n>>> {}", wl.name());
        println!(
            "{:<24} | {:>12} | {:>8} | {:>8} | {:>8} | {:>8} | {:>10} | {:>8}",
            "Target Structure",
            "Throughput",
            "p50 (ns)",
            "p95 (ns)",
            "p99 (ns)",
            "p99.9(ns)",
            "Memory (MB)",
            "B/key"
        );
        println!("{:-<100}", "");

        let ops = generate_operations(wl, &initial_keys, 25_000, 0x1234_5678_9ABC);

        // 1. ExpanseMap
        let mut expanse_map = ExpanseMap::new();
        for &k in &initial_keys {
            expanse_map.insert(k, k ^ 0x5CA1_AB1E);
        }
        let out_map = run_workload_expanse_map(&mut expanse_map, &ops, Sampling::Windows(64));
        let (stats_map, mem_map) = (out_map.stats.clone(), out_map.mem_bytes);
        // This test printed a table and asserted nothing: it would have passed
        // with every measurement replaced by a constant, while counting toward
        // the test floor. The sibling `test_ycsb_execution_across_all_targets`
        // already pins these, so pin them here too.
        assert!(
            stats_map.ops_per_sec > 0.0,
            "ExpanseMap did no work on {wl:?}"
        );
        assert!(mem_map > 0, "ExpanseMap reported no memory on {wl:?}");
        println!(
            "{:<24} | {:>9.2} M/s | {:>8.1} | {:>8.1} | {:>8.1} | {:>8.1} | {:>9.2} MB | {:>8.1}",
            "ExpanseMap (u64)",
            stats_map.ops_per_sec / 1_000_000.0,
            stats_map.p50_ns,
            stats_map.p95_ns,
            stats_map.p99_ns,
            stats_map.p999_ns,
            mem_map as f64 / (1024.0 * 1024.0),
            mem_map as f64 / initial_keys.len() as f64,
        );

        // 2. ExpanseBlobMap
        let mut blobmap = ExpanseBlobMap::new();
        for &k in &initial_keys {
            let _ = blobmap.insert(k, &payload, (k & 0xFF) as u32);
        }
        let out_blob =
            run_workload_expanse_blobmap(&mut blobmap, &ops, &payload, Sampling::Windows(64));
        let (stats_blob, mem_blob) = (out_blob.stats.clone(), out_blob.mem_bytes);
        // This test printed a table and asserted nothing: it would have passed
        // with every measurement replaced by a constant, while counting toward
        // the test floor. The sibling `test_ycsb_execution_across_all_targets`
        // already pins these, so pin them here too.
        assert!(
            stats_blob.ops_per_sec > 0.0,
            "ExpanseBlobMap did no work on {wl:?}"
        );
        assert!(mem_blob > 0, "ExpanseBlobMap reported no memory on {wl:?}");
        println!(
            "{:<24} | {:>9.2} M/s | {:>8.1} | {:>8.1} | {:>8.1} | {:>8.1} | {:>9.2} MB | {:>8.1}",
            "ExpanseBlobMap (128B)",
            stats_blob.ops_per_sec / 1_000_000.0,
            stats_blob.p50_ns,
            stats_blob.p95_ns,
            stats_blob.p99_ns,
            stats_blob.p999_ns,
            mem_blob as f64 / (1024.0 * 1024.0),
            mem_blob as f64 / initial_keys.len() as f64,
        );

        // 3. BTreeMap
        let mut btree = BTreeMap::new();
        for &k in &initial_keys {
            btree.insert(k, payload.to_vec().into_boxed_slice());
        }
        let out_btree = run_workload_btreemap(&mut btree, &ops, &payload, Sampling::Windows(64));
        let (stats_btree, mem_btree) = (out_btree.stats.clone(), out_btree.mem_bytes);
        // This test printed a table and asserted nothing: it would have passed
        // with every measurement replaced by a constant, while counting toward
        // the test floor. The sibling `test_ycsb_execution_across_all_targets`
        // already pins these, so pin them here too.
        assert!(
            stats_btree.ops_per_sec > 0.0,
            "BTreeMap did no work on {wl:?}"
        );
        assert!(mem_btree > 0, "BTreeMap reported no memory on {wl:?}");
        println!(
            "{:<24} | {:>9.2} M/s | {:>8.1} | {:>8.1} | {:>8.1} | {:>8.1} | {:>9.2} MB | {:>8.1}",
            "BTreeMap (128B)",
            stats_btree.ops_per_sec / 1_000_000.0,
            stats_btree.p50_ns,
            stats_btree.p95_ns,
            stats_btree.p99_ns,
            stats_btree.p999_ns,
            mem_btree as f64 / (1024.0 * 1024.0),
            mem_btree as f64 / initial_keys.len() as f64,
        );

        // 4. SkipMap
        let skipmap = SkipMap::new();
        for &k in &initial_keys {
            skipmap.insert(k, payload.to_vec().into_boxed_slice());
        }
        let out_skip = run_workload_skipmap(&skipmap, &ops, &payload, Sampling::Windows(64));
        let (stats_skip, mem_skip) = (out_skip.stats.clone(), out_skip.mem_bytes);
        // This test printed a table and asserted nothing: it would have passed
        // with every measurement replaced by a constant, while counting toward
        // the test floor. The sibling `test_ycsb_execution_across_all_targets`
        // already pins these, so pin them here too.
        assert!(
            stats_skip.ops_per_sec > 0.0,
            "SkipMap did no work on {wl:?}"
        );
        assert!(mem_skip > 0, "SkipMap reported no memory on {wl:?}");
        println!(
            "{:<24} | {:>9.2} M/s | {:>8.1} | {:>8.1} | {:>8.1} | {:>8.1} | {:>9.2} MB | {:>8.1}",
            "SkipMap (128B)",
            stats_skip.ops_per_sec / 1_000_000.0,
            stats_skip.p50_ns,
            stats_skip.p95_ns,
            stats_skip.p99_ns,
            stats_skip.p999_ns,
            mem_skip as f64 / (1024.0 * 1024.0),
            mem_skip as f64 / initial_keys.len() as f64,
        );
    }

    println!(
        "\n===================================================================================================="
    );
    println!(
        " SyncExpanseMap Multithreaded Concurrency Scaling (Workload B: 95% Read / 5% Update)"
    );
    println!(
        "===================================================================================================="
    );
    println!(
        "{:>8} | {:>16} | {:>16} | {:>16} | {:>10}",
        "Threads", "Read Ops/sec", "Write Ops/sec", "Total Ops/sec", "Scaling"
    );
    println!("{:-<76}", "");

    let mut base_total = 0.0;
    for &threads in &[1, 2, 4, 8, 16] {
        let (r_ops, w_ops) = run_concurrent_ycsb(threads, Workload::B, Duration::from_millis(300));
        let total = r_ops + w_ops;
        if threads == 1 {
            base_total = total;
        }
        println!(
            "{:>8} | {:>13.2} M/s | {:>13.2} M/s | {:>13.2} M/s | {:>9.2}x",
            threads,
            r_ops / 1_000_000.0,
            w_ops / 1_000_000.0,
            total / 1_000_000.0,
            total / base_total
        );
    }
}

// ---------------------------------------------------------------------------
// #1005: key shapes, insertion orders, the work checksum and window sampling
// ---------------------------------------------------------------------------

#[test]
fn test_dense_shape_is_distinct_runs_below_the_insert_sequence() {
    let n = 100_000usize;
    let keys = generate_keys(KeyShape::DenseClustered, n);
    assert_eq!(keys.len(), n);
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), n, "dense population must be distinct");
    for (i, &k) in keys.iter().enumerate() {
        assert!(
            k < INSERT_SEQ_BASE,
            "key {k:#x} collides with the in-run insert sequence"
        );
        let pos = i as u64 % CLUSTER_RUN;
        assert_eq!(
            k & (CLUSTER_RUN - 1),
            pos,
            "key {i} is not at its run offset"
        );
        if pos != 0 {
            assert_eq!(k, keys[i - 1] + 1, "run is not consecutive at {i}");
        }
    }
    // The uniform shape stays what `generate_initial_keys` always produced.
    assert_eq!(
        generate_keys(KeyShape::UniformRandom, 1000),
        generate_initial_keys(1000)
    );
}

#[test]
fn test_build_orders_are_permutations_of_the_canonical_population() {
    for shape in [KeyShape::UniformRandom, KeyShape::DenseClustered] {
        let canonical = generate_keys(shape, 10_000);
        let sorted = build_order(&canonical, InsertionOrder::Sorted);
        let shuffled = build_order(&canonical, InsertionOrder::Shuffled);
        assert!(
            sorted.windows(2).all(|w| w[0] < w[1]),
            "{shape:?}: not ascending"
        );
        assert!(
            !shuffled.windows(2).all(|w| w[0] < w[1]),
            "{shape:?}: the shuffled order came out sorted"
        );
        assert_ne!(
            shuffled, canonical,
            "{shape:?}: the shuffle was the identity"
        );
        let mut back = shuffled.clone();
        back.sort_unstable();
        assert_eq!(
            back, sorted,
            "{shape:?}: the shuffle lost or invented a key"
        );
        assert_eq!(
            shuffled,
            build_order(&canonical, InsertionOrder::Shuffled),
            "{shape:?}: the shuffle is not deterministic"
        );
    }
}

#[test]
fn test_workload_d_reads_the_keys_the_run_inserted() {
    let keys = generate_initial_keys(10_000);
    let ops = generate_operations(Workload::D, &keys, 20_000, 0x1234_5678_9ABC);
    let population: std::collections::HashSet<u64> = keys.iter().copied().collect();
    let mut inserted = std::collections::HashSet::new();
    let mut reads_of_inserted = 0usize;
    for op in &ops {
        match *op {
            YcsbOp::Insert(k, _) => {
                assert!(k > INSERT_SEQ_BASE);
                inserted.insert(k);
            }
            YcsbOp::Read(k) => {
                if inserted.contains(&k) {
                    reads_of_inserted += 1;
                } else {
                    assert!(
                        population.contains(&k),
                        "workload D read {k:#x}: not in the population, not inserted earlier"
                    );
                }
            }
            other => panic!("workload D produced {other:?}"),
        }
    }
    // Read-latest: once the stream has inserted anything, the newest keys are
    // the hottest ranks, so most reads land on in-run inserts.
    assert!(
        reads_of_inserted * 2 > ops.len(),
        "only {reads_of_inserted} of {} ops read an in-run insert; workload D is not read-latest",
        ops.len()
    );
}

#[test]
fn test_work_checksum_is_identical_across_engines_shapes_and_orders() {
    let payload = generate_payload(0xFEED_FACE_CAFE_BEEF);
    for shape in [KeyShape::UniformRandom, KeyShape::DenseClustered] {
        let canonical = generate_keys(shape, 4_096);
        for &wl in &[
            Workload::A,
            Workload::B,
            Workload::C,
            Workload::D,
            Workload::E,
            Workload::F,
        ] {
            let ops = generate_operations(wl, &canonical, 3_000, 0x1234_5678_9ABC);
            let mut seen: Option<u64> = None;
            for order in InsertionOrder::BOTH {
                let keys = build_order(&canonical, order);
                for engine in Engine::ALL {
                    let off = run_cell(engine, &keys, &ops, &payload, Sampling::Off);
                    let win = run_cell(engine, &keys, &ops, &payload, Sampling::Windows(64));
                    assert_eq!(off.consumed, win.consumed, "{shape:?}/{wl:?}/{engine:?}");
                    assert!(
                        off.consumed > 0,
                        "{shape:?}/{wl:?}/{engine:?} consumed nothing"
                    );
                    match seen {
                        None => seen = Some(off.consumed),
                        Some(c) => assert_eq!(
                            c, off.consumed,
                            "{shape:?}/{wl:?}/{engine:?}/{order:?} did different work"
                        ),
                    }
                }
            }
        }
    }
}

#[test]
fn test_window_sampling_counts_windows_and_never_samples_when_off() {
    let keys = generate_initial_keys(1_000);
    let ops = generate_operations(Workload::C, &keys, 1_000, 0x9999);
    let payload = generate_payload(1);

    let off = run_cell(Engine::Trie, &keys, &ops, &payload, Sampling::Off);
    assert_eq!(off.stats.count, 1_000);
    assert_eq!(off.stats.windows, 0);
    assert_eq!(off.stats.window_ops, 0);

    // 1000 ops in windows of 64: 15 full windows and one of 40.
    let win = run_cell(Engine::Trie, &keys, &ops, &payload, Sampling::Windows(64));
    assert_eq!(win.stats.count, 1_000);
    assert_eq!(win.stats.windows, 16);
    assert_eq!(win.stats.window_ops, 64);
    assert!(win.stats.min_ns <= win.stats.p50_ns && win.stats.p50_ns <= win.stats.max_ns);
}

#[test]
fn test_latency_stats_percentiles_over_known_window_means() {
    // 1000 window means 1.0 ..= 1000.0 ns/op, fed in descending order.
    let samples: Vec<f64> = (1..=1000).rev().map(|v| v as f64).collect();
    let s = LatencyStats::compute(samples, 64_000, 64, Duration::from_secs(1));
    assert_eq!(s.windows, 1000);
    assert_eq!(s.min_ns, 1.0);
    assert_eq!(s.max_ns, 1000.0);
    assert_eq!(s.p50_ns, 501.0);
    assert_eq!(s.p95_ns, 951.0);
    assert_eq!(s.p99_ns, 991.0);
    assert_eq!(s.p999_ns, 1000.0);
    // Slow = above 8x the median (4008.0): none here.
    assert_eq!(s.slow_windows, 0);
    assert_eq!(s.ops_per_sec, 64_000.0);

    let mut tail = vec![10.0; 99];
    tail.push(10_000.0);
    let s = LatencyStats::compute(tail, 6_400, 64, Duration::from_secs(1));
    assert_eq!(s.slow_windows, 1, "one window at 1000x the median is slow");
}

#[test]
fn test_population_tokens() {
    assert_eq!(parse_population("100k"), Ok(100_000));
    assert_eq!(parse_population("1m"), Ok(1_000_000));
    assert_eq!(parse_population(" 10M "), Ok(10_000_000));
    assert_eq!(parse_population("4096"), Ok(4_096));
    assert!(parse_population("lots").is_err());
    assert!(parse_population("1").is_err());
    assert!(parse_population("").is_err());
}

/// The defect the blob-arm mask fixes, pinned on the shipped stream: the ops
/// carry 64 random bits of metadata, the arena field is 24 bits, and an
/// unmasked value is an `Err` the old runner discarded.
#[test]
fn test_blob_writes_need_the_metadata_mask_and_land_with_it() {
    let keys = generate_initial_keys(10_000);
    let payload = generate_payload(7);
    let ops = generate_operations(Workload::A, &keys, 20_000, 0x1234_5678_9ABC);
    let (mut writes, mut overflowing) = (0usize, 0usize);
    for op in &ops {
        if let YcsbOp::Update(_, meta) = *op {
            writes += 1;
            if meta as u32 > BLOB_META_MASK {
                overflowing += 1;
            }
        }
    }
    // Expected 255/256 of writes; anything above 99% shows the mask is load-bearing.
    assert!(
        overflowing * 100 > writes * 99,
        "{overflowing} of {writes} raw metadata values overflow the arena field"
    );
    let mut probe = ExpanseBlobMap::new();
    assert!(
        probe.insert(1, &payload, BLOB_META_MASK + 1).is_err(),
        "an unmasked value is rejected, which is what the old runner discarded"
    );

    // With the mask every update lands: the metadata read back is the masked op value.
    let mut map = ExpanseBlobMap::new();
    for &k in &keys {
        map.insert(k, &payload, 0).expect("build insert");
    }
    let _ = run_workload_expanse_blobmap(&mut map, &ops, &payload, Sampling::Off);
    let mut last: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
    for op in &ops {
        if let YcsbOp::Update(k, meta) = *op {
            last.insert(k, meta as u32 & BLOB_META_MASK);
        }
    }
    assert!(!last.is_empty());
    for (k, want) in last {
        assert_eq!(
            map.get(k).map(|(_, m)| m),
            Some(want),
            "update of {k:#x} did not land"
        );
    }
}
