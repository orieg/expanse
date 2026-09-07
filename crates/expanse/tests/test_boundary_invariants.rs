//! Comprehensive boundary value and invariant tests for Expanse data structures.

use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::map::ExpanseMap;
use expanse_trie::map32::ExpanseMap32;
use expanse_trie::set::ExpanseSet;
use expanse_trie::set32::ExpanseSet32;
use expanse_trie::strmap::ExpanseStrMap;

#[test]
fn test_extreme_u64_boundary_keys() {
    let mut map = ExpanseMap::new();
    let mut set = ExpanseSet::new();

    // 1. Boundary checks on empty set and map (checked_sub(1) / checked_add(1) guards)
    assert_eq!(set.prev_before(0), None);
    assert_eq!(set.next_after(u64::MAX), None);
    assert_eq!(map.prev_before(0), None);
    assert_eq!(map.next_after(u64::MAX), None);

    let boundary_keys = vec![
        0u64,
        1,
        2,
        0x5555_5555_5555_5555,
        0xAAAA_AAAA_AAAA_AAAA,
        u64::MAX / 2,
        u64::MAX - 1,
        u64::MAX,
    ];

    for &k in &boundary_keys {
        assert!(set.insert(k), "set insert failed for {k:#x}");
        assert_eq!(map.insert(k, k ^ 0xDEAD_BEEF), None);
    }

    assert_eq!(set.len(), boundary_keys.len() as u64);
    assert_eq!(map.len(), boundary_keys.len() as u64);

    for &k in &boundary_keys {
        assert!(set.contains(k), "set should contain {k:#x}");
        assert_eq!(map.get(k), Some(k ^ 0xDEAD_BEEF));
    }

    // Single-bit powers of two (64 total)
    let mut bit_set = ExpanseSet::new();
    for bit in 0..64 {
        let k = 1u64 << bit;
        assert!(bit_set.insert(k));
    }
    assert_eq!(bit_set.len(), 64);
    for bit in 0..64 {
        let k = 1u64 << bit;
        assert!(bit_set.contains(k));
    }

    // Successor / Predecessor navigation around extreme boundaries
    let first = set.first().expect("first key");
    assert_eq!(first, 0);
    let last = set.last().expect("last key");
    assert_eq!(last, u64::MAX);

    let next_after_0 = set.next_after(0).expect("next after 0");
    assert_eq!(next_after_0, 1);

    let prev_before_max = set.prev_before(u64::MAX).expect("prev before MAX");
    assert_eq!(prev_before_max, u64::MAX - 1);

    // CRITICAL: Verify that boundary guards NEVER wrap on populated sets/maps
    // If checked_sub(1) in prev_before(0) were replaced with wrapping_sub, it would
    // look for prev_at_or_before(u64::MAX) and incorrectly return Some(u64::MAX).
    assert_eq!(
        set.prev_before(0),
        None,
        "prev_before(0) must return None on populated set"
    );
    assert_eq!(
        map.prev_before(0),
        None,
        "prev_before(0) must return None on populated map"
    );

    // If checked_add(1) in next_after(u64::MAX) were replaced with wrapping_add, it would
    // look for next_at_or_after(0) and incorrectly return Some(0).
    assert_eq!(
        set.next_after(u64::MAX),
        None,
        "next_after(u64::MAX) must return None on populated set"
    );
    assert_eq!(
        map.next_after(u64::MAX),
        None,
        "next_after(u64::MAX) must return None on populated map"
    );
}

