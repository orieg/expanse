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
pub use crate::validate::{BytesMapNodeBytes, BytesMapStats};
use core::hash::BuildHasher;
use core::ptr::NonNull;
#[cfg(feature = "std")]
use core_alloc::sync::Arc;
use core_alloc::vec::Vec;
#[cfg(feature = "std")]
use std::sync::OnceLock;

/// Default build hasher type: [`std::hash::RandomState`] under `std`, or [`core::hash::BuildHasherDefault<FnvHasher>`] in `no_std`.
///
/// In `std` builds, [`std::hash::RandomState`] provides per-process randomized keys to resist hash-flooding DoS attacks.
#[cfg(feature = "std")]
pub type DefaultBuildHasher = std::hash::RandomState;

/// Deterministic 64-bit FNV-1a hasher for `no_std` environments.
///
/// # Security
///
/// FNV-1a is deterministic with a fixed basis. When processing untrusted or attacker-controlled keys in `no_std`,
/// prefer providing a seeded or cryptographically secure [`core::hash::BuildHasher`] via [`ExpanseBytesMap::with_hasher`].
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
///
/// In `no_std` builds, this defaults to deterministic 64-bit FNV-1a. If keys are untrusted, supply a custom
/// [`core::hash::BuildHasher`] via [`ExpanseBytesMap::with_hasher`].
#[cfg(not(feature = "std"))]
pub type DefaultBuildHasher = core::hash::BuildHasherDefault<FnvHasher>;

/// The 64-bit hash a key is filed under: its bytes, written once.
///
/// `BuildHasher::hash_one(&[u8])` would write the slice's length ahead of its
/// bytes, which a composite key needs to stay prefix-free and a single byte
/// string does not: the map hashes nothing but the key, SipHash folds the
/// message length into its finalization, and every bucket compares keys
/// byte-exactly, so two keys that hash alike only share a bucket. Skipping
/// the prefix saves one hasher `write` per operation. Every path that files
/// or finds a key (the plain map, the shared map's readers and optimistic
/// writers) hashes through this function, so they agree.
#[inline(always)]
pub(crate) fn hash_key<S: BuildHasher>(hasher: &S, key: &[u8]) -> u64 {
    use core::hash::Hasher;
    let mut h = hasher.build_hasher();
    h.write(key);
    h.finish()
}

/// One collision-bucket entry: its key bytes, which sit inside the
/// bucket's own allocation, and the value word.
///
/// `key` points into the same allocation as the entry, with that
/// allocation's provenance, so comparing a key is one load and no address
/// arithmetic. A bucket is never moved or copied bytewise: a replacement
/// is built entry by entry, so every `key` it holds points into itself.
#[repr(C)]
pub(crate) struct Entry {
    key: *const u8,
    key_len: usize,
    val: u64,
}

/// The header of one hash bucket: entries whose keys share a 64-bit
/// hash. Almost always a single entry; compared byte-exactly on every
/// operation.
///
/// A bucket is **one allocation** (#1760): this header, then `len`
/// [`Entry`] records, then the entries' key bytes, concatenated in entry
/// order. A fresh key therefore costs one allocation, and a bucket is
/// freed or retired as one block. The value words sit at a fixed stride
/// and are 8-aligned on 64-bit targets (a two-word header, 24-byte
/// entries, the value last), so a value slot pointer is a plain `*mut u64`
/// into the allocation.
///
/// The header holds the entry array's address as well as its length,
/// although the array always starts right after it. A lookup reads both
/// from the header, as it read a `Vec`'s, so the pointer it walks the
/// entries with is loaded straight into a register the key compare's
/// call preserves. Walking from the bucket word instead keeps the word
/// itself live across that call, and the x86 Callgrind arms measured the
/// copies and the extra jump that cost on every probe (#1760).
/// Only [`bucket_alloc_size`] and the functions here know the layout;
/// every other module reaches a bucket through them.
///
/// Phase 7 contract (issue #219): a published bucket is **write-once
/// except its value words**. Structural changes (adding or removing an
/// entry) build a replacement bucket of exact size, publish it over the
/// trie entry, and dispose of the old one — retired through the epoch
/// collector when the map is concurrently shared, so a reader that
/// validated the bucket pointer may keep reading the header, entries and
/// key bytes under its pin. Only the value word mutates in place (a
/// single `u64`).
///
/// #929: under the concurrent wrapper the value word is the **one**
/// field two threads may touch at once, so both sides reach it through
/// [`entry_value_atomic`] rather than as a plain `u64`. A writer stores
/// only while it holds the terminal's parent version lock and only after
/// that lock's compare has confirmed the bucket is still the published
/// one, so an unlinked bucket's value words are frozen from the moment
/// of the replacement; a reader loads without any lock. See
/// [`ExpanseBytesMap::get_validated`] for what a reader can observe.
#[repr(C)]
pub(crate) struct Bucket {
    len: usize,
    /// The entry array, which starts right after this header, with the
    /// allocation's provenance.
    entries: *mut Entry,
}

/// Alignment of a bucket allocation: the alignment of its words.
const BUCKET_ALIGN: usize = align_of::<Entry>();

// The value word must be 8-aligned for `AtomicU64::from_ptr`, and the
// entries must start right after the header for the stride arithmetic.
const _: () = {
    assert!(size_of::<Bucket>().is_multiple_of(BUCKET_ALIGN));
    assert!(size_of::<Entry>().is_multiple_of(BUCKET_ALIGN));
    assert!(core::mem::offset_of!(Entry, val).is_multiple_of(align_of::<u64>()));
    assert!(BUCKET_ALIGN >= align_of::<u64>());
};

/// Bytes of a bucket holding `n` entries whose keys total `key_bytes`.
#[inline(always)]
const fn bucket_alloc_size(n: usize, key_bytes: usize) -> usize {
    size_of::<Bucket>() + n * size_of::<Entry>() + key_bytes
}

/// The layout of a bucket of `bytes` bytes.
#[inline(always)]
fn bucket_layout(bytes: usize) -> core::alloc::Layout {
    core::alloc::Layout::from_size_align(bytes, BUCKET_ALIGN).expect("bucket layout")
}

/// The number of entries of the bucket at `b`.
///
/// # Safety
///
/// `b` is a live (or EBR-live) bucket built by [`BucketBuilder`].
#[inline(always)]
pub(crate) unsafe fn bucket_len(b: *const Bucket) -> usize {
    // SAFETY: caller contract; the header is write-once.
    unsafe { (*b).len }
}

/// The entry array of the bucket at `b`, with the allocation's provenance.
///
/// # Safety
///
/// As [`bucket_len`].
#[inline(always)]
pub(crate) unsafe fn bucket_entries(b: *const Bucket) -> *mut Entry {
    // SAFETY: caller contract; the header is write-once, and its pointer
    // was derived from the allocation by `BucketBuilder::new`.
    unsafe { (*b).entries }
}

/// The key bytes of entry `i` of the bucket at `b`.
///
/// Reads only the write-once key fields, never the value word.
///
/// # Safety
///
/// As [`bucket_len`], with `i < bucket_len(b)`; the slice must not
/// outlive the bucket (or the caller's epoch pin).
#[inline(always)]
pub(crate) unsafe fn bucket_key<'a>(b: *const Bucket, i: usize) -> &'a [u8] {
    // SAFETY: `i` is in bounds; the projections name the two key fields
    // only, and the bytes they locate are inside `b`'s allocation and
    // write-once after publication.
    unsafe {
        let e = bucket_entries(b).add(i);
        core::slice::from_raw_parts((*e).key, (*e).key_len)
    }
}

/// A raw pointer to entry `i`'s value word, with `b`'s provenance.
///
/// # Safety
///
/// As [`bucket_key`].
#[inline(always)]
pub(crate) unsafe fn bucket_value_ptr(b: *const Bucket, i: usize) -> *mut u64 {
    // SAFETY: caller contract; a raw place projection, no reference.
    unsafe { &raw mut (*bucket_entries(b).add(i)).val }
}

/// The entry of `key` in the bucket at `b`, if present, with the
/// allocation's provenance.
///
/// The search of every lookup that wants the entry rather than only its
/// value. Which loop shape costs least differs by call site, so each site
/// keeps the one its Callgrind arm measured best (#1760): the plain
/// [`ExpanseBytesMap::get`] counts an index, and the `&mut self` insert
/// walks from the bucket word ([`bucket_search_from_base`]).
///
/// Reads only the write-once key fields, never a value word.
///
/// # Safety
///
/// As [`bucket_len`]; the returned pointer must not outlive the bucket (or
/// the caller's epoch pin).
#[inline(always)]
pub(crate) unsafe fn bucket_search(b: *const Bucket, key: &[u8]) -> Option<*mut Entry> {
    // SAFETY: caller contract, forwarded.
    unsafe { entries_search(bucket_entries(b), bucket_len(b), key) }
}

