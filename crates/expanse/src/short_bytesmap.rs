//! A byte-string map that keeps keys of at most one machine word out of
//! the hash path (#1681).
//!
//! [`ExpanseShortBytesMap`] composes two plain [`ExpanseMap`] tries with an
//! unchanged [`ExpanseBytesMap`]:
//!
//! - an 8-byte key is its own 64-bit trie key, read big-endian;
//! - a 0- to 7-byte key is packed big-endian into the high bytes of a word
//!   whose low byte is its length, in a second trie, so `[]`, `[0]` and
//!   `[0, 0]` stay distinct;
//! - a longer key goes to the [`ExpanseBytesMap`], hashed as there.
//!
//! A short key costs one ordered-trie operation: no hash, no allocation of
//! its own, and keys that share a prefix share nodes. A long key pays one
//! length compare before the hash path. The two short-key tries are boxed
//! and allocated on the first short-key insert, so a map that never sees a
//! short key carries one null pointer beside the hashed map.
//!
//! This is the engine behind the legacy `JudyHS*` C entry points, whose
//! callers (zmap's 8-byte response-dedup keys) are dominated by short keys.
//! [`ExpanseBytesMap`] itself, the `expanse_bytesmap_*` C API and every
//! language binding stay on the hash path.
//!
//! **Not shareable.** The type has no `defer_to` and no concurrent wrapper:
//! the concurrent protocol publishes and validates one root per wrapper, and
//! this map has three. Concurrent use of byte keys goes through
//! [`crate::sync::SyncExpanseBytesMap`], whose keys all take the hash path;
//! concurrent ≤8-byte keys can use [`crate::sync::SyncExpanseMap`] directly.
//!
//! ```compile_fail,E0277
//! use expanse_trie::short_bytesmap::ExpanseShortBytesMap;
//! use expanse_trie::sync::SyncExpanseBytesMap;
//! // No conversion into the shared wrapper exists, by design.
//! let _shared = SyncExpanseBytesMap::from(ExpanseShortBytesMap::new());
//! ```
//!
//! **Slot lifetime.** [`ExpanseShortBytesMap::get_slot_ptr`] and
//! [`ExpanseShortBytesMap::ins_slot`] return a pointer to the key's value
//! word. A short key's word lives in a trie leaf and moves when the leaf is
//! rebuilt, so the pointer is valid only until the next insert or remove of
//! any key in the map; a long key's word follows [`ExpanseBytesMap`]'s
//! contract.
//!
//! **Iteration order.** [`ExpanseShortBytesMap::try_for_each`] visits the
//! 8-byte keys in ascending byte order, then the 0- to 7-byte keys in
//! ascending order of their packed word, then the longer keys in
//! [`ExpanseBytesMap`]'s unspecified order. The order is not part of the
//! contract.

use crate::bytesmap::{DefaultBuildHasher, ExpanseBytesMap};
use crate::map::ExpanseMap;
use crate::validate::{BytesMapStats, ExpanseStats};
use core::hash::BuildHasher;
use core::ptr::NonNull;
use core_alloc::boxed::Box;

/// The two short-key tries; see the module documentation for the packing.
struct ShortKeys {
    /// 8-byte keys, each read as a big-endian `u64`.
    full: ExpanseMap,
    /// 0- to 7-byte keys: the bytes big-endian in the high bytes of the word,
    /// zeros below them, and the length in the low byte.
    part: ExpanseMap,
}

impl ShortKeys {
    fn new() -> Self {
        Self {
            full: ExpanseMap::new(),
            part: ExpanseMap::new(),
        }
    }

    /// The trie and trie key a key of at most 8 bytes maps to.
    #[inline(always)]
    fn route(&self, key: &[u8]) -> (&ExpanseMap, u64) {
        match short_word(key) {
            (true, w) => (&self.full, w),
            (false, w) => (&self.part, w),
        }
    }

    /// [`Self::route`], mutably.
    #[inline(always)]
    fn route_mut(&mut self, key: &[u8]) -> (&mut ExpanseMap, u64) {
        match short_word(key) {
            (true, w) => (&mut self.full, w),
            (false, w) => (&mut self.part, w),
        }
    }

    fn len(&self) -> u64 {
        self.full.len() + self.part.len()
    }