#[test]
fn test_extreme_u32_boundary_keys() {
    let mut map = ExpanseMap32::new();
    let mut set = ExpanseSet32::new();

    // 1. Boundary checks on empty set32/map32
    assert_eq!(set.prev(0), None);
    assert_eq!(set.next(u32::MAX), None);
    assert_eq!(map.prev(0), None);
    assert_eq!(map.next(u32::MAX), None);

    let boundary_keys = vec![
        0u32,
        1,
        2,
        0x5555_5555,
        0xAAAA_AAAA,
        u32::MAX / 2,
        u32::MAX - 1,
        u32::MAX,
    ];

    for &k in &boundary_keys {
        assert!(set.insert(k), "set32 insert failed for {k:#x}");
        assert_eq!(map.insert(k, k ^ 0xCAFE), None);
    }

    assert_eq!(set.len(), boundary_keys.len());
    assert_eq!(map.len(), boundary_keys.len());

    for &k in &boundary_keys {
        assert!(set.contains(k), "set32 should contain {k:#x}");
        assert_eq!(map.get(k), Some(k ^ 0xCAFE));
    }

    let first = set.first().expect("first key 32");
    assert_eq!(first, 0);
    let last = set.last().expect("last key 32");
    assert_eq!(last, u32::MAX);

    // CRITICAL: Boundary navigation wrapping guards on 32-bit populated structures
    assert_eq!(set.prev(0), None);
    assert_eq!(map.prev(0), None);
    assert_eq!(set.next(u32::MAX), None);
    assert_eq!(map.next(u32::MAX), None);
}

#[test]
fn test_bytesmap_and_strmap_edge_cases() {
    let mut bytes_map = ExpanseBytesMap::new();

    // 1. Empty byte slice
    assert_eq!(bytes_map.insert(b"", 42), None);
    assert_eq!(bytes_map.get(b""), Some(42));
    assert_eq!(bytes_map.len(), 1);

    // 2. Embedded null bytes in keys
    let null_keys: Vec<&[u8]> = vec![
        b"\x00",
        b"\x00\x00",
        b"foo\x00bar",
        b"foo\x00bar\x00baz",
        b"\x00leading",
        b"trailing\x00",
    ];

    for (idx, &k) in null_keys.iter().enumerate() {
        assert_eq!(bytes_map.insert(k, (idx + 100) as u64), None);
    }

    for (idx, &k) in null_keys.iter().enumerate() {
        assert_eq!(
            bytes_map.get(k),
            Some((idx + 100) as u64),
            "lookup failed for byte key with embedded nulls"
        );
    }

    // 3. Very long key crossing multiple chunk levels
    let long_key = vec![0xABu8; 1500];
    assert_eq!(bytes_map.insert(&long_key, 9999), None);
    assert_eq!(bytes_map.get(&long_key), Some(9999));

    // 4. StrMap empty and prefix hierarchy
    let mut str_map = ExpanseStrMap::new();
    assert_eq!(str_map.insert(b"", 10), None);
    assert_eq!(str_map.insert(b"a", 20), None);
    assert_eq!(str_map.insert(b"aa", 30), None);
    assert_eq!(str_map.insert(b"aaa", 40), None);
    assert_eq!(str_map.insert(b"aab", 50), None);

    assert_eq!(str_map.get(b""), Some(10));
    assert_eq!(str_map.get(b"a"), Some(20));
    assert_eq!(str_map.get(b"aa"), Some(30));
    assert_eq!(str_map.get(b"aaa"), Some(40));
    assert_eq!(str_map.get(b"aab"), Some(50));
    assert_eq!(str_map.len(), 5);

    // StrMap ordered navigation
    assert_eq!(str_map.first().unwrap().0.as_slice(), b"");
    assert_eq!(str_map.last().unwrap().0.as_slice(), b"aab");
}

