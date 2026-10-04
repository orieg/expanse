//! Memory retention and compaction verification for 32-bit types (`ExpanseSet32`, `ExpanseMap32`).
//!
//! Gates evaluated per `docs/benchmarks/remove_retention/METHODOLOGY.md` Section 13:
//! - G-held (L460): `mem_held() ÷ held_fresh <= 1.10`, `mem_used() == fresh.mem_used()`.
//! - G-peak (L470): `held_before + held_compact <= held_before + held_fresh * 1.10`.
//! - G-valid (L495): Structural and behavioral validation after `compact()`,
//!   differential model checking vs `BTreeSet`/`BTreeMap`, and proptest interleaving.
//! - Shared tree contract: `compact()` on a deferred arena (`is_deferred()`) is a strict no-op.
#![cfg(not(miri))]

use expanse_trie::map32::ExpanseMap32;
use expanse_trie::set32::ExpanseSet32;
use expanse_trie::types32::{Key32, Value32};
use std::collections::{BTreeMap, BTreeSet};

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

/// Validates internal integrity and model equivalence of an `ExpanseSet32`.
fn validate_set32(set: &ExpanseSet32, model: &BTreeSet<Key32>) {
    assert_eq!(set.len(), model.len(), "length mismatch");
    assert_eq!(set.is_empty(), model.is_empty(), "is_empty mismatch");

    // Ordered forward iteration matches model exactly.
    let fwd: Vec<Key32> = set.iter().collect();
    let model_vec: Vec<Key32> = model.iter().copied().collect();
    assert_eq!(fwd, model_vec, "forward iteration mismatch");

    // Forward iteration is strictly ascending.
    if !fwd.is_empty() {
        assert!(
            fwd.windows(2).all(|w| w[0] < w[1]),
            "forward iteration not strictly ascending"
        );
    }

    // Reverse iteration matches reverse model.
    let rev: Vec<Key32> = set.iter_rev().collect();
    let mut model_rev = model_vec.clone();
    model_rev.reverse();
    assert_eq!(rev, model_rev, "reverse iteration mismatch");

    // First / last endpoints.
    assert_eq!(set.first(), model.iter().next().copied(), "first mismatch");
    assert_eq!(
        set.last(),
        model.iter().next_back().copied(),
        "last mismatch"
    );

    // Point lookup for every element in the model.
    for &k in model {
        assert!(set.contains(k), "missing key {k:#x}");
    }

    // Probe misses around present keys.
    for &k in model.iter().take(50) {
        if k > 0 && !model.contains(&(k - 1)) {
            assert!(!set.contains(k - 1), "false positive at {:#x}", k - 1);
        }
        if k < Key32::MAX && !model.contains(&(k + 1)) {
            assert!(!set.contains(k + 1), "false positive at {:#x}", k + 1);
        }
    }
}