    /// Heap bytes: the box and both tries' nodes.
    fn mem_used(&self) -> usize {
        core::mem::size_of::<Self>() + self.full.mem_used() + self.part.mem_used()
    }
}

/// `(is 8 bytes, trie key)` for a key of at most 8 bytes.
#[inline(always)]
fn short_word(key: &[u8]) -> (bool, u64) {
    debug_assert!(key.len() <= 8, "only keys of at most 8 bytes are short");
    let n = key.len();
    let mut buf = [0u8; 8];
    buf[..n].copy_from_slice(key);
    if n == 8 {
        return (true, u64::from_be_bytes(buf));
    }
    buf[7] = n as u8;
    (false, u64::from_be_bytes(buf))
}

/// The key a short-key trie key stands for: its bytes, and how many of them
/// are the key.
#[inline]
fn short_key_bytes(full: bool, w: u64) -> ([u8; 8], usize) {
    let bytes = w.to_be_bytes();
    if full {
        (bytes, 8)
    } else {
        (bytes, usize::from(bytes[7]))
    }
}

/// Structural statistics of an [`ExpanseShortBytesMap`], one component per
/// part. [`Self::total_bytes`] equals [`ExpanseShortBytesMap::mem_used`].
#[derive(Debug, Clone, Default)]
pub struct ShortBytesMapStats {
    /// The 8-byte-key trie, or `None` before the first short-key insert.
    pub short_full: Option<ExpanseStats>,
    /// The 0- to 7-byte-key trie, or `None` before the first short-key insert.
    pub short_part: Option<ExpanseStats>,
    /// The box holding the two short-key tries; 0 before the first
    /// short-key insert.
    pub short_box_bytes: usize,
    /// Keys in the two short-key tries.
    pub short_entries: u64,
    /// The hashed map holding every key longer than 8 bytes.
    pub hashed: BytesMapStats,
}

impl ShortBytesMapStats {
    /// Every component's bytes, summed; equals `mem_used()`.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.short_box_bytes
            + self.short_full.as_ref().map_or(0, |s| s.node_bytes.total())
            + self.short_part.as_ref().map_or(0, |s| s.node_bytes.total())
            + self.hashed.node_bytes.total()
    }
}

/// A byte-string → `u64` map that stores keys of at most 8 bytes in ordered
/// tries and hashes only longer keys. See the [module documentation](self)
/// for the layout, the slot lifetime and why the type cannot be shared.
pub struct ExpanseShortBytesMap<S: BuildHasher = DefaultBuildHasher> {
    /// The short-key tries, allocated on the first short-key insert.
    short: Option<Box<ShortKeys>>,
    /// Every key longer than 8 bytes.
    hashed: ExpanseBytesMap<S>,
}

impl ExpanseShortBytesMap<DefaultBuildHasher> {
    /// An empty map whose long keys use the default hasher.
    #[must_use]
    pub fn new() -> Self {
        Self::with_hasher(DefaultBuildHasher::default())
    }
}

