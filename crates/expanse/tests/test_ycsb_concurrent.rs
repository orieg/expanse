//! Tests for concurrent YCSB harness and statistical bounds (METHODOLOGY §20.12 item 2, §20.5).

#![cfg(not(miri))]

use std::sync::{Arc, Barrier, Mutex};

use expanse_trie::sync::SyncExpanseMap;

#[path = "../benches/ycsb_common/mod.rs"]
mod ycsb_common;
use ycsb_common::{XorShift64, ZIPFIAN_THETA, ZipfianGenerator};

/// Computes the exact share of draws on the k lowest ranks under Gray's generator closed form
/// (METHODOLOGY §20.7, `scripts/ycsb_concurrent_bounds.py::gray_top_k_share`).
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

/// Rank-histogram unit test holding a stream's shares to `gray_top_k_share`
/// within binomial tolerance (METHODOLOGY §20.12 item 2).
#[test]
fn test_rank_histogram_matches_gray_top_k_share() {
    let n = 100_000usize;
    let theta = ZIPFIAN_THETA;
    let zipf = ZipfianGenerator::new(n as u64, theta);
    let mut rng = XorShift64::new(0xABCD_EF01_2345_6789);

    let m = 200_000usize; // sample size
    let test_k_points = [1, 2, 5, 10, 50, 100, 500, 1_000, 10_000];
    let mut rank_counts = vec![0u32; n];

    for _ in 0..m {
        let u = rng.next_f64();
        let r = zipf.next(u) as usize;
        assert!(r < n, "draw {r} exceeds population {n}");
        rank_counts[r] += 1;
    }

    let mut cumulative = 0u64;
    let mut k_idx = 0;

    for (r, &count) in rank_counts.iter().enumerate() {
        cumulative += count as u64;
        let k = r + 1;
        if k_idx < test_k_points.len() && k == test_k_points[k_idx] {
            let expected_share = gray_top_k_share(k, n, theta);
            let observed_share = cumulative as f64 / m as f64;

            // Binomial standard deviation: sqrt(p * (1 - p) / m)
            let sigma = (expected_share * (1.0 - expected_share) / m as f64).sqrt();
            // Stated binomial tolerance: 4.5 standard deviations (< 1e-5 chance of false rejection)
            let tolerance = 4.5 * sigma;

            assert!(
                (observed_share - expected_share).abs() <= tolerance,
                "Rank histogram deviation at k={k}: observed {observed_share:.6}, expected {expected_share:.6}, sigma {sigma:.6}, diff {:.6} > tolerance {:.6}",
                (observed_share - expected_share).abs(),
                tolerance
            );

            k_idx += 1;
        }
    }
}

/// G5 Negative Control: an `olc` variant that skips the striped lock MUST fail
/// the lost-update invariant at T = 8 (METHODOLOGY §20.5, §20.12 item 2).
///
/// Asserted on the diagnostic string ("VOID_LOST_UPDATE") and not the exit code (AGENTS.md §5).
#[test]
fn test_g5_negative_control_fails_without_striped_lock() {
    let map = Arc::new(SyncExpanseMap::new());
    const POPULATION: usize = 64; // small population to guarantee collision
    for k in 0..POPULATION as u64 {
        map.insert(k, 0);
    }

    const THREADS: usize = 8;
    const OPS_PER_THREAD: usize = 2_000;
    let barrier = Arc::new(Barrier::new(THREADS));

    let mut handles = Vec::with_capacity(THREADS);
    for t_id in 0..THREADS {
        let map = Arc::clone(&map);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            let mut rng = XorShift64::new(0x1006_0000 + t_id as u64);
            barrier.wait();

            for _ in 0..OPS_PER_THREAD {
                // Hot key choice concentrated on lowest keys
                let k = rng.next_u64() % 4;

                // BUG INJECTION / NEGATIVE CONTROL: Unsynchronized get + insert (skipping lock)
                let cur = map.get(k).unwrap_or(0);
                map.insert(k, cur + 1);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let total_expected = (THREADS * OPS_PER_THREAD) as u64;
    let mut total_actual = 0u64;
    for k in 0..POPULATION as u64 {
        total_actual += map.get(k).unwrap_or(0);
    }

    // Must fail invariant and detect lost updates
    let outcome = if total_actual != total_expected {
        let lost = total_expected.saturating_sub(total_actual);
        format!(
            "VOID_LOST_UPDATE: lost {lost} updates out of {total_expected} (actual sum {total_actual})"
        )
    } else {
        "PASS".to_string()
    };

    assert!(
        outcome.starts_with("VOID_LOST_UPDATE"),
        "negative control MUST fail with VOID_LOST_UPDATE, got {outcome}"
    );
}

/// Positive control: Concurrent RMW with external striped lock passes G5 with 0 lost updates.
#[test]
fn test_g5_positive_control_passes_with_striped_lock() {
    const THREADS: usize = 8;
    const OPS_PER_THREAD: usize = 10_000;
    const NUM_STRIPES: usize = 1_024;
    const POPULATION: usize = 64;

    let map = Arc::new(SyncExpanseMap::new());
    for k in 0..POPULATION as u64 {
        map.insert(k, 0);
    }

    let mut stripes_vec = Vec::with_capacity(NUM_STRIPES);
    for _ in 0..NUM_STRIPES {
        stripes_vec.push(Mutex::new(()));
    }
    let stripes = Arc::new(stripes_vec);
    let barrier = Arc::new(Barrier::new(THREADS));

    let mut handles = Vec::with_capacity(THREADS);
    for t_id in 0..THREADS {
        let map = Arc::clone(&map);
        let stripes = Arc::clone(&stripes);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            let mut rng = XorShift64::new(0x1006_0000 + t_id as u64);
            barrier.wait();

            for _ in 0..OPS_PER_THREAD {
                let k = rng.next_u64() % 4;
                let stripe_idx =
                    (k.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 54) as usize % NUM_STRIPES;
                let _guard = stripes[stripe_idx].lock().unwrap();

                let cur = map.get(k).unwrap_or(0);
                map.insert(k, cur + 1);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let total_expected = (THREADS * OPS_PER_THREAD) as u64;
    let mut total_actual = 0u64;
    for k in 0..POPULATION as u64 {
        total_actual += map.get(k).unwrap_or(0);
    }

    assert_eq!(
        total_actual, total_expected,
        "Striped lock must preserve exact RMW count without lost updates"
    );
}
