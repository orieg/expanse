//! Deterministic dependent node visits per probe tests (Refs #1249).
//!
//! Validates that `probe_visits` mirrors the descent of `get` exactly,
//! correctly attributes edges followed, BranchB subarray loads, and leaf loads,
//! and fails closed if a BranchB subarray load is skipped (§5 scanner-mutation rule).

#![cfg(not(miri))]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[test]
fn test_probe_visits_empty_and_root_leaf() {
    let empty_map = ExpanseMap::new();
    let v_empty = empty_map.probe_visits(42);
    assert_eq!(v_empty.edges_followed, 0);
    assert_eq!(v_empty.branch_b_subarrays, 0);
    assert_eq!(v_empty.leaf_loads, 0);
    assert_eq!(v_empty.total_visits(), 0);
    assert!(!v_empty.found);
    assert_eq!(v_empty.value, None);

    let empty_set = ExpanseSet::new();
    let v_empty_set = empty_set.probe_visits(42);
    assert_eq!(v_empty_set.total_visits(), 0);
    assert!(!v_empty_set.found);

    // Root leaf population (<= 24 keys)
    let mut map = ExpanseMap::new();
    for i in 1..=10u64 {
        map.insert(i * 10, i * 100);
    }
    for i in 1..=10u64 {
        let k = i * 10;
        let v = map.probe_visits(k);
        assert_eq!(v.edges_followed, 0, "root leaf has no branch edges");
        assert_eq!(v.branch_b_subarrays, 0, "root leaf has no BranchB");
        assert_eq!(v.leaf_loads, 1, "root leaf counts as 1 leaf load");
        assert_eq!(v.total_visits(), 1);
        assert!(v.found);
        assert_eq!(v.value, Some(i * 100));
        assert_eq!(v.value, map.get(k));
    }

    // Miss on root leaf
    let v_miss = map.probe_visits(999);
    assert_eq!(v_miss.edges_followed, 0);
    assert_eq!(v_miss.branch_b_subarrays, 0);
    assert_eq!(v_miss.leaf_loads, 1, "miss on root leaf checked the leaf");
    assert_eq!(v_miss.total_visits(), 1);
    assert!(!v_miss.found);
    assert_eq!(v_miss.value, None);
    assert_eq!(v_miss.value, map.get(999));
}

#[test]
fn test_probe_visits_matches_get_for_all_distributions() {
    let dists = [
        "sequential",
        "random",
        "clustered",
        "dense_leaf",
        "linear_leaf",
    ];
    let pop = 10_000;

    for dist in dists {
        let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
        let mut keys = Vec::with_capacity(pop);
        match dist {
            "sequential" => keys.extend(0..pop as u64),
            "random" => keys.extend((0..pop).map(|_| rng.next())),
            "clustered" => {
                let mut base = 0;
                for i in 0..pop as u64 {
                    if i % 256 == 0 {
                        base = rng.next() & !0xFF;
                    }
                    keys.push(base + (i % 256));
                }
            }
            "dense_leaf" => {
                for _ in 0..(pop / 32) {
                    let prefix = rng.next() & !0xFF;
                    for j in 0..32 {
                        keys.push(prefix | (j as u64));
                    }
                }
            }
            "linear_leaf" => {
                for _ in 0..(pop / 15) {
                    let prefix = rng.next() & !0xFF;
                    for j in 0..15 {
                        keys.push(prefix | (j as u64));
                    }
                }
            }
            _ => unreachable!(),
        }

        let mut map = ExpanseMap::new();
        for &k in &keys {
            map.insert(k, !k);
        }

        // Test present keys (hits)
        for &k in keys.iter().take(1000) {
            let v = map.probe_visits(k);
            assert_eq!(v.value, Some(!k), "dist {dist} hit key {k}");
            assert_eq!(v.value, map.get(k), "dist {dist} must match get");
            assert!(v.found);
            assert_eq!(
                v.total_visits(),
                v.edges_followed + v.branch_b_subarrays + v.leaf_loads,
                "total visits invariant"
            );
        }

        // Test absent keys (misses)
        let mut miss_rng = XorShift(0x5EED_F2E5_0000_0002);
        for _ in 0..500 {
            let absent = miss_rng.next();
            if map.get(absent).is_none() {
                let v = map.probe_visits(absent);
                assert_eq!(v.value, None, "dist {dist} miss key {absent}");
                assert_eq!(v.value, map.get(absent), "dist {dist} miss matches get");
                assert!(!v.found);
                assert_eq!(
                    v.total_visits(),
                    v.edges_followed + v.branch_b_subarrays + v.leaf_loads
                );
            }
        }
    }
}

#[test]
fn test_probe_visits_set_matches_contains() {
    let mut rng = XorShift(0xCAFE_BABE_0001);
    let mut set = ExpanseSet::new();
    let mut keys = Vec::new();
    for _ in 0..5_000 {
        let k = rng.next();
        set.insert(k);
        keys.push(k);
    }

    for &k in keys.iter().take(500) {
        let v = set.probe_visits(k);
        assert!(v.found);
        assert_eq!(v.found, set.contains(k));
        assert_eq!(v.value, None);
        assert_eq!(
            v.total_visits(),
            v.edges_followed + v.branch_b_subarrays + v.leaf_loads
        );
    }

    for _ in 0..500 {
        let absent = rng.next();
        if !set.contains(absent) {
            let v = set.probe_visits(absent);
            assert!(!v.found);
            assert_eq!(v.found, set.contains(absent));
            assert_eq!(v.value, None);
        }
    }
}

/// §5 Scanner-mutation / negative-control gate:
/// Verifies that the walker counts BranchB subarray loads, and goes red if
/// a walker skips counting BranchB subarray loads.
#[test]
fn test_walker_detects_skipped_branchb_subarray() {
    // Build a map with BranchB nodes.
    // 65,536 random u32 keys create 256 BranchB nodes at level 3.
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut map = ExpanseMap::new();
    let mut keys = Vec::with_capacity(5_000);
    for _ in 0..5_000 {
        // u32 keys clustered to force BranchB branches
        let k = (rng.next() as u32) as u64;
        map.insert(k, !k);
        keys.push(k);
    }

    let stats = map.stats();
    assert!(
        stats.node_counts.branch_b > 0,
        "precondition: tree must contain BranchB nodes, found {}",
        stats.node_counts.branch_b
    );

    let mut tested_branch_b = 0usize;
    for &k in &keys {
        let real = map.probe_visits(k);
        let skipped = map.probe_visits_skipping_branchb_subarray(k);

        // Both walkers must agree on key lookup result
        assert_eq!(real.value, Some(!k));
        assert_eq!(skipped.value, Some(!k));

        if real.branch_b_subarrays > 0 {
            tested_branch_b += 1;
            // The skipped walker must have 0 BranchB subarray loads
            assert_eq!(
                skipped.branch_b_subarrays, 0,
                "mutated walker must have skipped BranchB subarray count"
            );
            // The real walker must have counted the BranchB subarray loads
            assert!(
                real.branch_b_subarrays > 0,
                "real walker must count BranchB subarray loads"
            );
            // Total visits must strictly differ by the skipped BranchB subarray count
            assert_eq!(
                real.total_visits(),
                skipped.total_visits() + real.branch_b_subarrays,
                "skipping BranchB subarray load must reduce total visits by exactly the skipped count"
            );
        }
    }

    assert!(
        tested_branch_b > 0,
        "precondition: at least one probed key must descend through BranchB, tested {}",
        tested_branch_b
    );
}