/// [`bucket_search`] with the entry array located by its offset from `b`
/// rather than read from the header. The `&mut self` insert searches this
/// way: the bucket word stays live there anyway, and the header read
/// costs it instructions (#1760).
///
/// # Safety
///
/// As [`bucket_search`].
#[inline(always)]
unsafe fn bucket_search_from_base(b: *const Bucket, key: &[u8]) -> Option<*mut Entry> {
    // SAFETY: caller contract; the entries follow the header inside the
    // same allocation.
    unsafe { entries_search(b.cast_mut().add(1).cast::<Entry>(), bucket_len(b), key) }
}

/// The search behind [`bucket_search`]: the entry of `key` among the `len`
/// entries at `entries`.
///
/// # Safety
///
/// `entries` and `len` describe the entry array of a live (or EBR-live)
/// bucket.
#[inline(always)]
unsafe fn entries_search(entries: *mut Entry, len: usize, key: &[u8]) -> Option<*mut Entry> {
    // SAFETY: caller contract; `e` stays in `[entries, entries + len)`, and
    // each key slice lies inside the bucket's allocation and is write-once.
    unsafe {
        let mut e = entries;
        let end = e.add(len);
        while e != end {
            if core::slice::from_raw_parts((*e).key, (*e).key_len) == key {
                return Some(e);
            }
            e = e.add(1);
        }
    }
    None
}

/// The index of `key` in the bucket at `b`, if present: [`bucket_search`]
/// for the callers that rebuild a bucket around one entry.
///
/// # Safety
///
/// As [`bucket_len`].
#[inline(always)]
unsafe fn bucket_position(b: *const Bucket, key: &[u8]) -> Option<usize> {
    // SAFETY: caller contract; a found entry lies in the entry array, at or
    // after its start.
    unsafe {
        let e = bucket_search(b, key)?;
        Some(e.offset_from(bucket_entries(b)) as usize)
    }
}

/// The bytes of the bucket at `b`'s allocation: its keys end it, in entry
/// order, so the last entry's key ends where the allocation does.
///
/// # Safety
///
/// As [`bucket_len`]; a bucket always holds at least one entry.
#[inline(always)]
unsafe fn bucket_bytes(b: *const Bucket) -> usize {
    // SAFETY: caller contract; `len >= 1` for every built bucket.
    unsafe {
        let len = bucket_len(b);
        debug_assert!(len >= 1, "a bucket is never empty");
        let last = bucket_entries(b).add(len - 1);
        (*last).key.addr() - b.addr() + (*last).key_len
    }
}

/// Builds a bucket of a known entry count and key byte total in place.
///
/// Entries are pushed in order; [`Self::finish`] checks the counts and
/// hands out the bucket. Dropping an unfinished builder frees its
/// allocation, so a panic between pushes leaks nothing.
pub(crate) struct BucketBuilder {
    base: NonNull<Bucket>,
    n: usize,
    bytes: usize,
    pushed: usize,
    key_end: usize,
}

impl BucketBuilder {
    /// An allocation for `n >= 1` entries whose keys total `key_bytes`.
    pub(crate) fn new(n: usize, key_bytes: usize) -> Self {
        assert!(n >= 1, "a bucket is never empty");
        let bytes = size_of::<Entry>()
            .checked_mul(n)
            .and_then(|e| e.checked_add(size_of::<Bucket>()))
            .and_then(|h| h.checked_add(key_bytes))
            .expect("bucket size overflows usize");
        let layout = bucket_layout(bytes);
        // SAFETY: `layout` has a non-zero size (at least one entry).
        let raw = unsafe { core_alloc::alloc::alloc(layout) };
        let Some(raw) = NonNull::new(raw.cast::<Bucket>()) else {
            core_alloc::alloc::handle_alloc_error(layout)
        };
        // SAFETY: the header is at the start of the allocation, and the
        // entries follow it inside the allocation (`bytes` covers both).
        unsafe {
            raw.as_ptr().write(Bucket {
                len: n,
                entries: raw.as_ptr().add(1).cast::<Entry>(),
            });
        }
        Self {
            base: raw,
            n,
            bytes,
            pushed: 0,
            key_end: bucket_alloc_size(n, 0),
        }
    }

    /// Appends the next entry.
    #[inline(always)]
    pub(crate) fn push(&mut self, key: &[u8], val: u64) {
        assert!(self.pushed < self.n, "bucket entry count exceeded");
        assert!(
            key.len() <= self.bytes - self.key_end,
            "bucket key bytes exceeded"
        );
        let b = self.base.as_ptr();
        // SAFETY: `pushed < n` indexes the entry array, and the key bytes
        // land in `[key_end, key_end + key.len())`, inside the allocation by
        // the assert above. Neither region has been handed out.
        unsafe {
            bucket_entries(b).add(self.pushed).write(Entry {
                key: b.cast::<u8>().add(self.key_end).cast_const(),
                key_len: key.len(),
                val,
            });
            core::ptr::copy_nonoverlapping(
                key.as_ptr(),
                b.cast::<u8>().add(self.key_end),
                key.len(),
            );
        }
        self.pushed += 1;
        self.key_end += key.len();
    }

    /// The built bucket, with every entry and key byte written.
    #[inline(always)]
    pub(crate) fn finish(self) -> *mut Bucket {
        assert!(
            self.pushed == self.n && self.key_end == self.bytes,
            "bucket built short"
        );
        let b = self.base.as_ptr();
        core::mem::forget(self);
        b
    }
}

impl Drop for BucketBuilder {
    fn drop(&mut self) {
        // SAFETY: the builder still owns its allocation of `bytes` bytes.
        unsafe { core_alloc::alloc::dealloc(self.base.as_ptr().cast(), bucket_layout(self.bytes)) };
    }
}

/// A copy of the bucket at `old` with `key → val` appended: the
/// replacement a colliding insert publishes on the `&mut self` path.
///
/// Out of line and cold. A 64-bit hash collision is rare, and keeping this
/// loop out of the insert keeps the insert's body, and the inlining choices
/// LTO makes around it, the size of the paths that run (#1760).
///
/// # Safety
///
/// `old` is a live bucket whose value words no other thread stores to
/// (they are read plainly here).
#[cold]
#[inline(never)]
unsafe fn bucket_appending(old: *const Bucket, key: &[u8], val: u64) -> *mut Bucket {
    // SAFETY: caller contract; `i < len` indexes the live bucket.
    unsafe {
        let len = bucket_len(old);
        let keys: usize = (0..len).map(|i| bucket_key(old, i).len()).sum();
        let mut fresh = BucketBuilder::new(len + 1, keys + key.len());
        for i in 0..len {
            fresh.push(bucket_key(old, i), *bucket_value_ptr(old, i));
        }
        fresh.push(key, val);
        fresh.finish()
    }
}

/// A copy of the bucket at `old` without entry `at`: the replacement a
/// removal from a colliding bucket publishes on the `&mut self` path. Cold
/// and out of line, as [`bucket_appending`].
///
/// # Safety
///
/// As [`bucket_appending`], with `at < bucket_len(old)` and the bucket
/// holding at least two entries.
#[cold]
#[inline(never)]
unsafe fn bucket_removing(old: *const Bucket, at: usize) -> *mut Bucket {
    // SAFETY: caller contract; `i < len` indexes the live bucket.
    unsafe {
        let len = bucket_len(old);
        let keys: usize = (0..len)
            .filter(|&i| i != at)
            .map(|i| bucket_key(old, i).len())
            .sum();
        let mut repl = BucketBuilder::new(len - 1, keys);
        for i in (0..len).filter(|&i| i != at) {
            repl.push(bucket_key(old, i), *bucket_value_ptr(old, i));
        }
        repl.finish()
    }
}

/// A one-entry bucket: `key → val`.
#[inline(always)]
pub(crate) fn bucket_single(key: &[u8], val: u64) -> *mut Bucket {
    let mut b = BucketBuilder::new(1, key.len());
    b.push(key, val);
    b.finish()
}

/// Frees a bucket that was never published: no reader can hold it.
///
/// # Safety
///
/// `b` was built by [`BucketBuilder`], never published (or unlinked with
/// no concurrent reader possible), and is not used again.
pub(crate) unsafe fn free_bucket(b: *mut Bucket) {
    // SAFETY: caller contract; the layout is the one it was allocated with.
    unsafe { core_alloc::alloc::dealloc(b.cast(), bucket_layout(bucket_bytes(b))) };
}

