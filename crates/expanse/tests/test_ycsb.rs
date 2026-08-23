//! Unit and integration tests for YCSB benchmark generation and workload execution.

use crossbeam_skiplist::SkipMap;
use expanse_trie::blobmap::ExpanseBlobMap;
use expanse_trie::map::ExpanseMap;
use std::collections::BTreeMap;
use std::time::Duration;

#[allow(dead_code)]
#[path = "../benches/ycsb.rs"]
mod ycsb;

use ycsb::{
    BLOB_PAYLOAD_SIZE, POPULATION_N, Workload, XorShift64, YcsbOp, ZipfianGenerator,
    generate_initial_keys, generate_operations, generate_payload, run_concurrent_ycsb,
    run_workload_btreemap, run_workload_expanse_blobmap, run_workload_expanse_map,
    run_workload_skipmap,
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
    let (stats_map, mem_map) = run_workload_expanse_map(&mut expanse_map, &ops, true);
    assert_eq!(stats_map.count, 500);
    assert!(stats_map.ops_per_sec > 0.0);
    assert!(mem_map > 0);

    // 2. ExpanseBlobMap
    let mut blobmap = ExpanseBlobMap::new();
    for &k in &initial_keys {
        let _ = blobmap.insert(k, &payload, (k & 0xFF) as u32);
    }
    let (stats_blob, mem_blob) = run_workload_expanse_blobmap(&mut blobmap, &ops, &payload, true);
    assert_eq!(stats_blob.count, 500);
    assert!(stats_blob.ops_per_sec > 0.0);
    assert!(mem_blob > 0);

    // 3. BTreeMap
    let mut btree = BTreeMap::new();
    for &k in &initial_keys {
        btree.insert(k, payload.to_vec().into_boxed_slice());
    }
    let (stats_btree, mem_btree) = run_workload_btreemap(&mut btree, &ops, &payload, true);
    assert_eq!(stats_btree.count, 500);
    assert!(stats_btree.ops_per_sec > 0.0);
    assert!(mem_btree > 0);

    // 4. SkipMap
    let skipmap = SkipMap::new();
    for &k in &initial_keys {
        skipmap.insert(k, payload.to_vec().into_boxed_slice());
    }
    let (stats_skip, mem_skip) = run_workload_skipmap(&skipmap, &ops, &payload, true);
    assert_eq!(stats_skip.count, 500);
    assert!(stats_skip.ops_per_sec > 0.0);
    assert!(mem_skip > 0);
}

#[test]
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
        let (stats_map, mem_map) = run_workload_expanse_map(&mut expanse_map, &ops, true);
        println!(
            "{:<24} | {:>9.2} M/s | {:>8} | {:>8} | {:>8} | {:>8} | {:>9.2} MB | {:>8.1}",
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
        let (stats_blob, mem_blob) =
            run_workload_expanse_blobmap(&mut blobmap, &ops, &payload, true);
        println!(
            "{:<24} | {:>9.2} M/s | {:>8} | {:>8} | {:>8} | {:>8} | {:>9.2} MB | {:>8.1}",
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
        let (stats_btree, mem_btree) = run_workload_btreemap(&mut btree, &ops, &payload, true);
        println!(
            "{:<24} | {:>9.2} M/s | {:>8} | {:>8} | {:>8} | {:>8} | {:>9.2} MB | {:>8.1}",
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
        let (stats_skip, mem_skip) = run_workload_skipmap(&skipmap, &ops, &payload, true);
        println!(
            "{:<24} | {:>9.2} M/s | {:>8} | {:>8} | {:>8} | {:>8} | {:>9.2} MB | {:>8.1}",
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
