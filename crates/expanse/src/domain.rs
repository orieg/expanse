//! Interned set domain: shared dictionary vending posting-list sets over a single ordinal space (issue #611).
//!
//! [`ExpanseSet`] algebra runs on `u64` keys. When real-world identities are byte slices
//! (term IDs, UUIDs, URLs, tag names), uncoordinated integer sets risk silent cross-dictionary
//! corruption if two sets built from different vocabularies are intersected.
//!
//! [`ExpanseDomainDict`] owns a single shared prefix-compressed dictionary ([`ExpanseStrMap`])
//! and reverse slab arena ([`BlobArena`]), vending [`DomainSet`] values tagged with the
//! dictionary's provenance (`domain_id: u64`).
//!
//! ### Key Architectural Guarantees
//!
//! 1. **First-Class Value Semantics**: [`DomainSet`] values are standalone containers managed
//!    by standard Rust RAII. Sets are never locked inside an internal table, eliminating
//!    manual cleanup leaks in multi-predicate query pipelines.
//! 2. **Pure &self Algebra**: Set algebra operations ([`DomainSet::intersection`],
//!    [`DomainSet::union`], [`DomainSet::difference`]) take immutable references and execute
//!    pure calculations requiring zero locks on the dictionary.
//! 3. **Prefix Compression & Order Preservation**: Keys are stored in an 8-byte-chunked digital
//!    trie, compressing common prefixes. Arbitrary binary slices (including NUL-carrying UUIDs)
//!    are escaped via order-preserving byte-stuffing (`0x00 -> [0x01, 0x01]`, `0x01 -> [0x01, 0x02]`),
//!    strictly preventing silent NUL truncation while preserving lexicographical order.
//! 4. **Stable Slab Reverse Storage**: Payloads are stored in [`BlobArena`] using 64-bit global
//!    offsets. Memory addresses never move on chunk allocation, and reverse resolution yields
//!    uniform borrowed `&[u8]` slices directly from stable chunks.

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::blobmap::{ArenaError, BlobArena, DEFAULT_CHUNK_SIZE};
use crate::set::ExpanseSet;
use crate::strmap::ExpanseStrMap;

static NEXT_DOMAIN_ID: AtomicU64 = AtomicU64::new(1);

/// Error returned when attempting set algebra or dictionary operations across distinct domains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainMismatch {
    /// The expected domain identifier.
    pub expected: u64,
    /// The actual domain identifier encountered.
    pub got: u64,
}

impl core::fmt::Display for DomainMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "domain mismatch: expected domain ID {}, got {}",
            self.expected, self.got
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DomainMismatch {}

/// Unified error type for domain dictionary operations.
#[derive(Debug, PartialEq, Eq)]
pub enum DomainError {
    /// Operands originated from different domain dictionaries.
    Mismatch(DomainMismatch),
    /// Slab arena allocation failure (e.g. capacity exhausted or oversized record).
    Arena(ArenaError),
}

impl core::fmt::Display for DomainError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Mismatch(m) => write!(f, "{m}"),
            Self::Arena(a) => write!(f, "arena allocation error: {a:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DomainError {}

impl From<DomainMismatch> for DomainError {
    fn from(m: DomainMismatch) -> Self {
        Self::Mismatch(m)
    }
}

impl From<ArenaError> for DomainError {
    fn from(a: ArenaError) -> Self {
        Self::Arena(a)
    }
}

/// A branded ordinal identifying an interned identity within a specific [`ExpanseDomainDict`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainOrdinal {
    pub(crate) domain_id: u64,
    pub(crate) ordinal: u64,
}

impl DomainOrdinal {
    /// Constructs a branded ordinal for the given domain and ordinal values.
    #[inline]
    #[must_use]
    pub const fn new(domain_id: u64, ordinal: u64) -> Self {
        Self { domain_id, ordinal }
    }

    /// Returns the domain identifier that vended this ordinal.
    #[inline]
    #[must_use]
    pub fn domain_id(&self) -> u64 {
        self.domain_id
    }