/// The atomic view of one published entry's value word.
///
/// [`Entry`] declares the value as a plain `u64` because the
/// single-threaded engine owns it outright and the `JudyHS` slot
/// contract hands it out as `*mut u64`
/// ([`ExpanseBytesMap::ins_slot`]). The concurrent paths reach the same
/// word through [`core::sync::atomic::AtomicU64::from_ptr`], which is
/// exactly the "atomic access to a location declared non-atomically"
/// case that constructor exists for: the value field of the `#[repr(C)]`
/// [`Entry`] is 8-aligned (pinned by a `const` assertion above), which is
/// `AtomicU64`'s alignment on every 64-bit target this is compiled for.
///
/// # Safety
///
/// `entries` must be the entry array of an EBR-live bucket with more
/// than `at` entries, and every access to that word that is **not**
/// through this view must be synchronized against the ones that are —
/// which for the concurrent wrapper means it happens only with the
/// optimistic writers quiesced. The returned reference must not outlive
/// the caller's epoch pin.
#[cfg(all(target_pointer_width = "64", feature = "std"))]
#[inline(always)]
pub(crate) unsafe fn entry_value_atomic<'a>(
    entries: *const Entry,
    at: usize,
) -> &'a core::sync::atomic::AtomicU64 {
    // SAFETY: `at` is in bounds of the caller's EBR-live entry array, so
    // the field projection names a live, 8-aligned `u64`.
    let word = unsafe { &raw const (*entries.add(at)).val };
    // SAFETY: forwarded contract — the word is valid for reads and
    // writes for the pin's duration, 8-aligned, and every non-atomic
    // access to it is synchronized against this one.
    unsafe { core::sync::atomic::AtomicU64::from_ptr(word.cast_mut()) }
}

/// Locates `key` in the published bucket at `word`, returning the
/// bucket's length and the matching entry index.
///
/// Reads only the write-once key fields, through raw pointers: it never
/// forms a reference covering the value word, which a concurrent writer
/// may be storing into through [`entry_value_atomic`].
///
/// # Safety
///
/// `word` must be a bucket pointer the caller validated under its
/// epoch pin (see [`ExpanseBytesMap::get_validated`] for the contract).
#[cfg(all(
    target_pointer_width = "64",
    feature = "std",
    not(feature = "ablation-bytes-serial-writers")
))]
#[inline]
pub(crate) unsafe fn bucket_find(word: u64, key: &[u8]) -> (usize, Option<usize>) {
    let bucket = word as *const Bucket;
    // SAFETY: EBR-live published bucket; its header and key fields are
    // write-once after publication.
    // `bucket_position` reads the key fields only, never a value word.
    unsafe { (bucket_len(bucket), bucket_position(bucket, key)) }
}

/// Builds the replacement bucket a colliding insert publishes: a
/// positional copy of the `len` entries of the published bucket at
/// `word`, with `key → val` appended.
///
/// Key bytes are copied (the old bucket keeps its own, and is disposed of
/// with them). Value words are read atomically, and re-read under the
/// terminal's version lock by [`refresh_replacement_values`] before the
/// replacement is published.
///
/// # Safety
///
/// As [`bucket_find`], with `len` the length it returned.
#[cfg(all(
    target_pointer_width = "64",
    feature = "std",
    not(feature = "ablation-bytes-serial-writers")
))]
pub(crate) unsafe fn clone_bucket_with(word: u64, len: usize, key: &[u8], val: u64) -> *mut Bucket {
    let bucket = word as *const Bucket;
    // SAFETY: EBR-live published bucket of `len` entries; key fields are
    // write-once and value words are loaded through their atomic view.
    unsafe {
        let entries = bucket_entries(bucket);
        let keys: usize = (0..len).map(|i| bucket_key(bucket, i).len()).sum();
        let mut fresh = BucketBuilder::new(len + 1, keys + key.len());
        for i in 0..len {
            let v = entry_value_atomic(entries, i).load(core::sync::atomic::Ordering::Relaxed);
            fresh.push(bucket_key(bucket, i), v);
        }
        fresh.push(key, val);
        fresh.finish()
    }
}

/// Publishes `val` as entry `at`'s value inside the already-published
/// bucket at `word`, returning the value it replaced — the whole of an
/// overwrite's write set (#929). No allocation, no bucket replacement,
/// no epoch retirement.
///
/// The load and the store are separate because the caller holds the
/// terminal's parent version lock, which every other in-place publish
/// and every bucket replacement for this hash must also hold: no other
/// writer can interleave, so a read-modify-write instruction would buy
/// nothing.
///
/// # Safety
///
/// The caller must hold the terminal's parent version lock **and** have
/// confirmed under that lock that `word` is still the published bucket
/// word for this hash (see `sync::olc_bucket_value_inplace_map`), and
/// `at` must index an entry of that bucket.
#[cfg(all(
    target_pointer_width = "64",
    feature = "std",
    not(feature = "ablation-bytes-serial-writers")
))]
#[inline]
pub(crate) unsafe fn publish_entry_value(word: u64, at: usize, val: u64) -> u64 {
    use core::sync::atomic::Ordering;
    let bucket = word as *const Bucket;
    // SAFETY: `word` is the published bucket for this hash, so it is
    // EBR-live and holds more than `at` entries.
    let entries = unsafe { bucket_entries(bucket) };
    // SAFETY: forwarded contract. `Relaxed` is enough on both: the
    // version unlock that follows is the release a reader's validation
    // acquires, and a reader that misses it returns the value the key
    // held before this store.
    let slot = unsafe { entry_value_atomic(entries, at) };
    let prev = slot.load(Ordering::Relaxed);
    slot.store(val, Ordering::Relaxed);
    prev
}

/// Reads entry `at`'s value word from the bucket at `word`.
///
/// The read half of [`publish_entry_value`], for a caller that has just
/// **unlinked** this bucket: once the trie word no longer points at it,
/// no writer can reach it, so the word this returns is the last value
/// the key held (Refs #1047). Pinned readers may still be loading it,
/// which is why the load goes through the atomic view.
///
/// # Safety
///
/// `word` must be a bucket the caller unlinked under the terminal's
/// parent version lock, with more than `at` entries.
#[cfg(all(
    target_pointer_width = "64",
    feature = "std",
    not(feature = "ablation-bytes-serial-writers")
))]
#[inline]
pub(crate) unsafe fn read_entry_value(word: u64, at: usize) -> u64 {
    let bucket = word as *const Bucket;
    // SAFETY: forwarded contract — an EBR-live bucket with more than
    // `at` entries.
    let entries = unsafe { bucket_entries(bucket) };
    // SAFETY: forwarded contract. `Relaxed` is enough: the unlinking
    // caller has already excluded every writer, and a validating reader
    // acquires the version word.
    unsafe { entry_value_atomic(entries, at) }.load(core::sync::atomic::Ordering::Relaxed)
}

/// Builds the replacement bucket a colliding **remove** publishes: a
/// copy of the `len` entries of the published bucket at `word` with
/// entry `at` left out (Refs #1047).
///
/// The mirror of [`clone_bucket_with`], with the same ownership split:
/// key bytes are copied, so the old bucket keeps its own and is disposed
/// of with them. Value words are read atomically here and re-read under the
/// terminal's version lock by [`refresh_replacement_values_removing`]
/// before the replacement is published.
///
/// # Safety
///
/// As [`bucket_find`], with `len` the length it returned, and `at < len`.
#[cfg(all(
    target_pointer_width = "64",
    feature = "std",
    not(feature = "ablation-bytes-serial-writers")
))]
pub(crate) unsafe fn clone_bucket_without(word: u64, len: usize, at: usize) -> *mut Bucket {
    debug_assert!(at < len, "entry index inside the bucket");
    let bucket = word as *const Bucket;
    // SAFETY: EBR-live published bucket of `len` entries; key fields are
    // write-once and value words are loaded through their atomic view.
    unsafe {
        let entries = bucket_entries(bucket);
        let keys: usize = (0..len)
            .filter(|&i| i != at)
            .map(|i| bucket_key(bucket, i).len())
            .sum();
        let mut fresh = BucketBuilder::new(len - 1, keys);
        for i in (0..len).filter(|&i| i != at) {
            let v = entry_value_atomic(entries, i).load(core::sync::atomic::Ordering::Relaxed);
            fresh.push(bucket_key(bucket, i), v);
        }
        fresh.finish()
    }
}

/// [`refresh_replacement_values`] for a replacement built by
/// [`clone_bucket_without`]: the entries of `repl` are those of the
/// published bucket at `word` with index `at` left out, so entry `i` of
/// `repl` is entry `i` of the published bucket while `i < at`, and entry
/// `i + 1` after it (Refs #1047).
///
/// Refreshing under the terminal's version lock is what makes this
/// mutually exclusive with an in-place value publish, exactly as in
/// [`refresh_replacement_values`]: without it, removing one colliding key
/// would silently drop an acknowledged overwrite of another.
///
/// # Safety
///
/// As [`refresh_replacement_values`], with `len` the published bucket's
/// length, `at < len`, and `repl` holding `len - 1` entries in the order
/// above.
#[cfg(all(
    target_pointer_width = "64",
    feature = "std",
    not(feature = "ablation-bytes-serial-writers")
))]
#[inline]
pub(crate) unsafe fn refresh_replacement_values_removing(
    word: u64,
    repl: *mut Bucket,
    len: usize,
    at: usize,
) {
    debug_assert!(at < len, "entry index inside the bucket");
    let entries = word as *const Bucket;
    // SAFETY: `word` is the published bucket for this hash under the
    // caller's lock; `repl` is the caller's own unpublished bucket.
    let (base, dst) = unsafe { (bucket_entries(entries), bucket_entries(repl)) };
    for i in 0..len {
        if i == at {
            continue;
        }
        let j = if i < at { i } else { i - 1 };
        // SAFETY: `i < len` indexes the published bucket and `j < len - 1`
        // the caller's own, by the contract above.
        let v = unsafe { entry_value_atomic(base, i) }.load(core::sync::atomic::Ordering::Relaxed);
        // SAFETY: `dst.add(j)` is an entry of the caller's own bucket,
        // which no other thread can reach until it is published.
        unsafe { (*dst.add(j)).val = v };
    }
}

