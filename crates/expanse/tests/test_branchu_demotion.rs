//! Optimistic removals keep every `BranchU` above its demotion floor
//! (Refs #1079).
//!
//! An optimistic removal whose child empties nulls that child's slot in a
//! `BranchU` parent in place. Before #1079 it did so unconditionally, and a
//! branch could sit at or below `BRANCHU_TO_B_DOWN` (191 digits) where the
//! plain engine would have demoted it to a `BranchB`; `validate()` rejected
//! the tree. Every such store now goes through one routine,
//! `sync::null_branch_u_slot`, which falls back to the exclusive remove
//! (`BranchSplitKind::DemoteU`) when the store would reach the floor, and the
//! exclusive remove demotes the branch.
//!
//! This file holds the wrapper-level reproductions for the map and the set,
//! a drain that validates after every removal, and the structural census of
//! the routine's call sites. The bytes map and the string map keep their
//! reproductions beside their private validators (`bytesmap::tests`,
//! `strmap::tests`).
// Kept as its own attribute, in this exact form, for the nightly Miri shard
// census (`scripts/check_miri_shards.py`). Each test builds and validates
// trees of hundreds of keys after every step; the Miri-visible regression
// test lives in the Tier-1 filter instead
// (`sync::tests::map_wrapper_survives_the_root_promotion_under_miri`).
#![cfg(not(miri))]
#![cfg(all(feature = "std", target_pointer_width = "64"))]

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};

