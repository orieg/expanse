//! The collector's cumulative counters through the `Sync*` wrappers
//! (feature `collector-census`, #1310): they agree with the census class by
//! class on a quiesced wrapper, reuse rises when the writer that retired a
//! class allocates it again, and `shrink_to_fit` counts exactly the bytes it
//! returns.
//!
//! Every build runs on a thread of its own and the counters are read with no
//! writer running. Run by `scripts/test_collector_census.sh`, which fails a
//! binary that runs zero tests.
//!
//! Excluded from Miri: tens of thousands of keys.
#![cfg(not(miri))]
#![cfg(feature = "collector-census")]

use expanse_trie::occ::{CollectorCensus, CollectorCounters};
use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseStrMap};

const N: u64 = 60_000;

fn splitmix64(i: u64) -> u64 {
    let mut z = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn str_key(k: u64) -> String {
    format!("user:{k:016x}")
}

fn on_fresh_thread<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    std::thread::scope(|s| s.spawn(f).join().expect("build panicked"))
}

/// The identities stated on `ClassCounters` and `UnclassedCounters`, for a
/// collector no writer is using that has not drained.
fn assert_counters_match_census(label: &str, s: &CollectorCensus, k: &CollectorCounters) {
    assert_eq!(s.classes.len(), k.classes.len());
    for (sc, kc) in s.classes.iter().zip(&k.classes) {
        assert_eq!((sc.block_bytes, sc.align), (kc.block_bytes, kc.align));
        assert_eq!(
            sc.free.blocks as u64,
            kc.reclaimed + kc.recycled - kc.reused - kc.released,
            "{label}: freelist blocks of class ({}, {})",
            sc.block_bytes,
            sc.align
        );
        assert_eq!(
            sc.grace.blocks as u64,
            kc.retired - kc.reclaimed,
            "{label}: grace blocks of class ({}, {})",
            sc.block_bytes,
            sc.align
        );
    }
    assert_eq!(
        s.unclassed_grace.blocks as u64,
        k.unclassed.retired_blocks - k.unclassed.released_blocks,
        "{label}: unclassed grace blocks"
    );
    assert_eq!(
        s.unclassed_grace.bytes as u64,
        k.unclassed.retired_bytes - k.unclassed.released_bytes,
        "{label}: unclassed grace bytes"
    );
}

#[test]
fn counters_match_the_census_on_quiesced_wrappers() {
    let m = SyncExpanseMap::new();
    let sm = SyncExpanseStrMap::new();
    on_fresh_thread(|| {
        // One wrapper after the other, so each collector's census reflects
        // only its own writes (the advance cadence is per tree since #1314).
        for k in (0..N).map(splitmix64) {
            m.insert(k, !k);
        }
        for k in (0..N).step_by(3).map(splitmix64) {
            m.remove(k);
        }
        for k in (0..N).map(splitmix64) {
            let s = str_key(k);
            sm.insert(NulFreeStr::new(s.as_bytes()).unwrap(), k);
        }
        for k in (0..N).step_by(3).map(splitmix64) {
            let s = str_key(k);
            sm.remove(NulFreeStr::new(s.as_bytes()).unwrap());
        }
    });
    let mk = m.collector_counters();
    assert!(
        mk.classes
            .iter()
            .any(|c| c.retired > 0 && c.reclaimed > 0 && c.reused > 0)
    );
    assert_counters_match_census("map", &m.collector_census(), &mk);
    let sk = sm.collector_counters();
    assert!(sk.unclassed.retired_blocks > 0 && sk.unclassed.released_blocks > 0);
    assert_counters_match_census("strmap", &sm.collector_census(), &sk);
}

/// A writer that removes keys frees the blocks its leaves shrink out of;
/// re-inserting the same keys from the same writer grows the leaves back
/// into those classes and takes the blocks from its own stripe's freelists:
/// `reused` rises in a class that had freelist blocks before the
/// re-insertion.
#[test]
fn reused_rises_when_the_same_writer_reallocates_a_freed_class() {
    let m = SyncExpanseMap::new();
    on_fresh_thread(|| {
        for k in (0..N).map(splitmix64) {
            m.insert(k, !k);
        }
        for k in (0..N).step_by(3).map(splitmix64) {
            m.remove(k);
        }
        let census = m.collector_census();
        let before = m.collector_counters();
        for k in (0..N).step_by(3).map(splitmix64) {
            m.insert(k, !k);
        }
        let after = m.collector_counters();
        assert!(after.reused_blocks() > before.reused_blocks());
        let rose = census
            .classes
            .iter()
            .zip(before.classes.iter().zip(&after.classes))
            .filter(|(c, (b, a))| c.free.blocks > 0 && a.reused > b.reused)
            .count();
        assert!(
            rose > 0,
            "no class with freelist blocks before the re-insertion was reused during it"
        );
    });
}

/// After `shrink_to_fit` with no writer running, the census shows no
/// freelist bytes and the `released` counters rose by exactly the bytes the
/// call returned.
#[test]
fn shrink_to_fit_counts_exactly_the_bytes_it_returned() {
    let m = SyncExpanseMap::new();
    let sm = SyncExpanseStrMap::new();
    on_fresh_thread(|| {
        // One wrapper after the other (see above).
        for k in (0..N).map(splitmix64) {
            m.insert(k, !k);
        }
        for k in (0..N).map(splitmix64) {
            let s = str_key(k);
            sm.insert(NulFreeStr::new(s.as_bytes()).unwrap(), k);
        }
    });
    for (label, before, released, census, after) in [
        {
            let before = m.collector_counters();
            let released = m.shrink_to_fit();
            (
                "map",
                before,
                released,
                m.collector_census(),
                m.collector_counters(),
            )
        },
        {
            let before = sm.collector_counters();
            let released = sm.shrink_to_fit();
            (
                "strmap",
                before,
                released,
                sm.collector_census(),
                sm.collector_counters(),
            )
        },
    ] {
        assert!(
            released > 0,
            "{label}: the build left freelist blocks to release"
        );
        assert_eq!(
            census.free_bytes(),
            0,
            "{label}: freelists empty after the shrink"
        );
        assert_eq!(
            after.released_bytes() - before.released_bytes(),
            released as u64,
            "{label}: released counters rose by the bytes returned"
        );
        assert_counters_match_census(label, &census, &after);
    }
}