/// Re-reads the first `n` value words of the published bucket at `word`
/// into the still-unpublished replacement `repl`, whose first `n`
/// entries are a positional copy of it.
///
/// A replacement bucket is built outside the terminal's version lock, so
/// an in-place value publish can land between the copy and the
/// publishing compare-and-swap. That publish holds the same lock the
/// caller holds here, so refreshing under the lock is what makes the two
/// writers mutually exclusive: without it the replacement would carry a
/// stale value word and silently drop an acknowledged overwrite.
///
/// # Safety
///
/// As [`publish_entry_value`], plus: `repl` is owned by the caller and
/// unpublished, and its first `n` entries hold the same keys, in the
/// same order, as the bucket at `word`.
#[cfg(all(
    target_pointer_width = "64",
    feature = "std",
    not(feature = "ablation-bytes-serial-writers")
))]
#[inline]
pub(crate) unsafe fn refresh_replacement_values(word: u64, repl: *mut Bucket, n: usize) {
    let entries = word as *const Bucket;
    // SAFETY: `word` is the published bucket for this hash under the
    // caller's lock; `repl` is the caller's own unpublished bucket.
    let (base, dst) = unsafe { (bucket_entries(entries), bucket_entries(repl)) };
    for i in 0..n {
        // SAFETY: `i < n` indexes both arrays by the caller's contract.
        let v = unsafe { entry_value_atomic(base, i) }.load(core::sync::atomic::Ordering::Relaxed);
        // SAFETY: `dst.add(i)` is an entry of the caller's own bucket,
        // which no other thread can reach until it is published.
        unsafe { (*dst.add(i)).val = v };
    }
}

/// Heap bytes of one entry beyond its key bytes: its record in the
/// bucket's entry array.
const ENTRY_OVERHEAD: usize = size_of::<Entry>();

/// Heap bytes of one bucket beyond its entries: its header.
const BUCKET_OVERHEAD: usize = size_of::<Bucket>();

/// Publishes a bucket pointer into its trie value slot. The shared instance
/// stores it as an atomic word, the size and kind readers load it as
/// (#1086); a reader dereferences the bucket only after the walk validated
/// the word, so the bucket's initialising stores happen before its reads
/// through the version word. The plain instance is the plain store it
/// always was.
///
/// # Safety
///
/// `slot` is the live value slot `ins_slot` handed out, valid until the next
/// structural mutation.
#[inline(always)]
unsafe fn publish_bucket_word<const SHARED: bool>(slot: NonNull<u64>, word: u64) {
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    if SHARED {
        // SAFETY: caller contract; an 8-aligned value word.
        unsafe {
            core::sync::atomic::AtomicU64::from_ptr(slot.as_ptr())
                .store(word, core::sync::atomic::Ordering::Relaxed);
        }
        return;
    }
    // SAFETY: caller contract.
    unsafe { *slot.as_ptr() = word };
}

/// Disposes an unlinked bucket: its header, entries and key bytes are one
/// allocation, freed at once when the map is not shared and retired as one
/// block when it is — a concurrent reader that validated the bucket at an
/// earlier snapshot may still be comparing its keys under its pin.
///
/// Retired layout: `(bucket bytes, 8)` — align 8 is neither `RAW_ALIGN`
/// nor `CACHE_LINE`, so it always misses the size classes and takes the
/// collector's exact-layout `free_raw` path, the layout it was allocated
/// with.
#[cfg(feature = "std")]
pub(crate) fn dispose_bucket(ptr: *mut Bucket, defer: Option<&Arc<Collector>>) {
    match defer {
        // SAFETY: caller unlinked `ptr`; this is the last reference, and no
        // reader can hold a map that is not shared.
        None => unsafe { free_bucket(ptr) },
        Some(c) => {
            // SAFETY: caller unlinked `ptr`, a bucket of this many bytes,
            // which it owns alone from here; the header is write-once.
            let bytes = unsafe { bucket_bytes(ptr) };
            // SAFETY: `ptr` carries its allocation's provenance (published
            // and read back as an exposed address), the layout is the one it
            // was allocated with, and it is retired once and never used again.
            unsafe {
                c.retire(
                    NonNull::new(ptr.cast::<u8>()).expect("non-null bucket"),
                    bytes,
                    BUCKET_ALIGN,
                );
            }
        }
    }
}

#[cfg(not(feature = "std"))]
pub(crate) fn dispose_bucket(ptr: *mut Bucket, _defer: Option<&()>) {
    // SAFETY: caller unlinked `ptr`; this is the last reference.
    unsafe { free_bucket(ptr) };
}

/// A sparse, dynamic, **unordered** map from byte strings to `u64`
/// values (compat: JudyHS).
///
/// Under `feature = "std"`, hashing uses [`RandomState`] (per-map seeding, hash-flood resistant).
/// In `no_std`, hashing defaults to deterministic 64-bit FNV-1a.
/// Use [`ExpanseBytesMap::with_hasher`] to pin a custom hasher.
pub struct ExpanseBytesMap<S: BuildHasher = DefaultBuildHasher> {
    /// hash → bucket pointer (one allocation, see [`Bucket`]), stored as
    /// the map value word.
    map: ExpanseMap,
    hasher: S,
    len: u64,
    /// Bucket heap bytes: the requested size of every bucket allocation
    /// (allocator rounding is not counted; the trie's own bytes are exact
    /// via [`ExpanseMap::mem_used`]).
    extra_bytes: usize,
    /// Phase 7 (issue #219): when set, unlinked buckets (key bytes
    /// included) are retired through the collector instead of freed (concurrent
    /// readers may still hold pointers into them), and the hash trie's
    /// `NodeAlloc` is deferred so its frees and mutation brackets
    /// participate too.
    #[cfg(feature = "std")]
    deferred: OnceLock<Arc<Collector>>,
}