/// Validates internal integrity and model equivalence of an `ExpanseMap32`.
fn validate_map32(map: &ExpanseMap32, model: &BTreeMap<Key32, Value32>) {
    assert_eq!(map.len(), model.len(), "length mismatch");
    assert_eq!(map.is_empty(), model.is_empty(), "is_empty mismatch");

    // Ordered forward iteration matches model exactly.
    let fwd: Vec<(Key32, Value32)> = map.iter().collect();
    let model_vec: Vec<(Key32, Value32)> = model.iter().map(|(&k, &v)| (k, v)).collect();
    assert_eq!(fwd, model_vec, "forward iteration mismatch");

    // Keys are strictly ascending.
    if !fwd.is_empty() {
        assert!(
            fwd.windows(2).all(|w| w[0].0 < w[1].0),
            "keys not strictly ascending"
        );
    }

    // Reverse iteration matches reverse model.
    let rev: Vec<(Key32, Value32)> = map.iter_rev().collect();
    let mut model_rev = model_vec.clone();
    model_rev.reverse();
    assert_eq!(rev, model_rev, "reverse iteration mismatch");

    // First / last endpoints.
    assert_eq!(
        map.first(),
        model.iter().next().map(|(&k, &v)| (k, v)),
        "first mismatch"
    );
    assert_eq!(
        map.last(),
        model.iter().next_back().map(|(&k, &v)| (k, v)),
        "last mismatch"
    );

    // Point lookup for every element in the model.
    for (&k, &v) in model {
        assert_eq!(map.get(k), Some(v), "missing or wrong value at {k:#x}");
        assert!(map.contains_key(k), "missing key {k:#x}");
    }

    // Probe misses around present keys.
    for &k in model.keys().take(50) {
        if k > 0 && !model.contains_key(&(k - 1)) {
            assert_eq!(map.get(k - 1), None, "false positive at {:#x}", k - 1);
            assert!(!map.contains_key(k - 1));
        }
        if k < Key32::MAX && !model.contains_key(&(k + 1)) {
            assert_eq!(map.get(k + 1), None, "false positive at {:#x}", k + 1);
            assert!(!map.contains_key(k + 1));
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Empty and Single-Element Trees
// ---------------------------------------------------------------------------

#[test]
fn compact_empty_set_and_map() {
    let mut s = ExpanseSet32::new();
    s.compact();
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert_eq!(s.mem_used(), 0);
    assert_eq!(s.mem_held(), 0);
    validate_set32(&s, &BTreeSet::new());

    let mut m = ExpanseMap32::new();
    m.compact();
    assert_eq!(m.len(), 0);
    assert!(m.is_empty());
    assert_eq!(m.mem_used(), 0);
    assert_eq!(m.mem_held(), 0);
    validate_map32(&m, &BTreeMap::new());
}

#[test]
fn compact_single_element() {
    let mut s = ExpanseSet32::new();
    s.insert(42);
    s.compact();
    assert_eq!(s.len(), 1);
    assert!(s.contains(42));
    assert_eq!(s.first(), Some(42));
    assert_eq!(s.last(), Some(42));
    let model: BTreeSet<Key32> = [42].into_iter().collect();
    validate_set32(&s, &model);

    let mut m = ExpanseMap32::new();
    m.insert(42, 100);
    m.compact();
    assert_eq!(m.len(), 1);
    assert_eq!(m.get(42), Some(100));
    assert_eq!(m.first(), Some((42, 100)));
    let model_m: BTreeMap<Key32, Value32> = [(42, 100)].into_iter().collect();
    validate_map32(&m, &model_m);
}

// ---------------------------------------------------------------------------
// 2. Small Population (Root Leaf / Immediate Edge Boundaries)
// ---------------------------------------------------------------------------

#[test]
fn compact_root_leaf_set_and_map() {
    // Populate up to 64 keys, remove down to 8 keys (settling into root leaf/immediate).
    let mut s = ExpanseSet32::new();
    let mut model_s = BTreeSet::new();
    for k in 0..64u32 {
        s.insert(k * 7);
        model_s.insert(k * 7);
    }
    for k in 8..64u32 {
        assert!(s.remove(k * 7));
        model_s.remove(&(k * 7));
    }
    let mut fresh_s = ExpanseSet32::new();
    for &k in &model_s {
        fresh_s.insert(k);
    }

    s.compact();
    validate_set32(&s, &model_s);
    assert_eq!(s.mem_used(), fresh_s.mem_used(), "G-held mem_used set");
    assert!(
        s.mem_held() <= (fresh_s.mem_held() as f64 * 1.10).ceil() as usize,
        "G-held mem_held set: {} vs fresh {}",
        s.mem_held(),
        fresh_s.mem_held()
    );

    let mut m = ExpanseMap32::new();
    let mut model_m = BTreeMap::new();
    for k in 0..64u32 {
        m.insert(k * 7, !k);
        model_m.insert(k * 7, !k);
    }
    for k in 8..64u32 {
        assert_eq!(m.remove(k * 7), Some(!k));
        model_m.remove(&(k * 7));
    }
    let mut fresh_m = ExpanseMap32::new();
    for (&k, &v) in &model_m {
        fresh_m.insert(k, v);
    }

    m.compact();
    validate_map32(&m, &model_m);
    assert_eq!(m.mem_used(), fresh_m.mem_used(), "G-held mem_used map");
    assert!(
        m.mem_held() <= (fresh_m.mem_held() as f64 * 1.10).ceil() as usize,
        "G-held mem_held map: {} vs fresh {}",
        m.mem_held(),
        fresh_m.mem_held()
    );
}

// ---------------------------------------------------------------------------
// 3. G-Held & G-Peak Retention Tests Across Shapes
// ---------------------------------------------------------------------------

fn check_g_held_and_g_peak_set(name: &str, keys: &[Key32], keep_indices: &[usize]) {
    let mut set = ExpanseSet32::new();
    for &k in keys {
        set.insert(k);
    }

    let to_keep: BTreeSet<Key32> = keep_indices.iter().map(|&i| keys[i]).collect();
    for &k in keys {
        if !to_keep.contains(&k) {
            set.remove(k);
        }
    }

    let held_before = set.mem_held();

    // Independent fresh build of identical survivor keys.
    let mut fresh = ExpanseSet32::new();
    for &k in &to_keep {
        fresh.insert(k);
    }
    let held_fresh = fresh.mem_held();
    let used_fresh = fresh.mem_used();

    // Call compact().
    set.compact();
    let held_after = set.mem_held();
    let used_after = set.mem_used();

    // G-held: mem_held ÷ held_fresh <= 1.10.
    if held_fresh > 0 {
        let ratio = held_after as f64 / held_fresh as f64;
        assert!(
            ratio <= 1.10,
            "{name}: G-held violated for set: held_after={held_after}, held_fresh={held_fresh}, ratio={ratio:.4}"
        );
    } else {
        assert_eq!(
            held_after, 0,
            "{name}: held_after must be 0 when fresh is 0"
        );
    }

    // Independent from_sorted_iter rebuild (same run per §13).
    let rebuilt = ExpanseSet32::from_sorted_iter(to_keep.iter().copied());
    let used_rebuilt = rebuilt.mem_used();

    // G-held: mem_used equals used_rebuilt and is <= fresh build.
    assert_eq!(
        used_after, used_rebuilt,
        "{name}: G-held mem_used mismatch for set: after={used_after}, rebuilt={used_rebuilt}"
    );
    assert!(
        used_after <= used_fresh,
        "{name}: G-held mem_used exceeds fresh build: after={used_after}, fresh={used_fresh}"
    );

    // G-peak: held_before + held_after <= held_before + held_fresh * 1.10.
    let peak_held = held_before + held_after;
    let peak_ceiling = held_before as f64 + (held_fresh as f64 * 1.10);
    assert!(
        peak_held as f64 <= peak_ceiling + 1.0,
        "{name}: G-peak violated for set: peak={peak_held}, ceiling={peak_ceiling}"
    );

    // Validator check on compacted tree.
    validate_set32(&set, &to_keep);

    // Compacted tree continues to work: can remove remaining keys, then refill.
    let survivors: Vec<Key32> = to_keep.iter().copied().collect();
    if let Some(&first) = survivors.first() {
        assert!(set.remove(first));
        assert!(!set.contains(first));
        assert!(set.insert(first));
        assert!(set.contains(first));
    }
}

fn check_g_held_and_g_peak_map(name: &str, keys: &[Key32], keep_indices: &[usize]) {
    let mut map = ExpanseMap32::new();
    for &k in keys {
        map.insert(k, !k);
    }

    let to_keep: BTreeMap<Key32, Value32> =
        keep_indices.iter().map(|&i| (keys[i], !keys[i])).collect();
    for &k in keys {
        if !to_keep.contains_key(&k) {
            map.remove(k);
        }
    }

    let held_before = map.mem_held();

    // Independent fresh build of identical survivor keys.
    let mut fresh = ExpanseMap32::new();
    for (&k, &v) in &to_keep {
        fresh.insert(k, v);
    }
    let held_fresh = fresh.mem_held();
    let used_fresh = fresh.mem_used();

    // Call compact().
    map.compact();
    let held_after = map.mem_held();
    let used_after = map.mem_used();

    // G-held: mem_held ÷ held_fresh <= 1.10.
    if held_fresh > 0 {
        let ratio = held_after as f64 / held_fresh as f64;
        assert!(
            ratio <= 1.10,
            "{name}: G-held violated for map: held_after={held_after}, held_fresh={held_fresh}, ratio={ratio:.4}"
        );
    } else {
        assert_eq!(
            held_after, 0,
            "{name}: held_after must be 0 when fresh is 0"
        );
    }

    // G-held: mem_used equals fresh build.
    assert_eq!(
        used_after, used_fresh,
        "{name}: G-held mem_used mismatch for map: after={used_after}, fresh={used_fresh}"
    );

    // G-peak: held_before + held_after <= held_before + held_fresh * 1.10.
    let peak_held = held_before + held_after;
    let peak_ceiling = held_before as f64 + (held_fresh as f64 * 1.10);
    assert!(
        peak_held as f64 <= peak_ceiling + 1.0,
        "{name}: G-peak violated for map: peak={peak_held}, ceiling={peak_ceiling}"
    );

    // Validator check on compacted tree.
    validate_map32(&map, &to_keep);

    // Compacted tree continues to work.
    if let Some((&first, &v)) = to_keep.iter().next() {
        assert_eq!(map.remove(first), Some(v));
        assert_eq!(map.get(first), None);
        assert_eq!(map.insert(first, v), None);
        assert_eq!(map.get(first), Some(v));
    }
}

#[test]
fn retention_uniform_random_28bit() {
    // 28-bit uniform keys (top 4 bits 0) spanning 4,096 2-byte expanses.
    // Drained from 20,000 to 5,000 keys (λ = 4.88 -> 1.22 keys/expanse).
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mask = (1u64 << 28) - 1;
    let mut seen = std::collections::HashSet::with_capacity(20_000);
    let mut keys = Vec::with_capacity(20_000);
    while keys.len() < 20_000 {
        let k = (rng.next() & mask) as Key32;
        if seen.insert(k) {
            keys.push(k);
        }
    }
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 4 == 0).collect();

    check_g_held_and_g_peak_set("uniform_28bit", &keys, &keep_indices);
    check_g_held_and_g_peak_map("uniform_28bit", &keys, &keep_indices);
}

#[test]
fn retention_clustered_expanses() {
    // 32 clusters of 256 keys each sharing prefix (Level 4/3 branch structures).
    // Drained down to 1 key per cluster (surviving 32 keys).
    let mut keys = Vec::with_capacity(32 * 256);
    for c in 0..32u32 {
        let prefix = (c << 16) | 0xAA00;
        for i in 0..256u32 {
            keys.push(prefix | i);
        }
    }
    // Keep 1 in every 256 keys (1 key per cluster).
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 256 == 0).collect();

    check_g_held_and_g_peak_set("clustered", &keys, &keep_indices);
    check_g_held_and_g_peak_map("clustered", &keys, &keep_indices);
}

#[test]
fn retention_sequential_dense_run() {
    // Dense run 0..10,000 keys drained down to 500 keys (keeping every 20th).
    let keys: Vec<Key32> = (0..10_000u32).collect();
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 20 == 0).collect();

    check_g_held_and_g_peak_set("sequential", &keys, &keep_indices);
    check_g_held_and_g_peak_map("sequential", &keys, &keep_indices);
}

