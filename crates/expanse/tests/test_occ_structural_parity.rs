//! Structural equivalence tests holding flat and OCC mutation walks to identical
//! digital trie structure across ExpanseMap and ExpanseSet (Refs #1248).
//!
//! Asserts that single-threaded mutations across both covering modes:
//! `<true, false>` (`engine_covers_root == true`) and
//! `<true, true>` (`engine_covers_root == false`)
//! make identical node, depth, and hysteresis decisions as the plain walk
//! `<false, false>`, matching iteration order, values, node counts, depth
//! histograms, leaf population histograms, node bytes, mem_used, and
//! post-drain live allocations.
#![cfg(not(miri))]
#![cfg(feature = "std")]
#![cfg(target_pointer_width = "64")]

use expanse_trie::map::ExpanseMap;
use expanse_trie::occ::DeferredTestTree;
use expanse_trie::set::ExpanseSet;

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Key shapes from `compact_shapes()`: dense, wide, scattered, cascades, and sparse.
fn shape_dense() -> Vec<u64> {
    (0..600u64).collect()
}

fn shape_wide() -> Vec<u64> {
    // 250 runs of two keys under one skipped prefix: promotes through BranchB into BranchU.
    (0..250u64)
        .flat_map(|d| [(0x42u64 << 56) | (d << 8), (0x42u64 << 56) | (d << 8) | 1])
        .collect()
}

fn shape_scattered() -> Vec<u64> {
    let mut rng = XorShift64(0x0DDB_1A5E_5EED_0001);
    (0..600).map(|_| rng.next()).collect()
}

fn shape_cascades() -> Vec<u64> {
    // 40 keys differing only in final byte behind skip + 40 keys shifted by 56 (cascades).
    let mut skipped: Vec<u64> = (0..40u64).map(|i| 0x1234_5678_9ABC_DE00 | i).collect();
    skipped.extend((0..40u64).map(|i| (i + 1) << 56));
    skipped
}

fn shape_sparse() -> Vec<u64> {
    // Sparse high-digit keys: single-key chains and immediates at every level.
    (0..300u64).map(|i| i << 40).collect()
}

fn assert_parity_map(plain: &ExpanseMap, deferred: &DeferredTestTree<ExpanseMap>, phase: &str) {
    // 1. Length equality
    assert_eq!(plain.len(), deferred.len(), "{phase}: len mismatch");

    // 2. Iteration order and values equality
    assert!(
        plain.iter().eq(deferred.iter()),
        "{phase}: iteration key-value mismatch"
    );

    // 3. Defensive validation
    plain.validate();
    deferred.validate();

    // 4. Drain deferred collector so all retired blocks are freed
    deferred.drain();

    // 5. ExpanseStats comparison: node forms, depth, leaf populations, node bytes
    let p_stats = plain.stats();
    let d_stats = deferred.stats();
    assert_eq!(
        p_stats.node_counts, d_stats.node_counts,
        "{phase}: node_counts mismatch (plain: {:?}, deferred: {:?})",
        p_stats.node_counts, d_stats.node_counts
    );
    assert_eq!(
        p_stats.depth_histogram, d_stats.depth_histogram,
        "{phase}: depth_histogram mismatch"
    );
    assert_eq!(
        p_stats.leaf_pop_histogram, d_stats.leaf_pop_histogram,
        "{phase}: leaf_pop_histogram mismatch"
    );
    assert_eq!(
        p_stats.node_bytes, d_stats.node_bytes,
        "{phase}: node_bytes mismatch"
    );
    assert_eq!(
        p_stats.branch_depth_histogram, d_stats.branch_depth_histogram,
        "{phase}: branch_depth_histogram mismatch"
    );
    assert_eq!(
        p_stats.leaf_depth_histogram, d_stats.leaf_depth_histogram,
        "{phase}: leaf_depth_histogram mismatch"
    );
    assert_eq!(p_stats, d_stats, "{phase}: full ExpanseStats mismatch");

    // 6. mem_used equality
    assert_eq!(
        plain.mem_used(),
        deferred.mem_used(),
        "{phase}: mem_used mismatch (plain: {}, deferred: {})",
        plain.mem_used(),
        deferred.mem_used()
    );

    // 7. live allocations equality
    assert_eq!(
        plain.live_allocs(),
        deferred.live_allocs(),
        "{phase}: live_allocs mismatch (plain: {}, deferred: {})",
        plain.live_allocs(),
        deferred.live_allocs()
    );
}