#[test]
fn test_expanse_map32_removal_invariants() {
    use std::collections::BTreeMap;
    let mut map = ExpanseMap32::new();
    let mut model = BTreeMap::new();

    // 1. Single element insert and remove (null -> immed -> null)
    map.insert(0x1234_5678, 42);
    assert_eq!(map.len(), 1);
    assert_eq!(map.remove(0x1234_5678), Some(42));
    assert_eq!(map.len(), 0);
    assert_eq!(map.mem_used(), 0);

    // 2. Multi-element linear leaf transitions (immed -> leaf -> demote -> immed -> null)
    let keys = [10u32, 20, 30, 40, 50];
    for &k in &keys {
        map.insert(k, k * 2);
    }
    assert_eq!(map.len(), 5);
    // Remove in reverse order
    for &k in keys.iter().rev() {
        assert_eq!(map.remove(k), Some(k * 2));
    }
    assert_eq!(map.len(), 0);
    assert_eq!(map.mem_used(), 0);

    // 3. Demotions across BranchL2, BranchL6, BranchB, and MapBitmap
    let mut rng_state = 0x1234_5678_9ABC_DEF0u64;
    let mut next_u32 = || {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        (rng_state >> 16) as u32
    };

    let count = 1000;
    let mut inserted_keys = Vec::with_capacity(count);
    for _ in 0..count {
        let k = next_u32();
        let v = next_u32();
        if model.insert(k, v).is_none() {
            map.insert(k, v);
            inserted_keys.push(k);
        }
    }
    assert_eq!(map.len(), model.len());

    // Remove half the keys in insertion order (scattered across key space)
    let half = inserted_keys.len() / 2;
    for &k in &inserted_keys[..half] {
        let expected = model.remove(&k);
        let actual = map.remove(k);
        assert_eq!(actual, expected, "removal mismatch for key {k:#x}");
    }
    assert_eq!(map.len(), model.len());
    assert_eq!(map.count_range(0, u32::MAX), model.len());

    // Verify all remaining keys in model are in map
    for (&k, &v) in &model {
        assert_eq!(map.get(k), Some(v));
    }

    // Drain the remaining half
    for &k in &inserted_keys[half..] {
        let expected = model.remove(&k);
        let actual = map.remove(k);
        assert_eq!(
            actual, expected,
            "removal mismatch on second half for key {k:#x}"
        );
    }
    assert_eq!(map.len(), 0);
    assert_eq!(map.count_range(0, u32::MAX), 0);
    assert_eq!(map.mem_used(), 0, "drained map must leave 0 memory in use");
}

#[test]
fn test_expanse_set32_removal_invariants() {
    use std::collections::BTreeSet;
    let mut set = ExpanseSet32::new();
    let mut model = BTreeSet::new();

    let mut rng_state = 0xFEDC_BA98_7654_3210u64;
    let mut next_u32 = || {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        (rng_state >> 16) as u32
    };

    let count = 1000;
    let mut inserted_keys = Vec::with_capacity(count);
    for _ in 0..count {
        let k = next_u32();
        if model.insert(k) {
            set.insert(k);
            inserted_keys.push(k);
        }
    }
    assert_eq!(set.len(), model.len());

    // Remove every key
    for &k in &inserted_keys {
        assert!(model.remove(&k));
        assert!(set.remove(k), "set removal failed for {k:#x}");
        assert!(!set.contains(k));
    }
    assert_eq!(set.len(), 0);
    assert_eq!(set.mem_used(), 0, "drained set must leave 0 memory in use");
}

#[test]
fn test_map_bitmap_full_drain_invariant() {
    let mut map = ExpanseMap32::new();
    // 100 keys in the same level-1 expanse forces promotion to MapBitmap (MAP_BITMAP_ENTER_32 = 64)
    for i in 0..100u32 {
        map.insert(0x5555_0000 | i, i * 10);
    }
    assert_eq!(map.len(), 100);

    // Drain down through demotion threshold (MAP_BITMAP_LEAVE_32 = 48) to 0
    for i in 0..100u32 {
        assert_eq!(map.remove(0x5555_0000 | i), Some(i * 10));
    }
    assert_eq!(map.len(), 0);
    assert_eq!(
        map.mem_used(),
        0,
        "fully drained MapBitmap must leave 0 memory in use"
    );
}

