//! `ExpanseBytesMap`: an **unordered** map from arbitrary byte strings to
//! `u64` values (compat: JudyHS).
//!
//! The documented JudyHS contract is a hash-keyed structure: no ordered
//! navigation, no neighbor searches — just exact-match insert/get/delete
//! that stays fast for long and similar keys (where a digital trie like
//! [`crate::strmap::ExpanseStrMap`] pays per-byte). The clean-room shape
//! here: each key's 64-bit hash indexes an [`ExpanseMap`] whose value
//! holds a collision bucket — a small vector of `(key bytes, value)`
//! entries compared byte-exactly. The word map gives the hash table its
//! sparse, population-proportional footprint; buckets stay at one entry
//! until real 64-bit collisions occur.
//!
//! Value-slot pointers returned by [`ExpanseBytesMap::ins_slot`] /
//! [`ExpanseBytesMap::get_value_slot`] follow the classic Judy contract:
//! valid until the next structural mutation of the map.

use crate::map::ExpanseMap;
#[cfg(feature = "std")]
use crate::occ::Collector;
use core::hash::BuildHasher;
use core::ptr::NonNull;
use core_alloc::boxed::Box;
#[cfg(feature = "std")]
use core_alloc::sync::Arc;
use core_alloc::vec;
use core_alloc::vec::Vec;
#[cfg(feature = "std")]
use std::sync::OnceLock;

/// Default build hasher type: [`std::hash::RandomState`] under `std`, or [`core::hash::BuildHasherDefault<FnvHasher>`] in `no_std`.
#[cfg(feature = "std")]
pub type DefaultBuildHasher = std::hash::RandomState;

/// Deterministic 64-bit FNV-1a hasher for `no_std` environments.
#[cfg(not(feature = "std"))]
#[derive(Clone, Copy, Debug, Default)]
pub struct FnvHasher(u64);

#[cfg(not(feature = "std"))]
impl core::hash::Hasher for FnvHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 {
            0xcbf29ce484222325
        } else {
            self.0
        };
        for &byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.0 = hash;
    }
}

/// Default build hasher type: [`std::hash::RandomState`] under `std`, or [`core::hash::BuildHasherDefault<FnvHasher>`] in `no_std`.
#[cfg(not(feature = "std"))]
pub type DefaultBuildHasher = core::hash::BuildHasherDefault<FnvHasher>;

/// One collision-bucket entry: the exact key bytes and the value word.
type Entry = (Box<[u8]>, u64);

/// One hash bucket: entries whose keys share a 64-bit hash. Almost
/// always a single entry; compared byte-exactly on every operation.
///
/// Phase 7 contract (issue #219): a published bucket is **write-once
/// except its value words**. Structural changes (adding or removing an
/// entry) build a replacement bucket with exact capacity, publish it
/// over the trie entry, and dispose of the old one — retired through
/// the epoch collector when the map is concurrently shared, so a reader
/// that validated the bucket pointer may keep reading the shell, entry
/// array, and key bytes under its pin. Only the value word mutates in
/// place (a single `u64`; the concurrent read path re-validates the
/// tree version before returning it).
type Bucket = Vec<Entry>;

/// Approximate heap cost of one entry beyond its key bytes (the boxed
/// key's pointer/len pair plus the value word in the bucket vector).
const ENTRY_OVERHEAD: usize = size_of::<(Box<[u8]>, u64)>();

/// Approximate heap cost of one bucket (its vector header, boxed).
const BUCKET_OVERHEAD: usize = size_of::<Bucket>();

/// Disposes an unlinked key buffer: dropped immediately when not shared,
/// retired raw when it is — a concurrent reader that validated the
/// bucket at an earlier snapshot may still be comparing these bytes
/// under its pin. The owning `Box` is consumed **by value** so its
/// provenance travels to the collector's deallocation. An empty
/// `Box<[u8]>` owns no allocation — nothing to retire.
///
/// Retired layout: `(len, 1)` — align 1 never matches a size class
/// (`class_for` accepts only `RAW_ALIGN`/`CACHE_LINE`), so the
/// collector frees it through `free_raw` with the exact original
/// `Box<[u8]>` layout.
#[cfg(feature = "std")]
fn dispose_key(key: Box<[u8]>, defer: Option<&Arc<Collector>>) {
    match defer {
        Some(c) if !key.is_empty() => {
            let len = key.len();
            let buf = Box::into_raw(key).cast::<u8>();
            c.retire(NonNull::new(buf).expect("non-null key buffer"), len, 1);
        }
        _ => drop(key),
    }
}