fn assert_parity_set(plain: &ExpanseSet, deferred: &DeferredTestTree<ExpanseSet>, phase: &str) {
    // 1. Length equality
    assert_eq!(plain.len(), deferred.len(), "{phase}: len mismatch");

    // 2. Iteration order equality
    assert!(
        plain.iter().eq(deferred.iter()),
        "{phase}: iteration key mismatch"
    );

    // 3. Defensive validation
    plain.validate();
    deferred.validate();

    // 4. Drain deferred collector so all retired blocks are freed
    deferred.drain();

    // 5. ExpanseStats comparison: node forms, depth, leaf populations, node bytes
    let p_stats = plain.stats();
    let d_stats = deferred.stats();
    assert_eq!(
        p_stats.node_counts, d_stats.node_counts,
        "{phase}: node_counts mismatch (plain: {:?}, deferred: {:?})",
        p_stats.node_counts, d_stats.node_counts
    );
    assert_eq!(
        p_stats.depth_histogram, d_stats.depth_histogram,
        "{phase}: depth_histogram mismatch"
    );
    assert_eq!(
        p_stats.leaf_pop_histogram, d_stats.leaf_pop_histogram,
        "{phase}: leaf_pop_histogram mismatch"
    );
    assert_eq!(
        p_stats.node_bytes, d_stats.node_bytes,
        "{phase}: node_bytes mismatch"
    );
    assert_eq!(
        p_stats.branch_depth_histogram, d_stats.branch_depth_histogram,
        "{phase}: branch_depth_histogram mismatch"
    );
    assert_eq!(
        p_stats.leaf_depth_histogram, d_stats.leaf_depth_histogram,
        "{phase}: leaf_depth_histogram mismatch"
    );
    assert_eq!(p_stats, d_stats, "{phase}: full ExpanseStats mismatch");

    // 6. mem_used equality
    assert_eq!(
        plain.mem_used(),
        deferred.mem_used(),
        "{phase}: mem_used mismatch (plain: {}, deferred: {})",
        plain.mem_used(),
        deferred.mem_used()
    );

    // 7. live allocations equality
    assert_eq!(
        plain.live_allocs(),
        deferred.live_allocs(),
        "{phase}: live_allocs mismatch (plain: {}, deferred: {})",
        plain.live_allocs(),
        deferred.live_allocs()
    );
}

fn drive_map_structural_parity(keys: &[u64], engine_covers_root: bool) {
    let mut plain = ExpanseMap::new();
    let mut deferred = ExpanseMap::deferred_for_test(engine_covers_root);

    // Initial empty state
    assert_parity_map(&plain, &deferred, "initial empty");

    // 1. Batch Insert
    for (i, &k) in keys.iter().enumerate() {
        let v = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        assert_eq!(plain.insert(k, v), deferred.insert(k, v));
    }
    assert_parity_map(&plain, &deferred, "after batch insert");

    // 2. In-place Updates (overwriting existing values)
    for &k in keys.iter().step_by(3) {
        let new_v = k ^ 0xCAFE_BABE_0000_0001;
        assert_eq!(plain.insert(k, new_v), deferred.insert(k, new_v));
    }
    assert_parity_map(&plain, &deferred, "after in-place updates");

    // 3. `ins_slot` calls on both warm and cold paths
    for i in 0..50u64 {
        let k = 0xFFFF_0000_0000_0000 | (i << 8) | 7;
        let v = i.wrapping_mul(17) | 3;
        let p_slot = plain.ins_slot(k);
        let d_slot = deferred.ins_slot(k);
        // SAFETY: `p_slot` and `d_slot` point to valid non-null value slots allocated
        // by `ins_slot`, accessed exclusively in this single-threaded test.
        unsafe {
            assert_eq!(*p_slot.as_ptr(), *d_slot.as_ptr());
            p_slot.as_ptr().write(v);
            d_slot.as_ptr().write(v);
        }
    }
    assert_parity_map(&plain, &deferred, "after ins_slot insertions");

    // 4. Partial Removals: remove subset crossing hysteresis floors
    for &k in keys.iter().step_by(2) {
        assert_eq!(plain.remove(k), deferred.remove(k));
    }
    assert_parity_map(&plain, &deferred, "after partial removal (stride 2)");

    // 5. Reverse Removals: remove remaining original keys
    for &k in keys.iter().rev() {
        assert_eq!(plain.remove(k), deferred.remove(k));
    }
    assert_parity_map(&plain, &deferred, "after reverse removal of remaining keys");

    // 6. Remove the ins_slot keys
    for i in 0..50u64 {
        let k = 0xFFFF_0000_0000_0000 | (i << 8) | 7;
        assert_eq!(plain.remove(k), deferred.remove(k));
    }
    assert_parity_map(&plain, &deferred, "after full drain to empty");

    assert!(plain.is_empty());
    assert!(deferred.is_empty());
    assert_eq!(plain.mem_used(), 0);
    assert_eq!(deferred.mem_used(), 0);
    assert_eq!(plain.live_allocs(), 0);
    assert_eq!(deferred.live_allocs(), 0);
}