    /// Returns the raw integer ordinal value.
    #[inline]
    #[must_use]
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
}

/// Order-preserving escape encoding for arbitrary byte slices into [`ExpanseStrMap`].
///
/// In `ExpanseStrMap`, an 8-byte chunk terminates when a `0x00` byte is encountered.
/// To support arbitrary byte slices (including 16-byte binary UUIDs, where $P(\ge 1\text{ NUL}) \approx 1.000$),
/// this helper transforms:
/// - `0x00 -> [0x01, 0x01]`
/// - `0x01 -> [0x01, 0x02]`
/// - `b -> [b]` (for `b in 0x02..=0xFF`)
#[inline]
pub(crate) fn escape_encode(data: &[u8]) -> Vec<u8> {
    if !data.iter().any(|&b| b <= 1) {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len() + 4);
    for &b in data {
        match b {
            0 => {
                out.push(1);
                out.push(1);
            }
            1 => {
                out.push(1);
                out.push(2);
            }
            _ => out.push(b),
        }
    }
    out
}

#[inline]
fn with_escaped_key<R>(key: &[u8], f: impl FnOnce(&[u8]) -> R) -> R {
    if !key.iter().any(|&b| b <= 1) {
        f(key)
    } else {
        let encoded = escape_encode(key);
        f(&encoded)
    }
}

/// A shared identity dictionary that vends domain-branded posting-list sets over a single ordinal space.
pub struct ExpanseDomainDict {
    domain_id: u64,
    forward: ExpanseStrMap,
    reverse_arena: BlobArena,
    reverse_offsets: Vec<u64>,
    next_ordinal: u64,
}

impl ExpanseDomainDict {
    /// Creates a new, empty domain dictionary with a globally unique domain identifier.
    #[must_use]
    pub fn new() -> Self {
        Self::with_chunk_size(DEFAULT_CHUNK_SIZE)
    }