#[cfg(not(feature = "std"))]
#[inline(always)]
fn dispose_key(key: Box<[u8]>, _defer: Option<&()>) {
    drop(key);
}

/// Disposes an unlinked bucket. `own_keys` says whether the entries'
/// key buffers still belong to this bucket (dispose of them too) or
/// were moved by value into a replacement bucket (leave them alone —
/// the old entry array keeps their bit pattern for concurrent readers,
/// but the allocations now belong to the replacement).
///
/// Retired layouts (both always miss the size classes — align 8 is
/// neither `RAW_ALIGN` nor `CACHE_LINE` — so both take the collector's
/// exact-layout `free_raw` path):
/// - entry buffer: `Layout::array::<Entry>(capacity)` — capacity, not
///   length, is the allocated size;
/// - shell: `Layout::new::<Bucket>()`.
#[cfg(feature = "std")]
fn dispose_bucket(ptr: *mut Bucket, own_keys: bool, defer: Option<&Arc<Collector>>) {
    match defer {
        None => {
            // SAFETY: caller unlinked `ptr`; this is the last reference.
            let mut bucket = unsafe { Box::from_raw(ptr) };
            if !own_keys {
                // Entries were moved out by value into the replacement:
                // free the shell and buffer without running their Drop.
                // SAFETY: 0 <= capacity; skips the moved-out entries.
                unsafe { bucket.set_len(0) };
            }
            drop(bucket);
        }
        Some(c) => {
            // Retire raw (no `Drop` runs; the collector frees plain
            // memory after the grace period). The vector is moved out of
            // the shell **by value** so the buffer pointer keeps its
            // original provenance — a pointer merely borrowed out of the
            // shell would not carry deallocation rights (Miri rejects
            // the later dealloc).
            // SAFETY: unlinked; last owner. The shell is never used as a
            // `Vec` again — it is retired below without running Drop,
            // and pinned readers only load its write-once bit pattern.
            let vec: Bucket = unsafe { core::ptr::read(ptr) };
            let mut vec = core::mem::ManuallyDrop::new(vec);
            if own_keys {
                for i in 0..vec.len() {
                    // SAFETY: in-bounds; each entry is moved out exactly
                    // once, so its key `Box` carries its provenance.
                    let (key, _val): Entry = unsafe { core::ptr::read(vec.as_ptr().add(i)) };
                    dispose_key(key, defer);
                }
            }
            let cap = vec.capacity();
            if cap > 0 {
                let buf = vec.as_mut_ptr().cast::<u8>();
                c.retire(
                    NonNull::new(buf).expect("non-null bucket buffer"),
                    cap * size_of::<Entry>(),
                    align_of::<Entry>(),
                );
            }
            c.retire(
                NonNull::new(ptr.cast::<u8>()).expect("non-null bucket shell"),
                size_of::<Bucket>(),
                align_of::<Bucket>(),
            );
        }
    }
}

#[cfg(not(feature = "std"))]
fn dispose_bucket(ptr: *mut Bucket, own_keys: bool, _defer: Option<&()>) {
    // SAFETY: caller unlinked `ptr`; this is the last reference.
    let mut bucket = unsafe { Box::from_raw(ptr) };
    if !own_keys {
        // SAFETY: 0 <= capacity; skips the moved-out entries.
        unsafe { bucket.set_len(0) };
    }
    drop(bucket);
}

/// A sparse, dynamic, **unordered** map from byte strings to `u64`
/// values (compat: JudyHS).
///
/// Under `feature = "std"`, hashing uses [`RandomState`] (per-map seeding, hash-flood resistant).
/// In `no_std`, hashing defaults to deterministic 64-bit FNV-1a.
/// Use [`ExpanseBytesMap::with_hasher`] to pin a custom hasher.
pub struct ExpanseBytesMap<S: BuildHasher = DefaultBuildHasher> {
    /// hash → `Box<Bucket>` pointer, stored as the map value word.
    map: ExpanseMap,
    hasher: S,
    len: u64,
    /// Bucket/entry heap bytes (estimate; the trie's own bytes are exact
    /// via [`ExpanseMap::mem_used`]).
    extra_bytes: usize,
    /// Phase 7 (issue #219): when set, unlinked buckets and key buffers
    /// are retired through the collector instead of freed (concurrent
    /// readers may still hold pointers into them), and the hash trie's
    /// `NodeAlloc` is deferred so its frees and mutation brackets
    /// participate too.
    #[cfg(feature = "std")]
    deferred: OnceLock<Arc<Collector>>,
}

