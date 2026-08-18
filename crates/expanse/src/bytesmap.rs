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
use core::ptr::NonNull;
use std::hash::{BuildHasher, RandomState};

/// One hash bucket: entries whose keys share a 64-bit hash. Almost
/// always a single entry; compared byte-exactly on every operation.
type Bucket = Vec<(Box<[u8]>, u64)>;

/// Approximate heap cost of one entry beyond its key bytes (the boxed
/// key's pointer/len pair plus the value word in the bucket vector).
const ENTRY_OVERHEAD: usize = size_of::<(Box<[u8]>, u64)>();

/// Approximate heap cost of one bucket (its vector header, boxed).
const BUCKET_OVERHEAD: usize = size_of::<Bucket>();

/// A sparse, dynamic, **unordered** map from byte strings to `u64`
/// values (compat: JudyHS).
///
/// Hashing uses [`RandomState`] (per-map seeding, hash-flood resistant);
/// use [`ExpanseBytesMap::with_hasher`] to pin a deterministic hasher.
pub struct ExpanseBytesMap<S: BuildHasher = RandomState> {
    /// hash → `Box<Bucket>` pointer, stored as the map value word.
    map: ExpanseMap,
    hasher: S,
    len: u64,
    /// Bucket/entry heap bytes (estimate; the trie's own bytes are exact
    /// via [`ExpanseMap::mem_used`]).
    extra_bytes: usize,
}

impl ExpanseBytesMap<RandomState> {
    /// Creates an empty map with a freshly seeded hasher.
    #[must_use]
    pub fn new() -> Self {
        Self::with_hasher(RandomState::new())
    }
}

impl Default for ExpanseBytesMap<RandomState> {
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
        }
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
        let h = self.hasher.hash_one(key);
        let slot = self.map.ins_slot(h);
        // SAFETY: `ins_slot` hands out the live (zero-initialized when
        // fresh) value slot for `h`; 0 is never a valid bucket pointer.
        let bucket: &mut Bucket = unsafe {
            if (*slot.as_ptr()) == 0 {
                let fresh: Box<Bucket> = Box::default();
                *slot.as_ptr() = Box::into_raw(fresh) as u64;
                self.extra_bytes += BUCKET_OVERHEAD;
            }
            &mut *((*slot.as_ptr()) as *mut Bucket)
        };
        if let Some(at) = bucket.iter().position(|(k, _)| &**k == key) {
            return NonNull::from(&mut bucket[at].1);
        }
        self.len += 1;
        self.extra_bytes += key.len() + ENTRY_OVERHEAD;
        bucket.push((key.into(), 0));
        NonNull::from(&mut bucket.last_mut().expect("just pushed").1)
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
        let h = self.hasher.hash_one(key);
        let word = self.map.get(h)?;
        let bucket_ptr = word as *mut Bucket;
        // SAFETY: live bucket owned by this map.
        let bucket = unsafe { &mut *bucket_ptr };
        let at = bucket.iter().position(|(k, _)| &**k == key)?;
        let (k, v) = bucket.swap_remove(at);
        self.len -= 1;
        self.extra_bytes -= k.len() + ENTRY_OVERHEAD;
        if bucket.is_empty() {
            self.map.remove(h);
            // SAFETY: the trie no longer references the bucket; drop it.
            drop(unsafe { Box::from_raw(bucket_ptr) });
            self.extra_bytes -= BUCKET_OVERHEAD;
        }
        Some(v)
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
        let buckets: Vec<u64> = self.map.iter().map(|(_, word)| word).collect();
        for word in buckets {
            // SAFETY: each trie value is a live bucket, dropped exactly
            // once here before the trie itself is cleared.
            drop(unsafe { Box::from_raw(word as *mut Bucket) });
        }
        self.map.clear();
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
    const OPS: usize = 300;
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
