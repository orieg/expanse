//! Which path the bytes `get_or_insert` linearizability mixes put their
//! removals on.
//!
//! `tests/linearizability.rs` runs `SyncExpanseBytesMap::get_or_insert` against
//! concurrent `compare_exchange(.., Some(v), None)` on three keys: with each
//! key alone in its bucket (`tree_last_entry`, exercising
//! `olc_cas_remove_bucket_cas_map`, #1396) and with all three keys sharing one
//! collision bucket (`tree_shared_bucket`, exercising
//! `olc_cas_replace_bucket_map`). A linearizability check passes even if a path
//! falls back to the lock, so this test reads the fallback counter on the same
//! map constructions, single-threaded, and pins that both mixes stay on the
//! optimistic OLC path.
// The counter is process-wide: one test, so nothing else moves it.
#![cfg(not(miri))]
#![cfg(all(feature = "occ-stats", feature = "std", target_pointer_width = "64"))]

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::SyncExpanseBytesMap;
use std::hash::{BuildHasher, Hasher};

// The three mix keys of `tests/linearizability.rs`, `bytes_key(0..3)`.
const MIX_KEYS: [&[u8]; 3] = [
    b"b_short",
    b"shared/prefix/aaaa",
    b"shared/prefix/bbbb/first",
];

/// A key containing `filler/` hashes to one of 96 values, every other key to
/// one value: the fillers make a tree, the mix keys share one bucket.
#[derive(Default, Clone)]
struct SharedBucketHasher {
    bytes: Vec<u8>,
}
impl Hasher for SharedBucketHasher {
    fn finish(&self) -> u64 {
        if self.bytes.windows(7).any(|w| w == b"filler/") {
            let mut h = 0xcbf2_9ce4_8422_2325u64;
            for &b in &self.bytes {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x100_0000_01b3);
            }
            h % 96
        } else {
            0x42
        }
    }
    fn write(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
}
#[derive(Default, Clone)]
struct SharedBucketState;
impl BuildHasher for SharedBucketState {
    type Hasher = SharedBucketHasher;
    fn build_hasher(&self) -> SharedBucketHasher {
        SharedBucketHasher::default()
    }
}

/// Lock fallbacks taken while `f` runs.
fn fallbacks(f: impl FnOnce()) -> u64 {
    let before = occ_stats::snapshot()[Stat::LockFallbacks as usize];
    f();
    occ_stats::snapshot()[Stat::LockFallbacks as usize] - before
}

/// Fills a map past `BITMAP_TO_UNCOMPRESSED_THRESHOLD` top-level digits so the
/// root is an uncompressed branch (`BranchU`), inserts the mix keys with
/// `get_or_insert` (value 1 each), then compare-and-removes each. Returns the
/// lock fallbacks the three removals took.
fn removal_fallbacks<S: BuildHasher + Clone + Send + Sync>(map: &SyncExpanseBytesMap<S>) -> u64 {
    for i in 0..1024u64 {
        map.insert(format!("filler/{i}").as_bytes(), i);
    }
    for k in MIX_KEYS {
        assert_eq!(map.get_or_insert(k, 1), None);
    }
    fallbacks(|| {
        for k in MIX_KEYS {
            assert_eq!(map.compare_exchange(k, Some(1), None), Ok(Some(1)));
        }
    })
}

#[test]
fn mix_removals_take_the_path_the_mix_is_named_for() {
    // Distinct buckets (the `tree_last_entry` mix): each removal is a
    // bucket's last entry, unlinked optimistically by
    // `olc_cas_remove_bucket_cas_map` (#1396).
    let n = removal_fallbacks(&SyncExpanseBytesMap::new());
    assert_eq!(
        n, 0,
        "last-entry removals took the serialised path {n} time(s), expected 0"
    );
    // One shared bucket (the `tree_shared_bucket` mix): takes the optimistic
    // shorter-bucket replacement path (`olc_cas_replace_bucket_map`) on all
    // three removals.
    let n = removal_fallbacks(&SyncExpanseBytesMap::with_hasher(SharedBucketState));
    assert_eq!(
        n, 0,
        "shared-bucket removals took the serialised path {n} time(s), expected 0"
    );
}