fn splitmix64(i: u64) -> u64 {
    let mut z = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The issue's reproduction on the map wrapper: 400 keys put the root at a
/// `BranchU` (about 202 distinct top digits), and removing every third key
/// leaves about 169. Red on `main` before #1079 with "uncompressed branch
/// population 169 at or below demotion threshold 191".
#[test]
fn sync_map_remove_keeps_branch_u_above_its_floor() {
    let map = SyncExpanseMap::new();
    for i in 0..400u64 {
        map.insert(splitmix64(i), i);
    }
    map.with_locked(|m| m.validate_defensive())
        .expect("precondition: valid before the removals");
    assert_eq!(
        map.with_locked(|m| m.stats().node_counts.branch_u),
        1,
        "precondition: the root is an uncompressed branch"
    );
    for i in (0..400u64).step_by(3) {
        assert_eq!(map.remove(splitmix64(i)), Some(i));
    }
    map.with_locked(|m| m.validate_defensive())
        .expect("the removals left a valid tree");
    assert_eq!(
        map.with_locked(|m| m.stats().node_counts.branch_u),
        0,
        "the root crossed its floor and was demoted"
    );
    for i in 0..400u64 {
        let want = (i % 3 != 0).then_some(i);
        assert_eq!(map.get(splitmix64(i)), want, "key {i}");
    }
    // The plain engine on the same sequence is the control: it demotes.
    let mut plain = ExpanseMap::new();
    for i in 0..400u64 {
        plain.insert(splitmix64(i), i);
    }
    for i in (0..400u64).step_by(3) {
        plain.remove(splitmix64(i));
    }
    plain.validate();
}

/// As above, on the set wrapper (`set.rs` reported the same message).
#[test]
fn sync_set_remove_keeps_branch_u_above_its_floor() {
    let set = SyncExpanseSet::new();
    for i in 0..400u64 {
        set.insert(splitmix64(i));
    }
    set.with_locked(|s| s.validate_defensive())
        .expect("precondition: valid before the removals");
    assert_eq!(
        set.with_locked(|s| s.stats().node_counts.branch_u),
        1,
        "precondition: the root is an uncompressed branch"
    );
    for i in (0..400u64).step_by(3) {
        assert!(set.remove(splitmix64(i)));
    }
    set.with_locked(|s| s.validate_defensive())
        .expect("the removals left a valid tree");
    assert_eq!(
        set.with_locked(|s| s.stats().node_counts.branch_u),
        0,
        "the root crossed its floor and was demoted"
    );
    for i in 0..400u64 {
        assert_eq!(set.contains(splitmix64(i)), i % 3 != 0, "key {i}");
    }
    let mut plain = ExpanseSet::new();
    for i in 0..400u64 {
        plain.insert(splitmix64(i));
    }
    for i in (0..400u64).step_by(3) {
        plain.remove(splitmix64(i));
    }
    plain.validate();
}

/// Keys that put a `BranchU` below the top of the tree: two top digits (a
/// linear root branch), 200 level-7 digits under each (a `BranchU` apiece),
/// and two keys under each level-7 digit (a two-key immediate). 800 keys.
fn deep_keys() -> Vec<u64> {
    let mut keys = Vec::new();
    for top in [0x11u64, 0x22] {
        for d7 in 0..200u64 {
            for low in [0x01u64, 0x02] {
                keys.push((top << 56) | (d7 << 48) | (0x0033_4455_6677 << 8) | low);
            }
        }
    }
    keys
}

/// Fisher–Yates with the file's splitmix64, so the drain order is fixed.
fn shuffled(mut keys: Vec<u64>, seed: u64) -> Vec<u64> {
    for i in (1..keys.len()).rev() {
        let j = (splitmix64(seed ^ i as u64) % (i as u64 + 1)) as usize;
        keys.swap(i, j);
    }
    keys
}

/// Drains a tree with a `BranchU` at level 7 under each of two top digits,
/// in a shuffled order, validating after every removal: every crossing of
/// the floor, in both branches, is checked at the step it happens, and the
/// tree ends empty. Red before #1079 at the first removal that left a
/// branch at 191 digits.
#[test]
fn sync_map_deep_drain_validates_after_every_removal() {
    let keys = deep_keys();
    let map = SyncExpanseMap::new();
    for (i, &k) in keys.iter().enumerate() {
        map.insert(k, i as u64);
    }
    map.with_locked(|m| m.validate_defensive())
        .expect("precondition: valid after the fill");
    assert_eq!(
        map.with_locked(|m| m.stats().node_counts.branch_u),
        2,
        "precondition: an uncompressed branch under each top digit"
    );
    let order = shuffled(keys.clone(), 0x1079);
    for (step, &k) in order.iter().enumerate() {
        assert!(map.remove(k).is_some(), "remove of {k:#x}");
        if let Err(e) = map.with_locked(|m| m.validate_defensive()) {
            panic!("step {step}: after removing {k:#x}: {e}");
        }
    }
    assert_eq!(map.len(), 0);
}

/// The set twin of the deep drain.
#[test]
fn sync_set_deep_drain_validates_after_every_removal() {
    let keys = deep_keys();
    let set = SyncExpanseSet::new();
    for &k in &keys {
        set.insert(k);
    }
    set.with_locked(|s| s.validate_defensive())
        .expect("precondition: valid after the fill");
    assert_eq!(
        set.with_locked(|s| s.stats().node_counts.branch_u),
        2,
        "precondition: an uncompressed branch under each top digit"
    );
    let order = shuffled(keys.clone(), 0x0107_95e7);
    for (step, &k) in order.iter().enumerate() {
        assert!(set.remove(k), "remove of {k:#x}");
        if let Err(e) = set.with_locked(|s| s.validate_defensive()) {
            panic!("step {step}: after removing {k:#x}: {e}");
        }
    }
    assert_eq!(set.len(), 0);
}

/// Strips `//` line comments and `/* */` block comments, leaving string
/// literals intact, so the census below counts code and not prose (AGENTS.md
/// §5: a structural scanner must turn red when the code it guards is
/// commented out). The same routine as `test_fallback_attribution.rs`'s,
/// with the same stated gaps: a character literal holding a quote or a
/// slash is read as ordinary text, and a raw string literal is treated as an
/// ordinary one. None of the scanned files holds such a literal on a line
/// the census counts.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0usize;
    let mut in_str = false;
    let mut in_line = false;
    let mut block = 0usize;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if in_line {
            if c == b'\n' {
                in_line = false;
                out.push(c);
            }
            i += 1;
        } else if block > 0 {
            if c == b'*' && next == Some(b'/') {
                block -= 1;
                i += 2;
            } else if c == b'/' && next == Some(b'*') {
                block += 1;
                i += 2;
            } else {
                if c == b'\n' {
                    out.push(c);
                }
                i += 1;
            }
        } else if in_str {
            if c == b'\\' {
                out.push(c);
                if let Some(n) = next {
                    out.push(n);
                }
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            out.push(c);
            i += 1;
        } else if c == b'/' && next == Some(b'/') {
            in_line = true;
            i += 2;
        } else if c == b'/' && next == Some(b'*') {
            block = 1;
            i += 2;
        } else {
            if c == b'"' {
                in_str = true;
            }
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8(out).expect("stripping ASCII delimiters preserves UTF-8 boundaries")
}

/// The code of a source file up to its unit-test module, comments stripped.
fn production_code(src: &str) -> String {
    let prod = match src.find("\nmod tests {") {
        Some(pos) => &src[..pos],
        None => src,
    };
    strip_comments(prod)
}

/// The structural census of #1079's fix, one assertion per clause, each
/// shown to turn red on its own mutation (recorded in the PR).
///
/// 1. Six optimistic null stores into a `BranchU` — three in
///    `olc_remove_set`, three in `olc_remove_map_body!` — each call the one
///    routine. Commenting a call out drops the count.
/// 2. The routine holds the only optimistic store of a null edge through
///    `edge_ptr`: a site that stores directly, bypassing the density check,
///    raises the count.
/// 3. The routine is the only producer of the `DemoteU` fallback.
/// 4. The routine decides the floor with the shared predicate
///    (`mutate::branch_u_below_floor`), and so do the exclusive walks' U → B
///    demotions (two per walk file) and the validator; no file compares
///    against `BRANCHU_TO_B_DOWN` by hand outside the predicate itself.
#[test]
fn branch_u_null_stores_route_through_one_routine() {
    let sync = production_code(include_str!("../src/sync.rs"));
    let mutate = production_code(include_str!("../src/mutate.rs"));
    let mutate_map = production_code(include_str!("../src/mutate_map.rs"));
    let validate = production_code(include_str!("../src/validate.rs"));

    // Clause 1: the call sites. Every call passes the ancestor frame.
    assert_eq!(
        sync.matches("null_branch_u_slot(&parent,").count(),
        6,
        "the six optimistic BranchU null stores must each call null_branch_u_slot"
    );

    // Clause 2: no store of a null edge bypasses the routine.
    assert_eq!(
        sync.matches("Edge::store_at::<true>(edge_ptr, Edge::NULL)")
            .count(),
        1,
        "only null_branch_u_slot may store a null edge into a BranchU slot optimistically"
    );

    // Clause 3: one producer of the DemoteU fallback.
    assert_eq!(
        sync.matches("branch_split(BranchSplitKind::DemoteU)")
            .count(),
        1,
        "only null_branch_u_slot may return the DemoteU fallback"
    );

    // Clause 4: one predicate for the floor.
    let pred = "mutate::branch_u_below_floor(";
    assert_eq!(
        sync.matches(pred).count(),
        1,
        "sync.rs: the routine's floor test"
    );
    assert_eq!(
        mutate.matches(pred).count(),
        2,
        "mutate.rs: both walks' U -> B demotion"
    );
    assert_eq!(
        mutate_map.matches(pred).count(),
        2,
        "mutate_map.rs: both walks' U -> B demotion"
    );
    assert_eq!(
        validate.matches(pred).count(),
        1,
        "validate.rs: the BranchU floor check"
    );
    // The predicate's own body is the one hand comparison against the floor.
    let by_hand = |code: &str| {
        code.matches("<= BRANCHU_TO_B_DOWN").count()
            + code.matches("<= crate::types::BRANCHU_TO_B_DOWN").count()
    };
    for (name, code, allowed) in [
        ("sync.rs", &sync, 0),
        ("mutate.rs", &mutate, 1),
        ("mutate_map.rs", &mutate_map, 0),
        ("validate.rs", &validate, 0),
    ] {
        assert_eq!(
            by_hand(code),
            allowed,
            "{name}: compare against the floor through branch_u_below_floor, not by hand"
        );
    }
}
