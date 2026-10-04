//! Comprehensive differential & census test suite for the 32-bit trie direct-emission
//! builder (`from_sorted_iter` on `ExpanseSet32` and `ExpanseMap32`, issue #1200 / Path B,
//! METHODOLOGY.md §14).
//!
//! Validates:
//! 1. Direct emission zero-freed invariant: `live_allocs() == total_node_allocs()`.
//! 2. Exact content equivalence vs `BTreeSet` / `BTreeMap` model and vs sequential insert.
//! 3. Exact per-class census comparison vs insert: `L2`, `L6`, `B`, `U`, `Leaf`, `Bitmap`, `MapBitmap`.
//! 4. Exact `mem_used()` equality vs fresh sequential insert.
//! 5. Navigation & range query equivalence (`first`, `last`, `next`, `prev`, `count_range`).
//! 6. Out-of-order & duplicate handling (last-value-wins for maps, dedup for sets).
//! 7. Compaction round-trip: post-compaction census and `mem_used` match fresh build.
#![cfg(not(miri))]

use std::collections::{BTreeMap, BTreeSet};

use expanse_trie::map32::ExpanseMap32;
use expanse_trie::set32::ExpanseSet32;
use expanse_trie::types32::{Key32, Value32};

// Deterministic XorShift32 PRNG (no external dependencies).
struct XorShift32(u32);

impl XorShift32 {
    fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x1234_5678 } else { seed })
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

// ---------------------------------------------------------------------------
// Model verification helpers
// ---------------------------------------------------------------------------

fn verify_set_against_model(built: &ExpanseSet32, model: &BTreeSet<Key32>, label: &str) {
    assert_eq!(built.len(), model.len(), "{label}: length mismatch");
    assert_eq!(
        built.is_empty(),
        model.is_empty(),
        "{label}: is_empty mismatch"
    );

    // Ordered iteration.
    let built_keys: Vec<Key32> = built.iter().collect();
    let model_keys: Vec<Key32> = model.iter().copied().collect();
    assert_eq!(built_keys, model_keys, "{label}: iter mismatch");

    // Endpoints.
    assert_eq!(
        built.first(),
        model.iter().next().copied(),
        "{label}: first mismatch"
    );
    assert_eq!(
        built.last(),
        model.iter().next_back().copied(),
        "{label}: last mismatch"
    );

    // Lookups & misses.
    for &k in model {
        assert!(built.contains(k), "{label}: missing key {k:#x}");
    }
    for &k in model.iter().take(50) {
        if k > 0 && !model.contains(&(k - 1)) {
            assert!(
                !built.contains(k - 1),
                "{label}: false positive at {:#x}",
                k - 1
            );
        }
        if k < Key32::MAX && !model.contains(&(k + 1)) {
            assert!(
                !built.contains(k + 1),
                "{label}: false positive at {:#x}",
                k + 1
            );
        }
    }

    // Step navigation.
    let mut cur = built.first();
    let mut count = 0;
    while let Some(k) = cur {
        count += 1;
        cur = built.next(k);
    }
    assert_eq!(count, model.len(), "{label}: next walk count mismatch");

    let mut rcur = built.last();
    let mut rcount = 0;
    while let Some(k) = rcur {
        rcount += 1;
        rcur = built.prev(k);
    }
    assert_eq!(rcount, model.len(), "{label}: prev walk count mismatch");
}