    /// Creates a new domain dictionary with a custom reverse slab chunk size.
    #[must_use]
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        let domain_id = NEXT_DOMAIN_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            domain_id,
            forward: ExpanseStrMap::new(),
            reverse_arena: BlobArena::new(chunk_size),
            reverse_offsets: Vec::new(),
            next_ordinal: 0,
        }
    }

    /// Returns the unique domain identifier.
    #[inline]
    #[must_use]
    pub fn domain_id(&self) -> u64 {
        self.domain_id
    }

    /// Vends a new, empty [`DomainSet`] bound to this domain.
    #[must_use]
    pub fn new_set(&self) -> DomainSet {
        DomainSet {
            domain_id: self.domain_id,
            set: ExpanseSet::new(),
        }
    }

    /// Interns an identity byte slice into the dictionary, returning its branded [`DomainOrdinal`].
    ///
    /// If `key` has already been interned, returns the existing ordinal with zero allocation.
    /// If `key` is new, copies its unencoded bytes into [`BlobArena`] and indexes its escaped
    /// representation in [`ExpanseStrMap`].
    pub fn intern(&mut self, key: &[u8]) -> Result<DomainOrdinal, ArenaError> {
        let ord = with_escaped_key(key, |enc_key| -> Result<u64, ArenaError> {
            if let Some(existing) = self.forward.get(enc_key) {
                return Ok(existing);
            }
            let ord = self.next_ordinal;
            let global_offset = self.reverse_arena.alloc_blob(key)?;
            self.forward.insert(enc_key, ord);
            self.reverse_offsets.push(global_offset);
            self.next_ordinal += 1;
            Ok(ord)
        })?;
        Ok(DomainOrdinal {
            domain_id: self.domain_id,
            ordinal: ord,
        })
    }

    /// Interns a batch of keys, populating `out_ordinals`.
    pub fn intern_batch(
        &mut self,
        keys: &[&[u8]],
        out_ordinals: &mut [DomainOrdinal],
    ) -> Result<(), ArenaError> {
        assert_eq!(
            keys.len(),
            out_ordinals.len(),
            "keys and out_ordinals length mismatch"
        );
        for (k, out) in keys.iter().zip(out_ordinals.iter_mut()) {
            *out = self.intern(k)?;
        }
        Ok(())
    }

    /// Inserts an identity byte slice into `set`, interning it into the dictionary if necessary.
    ///
    /// Returns `Ok(true)` if the identity was newly inserted into `set`, `Ok(false)` if it
    /// was already present in `set`, or `Err(DomainError::Mismatch)` if `set` belongs to a
    /// different domain.
    pub fn insert(&mut self, set: &mut DomainSet, key: &[u8]) -> Result<bool, DomainError> {
        if set.domain_id != self.domain_id {
            return Err(DomainError::Mismatch(DomainMismatch {
                expected: self.domain_id,
                got: set.domain_id,
            }));
        }
        let id = self.intern(key)?;
        Ok(set.set.insert(id.ordinal))
    }

    /// Inserts a batch of keys into `set`, returning the number of newly inserted elements.
    pub fn insert_batch(
        &mut self,
        set: &mut DomainSet,
        keys: &[&[u8]],
    ) -> Result<usize, DomainError> {
        if set.domain_id != self.domain_id {
            return Err(DomainError::Mismatch(DomainMismatch {
                expected: self.domain_id,
                got: set.domain_id,
            }));
        }
        let mut added = 0usize;
        for &k in keys {
            let id = self.intern(k)?;
            if set.set.insert(id.ordinal) {
                added += 1;
            }
        }
        Ok(added)
    }

    /// Checks whether `key` is present in `set`.
    ///
    /// If `key` is not present in the dictionary at all, returns `Ok(false)` immediately
    /// without mutating or growing the dictionary vocabulary.
    pub fn contains(&self, set: &DomainSet, key: &[u8]) -> Result<bool, DomainMismatch> {
        if set.domain_id != self.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: set.domain_id,
            });
        }
        let Some(ord) = with_escaped_key(key, |k| self.forward.get(k)) else {
            return Ok(false);
        };
        Ok(set.set.contains(ord))
    }

    /// Removes an identity from `set`.
    ///
    /// Returns `Ok(true)` if the identity was present in `set` and removed, or `Ok(false)`
    /// if it was absent. Does not mutate the dictionary or remove its interned ordinal.
    pub fn remove(&self, set: &mut DomainSet, key: &[u8]) -> Result<bool, DomainMismatch> {
        if set.domain_id != self.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: set.domain_id,
            });
        }
        let Some(ord) = with_escaped_key(key, |k| self.forward.get(k)) else {
            return Ok(false);
        };
        Ok(set.set.remove(ord))
    }

    /// Resolves a branded [`DomainOrdinal`] back to its original identity byte slice.
    pub fn resolve_id(&self, id: DomainOrdinal) -> Result<Option<&[u8]>, DomainMismatch> {
        if id.domain_id != self.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: id.domain_id,
            });
        }
        Ok(self.resolve_ordinal_unbranded(id.ordinal))
    }

    /// Internal unbranded ordinal resolution.
    #[inline]
    pub(crate) fn resolve_ordinal_unbranded(&self, ordinal: u64) -> Option<&[u8]> {
        let idx = usize::try_from(ordinal).ok()?;
        let global_offset = *self.reverse_offsets.get(idx)?;
        self.reverse_arena.get_blob_slice(global_offset)
    }

    /// Resolves an entire [`DomainSet`] into a zero-copy iterator over borrowed identity slices.
    pub fn resolve<'a>(&'a self, set: &'a DomainSet) -> Result<ResolveIter<'a>, DomainMismatch> {
        if set.domain_id != self.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: set.domain_id,
            });
        }
        Ok(ResolveIter {
            dict: self,
            iter: set.set.iter(),
        })
    }

    /// Returns the total memory (in bytes) used by the dictionary (forward trie + reverse arena + offset table).
    #[must_use]
    pub fn dictionary_mem_used(&self) -> usize {
        let forward_mem = self.forward.mem_used();
        let arena_mem = self.reverse_arena.mem_used();
        let offsets_mem = self.reverse_offsets.capacity() * core::mem::size_of::<u64>();
        let self_mem = core::mem::size_of::<Self>();
        forward_mem + arena_mem + offsets_mem + self_mem
    }

    /// Total number of unique identities interned in this dictionary.
    #[inline]
    #[must_use]
    pub fn len(&self) -> u64 {
        self.next_ordinal
    }

    /// True if no identities have been interned.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.next_ordinal == 0
    }
}