#[test]
fn test_branch_b_digit_removal_count_invariant() {
    let mut map = ExpanseMap32::new();
    // 25 distinct second-byte digits under prefix 0x1200_0000 exceeds MAP_LEAF_MAX_32 (16)
    // and BRANCH_L6_CAP_32 (6), forcing a BranchB at level 3.
    for d in 0..25u32 {
        map.insert(0x1200_0000 | (d << 16) | 1, d * 100);
    }
    assert_eq!(map.len(), 25);
    assert_eq!(map.count_range(0, u32::MAX), 25);

    // Remove one digit: triggers branch_remove_digit on BranchB
    assert_eq!(map.remove(0x1200_0001), Some(0));
    assert_eq!(map.len(), 24);
    // count_range reads subtree_count on the BranchB child of root
    assert_eq!(map.count_range(0, u32::MAX), 24);
}

// ---------------------------------------------------------------------------
// The 64-bit node ladder, crossed in both directions (#763)
//
// This file was named for boundary invariants and exercised only the 32-bit
// ladder: every 64-bit capacity, demotion floor and hysteresis constant was
// referenced by name in `test_encoding_reference_sync.rs` and
// `test_visualizer_sync.rs`, which pin a constant's *value* for doc-sync, and
// nowhere by *behaviour*. An off-by-one in a promote/demote comparison (`<` vs
// `<=`), or a hysteresis band collapsed to zero width, passed the whole suite.
//
// The engine does not expose which node form holds a population, and these
// tests deliberately do not guess at it. They assert the property that must
// hold whichever form is in play — agreement with an ordered oracle at
// `N - 1`, `N` and `N + 1` on the way up, and again at `floor + 1`, `floor`
// and `floor - 1` on the way back down. That is what an off-by-one breaks.
// ---------------------------------------------------------------------------

use expanse_trie::types::{
    BITMAP_TO_UNCOMPRESSED_THRESHOLD, BRANCH_L3_CAP, BRANCH_L7_CAP, BRANCHB_TO_L7_DOWN,
    BRANCHU_TO_B_DOWN, LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN, ROOT_LEAF_CAP,
};
use std::collections::{BTreeMap, BTreeSet};

/// `n` keys that all descend into the same leaf: identical in every byte but
/// the last, so no branch is created above them by construction.
fn keys_in_one_leaf(n: usize) -> Vec<u64> {
    (0..n as u64).map(|i| 0x1234_5678_9ABC_DE00 | i).collect()
}

/// `n` keys occupying `n` distinct subexpanses of the branch at `level`, so the
/// count that drives a branch's promotion is exactly `n`.
fn keys_in_n_subexpanses(level: u32, n: usize) -> Vec<u64> {
    let shift = 8 * (7 - level);
    (0..n as u64)
        .map(|i| 0xAA00_0000_0000_0000u64 | (i << shift) | 0x11)
        .collect()
}

/// Insert `keys` one at a time, checking every engine against an ordered oracle
/// after each step. Then remove them in reverse, checking again — so a demotion
/// floor is crossed with the structure fully populated above it.
fn walk_up_and_down(label: &str, keys: &[u64]) {
    let mut map = ExpanseMap::new();
    let mut set = ExpanseSet::new();
    let mut omap: BTreeMap<u64, u64> = BTreeMap::new();
    let mut oset: BTreeSet<u64> = BTreeSet::new();

    for (step, &k) in keys.iter().enumerate() {
        map.insert(k, k ^ 0xDEAD_BEEF);
        set.insert(k);
        omap.insert(k, k ^ 0xDEAD_BEEF);
        oset.insert(k);
        agree(label, "insert", step, &map, &set, &omap, &oset);
    }
    for (step, &k) in keys.iter().enumerate().rev() {
        map.remove(k);
        set.remove(k);
        omap.remove(&k);
        oset.remove(&k);
        agree(label, "remove", step, &map, &set, &omap, &oset);
    }
    assert_eq!(
        map.len(),
        0,
        "{label}: map not empty after removing every key"
    );
    assert_eq!(
        set.len(),
        0,
        "{label}: set not empty after removing every key"
    );
}