fn verify_map_against_model(built: &ExpanseMap32, model: &BTreeMap<Key32, Value32>, label: &str) {
    assert_eq!(built.len(), model.len(), "{label}: length mismatch");
    assert_eq!(
        built.is_empty(),
        model.is_empty(),
        "{label}: is_empty mismatch"
    );

    // Ordered iteration.
    let built_entries: Vec<(Key32, Value32)> = built.iter().collect();
    let model_entries: Vec<(Key32, Value32)> = model.iter().map(|(&k, &v)| (k, v)).collect();
    assert_eq!(built_entries, model_entries, "{label}: iter mismatch");

    // Endpoints.
    assert_eq!(
        built.first(),
        model.iter().next().map(|(&k, &v)| (k, v)),
        "{label}: first mismatch"
    );
    assert_eq!(
        built.last(),
        model.iter().next_back().map(|(&k, &v)| (k, v)),
        "{label}: last mismatch"
    );

    // Lookups & misses.
    for (&k, &v) in model {
        assert_eq!(built.get(k), Some(v), "{label}: wrong value at {k:#x}");
        assert!(built.contains_key(k), "{label}: missing key {k:#x}");
    }
    for &k in model.keys().take(50) {
        if k > 0 && !model.contains_key(&(k - 1)) {
            assert_eq!(
                built.get(k - 1),
                None,
                "{label}: false positive at {:#x}",
                k - 1
            );
            assert!(!built.contains_key(k - 1));
        }
        if k < Key32::MAX && !model.contains_key(&(k + 1)) {
            assert_eq!(
                built.get(k + 1),
                None,
                "{label}: false positive at {:#x}",
                k + 1
            );
            assert!(!built.contains_key(k + 1));
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Direct Emission Zero-Freed Invariant Tests
// ---------------------------------------------------------------------------

#[test]
fn test_set32_direct_emission_zero_freed_across_sizes() {
    let mut rng = XorShift32::new(0xCAFE_BABE);
    for size in [0, 1, 2, 7, 8, 24, 25, 64, 65, 100, 250, 1000] {
        let mut keys = BTreeSet::new();
        while keys.len() < size {
            keys.insert(rng.next_u32() & 0x00FF_FFFF);
        }
        let sorted_keys: Vec<Key32> = keys.into_iter().collect();

        let built = ExpanseSet32::from_sorted_iter(sorted_keys.iter().copied());

        // Invariant: direct emission must never free intermediate nodes.
        assert_eq!(
            built.live_allocs(),
            built.total_node_allocs(),
            "size {size}: direct emission freed intermediate nodes (live={}, total={})",
            built.live_allocs(),
            built.total_node_allocs()
        );

        // Census total matches live allocations.
        let census = built.node_census();
        assert_eq!(
            census.total(),
            built.live_allocs(),
            "size {size}: census total != live"
        );

        // Differential vs sequential insert.
        let mut inserted = ExpanseSet32::new();
        for &k in &sorted_keys {
            inserted.insert(k);
        }
        assert_eq!(
            built.mem_used(),
            inserted.mem_used(),
            "size {size}: mem_used mismatch"
        );
        assert_eq!(
            built.node_census(),
            inserted.node_census(),
            "size {size}: census mismatch"
        );
    }
}

#[test]
fn test_map32_direct_emission_zero_freed_across_sizes() {
    let mut rng = XorShift32::new(0xDEAD_1234);
    for size in [0, 1, 2, 16, 17, 32, 64, 100, 250, 1000] {
        let mut entries = BTreeMap::new();
        while entries.len() < size {
            let k = rng.next_u32() & 0x00FF_FFFF;
            entries.insert(k, !k ^ 0x55AA);
        }
        let sorted_entries: Vec<(Key32, Value32)> = entries.into_iter().collect();

        let built = ExpanseMap32::from_sorted_iter(sorted_entries.iter().copied());

        // Invariant: direct emission must never free intermediate nodes.
        assert_eq!(
            built.live_allocs(),
            built.total_node_allocs(),
            "size {size}: direct emission freed intermediate nodes (live={}, total={})",
            built.live_allocs(),
            built.total_node_allocs()
        );

        // Census total matches live allocations.
        let census = built.node_census();
        assert_eq!(
            census.total(),
            built.live_allocs(),
            "size {size}: census total != live"
        );

        // Differential vs sequential insert.
        let mut inserted = ExpanseMap32::new();
        for &(k, v) in &sorted_entries {
            inserted.insert(k, v);
        }
        assert_eq!(
            built.mem_used(),
            inserted.mem_used(),
            "size {size}: mem_used mismatch"
        );
        assert_eq!(
            built.node_census(),
            inserted.node_census(),
            "size {size}: census mismatch"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Exact Per-Class Node Census Reaching Every Node Variant
// ---------------------------------------------------------------------------

#[test]
fn test_set32_node_census_all_classes() {
    // A. Single key at root is an immediate edge (set_immed_cap(4) == 1), 0 arena allocs.
    let immed_set = ExpanseSet32::from_sorted_iter([42u32]);
    assert_eq!(immed_set.live_allocs(), 0);
    assert_eq!(immed_set.node_census().bitmap, 0);
    assert_eq!(immed_set.node_census().leaf, 0);

    // B. Linear leaf at root: 2..=24 keys in 0..255.
    let leaf1_set = ExpanseSet32::from_sorted_iter(0..20u32);
    assert_eq!(leaf1_set.node_census().leaf, 1, "must contain linear leaf");
    assert_eq!(leaf1_set.node_census().bitmap, 0);

    // C. Bitmap leaf at level 1: 65..=256 keys in 0..255.
    let bitmap_set = ExpanseSet32::from_sorted_iter(0..100u32);
    assert_eq!(
        bitmap_set.node_census().bitmap,
        1,
        "must contain 1 bitmap leaf"
    );
    assert_eq!(bitmap_set.node_census().leaf, 0);

    // D. BranchL2 (2 children): 2 clusters, each with 15 keys (total 30 > SET_LEAF_MAX 24).
    let l2_keys: Vec<Key32> = (0..15u32)
        .map(|i| (1 << 24) | i)
        .chain((0..15u32).map(|i| (2 << 24) | i))
        .collect();
    let l2_set = ExpanseSet32::from_sorted_iter(l2_keys.iter().copied());
    assert_eq!(l2_set.node_census().l2, 1, "root must be L2 branch");

    // E. BranchL6 (5 children): 5 clusters, each with 6 keys (total 30 > 24).
    let l6_keys: Vec<Key32> = (1..=5u32)
        .flat_map(|c| (0..6u32).map(move |i| (c << 24) | i))
        .collect();
    let l6_set = ExpanseSet32::from_sorted_iter(l6_keys.iter().copied());
    assert_eq!(l6_set.node_census().l6, 1, "root must be L6 branch");

    // F. BranchB (10 children): 10 clusters, each with 3 keys (total 30 > 24).
    let b_keys: Vec<Key32> = (1..=10u32)
        .flat_map(|c| (0..3u32).map(move |i| (c << 24) | i))
        .collect();
    let b_set = ExpanseSet32::from_sorted_iter(b_keys.iter().copied());
    assert_eq!(b_set.node_census().b, 1, "root must be BranchB");

    // G. BranchU (> 192 children): 200 clusters across top byte.
    let u_keys: Vec<Key32> = (1..=200u32).map(|i| i << 24).collect();
    let u_set = ExpanseSet32::from_sorted_iter(u_keys.iter().copied());
    assert_eq!(u_set.node_census().u, 1, "root must be BranchU");

    // Verify each matches sequential insert census and mem_used exactly.
    for (name, set, keys) in [
        ("immed", immed_set, vec![42u32]),
        ("leaf1", leaf1_set, (0..20u32).collect()),
        ("bitmap", bitmap_set, (0..100u32).collect()),
        ("l2", l2_set, l2_keys),
        ("l6", l6_set, l6_keys),
        ("b", b_set, b_keys),
        ("u", u_set, u_keys),
    ] {
        let mut insert_twin = ExpanseSet32::new();
        for &k in &keys {
            insert_twin.insert(k);
        }
        assert_eq!(
            set.node_census(),
            insert_twin.node_census(),
            "{name}: census mismatch vs insert"
        );
        assert_eq!(
            set.mem_used(),
            insert_twin.mem_used(),
            "{name}: mem_used mismatch vs insert"
        );
        assert_eq!(
            set.live_allocs(),
            set.total_node_allocs(),
            "{name}: zero-freed violated"
        );
    }
}

#[test]
fn test_map32_node_census_all_classes() {
    // A. Root single-entry map is a level-4 linear leaf (immediate map edge requires kb <= 3).
    let single_map = ExpanseMap32::from_sorted_iter([(42, 100)]);
    assert_eq!(single_map.node_census().map_bitmap, 0);
    assert_eq!(single_map.node_census().leaf, 1);

    // B. Linear leaf at level 1: <= 16 keys in 0..255.
    let leaf_map = ExpanseMap32::from_sorted_iter((0..15u32).map(|k| (k, k * 2)));
    assert_eq!(leaf_map.node_census().leaf, 1, "must contain linear leaf");
    assert_eq!(leaf_map.node_census().map_bitmap, 0);

    // C. MapBitmap leaf at level 1: > 64 keys in 0..255.
    let bitmap_map = ExpanseMap32::from_sorted_iter((0..100u32).map(|k| (k, k * 3)));
    assert_eq!(
        bitmap_map.node_census().map_bitmap,
        1,
        "must contain MapBitmap"
    );

    // D. BranchL2 (2 children): 2 clusters, each with 10 entries (total 20 > MAP_LEAF_MAX 16).
    let l2_entries: Vec<(Key32, Value32)> = (0..10u32)
        .map(|i| ((1 << 24) | i, i))
        .chain((0..10u32).map(|i| ((2 << 24) | i, i)))
        .collect();
    let l2_map = ExpanseMap32::from_sorted_iter(l2_entries.iter().copied());
    assert_eq!(l2_map.node_census().l2, 1, "root must be L2");

    // E. BranchL6 (5 children): 5 clusters, each with 4 entries (total 20 > 16).
    let l6_entries: Vec<(Key32, Value32)> = (1..=5u32)
        .flat_map(|c| (0..4u32).map(move |i| ((c << 24) | i, i)))
        .collect();
    let l6_map = ExpanseMap32::from_sorted_iter(l6_entries.iter().copied());
    assert_eq!(l6_map.node_census().l6, 1, "root must be L6");

    // F. BranchB (10 children): 10 clusters, each with 2 entries (total 20 > 16).
    let b_entries: Vec<(Key32, Value32)> = (1..=10u32)
        .flat_map(|c| (0..2u32).map(move |i| ((c << 24) | i, i)))
        .collect();
    let b_map = ExpanseMap32::from_sorted_iter(b_entries.iter().copied());
    assert_eq!(b_map.node_census().b, 1, "root must be B");

    // G. BranchU (> 192 children): 200 clusters across top byte.
    let u_entries: Vec<(Key32, Value32)> = (1..=200u32).map(|i| (i << 24, i)).collect();
    let u_map = ExpanseMap32::from_sorted_iter(u_entries.iter().copied());
    assert_eq!(u_map.node_census().u, 1, "root must be U");

    // Verify each matches sequential insert census and mem_used exactly.
    for (name, map, entries) in [
        ("single", single_map, vec![(42, 100)]),
        ("leaf", leaf_map, (0..15u32).map(|k| (k, k * 2)).collect()),
        (
            "bitmap",
            bitmap_map,
            (0..100u32).map(|k| (k, k * 3)).collect(),
        ),
        ("l2", l2_map, l2_entries),
        ("l6", l6_map, l6_entries),
        ("b", b_map, b_entries),
        ("u", u_map, u_entries),
    ] {
        let mut insert_twin = ExpanseMap32::new();
        for &(k, v) in &entries {
            insert_twin.insert(k, v);
        }
        assert_eq!(
            map.node_census(),
            insert_twin.node_census(),
            "{name}: census mismatch vs insert"
        );
        assert_eq!(
            map.mem_used(),
            insert_twin.mem_used(),
            "{name}: mem_used mismatch vs insert"
        );
        assert_eq!(
            map.live_allocs(),
            map.total_node_allocs(),
            "{name}: zero-freed violated"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Out-of-Order Input and Duplicate Handling
// ---------------------------------------------------------------------------

#[test]
fn test_set32_out_of_order_and_duplicates() {
    let mut rng = XorShift32::new(0x8765_4321);
    let mut distinct = BTreeSet::new();
    while distinct.len() < 200 {
        distinct.insert(rng.next_u32() & 0x000F_FFFF);
    }
    let ascending: Vec<Key32> = distinct.into_iter().collect();

    // Reverse input.
    let mut descending = ascending.clone();
    descending.reverse();
    let set_from_desc = ExpanseSet32::from_sorted_iter(descending);
    verify_set_against_model(
        &set_from_desc,
        &ascending.iter().copied().collect(),
        "descending",
    );

    // Permuted with duplicates.
    let mut duplicated = ascending.clone();
    duplicated.extend(ascending.iter().copied());
    duplicated.extend(ascending.iter().copied());
    for i in (1..duplicated.len()).rev() {
        let j = (rng.next_u32() as usize) % (i + 1);
        duplicated.swap(i, j);
    }
    let set_from_dup = ExpanseSet32::from_sorted_iter(duplicated);
    verify_set_against_model(
        &set_from_dup,
        &ascending.iter().copied().collect(),
        "duplicated",
    );
}

#[test]
fn test_map32_out_of_order_and_duplicates_last_value_wins() {
    let mut rng = XorShift32::new(0x1357_9BDF);
    let mut distinct_keys = BTreeSet::new();
    while distinct_keys.len() < 100 {
        distinct_keys.insert(rng.next_u32() & 0x000F_FFFF);
    }
    let keys: Vec<Key32> = distinct_keys.into_iter().collect();

    // Create entry pairs with duplicates where second occurrence has inverted value.
    let mut input = Vec::new();
    let mut expected_model = BTreeMap::new();
    for &k in &keys {
        let v1 = k.wrapping_mul(7);
        let v2 = !v1;
        input.push((k, v1));
        input.push((k, v2)); // v2 comes later, so v2 must win!
        expected_model.insert(k, v2);
    }

    let built = ExpanseMap32::from_sorted_iter(input);
    verify_map_against_model(&built, &expected_model, "last_value_wins");
}

// ---------------------------------------------------------------------------
// 4. Compaction Round-Trip and Survivor Equivalence
// ---------------------------------------------------------------------------

#[test]
fn test_compact_round_trip_preserves_census_and_zero_freed() {
    let mut rng = XorShift32::new(0xF00D_CAFE);
    let mut set = ExpanseSet32::new();
    let mut model_set = BTreeSet::new();
    for _ in 0..500 {
        let k = rng.next_u32() & 0x0007_FFFF;
        set.insert(k);
        model_set.insert(k);
    }

    // Delete 75% of elements.
    let to_remove: Vec<Key32> = model_set.iter().copied().step_by(4).collect();
    for k in to_remove {
        set.remove(k);
        model_set.remove(&k);
    }

    // Call compact().
    set.compact();
    verify_set_against_model(&set, &model_set, "compacted_set");

    // Check against fresh sequential build of survivors.
    let mut fresh_set = ExpanseSet32::new();
    for &k in &model_set {
        fresh_set.insert(k);
    }
    assert_eq!(
        set.mem_used(),
        fresh_set.mem_used(),
        "compacted mem_used vs fresh"
    );
    assert_eq!(
        set.node_census(),
        fresh_set.node_census(),
        "compacted census vs fresh"
    );
    assert_eq!(
        set.live_allocs(),
        set.total_node_allocs(),
        "compacted set must have live == total"
    );

    // Same check for ExpanseMap32.
    let mut map = ExpanseMap32::new();
    let mut model_map = BTreeMap::new();
    for _ in 0..500 {
        let k = rng.next_u32() & 0x0007_FFFF;
        let v = k ^ 0xDEAD;
        map.insert(k, v);
        model_map.insert(k, v);
    }
    let to_remove_map: Vec<Key32> = model_map.keys().copied().step_by(4).collect();
    for k in to_remove_map {
        map.remove(k);
        model_map.remove(&k);
    }

    map.compact();
    verify_map_against_model(&map, &model_map, "compacted_map");

    let mut fresh_map = ExpanseMap32::new();
    for (&k, &v) in &model_map {
        fresh_map.insert(k, v);
    }
    assert_eq!(
        map.mem_used(),
        fresh_map.mem_used(),
        "compacted map mem_used vs fresh"
    );
    assert_eq!(
        map.node_census(),
        fresh_map.node_census(),
        "compacted map census vs fresh"
    );
    assert_eq!(
        map.live_allocs(),
        map.total_node_allocs(),
        "compacted map must have live == total"
    );
}