/// Phase 7 (issue #219): one bounded, validated optimistic lookup —
/// the concurrent analogue of [`ExpanseBytesMap::get`]. One
/// validated `sync::walk_validated` over the hash trie, then a
/// byte-exact comparison against the collision bucket.
///
/// The bucket word yielded by the walk is validated at `snap`, and a
/// published bucket is write-once except its value words (structural
/// changes publish a replacement and retire the old bucket — see
/// [`Bucket`]), so the shell, entry array, and key bytes read here
/// are exactly the published state.
///
/// # What a reader can observe (#929)
///
/// The value word is the one field a concurrent writer may store
/// into, so it is loaded through [`entry_value_atomic`] and never as
/// a plain `u64`. Three races are possible and each yields a value
/// the key actually held:
///
/// - **An in-place value publish on the bucket this reader is
///   scanning.** The load returns the value before or after that
///   store; both are values the key held, and the read linearizes
///   at the load. That publish leaves every version alone — it
///   unlocks the terminal's parent clean — deliberately: forcing the
///   reader to retry would only trade a correct answer for an
///   equally correct fresher one.
/// - **A bucket replacement of the bucket this reader is
///   scanning.** The replacement stores the trie word under the
///   terminal's parent version lock and unlocks it dirty. A reader
///   still inside the walk fails that node's validation and
///   retries; a reader that had already taken the bucket word keeps
///   reading the retired bucket under its pin, whose value words
///   are **frozen** — a writer stores only after its locked compare
///   has seen its own bucket word still published, and no writer
///   can see it again once the trie entry has moved on. So it
///   returns the value the key held when it took the word, and
///   linearizes there. (The OLC write path moves node versions, not
///   the tree word, so `ver.validate` below is not what covers
///   this; it covers the serialised paths — root growth, `remove`,
///   `clear` — which bracket the tree word.)
/// - **Both at once.** The replacement re-reads the value words
///   under the same version lock the in-place publish takes
///   ([`refresh_replacement_values`]), so the two cannot interleave
///   and no acknowledged overwrite is dropped.
///
/// `root` is the hash trie's root state and `hasher` hashes as the map's
/// hasher does. Neither is read from the map here: the concurrent wrapper
/// passes its published root and its own copy of the hasher, so a reader
/// forms no reference to a map a covered writer may hold `&mut` to (#1086).
/// Inlined, hash included, into the reader's retry loop: the hash is only
/// specialised to the call when it is inlined with it.
///
/// # Safety
///
/// Same contract as `sync::walk_validated`: `snap` must be an even
/// version sampled from `ver` after the map switched to deferred
/// reclamation ([`ExpanseBytesMap::defer_to`]), `root` must have been
/// loaded after `snap` was sampled, and the caller must hold an epoch pin
/// for the whole call — every pointer read under a still-valid cover then
/// references EBR-live memory.
#[cfg(all(target_pointer_width = "64", feature = "std"))]
#[inline(always)]
pub(crate) unsafe fn get_validated<S: BuildHasher>(
    root: crate::sync::RootSnapshot,
    hasher: &S,
    key: &[u8],
    ver: &crate::occ::SeqVersion,
    snap: u64,
) -> Result<Option<u64>, crate::sync::Retry> {
    use crate::sync::Retry;
    let h = hash_key(hasher, key);
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
    // published bucket then, and EBR keeps its one allocation (header,
    // entries and key bytes) mapped under the caller's pin. Everything
    // but the value words is write-once after publication.
    // The search reads the key fields only, never a value word.
    let result = unsafe { bucket_search(bucket, key) }.map(|e| {
        // SAFETY: `e` is an entry of the EBR-live bucket under the caller's
        // pin. The atomic load is what makes a concurrent in-place publish
        // well-defined rather than a data race; see this function's docs
        // for what it returns.
        unsafe { entry_value_atomic(e, 0) }.load(core::sync::atomic::Ordering::Relaxed)
    });
    if !ver.validate(snap) {
        return Err(Retry);
    }
    Ok(result)
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

    /// Binds the wrapper's tree-level version word to the hash trie's
    /// allocator (#568 PR 3; see `NodeAlloc::bind_tree_word`).
    ///
    /// # Safety
    ///
    /// As `NodeAlloc::bind_tree_word`: `word` outlives every operation on
    /// this map.
    #[cfg(feature = "std")]
    pub(crate) unsafe fn bind_tree_word(&self, word: *const crate::occ::SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { self.map.occ_root().1.bind_tree_word(word) };
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

    #[inline(always)]
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-bytes-serial-writers")
    ))]
    pub(crate) fn set_len(&mut self, len: u64) {
        self.len = len;
    }

    /// Number of buckets in the underlying trie.
    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn bucket_count(&self) -> u64 {
        self.map.len()
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn set_bucket_pop(&mut self, pop: u64) {
        self.map.set_tree_pop(pop);
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Heap bytes used: exact for the hash trie, and the requested
    /// allocation size of every collision bucket (header, entries and key
    /// bytes; allocator rounding is not counted).
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.map.mem_used() + self.extra_bytes
    }

    /// Gathers structural statistics of the bytes map.
    ///
    /// Returns hash-trie node counts, depth and leaf-population histograms, bucket
    /// counts, entry counts, and a [`BytesMapNodeBytes`] breakdown whose fields
    /// sum exactly to [`Self::mem_used`].
    #[must_use]
    pub fn stats(&self) -> BytesMapStats {
        let mut stats = BytesMapStats::default();
        let sub_stats = self.map.stats();
        stats.node_bytes.sub_maps = sub_stats.node_bytes;
        stats.node_counts = sub_stats.node_counts;
        stats.depth_histogram = sub_stats.depth_histogram;
        stats.leaf_pop_histogram = sub_stats.leaf_pop_histogram;
        stats.branch_depth_histogram = sub_stats.branch_depth_histogram;
        stats.leaf_depth_histogram = sub_stats.leaf_depth_histogram;

        stats.buckets = self.map.len() as usize;
        stats.entries = self.len;

        let bucket_shells = stats.buckets * BUCKET_OVERHEAD;
        stats.node_bytes.node_shells = bucket_shells;
        stats.node_bytes.terminal_bytes = self.extra_bytes.saturating_sub(bucket_shells);

        #[cfg(debug_assertions)]
        {
            let mut counted_entries = 0u64;
            let mut counted_terminals = 0usize;
            for (_, word) in self.map.iter() {
                // SAFETY: every trie value is a live bucket owned by this map.
                let bucket = word as *const Bucket;
                // SAFETY: as above.
                for i in 0..unsafe { bucket_len(bucket) } {
                    counted_entries += 1;
                    // SAFETY: `i < len` of a live bucket.
                    counted_terminals += ENTRY_OVERHEAD + unsafe { bucket_key(bucket, i) }.len();
                }
            }
            debug_assert_eq!(stats.entries, counted_entries);
            debug_assert_eq!(stats.node_bytes.terminal_bytes, counted_terminals);
        }

        stats
    }

    #[inline(always)]
    #[cfg(feature = "std")]
    pub(crate) fn root_is_tree(&self) -> bool {
        self.map.root_is_tree()
    }

    #[inline(always)]
    #[allow(dead_code)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn alloc(&self) -> &crate::alloc::NodeAlloc {
        self.map.alloc()
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn clear_path(&self) {
        self.map.clear_path();
    }

    #[inline(always)]
    #[cfg(feature = "std")]
    pub(crate) unsafe fn root_top_ptr(&self) -> *mut crate::node::Edge {
        // SAFETY: forwarded contract.
        unsafe { self.map.root_top_ptr() }
    }

    #[allow(dead_code)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    pub(crate) fn occ_root(&self) -> (crate::sync::RootSnapshot, &crate::alloc::NodeAlloc) {
        self.map.occ_root()
    }

    /// The hash trie's allocator, reached through a raw pointer to the map
    /// without a reference to the whole map (#1086): the concurrent
    /// wrapper's optimistic writers use it while its covered writer may
    /// later take `&mut` to the map, and a shared reference to the map
    /// would cover the hasher and the counters that writer stores to.
    ///
    /// # Safety
    ///
    /// As [`ExpanseMap::alloc_of`]: `this` points to a live map for `'a`,
    /// and no `&mut` to it exists meanwhile.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[cfg_attr(feature = "ablation-bytes-serial-writers", allow(dead_code))]
    #[inline(always)]
    pub(crate) unsafe fn alloc_of<'a>(this: *const Self) -> &'a crate::alloc::NodeAlloc {
        // SAFETY: caller contract; the field is projected through the raw
        // pointer, so no reference to the whole map is formed.
        unsafe { ExpanseMap::alloc_of(core::ptr::addr_of!((*this).map)) }
    }

    fn bucket_of(&self, key: &[u8]) -> Option<NonNull<Bucket>> {
        let h = hash_key(&self.hasher, key);
        self.map
            .get(h)
            .and_then(|word| NonNull::new(word as *mut Bucket))
    }

    /// Returns the value stored for `key`.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        let bucket = self.bucket_of(key)?.as_ptr();
        // An index loop rather than `bucket_search`: on the x86 Callgrind
        // `bytesmap_get` arms it costs one instruction less per probe than
        // the pointer walk, whose block layout adds a jump (#1760).
        // SAFETY: bucket pointers stored in the hash trie are live buckets
        // owned by this map; `i < len` indexes one of its entries.
        unsafe {
            let (entries, len) = (bucket_entries(bucket), bucket_len(bucket));
            let at = (0..len).find(|&i| {
                let e = entries.add(i);
                core::slice::from_raw_parts((*e).key, (*e).key_len) == key
            })?;
            Some((*entries.add(at)).val)
        }
    }

    /// Membership test.
    #[must_use]
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None`
    /// if absent. Valid until the next structural mutation.
    ///
    /// Takes `&self` to allow read-only lookups without requiring a mutable borrow.
    #[must_use]
    pub fn get_slot_ptr(&self, key: &[u8]) -> Option<NonNull<u64>> {
        let b_ptr = self.bucket_of(key)?.as_ptr();
        // An index loop, as in `get`: the pointer walk forms its end pointer
        // with two more instructions per hit, which the x86 Callgrind
        // `judyhs_long_get` arms counted (#1760).
        // SAFETY: `b_ptr` is a live bucket owned by this map, and the key
        // search borrows key bytes only, never the value word the returned
        // pointer writes; `i < len` indexes one of its entries. The value
        // pointer is a raw place projection with the allocation's write
        // provenance and no intermediate reference.
        unsafe {
            let (entries, len) = (bucket_entries(b_ptr), bucket_len(b_ptr));
            let at = (0..len).find(|&i| {
                let e = entries.add(i);
                core::slice::from_raw_parts((*e).key, (*e).key_len) == key
            })?;
            Some(NonNull::new_unchecked(&raw mut (*entries.add(at)).val))
        }
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None`
    /// if absent — the compat `JudyHSGet` convention. Valid until the
    /// next structural mutation.
    #[must_use]
    pub fn get_value_slot(&mut self, key: &[u8]) -> Option<NonNull<u64>> {
        self.get_slot_ptr(key)
    }

    fn insert_slot_inner<const SHARED: bool>(
        &mut self,
        key: &[u8],
        init_val: u64,
    ) -> (NonNull<u64>, Option<u64>) {
        let defer = self.defer_handle();
        let h = hash_key(&self.hasher, key);
        // A shared map's root leaf is loaded by readers while this stores
        // to it, so the shared instance takes the engine's shared entry
        // (#1086); the plain instance compiles as before.
        #[cfg(all(target_pointer_width = "64", feature = "std"))]
        let slot = if SHARED {
            self.map.ins_slot_shared(h)
        } else {
            self.map.ins_slot(h)
        };
        #[cfg(not(all(target_pointer_width = "64", feature = "std")))]
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
            let raw = bucket_single(key, init_val);
            // SAFETY: the slot stays valid until the next structural
            // trie mutation; none happens between `ins_slot` and here.
            unsafe { publish_bucket_word::<SHARED>(slot, raw as u64) };
            self.len += 1;
            self.extra_bytes += BUCKET_OVERHEAD + key.len() + ENTRY_OVERHEAD;
            // SAFETY: freshly built above; entry 0 exists, and a pointer
            // into a live allocation is non-null.
            return (
                unsafe { NonNull::new_unchecked(bucket_value_ptr(raw, 0)) },
                None,
            );
        }
        let old = word as *mut Bucket;
        // SAFETY: live bucket owned by this map; read-only search.
        if let Some(e) = unsafe { bucket_search_from_base(old, key) } {
            // Existing key: only its value word will mutate, in place —
            // the one mutation published buckets allow.
            // SAFETY: as above; the slot pointer stays valid until the
            // next structural mutation.
            let val = unsafe { &raw mut (*e).val };
            // SAFETY: a live value word; only this `&mut self` call writes it.
            let old_val = unsafe { *val };
            // SAFETY: a pointer into a live allocation is non-null.
            return (unsafe { NonNull::new_unchecked(val) }, Some(old_val));
        }
        // 64-bit hash collision: publish a replacement bucket holding
        // copies of the old entries plus the new key, then dispose of the
        // old bucket, keys and all.
        // SAFETY: live bucket owned by this map. Its keys and value words
        // are read here only; this `&mut self` call is the one writer, and
        // concurrent readers only load them.
        let (raw, old_len) = unsafe { (bucket_appending(old, key, init_val), bucket_len(old)) };
        // Publish the replacement — the single word store that unlinks
        // the old bucket — then retire the old allocation.
        // SAFETY: slot valid as above.
        unsafe { publish_bucket_word::<SHARED>(slot, raw as u64) };
        dispose_bucket(old, defer.as_ref());
        self.len += 1;
        self.extra_bytes += key.len() + ENTRY_OVERHEAD;
        // SAFETY: freshly built above; the appended entry exists, and a
        // pointer into a live allocation is non-null.
        (
            unsafe { NonNull::new_unchecked(bucket_value_ptr(raw, old_len)) },
            None,
        )
    }

    /// Inserts `key` with value 0 if absent — an existing value is kept
    /// untouched — and returns a **writable pointer to its value slot**:
    /// the compat `JudyHSIns` contract. Valid until the next structural
    /// mutation.
    pub fn ins_slot(&mut self, key: &[u8]) -> NonNull<u64> {
        self.insert_slot_inner::<false>(key, 0).0
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        let (slot, prev) = self.insert_slot_inner::<false>(key, val);
        if prev.is_some() {
            // The one in-place value mutation of a published bucket on
            // this path. It is a `&mut self` method, so no other writer
            // is running; the concurrent wrapper's optimistic *readers*
            // are not excluded by that, and they load this word through
            // [`entry_value_atomic`], so the store goes through the same
            // view (#929). Relaxed, and the same instruction as a plain
            // store; the wrapper's version bracket is the release.
            #[cfg(all(target_pointer_width = "64", feature = "std"))]
            // SAFETY: the slot is the live value word of an entry in an
            // existing bucket, 8-aligned, and valid until the next
            // structural mutation, which this call does not perform.
            unsafe {
                core::sync::atomic::AtomicU64::from_ptr(slot.as_ptr())
                    .store(val, core::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(not(all(target_pointer_width = "64", feature = "std")))]
            // SAFETY: slot points to live entry within the existing bucket.
            unsafe {
                *slot.as_ptr() = val
            };
        }
        prev
    }

    /// [`Self::insert`] for the concurrent wrapper (#1086); see
    /// [`Self::remove_shared`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn insert_shared(&mut self, key: &[u8], val: u64) -> Option<u64> {
        let (slot, prev) = self.insert_slot_inner::<true>(key, val);
        if prev.is_some() {
            // The one in-place value mutation of a published bucket on
            // this path. It is a `&mut self` method, so no other writer
            // is running; the concurrent wrapper's optimistic *readers*
            // are not excluded by that, and they load this word through
            // [`entry_value_atomic`], so the store goes through the same
            // view (#929). Relaxed, and the same instruction as a plain
            // store; the wrapper's version bracket is the release.
            // SAFETY: the slot is the live value word of an entry in an
            // existing bucket, 8-aligned, and valid until the next
            // structural mutation, which this call does not perform.
            unsafe {
                core::sync::atomic::AtomicU64::from_ptr(slot.as_ptr())
                    .store(val, core::sync::atomic::Ordering::Relaxed);
            }
        }
        prev
    }

    /// Removes `key`; returns its value if it was present.
    pub fn remove(&mut self, key: &[u8]) -> Option<u64> {
        self.remove_impl::<false>(key)
    }

    /// Replaces the value for `key` with `f(current)`: `Some(v)` stores `v`,
    /// `None` removes the key. Returns the value that was replaced (or `None`
    /// if the key was absent).
    ///
    /// Performs two tree descents: one [`Self::get`] to read the current value,
    /// followed by [`Self::insert`] or [`Self::remove`] depending on the closure's
    /// result.
    pub fn update(
        &mut self,
        key: &[u8],
        f: impl FnOnce(Option<u64>) -> Option<u64>,
    ) -> Option<u64> {
        let old = self.get(key);
        match f(old) {
            Some(v) => {
                self.insert(key, v);
                old
            }
            None => {
                if old.is_some() {
                    self.remove(key);
                }
                old
            }
        }
    }

    /// [`Self::remove`] for the concurrent wrapper (#1086): the engine's
    /// shared entries, whose root-leaf stores are atomic words.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn remove_shared(&mut self, key: &[u8]) -> Option<u64> {
        self.remove_impl::<true>(key)
    }

    fn remove_impl<const SHARED: bool>(&mut self, key: &[u8]) -> Option<u64> {
        let defer = self.defer_handle();
        let h = hash_key(&self.hasher, key);
        let word = self.map.get(h)?;
        let old = word as *mut Bucket;
        // SAFETY: live bucket owned by this map; read-only search, and the
        // value word is read by its one writer, this `&mut self` call.
        let (at, val, key_len, old_len) = unsafe {
            let at = bucket_position(old, key)?;
            (
                at,
                *bucket_value_ptr(old, at),
                bucket_key(old, at).len(),
                bucket_len(old),
            )
        };
        if old_len == 1 {
            // Unlink the bucket from the trie first, then dispose of it
            // together with its one remaining key.
            #[cfg(all(target_pointer_width = "64", feature = "std"))]
            if SHARED {
                self.map.remove_shared(h);
            } else {
                self.map.remove(h);
            }
            #[cfg(not(all(target_pointer_width = "64", feature = "std")))]
            self.map.remove(h);
            dispose_bucket(old, defer.as_ref());
            self.extra_bytes = self.extra_bytes.saturating_sub(BUCKET_OVERHEAD);
        } else {
            // Collision bucket: publish a replacement holding copies of
            // the surviving entries, then dispose of the old bucket, the
            // removed key included.
            // SAFETY: as above; `i < old_len` indexes the live bucket.
            let raw = unsafe { bucket_removing(old, at) };
            // Publish the replacement through the engine (bracketed when
            // shared) — this unlinks the old bucket — then retire it.
            #[cfg(all(target_pointer_width = "64", feature = "std"))]
            let prev = if SHARED {
                self.map.insert_shared(h, raw as u64)
            } else {
                self.map.insert(h, raw as u64)
            };
            #[cfg(not(all(target_pointer_width = "64", feature = "std")))]
            let prev = self.map.insert(h, raw as u64);
            debug_assert_eq!(prev, Some(word), "bucket word moved mid-remove");
            dispose_bucket(old, defer.as_ref());
        }
        self.len = self.len.saturating_sub(1);
        self.extra_bytes = self.extra_bytes.saturating_sub(key_len + ENTRY_OVERHEAD);
        Some(val)
    }

    /// Visits every entry in unspecified order.
    pub fn for_each(&self, mut f: impl FnMut(&[u8], u64)) {
        self.try_for_each(|k, v| {
            f(k, v);
            true
        });
    }

    /// Visits entries in unspecified order until `f` returns `false`.
    /// Returns the number of entries visited.
    pub fn try_for_each(&self, mut f: impl FnMut(&[u8], u64) -> bool) -> usize {
        let mut count = 0;
        for (_, word) in self.map.iter() {
            // SAFETY: every trie value is a live bucket owned by this map.
            let bucket = word as *const Bucket;
            // SAFETY: as above; `i < len` indexes the live bucket.
            for i in 0..unsafe { bucket_len(bucket) } {
                count += 1;
                // SAFETY: as above.
                let (k, v) = unsafe { (bucket_key(bucket, i), *bucket_value_ptr(bucket, i)) };
                if !f(k, v) {
                    return count;
                }
            }
        }
        count
    }

    /// Removes every key and releases all memory.
    pub fn clear(&mut self) {
        self.clear_entries();
        self.map.shrink_to_fit();
    }

    /// [`Self::clear`] without returning the trie allocator's freed blocks,
    /// for `Drop`, where the trie's own `Drop` frees them.
    fn clear_entries(&mut self) {
        let defer = self.defer_handle();
        let buckets: Vec<u64> = self.map.iter().map(|(_, word)| word).collect();
        // Unlink everything from the trie first (its nodes free/retire
        // through its own `NodeAlloc`), then dispose of the buckets.
        self.map.clear_entries();
        for word in buckets {
            // Each collected word is a live bucket, unlinked above and
            // disposed of (with its keys) exactly once here.
            dispose_bucket(word as *mut Bucket, defer.as_ref());
        }
        self.len = 0;
        self.extra_bytes = 0;
    }
}