fn agree(
    label: &str,
    phase: &str,
    step: usize,
    map: &ExpanseMap,
    set: &ExpanseSet,
    omap: &BTreeMap<u64, u64>,
    oset: &BTreeSet<u64>,
) {
    assert_eq!(
        map.len(),
        omap.len() as u64,
        "{label}/{phase} step {step}: map len"
    );
    assert_eq!(
        set.len(),
        oset.len() as u64,
        "{label}/{phase} step {step}: set len"
    );
    for (k, v) in omap {
        assert_eq!(
            map.get(*k),
            Some(*v),
            "{label}/{phase} step {step}: map lost {k:#x}"
        );
        assert!(
            set.contains(*k),
            "{label}/{phase} step {step}: set lost {k:#x}"
        );
    }
    // Ordered iteration is where a mis-sized node shows up as a lost or
    // duplicated key rather than a wrong length.
    let got: Vec<u64> = set.iter().collect();
    let want: Vec<u64> = oset.iter().copied().collect();
    assert_eq!(
        got, want,
        "{label}/{phase} step {step}: set iteration order"
    );
    let got_m: Vec<u64> = map.iter().map(|(k, _)| k).collect();
    assert_eq!(
        got_m, want,
        "{label}/{phase} step {step}: map iteration order"
    );
}

#[test]
fn ladder_leaf_capacities_cross_in_both_directions() {
    // ROOT_LEAF_CAP (31), LEAF_CAP (32), LEAF1_CAP (25) and its demotion floor
    // LEAFB1_DOWN (21). Each is walked from below the threshold to above it and
    // back, so both the promote and the demote comparison are exercised.
    for &cap in &[ROOT_LEAF_CAP, LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN] {
        for n in [cap - 1, cap, cap + 1] {
            walk_up_and_down(&format!("leaf n={n} (cap {cap})"), &keys_in_one_leaf(n));
        }
    }
}

/// The node forms holding a population, as the engine reports them.
///
/// `stats()` is what makes this test able to see a *structurally benign*
/// off-by-one — one that moves the promotion by one key without losing data.
/// An oracle comparison cannot: it only fails when a threshold error corrupts
/// the structure, and the common `<` vs `<=` slip does not. Both checks are
/// kept: the oracle catches the destructive class, the census the silent one.
fn set_of(keys: &[u64]) -> ExpanseSet {
    let mut s = ExpanseSet::new();
    for &k in keys {
        s.insert(k);
    }
    s
}

#[test]
fn ladder_branch_capacities_cross_in_both_directions() {
    // BRANCH_L3_CAP (3), BRANCH_L7_CAP (7) and the hysteresis floor beneath it
    // BRANCHB_TO_L7_DOWN (6). Populations are subexpanse counts, not key counts,
    // so the keys are spread one per subexpanse.
    for &cap in &[BRANCH_L3_CAP, BRANCH_L7_CAP, BRANCHB_TO_L7_DOWN] {
        for n in [cap - 1, cap, cap + 1] {
            walk_up_and_down(
                &format!("branch n={n} (cap {cap})"),
                &keys_in_n_subexpanses(1, n),
            );
        }
    }
}

#[test]
fn root_leaf_cascades_into_a_branch_one_key_past_its_capacity() {
    // The census, not the oracle. An oracle comparison only fails when a
    // threshold error corrupts the structure; the common `<` vs `<=` slip moves
    // the promotion by one key without losing anything, and every behavioural
    // assertion in this file passes it. Verified: mutating `pop_val <
    // ROOT_LEAF_CAP` to `<=` in map.rs leaves the oracle walks green.
    let forms = |n: usize| {
        let c = set_of(&keys_in_one_leaf(n)).stats().node_counts;
        (c.leaf_linear, c.leaf_bitmap, c.branch_l3)
    };
    assert_eq!(
        forms(ROOT_LEAF_CAP),
        (1, 0, 0),
        "a population of exactly ROOT_LEAF_CAP should still be a flat root leaf"
    );
    let (_, _, branched) = forms(ROOT_LEAF_CAP + 1);
    assert_eq!(
        branched, 1,
        "ROOT_LEAF_CAP + 1 keys should have cascaded the root leaf into a branch"
    );
}