impl Default for ExpanseShortBytesMap<DefaultBuildHasher> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: BuildHasher> ExpanseShortBytesMap<S> {
    /// An empty map whose keys longer than 8 bytes are hashed with `hasher`.
    #[must_use]
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            short: None,
            hashed: ExpanseBytesMap::with_hasher(hasher),
        }
    }

    /// Number of keys.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.hashed.len() + self.short.as_deref().map_or(0, ShortKeys::len)
    }

    /// `true` if the map holds no key.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Heap bytes held: the short-key box and tries, and the hashed map's
    /// [`ExpanseBytesMap::mem_used`].
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.hashed.mem_used() + self.short.as_deref().map_or(0, ShortKeys::mem_used)
    }

    /// Structural statistics, one component per part; see
    /// [`ShortBytesMapStats`].
    #[must_use]
    pub fn stats(&self) -> ShortBytesMapStats {
        let short = self.short.as_deref();
        ShortBytesMapStats {
            short_full: short.map(|s| s.full.stats()),
            short_part: short.map(|s| s.part.stats()),
            short_box_bytes: short.map_or(0, |_| core::mem::size_of::<ShortKeys>()),
            short_entries: short.map_or(0, ShortKeys::len),
            hashed: self.hashed.stats(),
        }
    }

    /// Returns the value stored for `key`.
    #[inline]
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        if key.len() <= 8 {
            let (trie, w) = self.short.as_deref()?.route(key);
            return trie.get(w);
        }
        self.hashed.get(key)
    }

    /// Membership test.
    #[must_use]
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None` if
    /// absent. Valid until the next insert or remove of any key in the map
    /// (module documentation, *Slot lifetime*).
    #[must_use]
    pub fn get_slot_ptr(&self, key: &[u8]) -> Option<NonNull<u64>> {
        if key.len() <= 8 {
            let (trie, w) = self.short.as_deref()?.route(key);
            return trie.get_slot_ptr(w);
        }
        self.hashed.get_slot_ptr(key)
    }

    /// The short-key tries, allocated on first use.
    fn short_mut(&mut self) -> &mut ShortKeys {
        self.short.get_or_insert_with(|| Box::new(ShortKeys::new()))
    }

    /// Inserts `key` with value 0 if absent — an existing value is kept —
    /// and returns a **writable pointer to its value slot**: the compat
    /// `JudyHSIns` contract. Valid until the next insert or remove of any key
    /// in the map.
    pub fn ins_slot(&mut self, key: &[u8]) -> NonNull<u64> {
        if key.len() <= 8 {
            let (trie, w) = self.short_mut().route_mut(key);
            // A fresh slot starts at 0, which is the value JudyHSIns gives.
            return trie.ins_slot_plain(w);
        }
        self.hashed.ins_slot(key)
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        if key.len() <= 8 {
            let (trie, w) = self.short_mut().route_mut(key);
            let before = trie.len();
            let slot = trie.ins_slot_plain(w);
            // SAFETY: the live value slot `ins_slot_plain` returned, valid
            // until the next structural mutation, which this call does not
            // perform after it.
            let prev = unsafe { *slot.as_ptr() };
            // SAFETY: as above.
            unsafe { *slot.as_ptr() = val };
            return (trie.len() == before).then_some(prev);
        }
        self.hashed.insert(key, val)
    }

    /// Removes `key`; returns its value if it was present.
    pub fn remove(&mut self, key: &[u8]) -> Option<u64> {
        if key.len() <= 8 {
            let (trie, w) = self.short.as_deref_mut()?.route_mut(key);
            return trie.remove_plain(w);
        }
        self.hashed.remove(key)
    }

    /// Visits every entry, in the order the module documentation describes.
    pub fn for_each(&self, mut f: impl FnMut(&[u8], u64)) {
        self.try_for_each(|k, v| {
            f(k, v);
            true
        });
    }

    /// Visits entries until `f` returns `false`. Returns the number of
    /// entries visited.
    pub fn try_for_each(&self, mut f: impl FnMut(&[u8], u64) -> bool) -> usize {
        let mut count = 0;
        if let Some(short) = self.short.as_deref() {
            for (full, trie) in [(true, &short.full), (false, &short.part)] {
                for (w, v) in trie.iter() {
                    let (bytes, n) = short_key_bytes(full, w);
                    count += 1;
                    if !f(&bytes[..n], v) {
                        return count;
                    }
                }
            }
        }
        count + self.hashed.try_for_each(f)
    }

    /// Removes every key and releases all memory, the short-key tries'
    /// box included.
    pub fn clear(&mut self) {
        self.short = None;
        self.hashed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    type Det = core::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;

    /// Keys around the routing boundaries: 0, 1, 2, 7, 8 and 9 bytes, with
    /// trailing zeros and 0xFF bytes, so a packing that confused a length
    /// with a zero byte would merge two of them.
    fn boundary_keys() -> Vec<Vec<u8>> {
        vec![
            vec![],
            vec![0],
            vec![0, 0],
            vec![0xFF],
            vec![1, 2],
            vec![0, 0, 0, 0, 0, 0, 0],
            vec![0xFF; 7],
            vec![0; 8],
            vec![0xFF; 8],
            vec![1, 2, 3, 4, 5, 6, 7, 8],
            vec![0; 9],
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
        ]
    }

    #[test]
    fn short_and_long_keys_round_trip_against_a_model() {
        let mut m = ExpanseShortBytesMap::with_hasher(Det::default());
        let mut model = BTreeMap::new();
        assert!(m.short.is_none(), "no short-key tries before a short key");
        assert_eq!(m.mem_used(), m.stats().total_bytes());
        for (i, k) in boundary_keys().into_iter().enumerate() {
            assert_eq!(m.insert(&k, i as u64), None, "fresh {k:02x?}");
            model.insert(k, i as u64);
        }
        assert_eq!(m.len(), model.len() as u64);
        // Only the two 9-byte keys took the hash path.
        assert_eq!(m.hashed.len(), 2);
        assert_eq!(m.stats().short_entries, 10);
        for (k, v) in &model {
            assert_eq!(m.get(k), Some(*v), "{k:02x?}");
            // SAFETY: a live slot of a present key, read before any mutation.
            assert_eq!(m.get_slot_ptr(k).map(|p| unsafe { *p.as_ptr() }), Some(*v));
        }
        assert_eq!(m.get(&[0, 0, 0]), None);
        assert_eq!(m.get(&[7; 8]), None);
        // Overwrite returns the old value on both paths.
        assert_eq!(m.insert(&[0, 0], 100), Some(2));
        assert_eq!(m.insert(&[0; 9], 101), Some(10));
        model.insert(vec![0, 0], 100);
        model.insert(vec![0; 9], 101);
        // ins_slot keeps a present value and zeroes a fresh one.
        // SAFETY: slots read immediately, before any other mutation.
        unsafe {
            assert_eq!(*m.ins_slot(&[0xFF]).as_ptr(), 3);
            assert_eq!(*m.ins_slot(&[9, 9, 9]).as_ptr(), 0);
            *m.ins_slot(&[9, 9, 9]).as_ptr() = 77;
        }
        model.insert(vec![9, 9, 9], 77);
        // Iteration returns every key's exact bytes.
        let mut seen = BTreeMap::new();
        m.for_each(|k, v| {
            assert!(
                seen.insert(k.to_vec(), v).is_none(),
                "{k:02x?} visited twice"
            );
        });
        assert_eq!(seen, model);
        assert_eq!(
            m.try_for_each(|_, _| false),
            1,
            "stops after the first entry"
        );
        assert_eq!(m.mem_used(), m.stats().total_bytes());
        // Removing everything returns every value; the tries keep only the
        // box's bytes until clear.
        for (k, v) in &model {
            assert_eq!(m.remove(k), Some(*v), "{k:02x?}");
            assert_eq!(m.remove(k), None);
        }
        assert!(m.is_empty());
        assert_eq!(m.mem_used(), m.stats().total_bytes());
        m.clear();
        assert!(m.short.is_none());
        assert_eq!(
            m.mem_used(),
            ExpanseShortBytesMap::with_hasher(Det::default()).mem_used()
        );
    }

    #[test]
    fn long_keys_never_allocate_the_short_tries() {
        let mut m = ExpanseShortBytesMap::with_hasher(Det::default());
        for i in 0..1000u32 {
            let k = format!("/api/v2/tenants/{i:06}");
            m.insert(k.as_bytes(), u64::from(i));
            // A short-key miss allocates nothing either.
            assert_eq!(m.get(&i.to_be_bytes()), None);
            assert_eq!(m.remove(&i.to_be_bytes()), None);
        }
        assert!(m.short.is_none());
        assert_eq!(m.mem_used(), m.hashed.mem_used());
    }

    #[test]
    fn random_keys_match_a_model() {
        let mut m = ExpanseShortBytesMap::with_hasher(Det::default());
        let mut model = BTreeMap::new();
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        for step in 0..20_000u64 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let len = (x % 13) as usize;
            let k: Vec<u8> = (0..len).map(|i| (x >> (i * 5)) as u8 & 0x7).collect();
            match x >> 60 {
                0..=9 => assert_eq!(m.insert(&k, step), model.insert(k, step)),
                10..=13 => assert_eq!(m.remove(&k), model.remove(&k)),
                _ => assert_eq!(m.get(&k), model.get(&k).copied()),
            }
        }
        assert_eq!(m.len(), model.len() as u64);
        let mut seen = BTreeMap::new();
        m.for_each(|k, v| {
            seen.insert(k.to_vec(), v);
        });
        assert_eq!(seen, model);
        assert_eq!(m.mem_used(), m.stats().total_bytes());
    }
}