#[test]
fn retention_sparse_high_byte_keys() {
    // Keys spread across high bytes (one key per top expanse).
    // Drained from 2,000 to 200 keys.
    let keys: Vec<Key32> = (0..2_000u32).map(|i| (i << 12) | (i & 0xFF)).collect();
    let keep_indices: Vec<usize> = (0..keys.len()).filter(|i| i % 10 == 0).collect();

    check_g_held_and_g_peak_set("sparse", &keys, &keep_indices);
    check_g_held_and_g_peak_map("sparse", &keys, &keep_indices);
}

// ---------------------------------------------------------------------------
// 4. Compact vs Shrink-to-Fit Comparison
// ---------------------------------------------------------------------------

#[test]
fn compact_improves_retention_over_shrink_to_fit_alone() {
    // Build 64 expanses with 60 keys each, creating branch structures (Level 4 -> Level 3).
    // Drain each expanse down to 2 keys.
    let mut s = ExpanseSet32::new();
    let mut to_keep = Vec::new();
    for e in 0..64u32 {
        let prefix = e << 16;
        for i in 0..60u32 {
            let k = prefix | (i * 4);
            s.insert(k);
            if i < 2 {
                to_keep.push(k);
            }
        }
    }
    for e in 0..64u32 {
        let prefix = e << 16;
        for i in 2..60u32 {
            let k = prefix | (i * 4);
            s.remove(k);
        }
    }

    let held_drained = s.mem_held();
    s.shrink_to_fit();
    let held_shrunk = s.mem_held();
    assert!(
        held_shrunk <= held_drained,
        "shrink_to_fit returns idle slots"
    );

    // Clone tree to compare with compact.
    let mut s_compact = s.clone();
    s_compact.compact();
    let held_compact = s_compact.mem_held();

    // Independent fresh build of the surviving keys.
    let mut fresh = ExpanseSet32::new();
    for &k in &to_keep {
        fresh.insert(k);
    }
    let held_fresh = fresh.mem_held();

    // Verification: compact achieves strictly lower or equal held memory compared to shrink_to_fit.
    assert!(
        held_compact <= held_shrunk,
        "compact must hold <= shrink_to_fit: compact={held_compact}, shrunk={held_shrunk}"
    );
    assert_eq!(
        s_compact.mem_used(),
        fresh.mem_used(),
        "compact mem_used equals fresh"
    );
    assert!(
        held_compact as f64 / held_fresh as f64 <= 1.10,
        "compact held / fresh <= 1.10: {held_compact} vs {held_fresh}"
    );
}

