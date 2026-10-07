//! Which path the read-modify-write entry points take on the workloads that
//! `benches/rmw_instructions.rs` measures.
//!
//! The bench asserts that every operation succeeded. Success does not name a
//! path: each entry point also succeeds through its serialised fallback. This
//! test runs the same operations over the same populations and reads the
//! fallback counter, and checks the state each arm leaves behind.
// 400,000 operations over 50,000-key maps: not a Miri workload.
#![cfg(not(miri))]
#![cfg(all(feature = "occ-stats", feature = "std", target_pointer_width = "64"))]

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseStrMap,
};

// The generators of `benches/rmw_instructions.rs`, kept identical to it.
const POP: usize = 50_000;

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

type DetHasher = std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;

fn keys() -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    (0..POP).map(|_| rng.next()).collect()
}

fn str_keys() -> Vec<Vec<u8>> {
    (0..POP)
        .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
        .collect()
}

fn tk(bytes: &[u8]) -> &expanse_trie::strmap::NulFreeStr {
    expanse_trie::strmap::NulFreeStr::new(bytes).expect("route keys hold no NUL")
}

fn blob_payload(k: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&k.to_le_bytes());
    out[8..].copy_from_slice(&(!k).to_le_bytes());
    out
}

fn blob_meta(k: u64) -> u32 {
    (k as u32 & 0x00FF_FFFF) | 1
}

/// Lock fallbacks taken while `f` runs.
fn fallbacks(f: impl FnOnce()) -> u64 {
    let before = occ_stats::snapshot()[Stat::LockFallbacks as usize];
    f();
    occ_stats::snapshot()[Stat::LockFallbacks as usize] - before
}

// One test: the counter is process-wide, so the arms run one after another.
#[test]
fn rmw_arms_take_the_paths_they_are_named_for() {
    let pop = POP as u64;

    // sync_map_cas, sync_map_update_rmw
    let ks = keys();
    let map = SyncExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    let n = fallbacks(|| {
        for &k in &ks {
            assert!(
                map.compare_exchange(k, Some(!k), Some((!k).wrapping_add(1)))
                    .is_ok()
            );
        }
    });
    assert_eq!(n, 0, "sync_map_cas took the serialised path {n} time(s)");
    let n = fallbacks(|| {
        for &k in &ks {
            assert!(map.update(k, |v| v.map(|x| x.wrapping_add(1))).is_some());
        }
    });
    assert_eq!(
        n, 0,
        "sync_map_update_rmw took the serialised path {n} time(s)"
    );
    for &k in &ks {
        assert_eq!(map.get(k), Some((!k).wrapping_add(2)));
    }

    // sync_strmap_cas, sync_strmap_update_rmw
    let sk = str_keys();
    let smap = SyncExpanseStrMap::new();
    for (i, k) in sk.iter().enumerate() {
        smap.insert(tk(k), i as u64);
    }
    let n = fallbacks(|| {
        for (i, k) in sk.iter().enumerate() {
            assert!(
                smap.compare_exchange(tk(k), Some(i as u64), Some(i as u64 + 1))
                    .is_ok()
            );
        }
    });
    assert_eq!(n, 0, "sync_strmap_cas took the serialised path {n} time(s)");
    let n = fallbacks(|| {
        for k in &sk {
            assert!(
                smap.update(tk(k), |v| v.map(|x| x.wrapping_add(1)))
                    .is_some()
            );
        }
    });
    assert_eq!(
        n, 0,
        "sync_strmap_update_rmw took the serialised path {n} time(s)"
    );
    for (i, k) in sk.iter().enumerate() {
        assert_eq!(smap.get(tk(k)), Some(i as u64 + 2));
    }

    // sync_bytesmap_cas, sync_bytesmap_update_rmw
    let bmap = SyncExpanseBytesMap::with_hasher(DetHasher::default());
    for (i, k) in sk.iter().enumerate() {
        bmap.insert(k, i as u64);
    }
    let n = fallbacks(|| {
        for (i, k) in sk.iter().enumerate() {
            assert!(
                bmap.compare_exchange(k, Some(i as u64), Some(i as u64 + 1))
                    .is_ok()
            );
        }
    });
    assert_eq!(
        n, 0,
        "sync_bytesmap_cas took the serialised path {n} time(s)"
    );
    let n = fallbacks(|| {
        for k in &sk {
            assert!(bmap.update(k, |v| v.map(|x| x.wrapping_add(1))).is_some());
        }
    });
    assert_eq!(
        n, 0,
        "sync_bytesmap_update_rmw took the serialised path {n} time(s)"
    );

    // sync_bytesmap_cas_remove: value to absent on a bucket's last entry is
    // serialised by design, so the fallback is the path the arm is named for,
    // on every one of its operations.
    let n = fallbacks(|| {
        for (i, k) in sk.iter().enumerate() {
            assert!(bmap.compare_exchange(k, Some(i as u64 + 2), None).is_ok());
        }
    });
    assert_eq!(
        n, pop,
        "sync_bytesmap_cas_remove took the serialised path {n} time(s) of {pop}"
    );
    assert!(
        sk.iter().all(|k| bmap.get(k).is_none()),
        "cas_remove left keys behind"
    );

    // sync_blobmap_cas: the store goes through the arena.
    let blob = SyncExpanseBlobMap::new();
    for &k in &ks {
        blob.insert(k, &blob_payload(k), blob_meta(k))
            .expect("blob insert");
    }
    let n = fallbacks(|| {
        for &k in &ks {
            let (old, new) = (blob_payload(k), blob_payload(!k));
            assert!(
                blob.compare_exchange(k, Some((&old, blob_meta(k))), Some((&new, blob_meta(!k))))
                    .is_ok()
            );
        }
    });
    assert_eq!(
        n, 0,
        "sync_blobmap_cas took the serialised path {n} time(s)"
    );
}