impl ExpanseBytesMap<DefaultBuildHasher> {
    /// Creates an empty map with the default hasher.
    #[must_use]
    pub fn new() -> Self {
        Self::with_hasher(DefaultBuildHasher::default())
    }
}

impl Default for ExpanseBytesMap<DefaultBuildHasher> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: BuildHasher> ExpanseBytesMap<S> {
    /// Creates an empty map using `hasher` (tests use a degenerate
    /// hasher to force every key into one collision bucket).
    #[must_use]
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            map: ExpanseMap::new(),
            hasher,
            len: 0,
            extra_bytes: 0,
            #[cfg(feature = "std")]
            deferred: OnceLock::new(),
        }
    }

    #[inline(always)]
    #[cfg(feature = "std")]
    fn defer_handle(&self) -> Option<Arc<Collector>> {
        self.deferred.get().cloned()
    }

    #[inline(always)]
    #[cfg(not(feature = "std"))]
    fn defer_handle(&self) -> Option<()> {
        None
    }

    /// Switches this map to deferred reclamation through `collector`,
    /// permanently (the Phase 7 `sync` wrapper calls this once at
    /// construction). Idempotent for the same collector; a second call
    /// with a different collector panics.
    ///
    /// Requires an **empty** map: a populated map's hash trie holds
    /// slab-carved node memory, which must never be retired to the
    /// collector (see `NodeAlloc::defer_to`). The `sync` wrapper shares
    /// a populated map by rebuilding it through a pre-deferred one.
    ///
    /// `pub(crate)` deliberately — only the `sync` wrapper drives a
    /// collector's epochs (see `BlobArena::defer_to` for the rationale).
    #[cfg(feature = "std")]
    pub(crate) fn defer_to(&self, collector: Arc<Collector>) {
        assert!(
            self.len == 0 && self.map.is_empty(),
            "ExpanseBytesMap::defer_to requires an empty map; rebuild a \
             populated map through a pre-deferred one instead"
        );
        // Both steps are idempotent for the same collector and panic on
        // a different one.
        self.map.occ_root().1.defer_to(Arc::clone(&collector));
        let stored = self.deferred.get_or_init(|| Arc::clone(&collector));
        assert!(
            Arc::ptr_eq(stored, &collector),
            "ExpanseBytesMap already deferred to a different collector"
        );
    }

    /// Number of keys in the map.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.len
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Heap bytes used: exact for the hash trie, estimated for the
    /// collision buckets (key bytes + fixed per-entry/per-bucket
    /// overheads; vector spare capacity is not counted).
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.map.mem_used() + self.extra_bytes
    }

    fn bucket_of(&self, key: &[u8]) -> Option<NonNull<Bucket>> {
        let h = self.hasher.hash_one(key);
        self.map
            .get(h)
            .and_then(|word| NonNull::new(word as *mut Bucket))
    }

    /// Returns the value stored for `key`.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        let bucket = self.bucket_of(key)?;
        // SAFETY: bucket pointers stored in the hash trie are live boxes
        // owned by this map.
        unsafe { bucket.as_ref() }
            .iter()
            .find(|(k, _)| &**k == key)
            .map(|&(_, v)| v)
    }

    /// Membership test.
    #[must_use]
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Phase 7 (issue #219): one bounded, validated lock-free lookup —
    /// the concurrent analogue of [`Self::get`]. One 64-bit hash, one
    /// validated `sync::walk_validated` over the hash trie, then a
    /// byte-exact comparison against the collision bucket.
    ///
    /// The bucket word yielded by the walk is validated at `snap`, and a
    /// published bucket is write-once except its value words (structural
    /// changes publish a replacement and retire the old bucket — see
    /// [`Bucket`]), so the shell, entry array, and key bytes read here
    /// are exactly the published state; the value word may race with an
    /// in-place update and is covered by the final tree-version
    /// validation before anything is returned.
    ///
    /// # Safety
    ///
    /// Same contract as `sync::walk_validated`: `snap` must be an even
    /// version sampled from `ver` after this map switched to deferred
    /// reclamation ([`Self::defer_to`]), and the caller must hold an
    /// epoch pin for the whole call — every pointer read under a
    /// still-valid cover then references EBR-live memory.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn get_validated(
        &self,
        key: &[u8],
        ver: &crate::occ::SeqVersion,
        snap: u64,
    ) -> Result<Option<u64>, crate::sync::Retry> {
        use crate::sync::Retry;
        let h = self.hasher.hash_one(key);
        // Racy by-value root snapshot; the walk validates before use.
        let root = self.map.occ_root().0;
        // SAFETY: the caller's pin + snapshot contract carries through.
        let found = unsafe { crate::sync::walk_validated::<true>(root, h, ver, snap) }?;
        let Some(word) = found else { return Ok(None) };
        let Some(bucket) = NonNull::new(word as *mut Bucket) else {
            // A zero word is observable only mid-publication
            // (`ins_slot` inserts the trie entry before storing the
            // bucket pointer); the writer bracket is open, so retry.
            return Err(Retry);
        };
        let bucket: *const Bucket = bucket.as_ptr();
        // SAFETY: `word` was validated at `snap`, so `bucket` was the
        // published bucket then, and EBR keeps its shell, entry buffer,
        // and key buffers mapped under the caller's pin. Everything but
        // the value words is write-once after publication.
        let (len, entries) = unsafe { ((*bucket).len(), (*bucket).as_ptr()) };
        let mut result = None;
        for i in 0..len {
            // SAFETY: `i < len` of the write-once entry array; the value
            // word may race and is validated below before use.
            let (k, v) = unsafe {
                let e = &*entries.add(i);
                (&e.0[..], e.1)
            };
            if k == key {
                result = Some(v);
                break;
            }
        }
        if !ver.validate(snap) {
            return Err(Retry);
        }
        Ok(result)
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None`
    /// if absent — the compat `JudyHSGet` convention. Valid until the
    /// next structural mutation.
    #[must_use]
    pub fn get_value_slot(&mut self, key: &[u8]) -> Option<NonNull<u64>> {
        let mut bucket = self.bucket_of(key)?;
        // SAFETY: live bucket owned by this map; the slot pointer stays
        // valid until the bucket vector next mutates.
        unsafe { bucket.as_mut() }
            .iter_mut()
            .find(|(k, _)| &**k == key)
            .map(|(_, v)| NonNull::from(v))
    }

    /// Inserts `key` with value 0 if absent — an existing value is kept
    /// untouched — and returns a **writable pointer to its value slot**:
    /// the compat `JudyHSIns` contract. Valid until the next structural
    /// mutation.
    pub fn ins_slot(&mut self, key: &[u8]) -> NonNull<u64> {
        let defer = self.defer_handle();
        let h = self.hasher.hash_one(key);
        let slot = self.map.ins_slot(h);
        // SAFETY: `ins_slot` hands out the live (zero-initialized when
        // fresh) value slot for `h`; 0 is never a published bucket
        // pointer.
        let word = unsafe { *slot.as_ptr() };
        if word == 0 {
            // Fresh hash: build a complete single-entry bucket, then
            // publish it with one word store — a concurrent reader sees
            // either 0 (treated as retry-worthy mid-publication state)
            // or the fully initialized bucket.
            let bucket: Bucket = vec![(key.into(), 0)];
            let raw = Box::into_raw(Box::new(bucket));
            // SAFETY: the slot stays valid until the next structural
            // trie mutation; none happens between `ins_slot` and here.
            unsafe { *slot.as_ptr() = raw as u64 };
            self.len += 1;
            self.extra_bytes += BUCKET_OVERHEAD + key.len() + ENTRY_OVERHEAD;
            // SAFETY: freshly allocated above; entry 0 exists.
            let fresh: &mut Bucket = unsafe { &mut *raw };
            return NonNull::from(&mut fresh[0].1);
        }
        let old = word as *mut Bucket;
        // SAFETY: live bucket owned by this map; read-only search.
        if let Some(at) = unsafe { &*old }.iter().position(|(k, _)| &**k == key) {
            // Existing key: only its value word will mutate, in place —
            // the one mutation published buckets allow.
            // SAFETY: as above; the slot pointer stays valid until the
            // next structural mutation.
            let live: &mut Bucket = unsafe { &mut *old };
            return NonNull::from(&mut live[at].1);
        }
        // 64-bit hash collision: publish a replacement bucket holding
        // the moved-over entries plus the new key, then dispose of the
        // old shell/buffer (its key buffers now belong to the
        // replacement).
        // SAFETY: live bucket; length read before the moves below.
        let old_len = unsafe { (*old).len() };
        let mut bucket: Bucket = Vec::with_capacity(old_len + 1);
        for i in 0..old_len {
            // SAFETY: in-bounds move-by-value; the old buffer keeps the
            // bit pattern for concurrent pinned readers and is disposed
            // of below without running the moved entries' Drop.
            bucket.push(unsafe { core::ptr::read((*old).as_ptr().add(i)) });
        }
        bucket.push((key.into(), 0));
        let raw = Box::into_raw(Box::new(bucket));
        // Publish the replacement — the single word store that unlinks
        // the old bucket — then retire the old allocation.
        // SAFETY: slot valid as above.
        unsafe { *slot.as_ptr() = raw as u64 };
        dispose_bucket(old, false, defer.as_ref());
        self.len += 1;
        self.extra_bytes += key.len() + ENTRY_OVERHEAD;
        // SAFETY: freshly allocated above; the appended entry exists.
        let fresh: &mut Bucket = unsafe { &mut *raw };
        NonNull::from(&mut fresh[old_len].1)
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        let had = self.contains_key(key);
        let slot = self.ins_slot(key);
        // SAFETY: fresh slot from ins_slot, valid until next mutation.
        unsafe {
            let old = *slot.as_ptr();
            *slot.as_ptr() = val;
            had.then_some(old)
        }
    }

    /// Removes `key`; returns its value if it was present.
    pub fn remove(&mut self, key: &[u8]) -> Option<u64> {
        let defer = self.defer_handle();
        let h = self.hasher.hash_one(key);
        let word = self.map.get(h)?;
        let old = word as *mut Bucket;
        // SAFETY: live bucket owned by this map; read-only search.
        let bucket = unsafe { &*old };
        let at = bucket.iter().position(|(k, _)| &**k == key)?;
        let val = bucket[at].1;
        let key_len = bucket[at].0.len();
        let old_len = bucket.len();
        if old_len == 1 {
            // Unlink the bucket from the trie first, then dispose of it
            // together with its one remaining key.
            self.map.remove(h);
            dispose_bucket(old, true, defer.as_ref());
            self.extra_bytes -= BUCKET_OVERHEAD;
        } else {
            // Collision bucket: publish a replacement without the
            // removed entry (the survivors move over by value), then
            // dispose of the old shell/buffer plus the removed key.
            let mut repl: Bucket = Vec::with_capacity(old_len - 1);
            for i in 0..old_len {
                if i != at {
                    // SAFETY: in-bounds move-by-value (see `ins_slot`).
                    repl.push(unsafe { core::ptr::read(bucket.as_ptr().add(i)) });
                }
            }
            // SAFETY: the removed entry moves out exactly once, here;
            // the old buffer keeps its bit pattern for pinned readers.
            let (removed_key, _): Entry = unsafe { core::ptr::read(bucket.as_ptr().add(at)) };
            let raw = Box::into_raw(Box::new(repl));
            // Publish the replacement through the engine (bracketed when
            // shared) — this unlinks the old bucket — then retire it.
            let prev = self.map.insert(h, raw as u64);
            debug_assert_eq!(prev, Some(word), "bucket word moved mid-remove");
            dispose_bucket(old, false, defer.as_ref());
            dispose_key(removed_key, defer.as_ref());
        }
        self.len -= 1;
        self.extra_bytes -= key_len + ENTRY_OVERHEAD;
        Some(val)
    }

    /// Visits every entry in unspecified order.
    pub fn for_each(&self, mut f: impl FnMut(&[u8], u64)) {
        for (_, word) in self.map.iter() {
            // SAFETY: every trie value is a live bucket owned by this map.
            let bucket = unsafe { &*(word as *const Bucket) };
            for (k, v) in bucket {
                f(k, *v);
            }
        }
    }

    /// Removes every key and releases all memory.
    pub fn clear(&mut self) {
        let defer = self.defer_handle();
        let buckets: Vec<u64> = self.map.iter().map(|(_, word)| word).collect();
        // Unlink everything from the trie first (its nodes free/retire
        // through its own `NodeAlloc`), then dispose of the buckets.
        self.map.clear();
        for word in buckets {
            // Each collected word is a live bucket, unlinked above and
            // disposed of (with its keys) exactly once here.
            dispose_bucket(word as *mut Bucket, true, defer.as_ref());
        }
        self.len = 0;
        self.extra_bytes = 0;
    }
}

impl<S: BuildHasher> Drop for ExpanseBytesMap<S> {
    fn drop(&mut self) {
        self.clear();
    }
}

// SAFETY: the map exclusively owns the hash trie and every bucket box;
// moving it moves that ownership wholesale (mirrors `ExpanseMap`).
unsafe impl<S: BuildHasher + Send> Send for ExpanseBytesMap<S> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::hash::Hasher;

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

    fn random_key(rng: &mut XorShift) -> Vec<u8> {
        // Lengths 0..=40 over a small alphabet: plenty of repeats, some
        // shared prefixes, zero bytes included (keys are not C strings).
        let len = (rng.next() % 41) as usize;
        (0..len).map(|_| (rng.next() % 7) as u8 * 0x1F).collect()
    }

    fn model_run<S: BuildHasher>(mut m: ExpanseBytesMap<S>, ops: usize) {
        let mut rng = XorShift(0xB17E_5EED);
        let mut model: HashMap<Vec<u8>, u64> = HashMap::new();
        for _ in 0..ops {
            let key = random_key(&mut rng);
            match rng.next() % 4 {
                0 | 3 => {
                    let val = rng.next();
                    assert_eq!(
                        m.insert(&key, val),
                        model.insert(key.clone(), val),
                        "ins {key:02x?}"
                    );
                }
                1 => assert_eq!(m.remove(&key), model.remove(&key), "rem {key:02x?}"),
                _ => assert_eq!(m.get(&key), model.get(&key).copied(), "get {key:02x?}"),
            }
            assert_eq!(m.len(), model.len() as u64);
        }
        for (k, &v) in &model {
            assert_eq!(m.get(k), Some(v), "model entry {k:02x?}");
        }
        let mut seen = 0u64;
        m.for_each(|k, v| {
            assert_eq!(model.get(k).copied(), Some(v));
            seen += 1;
        });
        assert_eq!(seen, m.len());
        let keys: Vec<Vec<u8>> = model.keys().cloned().collect();
        for k in keys {
            assert_eq!(m.remove(&k), model.remove(&k));
        }
        assert!(m.is_empty());
        assert_eq!(m.mem_used(), 0);
    }

    #[cfg(miri)]
    const OPS: usize = 60;
    #[cfg(not(miri))]
    const OPS: usize = 6000;

    #[test]
    fn model_random_keys() {
        model_run(ExpanseBytesMap::new(), OPS);
    }

    /// Every key hashes identically: the entire map is one collision
    /// bucket, exercising the bucket paths that real hashing almost
    /// never reaches.
    struct Degenerate;
    impl Hasher for Degenerate {
        fn finish(&self) -> u64 {
            0x42
        }
        fn write(&mut self, _: &[u8]) {}
    }
    impl BuildHasher for Degenerate {
        type Hasher = Degenerate;
        fn build_hasher(&self) -> Degenerate {
            Degenerate
        }
    }

    #[test]
    fn model_full_collision() {
        model_run(ExpanseBytesMap::with_hasher(Degenerate), OPS / 4);
    }

    /// Phase 7 (issue #219): deferred-mode round trip — single-threaded
    /// and Miri-clean. Every disposal path (fresh-bucket publication,
    /// collision append via bucket replacement, in-place value
    /// overwrite, collision removal with removed-key retirement,
    /// last-entry bucket unlink, whole-map clear) routes unlinked
    /// allocations through the epoch collector, and everything drains
    /// without leaks or double frees. The degenerate hasher forces every
    /// key into one bucket so the replacement paths actually run.
    #[test]
    #[cfg(feature = "std")]
    fn deferred_bytesmap_dispose_round_trip() {
        use crate::occ::Collector;
        use core_alloc::sync::Arc;

        let collector = Arc::new(Collector::new());
        let mut m = ExpanseBytesMap::with_hasher(Degenerate);
        // Deferral must precede every allocation (`defer_to` requires an
        // empty map — slab-carved memory must never reach the collector).
        m.defer_to(Arc::clone(&collector));

        // Fresh bucket publication, then collision appends (each one a
        // publish-replacement-then-retire of the previous bucket).
        assert_eq!(m.insert(b"alpha", 1), None);
        assert_eq!(m.insert(b"beta", 2), None);
        assert_eq!(m.insert(b"gamma", 3), None);
        // In-place value overwrite (no disposal).
        assert_eq!(m.insert(b"beta", 20), Some(2));
        // ins_slot on existing and fresh keys.
        let slot = m.ins_slot(b"beta");
        // SAFETY: slot valid until the next structural mutation.
        unsafe { assert_eq!(*slot.as_ptr(), 20) };
        let slot = m.ins_slot(b"delta");
        // SAFETY: as above.
        unsafe { slot.as_ptr().write(4) };
        assert_eq!(m.get(b"delta"), Some(4));
        // An empty key's Box<[u8]> owns no allocation — nothing retires.
        assert_eq!(m.insert(b"", 5), None);
        assert_eq!(m.len(), 5);

        // Collision removals (replacement + removed-key retirement).
        assert_eq!(m.remove(b"alpha"), Some(1));
        assert_eq!(m.remove(b""), Some(5));
        assert_eq!(m.remove(b"beta"), Some(20));
        assert_eq!(m.remove(b"absent"), None);
        // Down to the last entry: bucket unlink + full disposal.
        assert_eq!(m.remove(b"gamma"), Some(3));
        assert_eq!(m.remove(b"delta"), Some(4));
        assert!(m.is_empty());
        assert_eq!(m.mem_used(), 0);

        // Repopulate, then whole-map clear.
        m.insert(b"x", 7);
        m.insert(b"y", 8);
        m.clear();
        assert!(m.is_empty());
        assert_eq!(m.mem_used(), 0);

        // Grace-period advances free the retired chain; drop drains the
        // rest.
        collector.try_advance();
        collector.try_advance();
        collector.try_advance();
        drop(m);
        drop(collector);
    }

    /// Deferred-mode model differential: the publish-replacement
    /// restructure must not change single-threaded semantics, with real
    /// hashing and with every key forced into one collision bucket.
    #[test]
    #[cfg(feature = "std")]
    fn deferred_model_round_trips() {
        use crate::occ::Collector;
        use core_alloc::sync::Arc;

        let collector = Arc::new(Collector::new());
        let m = ExpanseBytesMap::new();
        m.defer_to(Arc::clone(&collector));
        model_run(m, OPS / 4);
        collector.try_advance();
        collector.try_advance();
        collector.try_advance();
        drop(collector);

        let collector = Arc::new(Collector::new());
        let m = ExpanseBytesMap::with_hasher(Degenerate);
        m.defer_to(Arc::clone(&collector));
        model_run(m, OPS / 8);
        drop(collector);
    }

    #[test]
    fn slots_and_edge_keys() {
        let mut m = ExpanseBytesMap::new();
        // Zero-length and embedded-NUL keys are ordinary keys.
        assert_eq!(m.insert(b"", 1), None);
        assert_eq!(m.insert(b"\0", 2), None);
        assert_eq!(m.insert(b"\0\0", 3), None);
        assert_eq!(m.insert(b"a\0b", 4), None);
        assert_eq!(m.get(b""), Some(1));
        assert_eq!(m.get(b"\0"), Some(2));
        assert_eq!(m.get(b"\0\0"), Some(3));
        assert_eq!(m.get(b"a\0b"), Some(4));
        assert_eq!(m.len(), 4);

        // The JudyHS slot contract: write through ins_slot/get_value_slot.
        let slot = m.ins_slot(b"key");
        // SAFETY: slot valid until the next mutation.
        unsafe {
            assert_eq!(*slot.as_ptr(), 0);
            *slot.as_ptr() = 99;
        }
        assert_eq!(m.get(b"key"), Some(99));
        let slot = m.get_value_slot(b"key").expect("present");
        // SAFETY: as above.
        unsafe { *slot.as_ptr() = 100 };
        assert_eq!(m.get(b"key"), Some(100));
        assert_eq!(m.get_value_slot(b"absent"), None);

        // Values survive re-insert-with-keep (ins_slot on existing key).
        let slot = m.ins_slot(b"key");
        // SAFETY: as above.
        unsafe { assert_eq!(*slot.as_ptr(), 100) };

        m.clear();
        assert!(m.is_empty());
        assert_eq!(m.mem_used(), 0);
    }
}