// ---------------------------------------------------------------------------
// 5. Deferred Arena (sync32 Shared Tree) No-Op Contract
// ---------------------------------------------------------------------------

#[test]
fn compact_is_no_op_on_deferred_arena_set() {
    let mut s = ExpanseSet32::with_fixed_arena(256, 128);
    let used_before = s.mem_used();
    let held_before = s.mem_held();
    let len_before = s.len();
    assert!(held_before > 0, "fixed arena preallocates table capacity");

    // Call compact on the deferred set: must be a strict no-op.
    s.compact();

    assert_eq!(s.len(), len_before, "len changed on deferred set");
    assert_eq!(
        s.mem_used(),
        used_before,
        "mem_used changed on deferred set"
    );
    assert_eq!(
        s.mem_held(),
        held_before,
        "mem_held changed on deferred set"
    );
    let items_after: Vec<Key32> = s.iter().collect();
    assert!(items_after.is_empty(), "items changed on deferred set");
}

#[test]
fn compact_is_no_op_on_deferred_arena_map() {
    let mut m = ExpanseMap32::with_fixed_arena(256, 128);
    let used_before = m.mem_used();
    let held_before = m.mem_held();
    let len_before = m.len();
    assert!(held_before > 0, "fixed arena preallocates table capacity");

    // Call compact on the deferred map: must be a strict no-op.
    m.compact();

    assert_eq!(m.len(), len_before, "len changed on deferred map");
    assert_eq!(
        m.mem_used(),
        used_before,
        "mem_used changed on deferred map"
    );
    assert_eq!(
        m.mem_held(),
        held_before,
        "mem_held changed on deferred map"
    );
    let items_after: Vec<(Key32, Value32)> = m.iter().collect();
    assert!(items_after.is_empty(), "items changed on deferred map");
}

