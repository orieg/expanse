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