#[test]
fn a_leaf_converts_to_bitmap_one_key_past_leaf_cap() {
    let forms = |n: usize| {
        let c = set_of(&keys_in_one_leaf(n)).stats().node_counts;
        (c.leaf_linear, c.leaf_bitmap)
    };
    assert_eq!(
        forms(LEAF_CAP),
        (1, 0),
        "at LEAF_CAP the leaf should still be linear"
    );
    assert_eq!(
        forms(LEAF_CAP + 1),
        (0, 1),
        "one key past LEAF_CAP the leaf should have converted to a bitmap leaf"
    );
}

#[test]
fn bitmap_branch_converts_to_uncompressed_one_child_past_the_threshold() {
    // NOTE the off-by-one against the constant's own wording. `types.rs`
    // documents BITMAP_TO_UNCOMPRESSED_THRESHOLD as "the populated-subexpanse
    // count *at which* a bitmap branch converts", but the conversion is
    // measured one child later: 192 children are still a bitmap branch and 193
    // are uncompressed. The test pins the engine's behaviour and names the
    // discrepancy rather than quietly encoding 193, because which of the two is
    // wrong — the comparison or the doc comment — is a question for whoever
    // owns the ladder, and a test that silently agreed with the code would
    // remove the evidence.
    let forms = |n: usize| {
        let c = set_of(&keys_in_n_subexpanses(1, n)).stats().node_counts;
        (c.branch_b, c.branch_u)
    };
    assert_eq!(
        forms(BITMAP_TO_UNCOMPRESSED_THRESHOLD),
        (1, 0),
        "at BITMAP_TO_UNCOMPRESSED_THRESHOLD children the branch is still a bitmap"
    );
    assert_eq!(
        forms(BITMAP_TO_UNCOMPRESSED_THRESHOLD + 1),
        (0, 1),
        "one child past the threshold the branch should be uncompressed"
    );
}

#[test]
fn the_bitmap_branch_hysteresis_band_does_not_thrash_at_its_edge() {
    // The band is 192 up / 191 down. Taking one child back off an uncompressed
    // branch must NOT demote it — that one-index gap is what stops an
    // alternating insert/remove pair rebuilding the node every time.
    let keys = keys_in_n_subexpanses(1, BITMAP_TO_UNCOMPRESSED_THRESHOLD + 1);
    let mut set = set_of(&keys);
    assert_eq!(
        set.stats().node_counts.branch_u,
        1,
        "expected an uncompressed branch"
    );

    set.remove(*keys.last().unwrap());
    assert_eq!(
        set.stats().node_counts.branch_u,
        1,
        "one child below the promotion point the branch demoted immediately: the \
         {BITMAP_TO_UNCOMPRESSED_THRESHOLD}/{BRANCHU_TO_B_DOWN} hysteresis band is not holding"
    );

    // And it must give way eventually, or the band is a ratchet rather than a band.
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    for &k in sorted.iter().rev() {
        if set.stats().node_counts.branch_u == 0 {
            break;
        }
        set.remove(k);
    }
    assert_eq!(
        set.stats().node_counts.branch_u,
        0,
        "the branch never demoted out of its uncompressed form"
    );
}

#[test]
fn ladder_bitmap_to_uncompressed_crosses_in_both_directions() {
    // 192 up / 191 down — the widest band on the ladder, and the one whose
    // collapse would thrash a branch between two forms on every insert/remove
    // pair at the boundary.
    for n in [
        BRANCHU_TO_B_DOWN - 1,
        BRANCHU_TO_B_DOWN,
        BITMAP_TO_UNCOMPRESSED_THRESHOLD,
        BITMAP_TO_UNCOMPRESSED_THRESHOLD + 1,
    ] {
        walk_up_and_down(&format!("bitmap n={n}"), &keys_in_n_subexpanses(1, n));
    }
}

