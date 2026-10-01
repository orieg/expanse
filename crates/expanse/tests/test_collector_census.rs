//! The collector census of the `Sync*` wrappers (#1310): on a quiesced
//! wrapper built by a deterministic random-order workload, the census's
//! freelist and grace bytes sum to `mem_held()` minus the tree's own share,
//! and to `mem_held() - mem_used()` where the tree's share equals its used
//! bytes; `shrink_to_fit` empties the freelists and returns their bytes.
//!
//! Every build runs on a thread of its own and is read after the thread has
//! joined: a writer thread caches a reader per collector, and the census is
//! exact only with no writer running.
//!
//! Excluded from Miri: tens of thousands of keys. The census walk itself is
//! covered under Miri by the `occ::tests::collector_census_*` unit tests in
//! the Tier-1 filter.
#![cfg(not(miri))]

use expanse_trie::occ::CollectorCensus;
use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};

const N: u64 = 60_000;

fn splitmix64(i: u64) -> u64 {
    let mut z = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Random-order keys: inserted in this order, they grow leaves through their
/// size classes and retire the outgrown blocks to the collector.
fn keys() -> Vec<u64> {
    (0..N).map(splitmix64).collect()
}

fn str_key(k: u64) -> String {
    format!("user:{k:016x}")
}

/// Runs `f` on a fresh thread and waits for it: afterwards no writer runs
/// and the thread's cached reader is gone.
fn on_fresh_thread<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    std::thread::scope(|s| s.spawn(f).join().expect("build panicked"))
}

/// The census is consistent with itself: classes sum to the totals, and the
/// stripes sum to the freelist bytes.
fn assert_self_consistent(label: &str, c: &CollectorCensus) {
    let stripes: usize = c.stripes.iter().map(|s| s.bytes).sum();
    assert_eq!(
        stripes,
        c.free_bytes(),
        "{label}: stripes sum to the freelist bytes"
    );
    for class in &c.classes {
        assert_eq!(
            class.free.bytes,
            class.free.blocks * class.block_bytes,
            "{label}"
        );
        assert_eq!(
            class.grace.bytes,
            class.grace.blocks * class.block_bytes,
            "{label}"
        );
    }
}

/// `census.total_bytes() == mem_held - share`, and, where `share == used`,
/// `== mem_held - used`. Returns the census.
fn assert_totals(
    label: &str,
    census: CollectorCensus,
    mem_held: usize,
    share: usize,
    used: usize,
) -> CollectorCensus {
    assert_self_consistent(label, &census);
    assert!(
        census.total_bytes() > 0,
        "{label}: the workload must leave the collector holding something, or the identity is vacuous"
    );
    assert_eq!(
        census.total_bytes(),
        mem_held - share,
        "{label}: freelist + grace bytes = mem_held - the tree's share"
    );
    assert_eq!(
        share, used,
        "{label}: a tree built after sharing keeps nothing of its own"
    );
    assert_eq!(
        census.total_bytes(),
        mem_held - used,
        "{label}: = mem_held - mem_used"
    );
    census
}

#[test]
fn map_census_totals_match_mem_held() {
    let m = SyncExpanseMap::new();
    let ks = keys();
    on_fresh_thread(|| {
        for &k in &ks {
            m.insert(k, !k);
        }
        for &k in ks.iter().step_by(3) {
            m.remove(k);
        }
    });
    let census = assert_totals(
        "map",
        m.collector_census(),
        m.mem_held(),
        m.with_locked(|t| t.mem_held()),
        m.mem_used(),
    );
    assert!(
        census.free_bytes() > 0,
        "a random-order build leaves freelist blocks"
    );
}

#[test]
fn set_census_totals_match_mem_held() {
    let s = SyncExpanseSet::new();
    let ks = keys();
    on_fresh_thread(|| {
        for &k in &ks {
            s.insert(k);
        }
        for &k in ks.iter().step_by(3) {
            s.remove(k);
        }
    });
    assert_totals(
        "set",
        s.collector_census(),
        s.mem_held(),
        s.with_locked(|t| t.mem_held()),
        s.with_locked(|t| t.mem_used()),
    );
}

/// The string map retires suffix leaves no size class serves, so the
/// unclassed bucket is part of the identity: the workload ends with removals,
/// whose retired suffixes are still in their grace period when it is read.
#[test]
fn strmap_census_totals_match_mem_held_with_unclassed_blocks() {
    let m = SyncExpanseStrMap::new();
    let ks = keys();
    on_fresh_thread(|| {
        for &k in &ks {
            let s = str_key(k);
            m.insert(NulFreeStr::new(s.as_bytes()).unwrap(), k);
        }
        for &k in ks.iter().step_by(3) {
            let s = str_key(k);
            m.remove(NulFreeStr::new(s.as_bytes()).unwrap());
        }
    });
    let census = assert_totals(
        "strmap",
        m.collector_census(),
        m.mem_held(),
        m.with_locked(|t| t.mem_held()),
        m.with_locked(|t| t.mem_used()),
    );
    assert!(
        census.unclassed_grace.blocks > 0,
        "removals retire suffix leaves no size class serves; the last ones are in their grace period"
    );
}

#[test]
fn bytes_and_blob_census_totals_match_mem_held_minus_mem_used() {
    let b = SyncExpanseBytesMap::new();
    let blob = SyncExpanseBlobMap::new();
    let ks = keys();
    on_fresh_thread(|| {
        // One wrapper after the other, so each collector's census reflects
        // only its own writes (the advance cadence is per tree since #1314).
        for &k in &ks {
            b.insert(&k.to_be_bytes(), k);
        }
        for &k in ks.iter().step_by(3) {
            b.remove(&k.to_be_bytes());
        }
        for &k in &ks {
            blob.insert(k, &k.to_le_bytes(), 0).unwrap();
        }
        for &k in ks.iter().step_by(3) {
            blob.remove(k);
        }
    });
    // Both wrappers' `mem_held` is `mem_used` plus the collector's bytes.
    for (label, census, held, used) in [
        ("bytes", b.collector_census(), b.mem_held(), b.mem_used()),
        (
            "blob",
            blob.collector_census(),
            blob.mem_held(),
            blob.mem_used(),
        ),
    ] {
        assert_totals(label, census, held, used, used);
    }
}

/// `shrink_to_fit` releases the freelists and nothing else: afterwards the
/// census shows no freelist bytes, its grace bytes are unchanged, and the
/// bytes returned are the freelist bytes the census showed before.
#[test]
fn shrink_to_fit_returns_the_census_freelist_bytes() {
    let m = SyncExpanseMap::new();
    let ks = keys();
    on_fresh_thread(|| {
        for &k in &ks {
            m.insert(k, !k);
        }
    });
    let before = m.collector_census();
    assert!(before.free_bytes() > 0);
    let released = m.shrink_to_fit();
    let after = m.collector_census();
    assert_eq!(released, before.free_bytes());
    assert_eq!(after.free_bytes(), 0);
    assert!(after.stripes.iter().all(|s| s.blocks == 0));
    assert_eq!(after.grace_bytes(), before.grace_bytes());
}
