//! `Clone` is the supported snapshot of an `ExpanseMap` / `ExpanseSet`
//! (#1103): a snapshot taken before a write keeps reading what it held when
//! it was taken.
//!
//! The requested form of the test is "take a snapshot, insert or overwrite a
//! key in a leaf the snapshot shares, assert the snapshot still reads the old
//! value". A copy of the root edge fails it — pinned by
//! `map::tests::a_copied_root_edge_sees_later_writes` — and a clone passes it,
//! because it shares no node with the original.

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
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

/// Keys spanning the root leaf, packed runs and a sparse tail, so the
/// snapshot shares linear leaves, bitmap leaves and branches with the map.
fn keys(n: u64, seed: u64) -> Vec<u64> {
    let mut rng = XorShift(seed);
    let mut out: Vec<u64> = (0..n / 2).collect();
    out.extend((0..n / 4).map(|i| 0x0100_0000 + (i % 200) + ((i / 200) << 16)));
    out.extend((0..n / 4).map(|_| rng.next()));
    out
}

fn n() -> u64 {
    if cfg!(miri) { 512 } else { 20_000 }
}

#[test]
fn a_map_snapshot_keeps_its_values_after_writes() {
    let ks = keys(n(), 1);
    let mut map = ExpanseMap::new();
    let mut model = BTreeMap::new();
    for (i, &k) in ks.iter().enumerate() {
        map.insert(k, i as u64);
        model.insert(k, i as u64);
    }
    let snapshot = map.clone();
    let frozen = model.clone();
    assert_eq!(snapshot.len(), frozen.len() as u64);

    // Overwrite, insert next to existing keys (the same leaves), and remove.
    for (i, &k) in ks.iter().enumerate() {
        match i % 3 {
            0 => {
                map.insert(k, !k);
            }
            1 => {
                map.insert(k ^ 1, 1);
            }
            _ => {
                map.remove(k);
            }
        }
    }

    for (&k, &v) in &frozen {
        assert_eq!(snapshot.get(k), Some(v), "snapshot lost or changed {k:#x}");
    }
    assert_eq!(snapshot.len(), frozen.len() as u64);
    assert!(
        snapshot.iter().eq(frozen.iter().map(|(&k, &v)| (k, v))),
        "snapshot iteration differs from the frozen model"
    );
    snapshot.validate();
    map.validate();
}

#[test]
fn a_map_clone_costs_one_insert_built_tree() {
    let ks = keys(n(), 2);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(k, k);
    }
    let copy = map.clone();
    // `mem_used` is fixed by the key set, not by insertion order
    // (`test_mem_used_order_invariant.rs`), so the ascending rebuild matches
    // the original byte for byte.
    assert_eq!(copy.mem_used(), map.mem_used());
    // And the copy owns its memory: dropping the original leaves it intact.
    drop(map);
    for &k in &ks {
        assert_eq!(copy.get(k), Some(k));
    }
}

#[test]
fn a_set_snapshot_keeps_its_keys_after_writes() {
    let ks = keys(n(), 3);
    let mut set: ExpanseSet = ks.iter().copied().collect();
    let frozen: BTreeSet<u64> = ks.iter().copied().collect();
    let snapshot = set.clone();
    assert!(
        snapshot.mem_used() <= set.mem_used(),
        "the bulk build is never less compact"
    );

    for (i, &k) in ks.iter().enumerate() {
        if i % 2 == 0 {
            set.remove(k);
        } else {
            set.insert(k ^ 1);
        }
    }

    assert_eq!(snapshot.len(), frozen.len() as u64);
    assert!(snapshot.iter().eq(frozen.iter().copied()));
    snapshot.validate();
    set.validate();
}