#[test]
fn ladder_hysteresis_bands_are_wider_than_zero() {
    // AGENTS.md section 2.1.6 derives every demotion floor from its capacity so
    // the band cannot silently decay. A band of zero width means a structure
    // that promotes and demotes on the same population, thrashing on an
    // alternating insert/remove pair. Pinned as a property of the constants,
    // then demonstrated on the engine at the widest band.
    // The band widths are compile-time facts about the constants, so they are
    // pinned at compile time -- a collapsed band fails the build, not a test
    // run. Same idiom as the layout invariants in `node.rs` and `types32.rs`.
    const _: () = assert!(
        BRANCH_L7_CAP > BRANCHB_TO_L7_DOWN,
        "BranchB->L7 band collapsed"
    );
    const _: () = assert!(
        BITMAP_TO_UNCOMPRESSED_THRESHOLD > BRANCHU_TO_B_DOWN,
        "BranchU->B band collapsed"
    );
    const _: () = assert!(
        LEAF1_CAP > LEAFB1_DOWN,
        "bitmap-leaf demotion band collapsed"
    );

    let keys = keys_in_n_subexpanses(1, BITMAP_TO_UNCOMPRESSED_THRESHOLD);
    let mut set = ExpanseSet::new();
    for &k in &keys {
        set.insert(k);
    }
    let last = *keys.last().unwrap();
    for round in 0..8 {
        set.remove(last);
        assert_eq!(
            set.len(),
            keys.len() as u64 - 1,
            "round {round}: len after remove"
        );
        assert!(
            !set.contains(last),
            "round {round}: key present after remove"
        );
        set.insert(last);
        assert_eq!(
            set.len(),
            keys.len() as u64,
            "round {round}: len after re-insert"
        );
        assert!(
            set.contains(last),
            "round {round}: key absent after re-insert"
        );
        let got: Vec<u64> = set.iter().collect();
        let mut want = keys.clone();
        want.sort_unstable();
        assert_eq!(got, want, "round {round}: iteration after boundary churn");
    }
}

// ---------------------------------------------------------------------------
// Public surface with no caller and no test (#763 items 3 and 4)
//
// Each of these is reachable from outside the crate and had neither a caller in
// the repository nor a test. A public escape hatch with no caller is an
// untested contract either way, so the choice is to test it or withdraw it;
// these are tested, which also fixes what the contract *is*.
// ---------------------------------------------------------------------------

// `DomainSet::contains_ordinal` and `ExpanseBlobMap::arena_mut` are the other
// two callerless public items #763 names. They are deliberately NOT covered
// here: both need a decision about what their contract *is* (does a foreign
// ordinal mismatch or miss; is a direct arena write through the mutable hatch
// legal while the map holds entries), and inventing an answer in a test would
// fix the contract by accident. Left on #763.

#[test]
fn get_slot_ptr_addresses_the_live_value_and_is_none_when_absent() {
    let mut map = ExpanseMap::new();
    map.insert(42, 0xAAAA);
    assert_eq!(map.get_slot_ptr(99), None, "absent key must yield no slot");

    let p = map.get_slot_ptr(42).expect("present key must yield a slot");
    // SAFETY: `p` points at 42's value slot and no structural mutation has
    // happened since it was handed out, which is the documented validity
    // window (the JudyL contract in the doc comment above `get_slot_ptr`).
    let seen = unsafe { *p.as_ptr() };
    assert_eq!(
        seen, 0xAAAA,
        "slot pointer does not address the stored value"
    );

    // A non-structural overwrite is visible through the same pointer.
    map.insert(42, 0xBBBB);
    // SAFETY: as above; `insert` over an existing key replaces a value in
    // place and is not a structural mutation of the map.
    let seen = unsafe { *p.as_ptr() };
    assert_eq!(
        seen, 0xBBBB,
        "slot pointer went stale on an in-place value overwrite"
    );
}
