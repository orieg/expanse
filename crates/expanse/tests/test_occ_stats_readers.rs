//! Every `Sync*` reader family counts its optimistic reads (issue #721).
//!
//! `read_ops` and `read_attempts` are what the protocol-health cells divide
//! to publish a restart share, so a family that never bumps them publishes
//! `NOT_INSTRUMENTED` rather than 0% — indistinguishable, from the outside,
//! from a family that restarts never. The string, bytes and blob readers were
//! in that state: they bumped `read_fallbacks` only.
//!
//! Its own test binary, and one single test inside it, for the same reason
//! `no_heap_churn.rs` owns its process: the counters are **process-global**
//! statics, the harness runs `#[test]` functions on parallel threads, and a
//! neighbouring test doing lookups would inflate every delta measured here.
//! An inflated delta is not a harmless flake — it would let this test pass
//! with the counter bump it exists to pin deleted. Adding a second `#[test]`
//! to this file reintroduces exactly that.
//!
//! The counters are compiled out without the `occ-stats` feature (AGENTS.md
//! §2.1 invariant 5: the safeguard must not touch the single-threaded fast
//! path), so this file is empty in the default build and CI runs it as an
//! extra step with the feature on.
#![cfg(feature = "occ-stats")]
// Excluded from the nightly Miri lane rather than given a shard there: that
// lane builds without `occ-stats`, so a shard for this target would run zero
// tests and report green — the silent no-op AGENTS.md §8.1 forbids. Nothing
// here is a new unsafe surface; the reads it makes are the ones the other
// shards already cover.
#![cfg(not(miri))]

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};

/// Wraps a key for `ExpanseStrMap`; `str_key` emits NUL-free keys.
fn tk<B: AsRef<[u8]> + ?Sized>(bytes: &B) -> &expanse_trie::strmap::NulFreeStr {
    expanse_trie::strmap::NulFreeStr::new(bytes.as_ref()).expect("key contains a NUL")
}

/// Lookups per family. Large enough that an accidental single bump
/// somewhere else could not be mistaken for the loop.
const N: u64 = 256;

fn read_ops() -> u64 {
    occ_stats::snapshot()[Stat::ReadOps as usize]
}

fn read_attempts() -> u64 {
    occ_stats::snapshot()[Stat::ReadAttempts as usize]
}

/// Asserts that `body` accounted for exactly `expected` read ops, and at
/// least that many attempts (one attempt per op, more on a restart).
fn assert_counted(family: &str, expected: u64, body: impl FnOnce()) {
    let (ops0, att0) = (read_ops(), read_attempts());
    body();
    let (ops, att) = (read_ops() - ops0, read_attempts() - att0);
    assert_eq!(
        ops, expected,
        "{family}: {expected} optimistic lookups must bump read_ops {expected} times, got {ops} \
         — a family that does not count its reads publishes no restart share (#721)"
    );
    assert!(
        att >= ops,
        "{family}: read_attempts ({att}) must be at least read_ops ({ops}) — every read op \
         enters the retry loop at least once"
    );
}

fn str_key(k: u64) -> Vec<u8> {
    format!("masstree-style-key-{k:08}").into_bytes()
}

