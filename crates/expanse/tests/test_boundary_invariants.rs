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