fn drive_set_structural_parity(keys: &[u64], engine_covers_root: bool) {
    let mut plain = ExpanseSet::new();
    let mut deferred = ExpanseSet::deferred_for_test(engine_covers_root);

    // Initial empty state
    assert_parity_set(&plain, &deferred, "initial empty");

    // 1. Batch Insert
    for &k in keys {
        assert_eq!(plain.insert(k), deferred.insert(k));
    }
    assert_parity_set(&plain, &deferred, "after batch insert");

    // 2. Redundant Inserts (in-place membership verification)
    for &k in keys.iter().step_by(3) {
        assert_eq!(plain.insert(k), deferred.insert(k));
    }
    assert_parity_set(&plain, &deferred, "after redundant inserts");

    // 3. Additional batch of keys (chains and leaf levels)
    for i in 0..50u64 {
        let k = 0xEEEE_0000_0000_0000 | (i << 8) | 5;
        assert_eq!(plain.insert(k), deferred.insert(k));
    }
    assert_parity_set(&plain, &deferred, "after extra key batch");

    // 4. Partial Removals: remove subset crossing hysteresis floors
    for &k in keys.iter().step_by(2) {
        assert_eq!(plain.remove(k), deferred.remove(k));
    }
    assert_parity_set(&plain, &deferred, "after partial removal (stride 2)");

    // 5. Reverse Removals: remove remaining original keys
    for &k in keys.iter().rev() {
        assert_eq!(plain.remove(k), deferred.remove(k));
    }
    assert_parity_set(&plain, &deferred, "after reverse removal of remaining keys");

    // 6. Remove the extra batch keys
    for i in 0..50u64 {
        let k = 0xEEEE_0000_0000_0000 | (i << 8) | 5;
        assert_eq!(plain.remove(k), deferred.remove(k));
    }
    assert_parity_set(&plain, &deferred, "after full drain to empty");

    assert!(plain.is_empty());
    assert!(deferred.is_empty());
    assert_eq!(plain.mem_used(), 0);
    assert_eq!(deferred.mem_used(), 0);
    assert_eq!(plain.live_allocs(), 0);
    assert_eq!(deferred.live_allocs(), 0);
}

// ---------------------------------------------------------------------------
// Map Tests
// ---------------------------------------------------------------------------

#[test]
fn test_map_structural_parity_dense_engine_covers_root() {
    drive_map_structural_parity(&shape_dense(), true);
}

#[test]
fn test_map_structural_parity_dense_wrapper_covers_root() {
    drive_map_structural_parity(&shape_dense(), false);
}

#[test]
fn test_map_structural_parity_wide_engine_covers_root() {
    drive_map_structural_parity(&shape_wide(), true);
}

#[test]
fn test_map_structural_parity_wide_wrapper_covers_root() {
    drive_map_structural_parity(&shape_wide(), false);
}

#[test]
fn test_map_structural_parity_scattered_engine_covers_root() {
    drive_map_structural_parity(&shape_scattered(), true);
}

#[test]
fn test_map_structural_parity_scattered_wrapper_covers_root() {
    drive_map_structural_parity(&shape_scattered(), false);
}

#[test]
fn test_map_structural_parity_cascades_engine_covers_root() {
    drive_map_structural_parity(&shape_cascades(), true);
}

#[test]
fn test_map_structural_parity_cascades_wrapper_covers_root() {
    drive_map_structural_parity(&shape_cascades(), false);
}

#[test]
fn test_map_structural_parity_sparse_engine_covers_root() {
    drive_map_structural_parity(&shape_sparse(), true);
}

#[test]
fn test_map_structural_parity_sparse_wrapper_covers_root() {
    drive_map_structural_parity(&shape_sparse(), false);
}

// ---------------------------------------------------------------------------
// Set Tests
// ---------------------------------------------------------------------------

#[test]
fn test_set_structural_parity_dense_engine_covers_root() {
    drive_set_structural_parity(&shape_dense(), true);
}

#[test]
fn test_set_structural_parity_dense_wrapper_covers_root() {
    drive_set_structural_parity(&shape_dense(), false);
}

#[test]
fn test_set_structural_parity_wide_engine_covers_root() {
    drive_set_structural_parity(&shape_wide(), true);
}

#[test]
fn test_set_structural_parity_wide_wrapper_covers_root() {
    drive_set_structural_parity(&shape_wide(), false);
}

#[test]
fn test_set_structural_parity_scattered_engine_covers_root() {
    drive_set_structural_parity(&shape_scattered(), true);
}

#[test]
fn test_set_structural_parity_scattered_wrapper_covers_root() {
    drive_set_structural_parity(&shape_scattered(), false);
}

#[test]
fn test_set_structural_parity_cascades_engine_covers_root() {
    drive_set_structural_parity(&shape_cascades(), true);
}

#[test]
fn test_set_structural_parity_cascades_wrapper_covers_root() {
    drive_set_structural_parity(&shape_cascades(), false);
}

#[test]
fn test_set_structural_parity_sparse_engine_covers_root() {
    drive_set_structural_parity(&shape_sparse(), true);
}

#[test]
fn test_set_structural_parity_sparse_wrapper_covers_root() {
    drive_set_structural_parity(&shape_sparse(), false);
}