#[test]
fn every_sync_reader_family_counts_its_optimistic_reads() {
    assert!(
        occ_stats::enabled(),
        "this test measures counters that are compiled out without the `occ-stats` feature; \
         run it as `cargo test -p expanse-trie --features occ-stats --test test_occ_stats_readers`"
    );
    occ_stats::reset();

    // --- the two families that were already wired: the control arms. ---

    let set = SyncExpanseSet::new();
    for k in 0..N {
        set.insert(k);
    }
    let set_reader = set.reader();
    assert_counted("SetReader::contains", N, || {
        for k in 0..N {
            assert!(set_reader.contains(k));
        }
    });

    let map = SyncExpanseMap::new();
    for k in 0..N {
        map.insert(k, k * 3);
    }
    let map_reader = map.reader();
    assert_counted("MapReader::get", N, || {
        for k in 0..N {
            assert_eq!(map_reader.get(k), Some(k * 3));
        }
    });

    // --- string map: the family the MC2 health cells report on. ---

    let strmap = SyncExpanseStrMap::new();
    for k in 0..N {
        strmap.insert(tk(&str_key(k)), k);
    }
    let str_reader = strmap.reader();
    assert_counted("StrReader::get", N, || {
        for k in 0..N {
            assert_eq!(str_reader.get(tk(&str_key(k))), Some(k));
        }
    });
    // `contains` delegates to `get`, so it must count once, not twice.
    assert_counted("StrReader::contains", N, || {
        for k in 0..N {
            assert!(str_reader.contains(tk(&str_key(k))));
        }
    });

    // --- bytes map (the unordered JudyHS member). ---

    let bytesmap = SyncExpanseBytesMap::new();
    for k in 0..N {
        bytesmap.insert(&str_key(k), k);
    }
    let bytes_reader = bytesmap.reader();
    assert_counted("BytesReader::get", N, || {
        for k in 0..N {
            assert_eq!(bytes_reader.get(&str_key(k)), Some(k));
        }
    });
    assert_counted("BytesReader::contains", N, || {
        for k in 0..N {
            assert!(bytes_reader.contains(&str_key(k)));
        }
    });

    // --- blob map: four counted entry points per reader form, two distinct
    // retry loops. ---

    // Longer than the inline capacity, so the payload lives in the arena and
    // `get_meta` reports the stored metadata rather than the inline `0`.
    let payload = vec![0x5Au8; 64];
    let blobmap = SyncExpanseBlobMap::new();
    for k in 0..N {
        blobmap.insert(k, &payload, 7).expect("arena insert");
    }
    let mut blob_reader = blobmap.reader();
    // `blob_get_walk!` — the payload-resolving loop of `BlobReadGuard::get`.
    assert_counted("BlobReader::get", N, || {
        for k in 0..N {
            let (bytes, meta) = blob_reader.get(k).expect("key present");
            assert_eq!(bytes, payload);
            assert_eq!(meta, 7);
        }
    });
    // `blob_slot_walk!` — the slot-word loop, shared by both of these.
    assert_counted("BlobReader::get_meta", N, || {
        for k in 0..N {
            assert_eq!(blob_reader.get_meta(k), Some(7));
        }
    });
    assert_counted("BlobReader::contains", N, || {
        for k in 0..N {
            assert!(blob_reader.contains(k));
        }
    });
    assert_counted("BlobReader::get_into", N, || {
        let mut buf = [0u8; 64];
        for k in 0..N {
            assert_eq!(blob_reader.get_into(k, &mut buf), Ok(Some((64, 7))));
            assert_eq!(buf[..], payload[..]);
        }
    });
    drop(blob_reader);

    // The owned and detached blob readers (#1194) expand the same two loops:
    // four entry points each, one read op per call.
    let blobmap = std::sync::Arc::new(blobmap);
    let mut owned = blobmap.owned_reader();
    let mut detached = blobmap.detached_reader();
    assert_counted("OwnedBlobReader::get", N, || {
        for k in 0..N {
            assert_eq!(owned.get(k), Some((payload.clone(), 7)));
        }
    });
    assert_counted("OwnedBlobReader::get_into", N, || {
        let mut buf = [0u8; 64];
        for k in 0..N {
            assert_eq!(owned.get_into(k, &mut buf), Ok(Some((64, 7))));
            assert_eq!(buf[..], payload[..]);
        }
    });
    assert_counted("OwnedBlobReader::get_meta", N, || {
        for k in 0..N {
            assert_eq!(owned.get_meta(k), Some(7));
        }
    });
    assert_counted("OwnedBlobReader::contains", N, || {
        for k in 0..N {
            assert!(owned.contains(k));
        }
    });
    assert_counted("DetachedBlobReader::get", N, || {
        for k in 0..N {
            assert_eq!(detached.get(&blobmap, k), Some((payload.clone(), 7)));
        }
    });
    assert_counted("DetachedBlobReader::get_into", N, || {
        let mut buf = [0u8; 64];
        for k in 0..N {
            assert_eq!(detached.get_into(&blobmap, k, &mut buf), Ok(Some((64, 7))));
            assert_eq!(buf[..], payload[..]);
        }
    });
    assert_counted("DetachedBlobReader::get_meta", N, || {
        for k in 0..N {
            assert_eq!(detached.get_meta(&blobmap, k), Some(7));
        }
    });
    assert_counted("DetachedBlobReader::contains", N, || {
        for k in 0..N {
            assert!(detached.contains(&blobmap, k));
        }
    });

    // A detached blob reader handed a map of another collector panics before
    // its walk counts a read op or an attempt (#1194). For `get_meta` and
    // `contains` the attempt is counted before the pin, so an unchanged count
    // also places the check before the first pin; for `get` and `get_into`
    // the pin precedes the counters, and that order is read from the source.
    let foreign = SyncExpanseBlobMap::new();
    foreign.insert(1, &payload, 7).expect("arena insert");
    let rejected = |what: &str, call: &mut dyn FnMut()| {
        let (ops0, att0) = (read_ops(), read_attempts());
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(call));
        assert!(caught.is_err(), "{what}: a foreign map was read");
        assert_eq!(
            (read_ops(), read_attempts()),
            (ops0, att0),
            "{what}: the walk started before the foreign map was rejected"
        );
    };
    rejected("DetachedBlobReader::get", &mut || {
        let _ = detached.get(&foreign, 1);
    });
    rejected("DetachedBlobReader::get_into", &mut || {
        let _ = detached.get_into(&foreign, 1, &mut [0u8; 64]);
    });
    rejected("DetachedBlobReader::get_meta", &mut || {
        let _ = detached.get_meta(&foreign, 1);
    });
    rejected("DetachedBlobReader::contains", &mut || {
        let _ = detached.contains(&foreign, 1);
    });

    // Nothing above ran a concurrent writer, so no walk could have been
    // invalidated: every read op took exactly one attempt and none reached
    // the writer-mutex fallback. This is what makes the equality above a
    // measurement of the new bumps rather than of restart noise.
    let snap = occ_stats::snapshot();
    assert_eq!(
        snap[Stat::ReadAttempts as usize],
        snap[Stat::ReadOps as usize],
        "an uncontended single-threaded run must not restart any optimistic walk"
    );
    assert_eq!(
        snap[Stat::ReadFallbacks as usize],
        0,
        "an uncontended single-threaded run must not fall back to the writer mutex"
    );
}