// ---------------------------------------------------------------------------
// 6. Differential Interleaved Testing
// ---------------------------------------------------------------------------

#[test]
fn differential_interleaved_mutations_and_compact() {
    let mut s = ExpanseSet32::new();
    let mut model_s = BTreeSet::new();
    let mut m = ExpanseMap32::new();
    let mut model_m = BTreeMap::new();

    let mut rng = XorShift(0xDEAD_BEEF_CAFE_0001);
    for cycle in 0..500 {
        let op = rng.next() % 5;
        let k = (rng.next() & 0xFFFF) as Key32;
        match op {
            0 | 1 => {
                // Insert
                let v = (rng.next() & 0xFFFFFFFF) as Value32;
                assert_eq!(s.insert(k), model_s.insert(k));
                assert_eq!(m.insert(k, v), model_m.insert(k, v));
            }
            2 => {
                // Remove
                assert_eq!(s.remove(k), model_s.remove(&k));
                assert_eq!(m.remove(k), model_m.remove(&k));
            }
            3 => {
                // Point query
                assert_eq!(s.contains(k), model_s.contains(&k));
                assert_eq!(m.get(k), model_m.get(&k).copied());
            }
            4 => {
                // Periodic compact
                s.compact();
                m.compact();
                validate_set32(&s, &model_s);
                validate_map32(&m, &model_m);
            }
            _ => unreachable!(),
        }

        if cycle % 100 == 0 {
            validate_set32(&s, &model_s);
            validate_map32(&m, &model_m);
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Property-Based Testing (Interleaved Compact)
// ---------------------------------------------------------------------------

#[cfg(not(miri))]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    #[derive(Debug, Clone)]
    enum Op {
        Insert(u32, u32),
        Remove(u32),
        Compact,
    }

    fn op_strategy() -> impl Strategy<Value = Op> {
        prop_oneof![
            4 => (any::<u32>(), any::<u32>()).prop_map(|(k, v)| Op::Insert(k & 0x1FFFF, v)),
            3 => (any::<u32>()).prop_map(|k| Op::Remove(k & 0x1FFFF)),
            1 => Just(Op::Compact),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn proptest_compact_interleaved_operations(ops in prop::collection::vec(op_strategy(), 1..100)) {
            let mut s = ExpanseSet32::new();
            let mut model_s = BTreeSet::new();
            let mut m = ExpanseMap32::new();
            let mut model_m = BTreeMap::new();

            for op in ops {
                match op {
                    Op::Insert(k, v) => {
                        assert_eq!(s.insert(k), model_s.insert(k));
                        assert_eq!(m.insert(k, v), model_m.insert(k, v));
                    }
                    Op::Remove(k) => {
                        assert_eq!(s.remove(k), model_s.remove(&k));
                        assert_eq!(m.remove(k), model_m.remove(&k));
                    }
                    Op::Compact => {
                        s.compact();
                        m.compact();
                        validate_set32(&s, &model_s);
                        validate_map32(&m, &model_m);
                    }
                }
            }

            validate_set32(&s, &model_s);
            validate_map32(&m, &model_m);
        }
    }
}