impl<S: BuildHasher> Drop for ExpanseBytesMap<S> {
    fn drop(&mut self) {
        self.clear_entries();
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

    /// The one-allocation bucket (#1760): every key of a colliding bucket
    /// is found byte-exactly whatever its length (empty, shorter and longer
    /// than a word), each value slot is a distinct 8-aligned word inside the
    /// bucket, the accounting is the bucket's size (header, entries, key
    /// bytes), and removing from the middle keeps every other entry and its
    /// value.
    #[test]
    fn collision_bucket_layout_round_trips() {
        let keys: [&[u8]; 7] = [
            b"",
            b"a",
            b"abcdefg",
            b"abcdefgh",
            b"abcdefghi",
            b"/api/v2/tenants/000000/resources/0000",
            &[0u8; 100],
        ];
        let mut m = ExpanseBytesMap::with_hasher(Degenerate);
        for (i, k) in keys.iter().enumerate() {
            assert_eq!(m.insert(k, i as u64 + 1), None, "fresh {k:02x?}");
        }
        // A duplicate keeps one entry and replaces only the value.
        assert_eq!(m.insert(keys[4], 50), Some(5));
        assert_eq!(m.insert(keys[4], 5), Some(50));
        assert_eq!(m.len(), keys.len() as u64);

        let key_bytes: usize = keys.iter().map(|k| k.len()).sum();
        let s = m.stats();
        assert_eq!(s.buckets, 1);
        assert_eq!(s.node_bytes.node_shells, size_of::<Bucket>());
        assert_eq!(
            s.node_bytes.terminal_bytes,
            keys.len() * size_of::<Entry>() + key_bytes
        );
        assert_eq!(
            m.mem_used() - m.map.mem_used(),
            bucket_alloc_size(keys.len(), key_bytes)
        );

        let mut slots = Vec::new();
        for (i, k) in keys.iter().enumerate() {
            let p = m.get_slot_ptr(k).expect("present").as_ptr();
            assert_eq!(
                p as usize % align_of::<u64>(),
                0,
                "slot of {k:02x?} unaligned"
            );
            // SAFETY: a live slot; no mutation since `get_slot_ptr`.
            assert_eq!(unsafe { *p }, i as u64 + 1);
            slots.push(p as usize);
        }
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), keys.len(), "two keys share a value slot");
        assert_eq!(m.get(b"abcdefgx"), None);
        assert_eq!(m.get(&[0u8; 99]), None);