impl Default for ExpanseDomainDict {
    fn default() -> Self {
        Self::new()
    }
}

/// A posting-list set whose elements are ordinals scoped to an [`ExpanseDomainDict`].
pub struct DomainSet {
    domain_id: u64,
    set: ExpanseSet,
}

impl Clone for DomainSet {
    fn clone(&self) -> Self {
        Self {
            domain_id: self.domain_id,
            set: ExpanseSet::from_sorted_iter(self.set.iter()),
        }
    }
}

impl PartialEq for DomainSet {
    fn eq(&self, other: &Self) -> bool {
        self.domain_id == other.domain_id
            && self.set.len() == other.set.len()
            && self.set.iter().eq(other.set.iter())
    }
}

impl Eq for DomainSet {}

impl core::fmt::Debug for DomainSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DomainSet")
            .field("domain_id", &self.domain_id)
            .field("len", &self.len())
            .finish()
    }
}

impl DomainSet {
    /// Returns the domain identifier that vended this set.
    #[inline]
    #[must_use]
    pub fn domain_id(&self) -> u64 {
        self.domain_id
    }

    /// Returns the number of elements in the set.
    #[inline]
    #[must_use]
    pub fn len(&self) -> u64 {
        self.set.len()
    }

    /// True if the set contains no elements.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Returns the heap memory used by this set's trie nodes.
    #[inline]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.set.mem_used() + core::mem::size_of::<Self>()
    }

    /// Checks whether this set contains the specified branded ordinal.
    pub fn contains_ordinal(&self, id: DomainOrdinal) -> Result<bool, DomainMismatch> {
        if self.domain_id != id.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: id.domain_id,
            });
        }
        Ok(self.set.contains(id.ordinal))
    }

    /// Pure set intersection: returns a new [`DomainSet`] containing elements in both `self` and `other`.
    pub fn intersection(&self, other: &Self) -> Result<Self, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        Ok(Self {
            domain_id: self.domain_id,
            set: self.set.intersection(&other.set),
        })
    }

    /// Pure set union: returns a new [`DomainSet`] containing elements in `self` or `other`.
    pub fn union(&self, other: &Self) -> Result<Self, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        Ok(Self {
            domain_id: self.domain_id,
            set: self.set.union(&other.set),
        })
    }

    /// Pure set difference: returns a new [`DomainSet`] containing elements in `self` but not `other`.
    pub fn difference(&self, other: &Self) -> Result<Self, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        Ok(Self {
            domain_id: self.domain_id,
            set: self.set.difference(&other.set),
        })
    }

    /// Pure symmetric difference: returns a new [`DomainSet`] containing elements in either `self` or `other`, but not both.
    pub fn symmetric_difference(&self, other: &Self) -> Result<Self, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        Ok(Self {
            domain_id: self.domain_id,
            set: self.set.symmetric_difference(&other.set),
        })
    }

    /// Counts the elements in `self ∩ other` without allocating a result trie.
    pub fn intersection_len(&self, other: &Self) -> Result<u64, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        Ok(self.set.intersection_len(&other.set))
    }

    /// Counts the elements in `self ∪ other` without allocating a result trie.
    pub fn union_len(&self, other: &Self) -> Result<u64, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        Ok(self.set.union_len(&other.set))
    }

    /// True if `self` is a subset of `other`.
    pub fn is_subset(&self, other: &Self) -> Result<bool, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        // A ⊆ B iff |A ∩ B| == |A|
        Ok(self.set.intersection_len(&other.set) == self.set.len())
    }

    /// True if `self` and `other` share no common elements.
    pub fn is_disjoint(&self, other: &Self) -> Result<bool, DomainMismatch> {
        if self.domain_id != other.domain_id {
            return Err(DomainMismatch {
                expected: self.domain_id,
                got: other.domain_id,
            });
        }
        // A and B are disjoint iff |A ∩ B| == 0
        Ok(self.set.intersection_len(&other.set) == 0)
    }

    /// Intersects multiple domain sets (`⋂ S_i`) in a single pass (#610).
    ///
    /// Returns `Ok(None)` if `sets` is empty.
    pub fn intersection_many(sets: &[&Self]) -> Result<Option<Self>, DomainMismatch> {
        if sets.is_empty() {
            return Ok(None);
        }
        let expected_domain = sets[0].domain_id;
        for s in &sets[1..] {
            if s.domain_id != expected_domain {
                return Err(DomainMismatch {
                    expected: expected_domain,
                    got: s.domain_id,
                });
            }
        }
        let raw_sets: Vec<&ExpanseSet> = sets.iter().map(|s| &s.set).collect();
        Ok(Some(Self {
            domain_id: expected_domain,
            set: ExpanseSet::intersection_many(&raw_sets),
        }))
    }

    /// Unions multiple domain sets (`⋃ S_i`) in a single pass (#610).
    ///
    /// Returns `Ok(None)` if `sets` is empty.
    pub fn union_many(sets: &[&Self]) -> Result<Option<Self>, DomainMismatch> {
        if sets.is_empty() {
            return Ok(None);
        }
        let expected_domain = sets[0].domain_id;
        for s in &sets[1..] {
            if s.domain_id != expected_domain {
                return Err(DomainMismatch {
                    expected: expected_domain,
                    got: s.domain_id,
                });
            }
        }
        let raw_sets: Vec<&ExpanseSet> = sets.iter().map(|s| &s.set).collect();
        Ok(Some(Self {
            domain_id: expected_domain,
            set: ExpanseSet::union_many(&raw_sets),
        }))
    }

    /// Counts elements in `⋂ S_i` across multiple sets in a single pass without allocating a result trie.
    pub fn intersection_len_many(sets: &[&Self]) -> Result<u64, DomainMismatch> {
        if sets.is_empty() {
            return Ok(0);
        }
        let expected_domain = sets[0].domain_id;
        for s in &sets[1..] {
            if s.domain_id != expected_domain {
                return Err(DomainMismatch {
                    expected: expected_domain,
                    got: s.domain_id,
                });
            }
        }
        let raw_sets: Vec<&ExpanseSet> = sets.iter().map(|s| &s.set).collect();
        Ok(ExpanseSet::intersection_len_many(&raw_sets))
    }

    /// Counts elements in `⋃ S_i` across multiple sets in a single pass without allocating a result trie.
    pub fn union_len_many(sets: &[&Self]) -> Result<u64, DomainMismatch> {
        if sets.is_empty() {
            return Ok(0);
        }
        let expected_domain = sets[0].domain_id;
        for s in &sets[1..] {
            if s.domain_id != expected_domain {
                return Err(DomainMismatch {
                    expected: expected_domain,
                    got: s.domain_id,
                });
            }
        }
        let raw_sets: Vec<&ExpanseSet> = sets.iter().map(|s| &s.set).collect();
        Ok(ExpanseSet::union_len_many(&raw_sets))
    }
}

/// A zero-copy iterator resolving a [`DomainSet`]'s elements back to borrowed identity slices.
pub struct ResolveIter<'a> {
    dict: &'a ExpanseDomainDict,
    iter: crate::set::SetIter<'a>,
}

impl<'a> Iterator for ResolveIter<'a> {
    type Item = &'a [u8];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        for ord in self.iter.by_ref() {
            if let Some(bytes) = self.dict.resolve_ordinal_unbranded(ord) {
                return Some(bytes);
            }
        }
        None
    }
}