        // Remove from the middle, then the ends.
        for &at in &[3usize, 0, 6] {
            assert_eq!(m.remove(keys[at]), Some(at as u64 + 1));
            assert_eq!(m.get(keys[at]), None);
        }
        let live: Vec<usize> = vec![1, 2, 4, 5];
        let live_bytes: usize = live.iter().map(|&i| keys[i].len()).sum();
        assert_eq!(
            m.mem_used() - m.map.mem_used(),
            bucket_alloc_size(live.len(), live_bytes)
        );
        let mut seen = Vec::new();
        m.for_each(|k, v| seen.push((k.to_vec(), v)));
        seen.sort();
        let mut want: Vec<(Vec<u8>, u64)> = live
            .iter()
            .map(|&i| (keys[i].to_vec(), i as u64 + 1))
            .collect();
        want.sort();
        assert_eq!(seen, want);
        for &i in &live {
            assert_eq!(m.remove(keys[i]), Some(i as u64 + 1));
        }
        assert!(m.is_empty());
        assert_eq!(m.mem_used(), 0);
    }

    /// A bucket's size is read back from its last entry, so the layout
    /// must end with the keys in entry order; this pins that for every
    /// entry count a bucket can be built with here.
    #[test]
    fn bucket_bytes_reads_back_the_built_size() {
        for n in 1..=5usize {
            let keys: Vec<Vec<u8>> = (0..n).map(|i| vec![i as u8; i * 7]).collect();
            let total: usize = keys.iter().map(Vec::len).sum();
            let mut b = BucketBuilder::new(n, total);
            for (i, k) in keys.iter().enumerate() {
                b.push(k, i as u64);
            }
            let raw = b.finish();
            // SAFETY: a bucket just built and never published.
            unsafe {
                assert_eq!(bucket_len(raw), n);
                assert_eq!(bucket_bytes(raw), bucket_alloc_size(n, total));
                for (i, k) in keys.iter().enumerate() {
                    assert_eq!(bucket_key(raw, i), &k[..]);
                    assert_eq!(*bucket_value_ptr(raw, i), i as u64);
                }
                free_bucket(raw);
            }
        }
    }

    /// A builder that is dropped unfinished frees its allocation, and one
    /// pushed past its declared sizes panics rather than writing out of
    /// bounds.
    #[test]
    #[should_panic(expected = "bucket key bytes exceeded")]
    fn bucket_builder_rejects_overlong_keys() {
        let mut b = BucketBuilder::new(2, 3);
        b.push(b"ab", 1);
        b.push(b"cd", 2);
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

    #[test]
    fn insert_presence_and_replacement() {
        let mut m = ExpanseBytesMap::new();
        // Fresh insertion returns None and stores value directly.
        assert_eq!(m.insert(b"alpha", 42), None);
        assert_eq!(m.get(b"alpha"), Some(42));
        assert_eq!(m.len(), 1);

        // Overwrite returns previous value and stores new value.
        assert_eq!(m.insert(b"alpha", 99), Some(42));
        assert_eq!(m.get(b"alpha"), Some(99));
        assert_eq!(m.len(), 1);

        // JudyHS ins_slot on absent key returns slot initialized to 0.
        let slot = m.ins_slot(b"beta");
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*slot.as_ptr(), 0);
            *slot.as_ptr() = 123;
        }
        assert_eq!(m.get(b"beta"), Some(123));
        assert_eq!(m.len(), 2);

        // ins_slot on existing key keeps previous value untouched.
        let slot = m.ins_slot(b"beta");
        // SAFETY: slot is valid until next mutation.
        unsafe {
            assert_eq!(*slot.as_ptr(), 123);
        }
        assert_eq!(m.get(b"beta"), Some(123));
        assert_eq!(m.len(), 2);

        // Overwrite via insert on key created via ins_slot.
        assert_eq!(m.insert(b"beta", 456), Some(123));
        assert_eq!(m.get(b"beta"), Some(456));
        assert_eq!(m.len(), 2);
    }

    /// A hasher whose hash is splitmix64 of the key read as a little-endian
    /// word, so the keys below land on the hash trie exactly as the map
    /// wrapper's reproduction lands its keys (Refs #1079).
    #[cfg(feature = "std")]
    #[derive(Clone, Default)]
    struct SplitMix(u64);

    #[cfg(feature = "std")]
    impl Hasher for SplitMix {
        fn finish(&self) -> u64 {
            let mut z = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn write(&mut self, bytes: &[u8]) {
            for (i, &b) in bytes.iter().take(8).enumerate() {
                self.0 |= u64::from(b) << (8 * i);
            }
        }
    }

    #[cfg(feature = "std")]
    #[derive(Clone, Default)]
    struct SplitMixBuild;

    #[cfg(feature = "std")]
    impl BuildHasher for SplitMixBuild {
        type Hasher = SplitMix;
        fn build_hasher(&self) -> SplitMix {
            SplitMix(0)
        }
    }

    /// Optimistic removals on the shared bytes map run the map removal body
    /// on its hash trie (`olc_cas_remove_bucket_map`), so they shared the
    /// map wrapper's defect: 400 keys put the trie's root at a `BranchU` of
    /// 203 top digits, removing every other key leaves 145, and before #1079
    /// the branch stayed uncompressed below its demotion floor. The hash
    /// trie is validated directly, since the bytes map has no validator.
    #[cfg(all(feature = "std", target_pointer_width = "64"))]
    #[test]
    fn sync_bytes_map_remove_keeps_branch_u_above_its_floor() {
        let m = crate::sync::SyncExpanseBytesMap::with_hasher(SplitMixBuild);
        for i in 0..400u64 {
            assert_eq!(m.insert(&i.to_le_bytes(), i), None);
        }
        m.with_locked(|b| b.map.validate_defensive())
            .expect("precondition: valid before the removals");
        assert_eq!(
            m.with_locked(|b| b.map.stats().node_counts.branch_u),
            1,
            "precondition: the hash trie's root is an uncompressed branch"
        );
        for i in (0..400u64).step_by(2) {
            assert_eq!(m.remove(&i.to_le_bytes()), Some(i));
        }
        m.with_locked(|b| b.map.validate_defensive())
            .expect("the removals left a valid hash trie");
        assert_eq!(
            m.with_locked(|b| b.map.stats().node_counts.branch_u),
            0,
            "the root crossed its floor and was demoted"
        );
        for i in 0..400u64 {
            let want = (i % 2 != 0).then_some(i);
            assert_eq!(m.get(&i.to_le_bytes()), want, "key {i}");
        }
    }

    /// Refs #1612: the hash trie of a shared bytes map that joined writers
    /// filled and thinned validates exactly under `with_locked`. Their
    /// optimistic writes leave branch `pop0` and the bucket count stale, but
    /// they mark the top digit dirty in the wrapper, and `with_locked` folds
    /// every dirty subtree before the closure runs (`Shared::with_locked_pre`
    /// on the optimistic build, `Shared::with_locked` on the serial one), so
    /// no re-sync is needed. Red when that fold is skipped.
    #[cfg(all(feature = "std", target_pointer_width = "64"))]
    #[test]
    fn sync_bytes_map_from_joined_writers_validates_under_with_locked() {
        const WRITERS: u64 = 4;
        const PER: u64 = 4_000;
        let m = crate::sync::SyncExpanseBytesMap::with_hasher(SplitMixBuild);
        std::thread::scope(|s| {
            for t in 0..WRITERS {
                let m = &m;
                s.spawn(move || {
                    for i in 0..PER {
                        let k = t * PER + i;
                        assert_eq!(m.insert(&k.to_le_bytes(), k), None);
                    }
                    for i in (0..PER).step_by(3) {
                        let k = t * PER + i;
                        assert_eq!(m.remove(&k.to_le_bytes()), Some(k));
                    }
                });
            }
        });
        if let Err(e) = m.with_locked(|b| b.map.validate_defensive()) {
            panic!("a correct shared bytes map failed validation: {e}");
        }
        for k in 0..WRITERS * PER {
            let want = (!(k % PER).is_multiple_of(3)).then_some(k);
            assert_eq!(m.get(&k.to_le_bytes()), want, "key {k}");
        }
    }

    /// The concurrency bench's `bytes` workload on one thread: route-shaped
    /// keys drawn from a 200,000-key universe, a prefill of 100,000 draws,
    /// then 1,000,000 inserts and removes in equal measure, which take the
    /// map to about 100,000 keys and hold it there. The hash trie then puts
    /// about 390 keys under each of the 256 top digits, so each level-7
    /// branch holds about 200 digits and drifts around the
    /// bitmap-to-uncompressed threshold (192). Every such branch must settle
    /// as a `BranchU`: while one is a `BranchB`, every optimistic removal
    /// that empties one of its children falls back to the exclusive path
    /// (`BranchSplitKind::Remove`), and each crossing of the U <-> B band is
    /// an exclusive section too. With a one-digit band (demotion at 191) 18
    /// to 51 of the 257 branches were `BranchB` at every checkpoint of a
    /// 4,000,000-write run; with the 32-digit band none is past 800,000
    /// writes. A fixed hasher makes the trie shape deterministic. Under the
    /// diagnostic `ablation-one-digit-band` feature (AGENTS.md §2.7) the same
    /// workload must show the one-digit band's shape instead: some level-7
    /// branches sit as `BranchB`, so the ablation cannot compile to the
    /// default unnoticed.
    #[cfg(all(feature = "std", target_pointer_width = "64"))]
    #[test]
    #[cfg_attr(
        miri,
        ignore = "1.1M writes at 100,000 keys; the shape it checks needs that population, and it did not finish in the nightly lane's limit"
    )]
    fn sync_bytes_map_hash_trie_settles_uncompressed() {
        type Fixed = core::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>;
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
        const UNIVERSE: usize = 200_000;
        let keys: Vec<Vec<u8>> = (0..UNIVERSE)
            .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
            .collect();
        let m = crate::sync::SyncExpanseBytesMap::with_hasher(Fixed::default());
        let mut rng = XorShift(0x5CA1_AB1E);
        for _ in 0..UNIVERSE / 2 {
            let k = &keys[(rng.next() as usize) % UNIVERSE];
            m.insert(k, rng.next());
        }
        for _ in 0..1_000_000 {
            let k = &keys[(rng.next() as usize) % UNIVERSE];
            if rng.next() & 1 == 0 {
                m.insert(k, rng.next());
            } else {
                m.remove(k);
            }
        }
        let counts = m.with_locked(|b| {
            b.map
                .validate_defensive()
                .expect("the churn left a valid hash trie");
            b.map.stats().node_counts
        });
        #[cfg(not(feature = "ablation-one-digit-band"))]
        {
            assert_eq!(
                counts.branch_u, 257,
                "the root and every level-7 branch are uncompressed ({counts:?})"
            );
            assert_eq!(
                counts.branch_b, 0,
                "a level-7 branch sits as a BranchB, so its removals fall back ({counts:?})"
            );
        }
        #[cfg(feature = "ablation-one-digit-band")]
        {
            assert_eq!(
                counts.branch_u + counts.branch_b,
                257,
                "the root and every level-7 branch are a BranchU or a BranchB ({counts:?})"
            );
            assert!(
                counts.branch_b > 0,
                "under the one-digit band no level-7 branch sits as a BranchB, so the \
                 ablation did not restore the band #1221 widened ({counts:?})"
            );
        }
    }

    #[test]
    fn stats_breakdown_sums_to_mem_used() {
        let mut m = ExpanseBytesMap::new();
        assert_eq!(m.stats().node_bytes.total(), m.mem_used());
        assert_eq!(m.stats().node_bytes.total(), 0);

        m.insert(b"one", 1);
        m.insert(b"two", 2);
        m.insert(b"three", 3);
        let s = m.stats();
        assert_eq!(s.node_bytes.total(), m.mem_used());
        assert_eq!(s.entries, 3);
        assert_eq!(s.buckets, 3);
        assert_eq!(s.node_bytes.node_shells, 3 * BUCKET_OVERHEAD);
        assert_eq!(
            s.node_bytes.terminal_bytes,
            (3 + ENTRY_OVERHEAD) * 2 + (5 + ENTRY_OVERHEAD)
        );
        assert_eq!(
            s.node_bytes.sub_maps.total() + s.node_bytes.node_shells + s.node_bytes.terminal_bytes,
            m.mem_used()
        );

        #[derive(Default, Clone, Copy)]
        struct ZeroHasher;
        impl core::hash::Hasher for ZeroHasher {
            fn finish(&self) -> u64 {
                42
            }
            fn write(&mut self, _bytes: &[u8]) {}
        }
        let mut col_map =
            ExpanseBytesMap::with_hasher(core::hash::BuildHasherDefault::<ZeroHasher>::default());
        col_map.insert(b"first", 10);
        col_map.insert(b"second", 20);
        col_map.insert(b"third", 30);
        let s_col = col_map.stats();
        assert_eq!(s_col.buckets, 1);
        assert_eq!(s_col.entries, 3);
        assert_eq!(s_col.node_bytes.total(), col_map.mem_used());
        assert_eq!(s_col.node_bytes.node_shells, BUCKET_OVERHEAD);
        assert_eq!(
            s_col.node_bytes.terminal_bytes,
            (5 + ENTRY_OVERHEAD) + (6 + ENTRY_OVERHEAD) + (5 + ENTRY_OVERHEAD)
        );

        #[cfg(feature = "std")]
        {
            let sync = crate::sync::SyncExpanseBytesMap::new();
            sync.insert(b"alpha", 1);
            sync.insert(b"beta", 2);
            let s_sync = sync.stats();
            assert_eq!(s_sync.node_bytes.total(), sync.mem_used());
            assert_eq!(s_sync, sync.with_locked(|inner| inner.stats()));
        }
    }
}
