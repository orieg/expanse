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

/// One collision-bucket entry: the exact key bytes and the value word.
pub(crate) type Entry = (Box<[u8]>, u64);

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
/// place (a single `u64`).
///
/// #929: under the concurrent wrapper the value word is the **one**
/// field two threads may touch at once, so both sides reach it through
/// [`entry_value_atomic`] rather than as a plain `u64`. A writer stores
/// only while it holds the terminal's parent version lock and only after
/// that lock's compare has confirmed the bucket is still the published
/// one, so an unlinked bucket's value words are frozen from the moment
/// of the replacement; a reader loads without any lock. See
/// [`ExpanseBytesMap::get_validated`] for what a reader can observe.
pub(crate) type Bucket = Vec<Entry>;

/// The atomic view of one published entry's value word.
///
/// [`Entry`] declares the value as a plain `u64` because the
/// single-threaded engine owns it outright and the `JudyHS` slot
/// contract hands it out as `*mut u64`
/// ([`ExpanseBytesMap::ins_slot`]). The concurrent paths reach the same
/// word through [`core::sync::atomic::AtomicU64::from_ptr`], which is
/// exactly the "atomic access to a location declared non-atomically"
/// case that constructor exists for: a `u64` field of a `#[repr(Rust)]`
/// tuple is 8-aligned, which is `AtomicU64`'s alignment on every 64-bit
/// target this is compiled for.
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
    let word = unsafe { &raw const (*entries.add(at)).1 };
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
    // SAFETY: EBR-live published bucket; shell and entry array are
    // write-once after publication.
    let (len, entries) = unsafe { ((*bucket).len(), (*bucket).as_ptr()) };
    for i in 0..len {
        // SAFETY: `i < len` of the write-once entry array; the
        // projection names the key field only, never the value word.
        let k: &[u8] = unsafe { &(*entries.add(i)).0 };
        if k == key {
            return (len, Some(i));
        }
    }
    (len, None)
}

/// Builds the replacement bucket a colliding insert publishes: a
/// positional copy of the `len` entries of the published bucket at
/// `word`, with `key → val` appended.
///
/// Keys are cloned (the old bucket keeps ownership of its own, and is
/// disposed of with them). Value words are read atomically, and re-read
/// under the terminal's version lock by [`refresh_replacement_values`]
/// before the replacement is published.
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
    // SAFETY: EBR-live published bucket of `len` entries.
    let entries = unsafe { (*bucket).as_ptr() };
    let mut fresh: Bucket = Vec::with_capacity(len + 1);
    for i in 0..len {
        // SAFETY: `i < len`; the key field is write-once and the value
        // word is loaded through its atomic view.
        let (k, v) = unsafe {
            let k: &[u8] = &(*entries.add(i)).0;
            (
                Box::<[u8]>::from(k),
                entry_value_atomic(entries, i).load(core::sync::atomic::Ordering::Relaxed),
            )
        };
        fresh.push((k, v));
    }
    fresh.push((key.into(), val));
    Box::into_raw(Box::new(fresh))
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
    // SAFETY: `word` is the published bucket for this hash, so its shell
    // is EBR-live and its entry array holds more than `at` entries.
    let entries = unsafe { (*bucket).as_ptr() };
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
    let entries = unsafe { (*bucket).as_ptr() };
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
/// keys are cloned, so the old bucket keeps its own and is disposed of
/// with them. Value words are read atomically here and re-read under the
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
    // SAFETY: EBR-live published bucket of `len` entries.
    let entries = unsafe { (*bucket).as_ptr() };
    let mut fresh: Bucket = Vec::with_capacity(len - 1);
    for i in 0..len {
        if i == at {
            continue;
        }
        // SAFETY: `i < len`; the key field is write-once and the value
        // word is loaded through its atomic view.
        let (k, v) = unsafe {
            let k: &[u8] = &(*entries.add(i)).0;
            (
                Box::<[u8]>::from(k),
                entry_value_atomic(entries, i).load(core::sync::atomic::Ordering::Relaxed),
            )
        };
        fresh.push((k, v));
    }
    Box::into_raw(Box::new(fresh))
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
    let (base, dst) = unsafe { ((*entries).as_ptr(), (*repl).as_mut_ptr()) };
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
        unsafe { (*dst.add(j)).1 = v };
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
    let (base, dst) = unsafe { ((*entries).as_ptr(), (*repl).as_mut_ptr()) };
    for i in 0..n {
        // SAFETY: `i < n` indexes both arrays by the caller's contract.
        let v = unsafe { entry_value_atomic(base, i) }.load(core::sync::atomic::Ordering::Relaxed);
        // SAFETY: `dst.add(i)` is an entry of the caller's own bucket,
        // which no other thread can reach until it is published.
        unsafe { (*dst.add(i)).1 = v };
    }
}

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
pub(crate) fn dispose_bucket(ptr: *mut Bucket, own_keys: bool, defer: Option<&Arc<Collector>>) {
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
pub(crate) fn dispose_bucket(ptr: *mut Bucket, own_keys: bool, _defer: Option<&()>) {
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
    let h = hasher.hash_one(key);
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
        // SAFETY: `i < len` of the write-once entry array; the
        // projection names the key field only, never the value word.
        let k: &[u8] = unsafe { &(*entries.add(i)).0 };
        if k == key {
            // SAFETY: `i < len` of an EBR-live entry array under the
            // caller's pin. The atomic load is what makes a
            // concurrent in-place publish well-defined rather than a
            // data race; see this function's docs for what it returns.
            result = Some(
                unsafe { entry_value_atomic(entries, i) }
                    .load(core::sync::atomic::Ordering::Relaxed),
            );
            break;
        }
    }
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
    #[allow(dead_code)]
    pub(crate) fn set_len(&mut self, len: u64) {
        self.len = len;
    }

    /// Number of buckets in the underlying trie.
    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn bucket_count(&self) -> u64 {
        self.map.len()
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn set_bucket_pop(&mut self, pop: u64) {
        #[cfg(all(target_pointer_width = "64", feature = "std"))]
        self.map.set_tree_pop(pop);
        #[cfg(not(all(target_pointer_width = "64", feature = "std")))]
        let _ = pop;
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
    /// if absent. Valid until the next structural mutation.
    ///
    /// Takes `&self` to allow read-only lookups without requiring a mutable borrow.
    #[must_use]
    pub fn get_slot_ptr(&self, key: &[u8]) -> Option<NonNull<u64>> {
        let b_ptr = self.bucket_of(key)?.as_ptr();
        // SAFETY: `b_ptr` is a live bucket owned by this map. `as_mut_ptr`
        // borrows the bucket's `Vec` header mutably for the call, as the
        // `&mut self` lookup this replaces did; it does not reference the
        // entries, so the returned pointer keeps the buffer's provenance.
        let (len, entries) = unsafe { ((*b_ptr).len(), (*b_ptr).as_mut_ptr()) };
        for i in 0..len {
            // SAFETY: `i < len`, so `entries + i` is an initialised entry.
            let entry = unsafe { entries.add(i) };
            // SAFETY: a live entry; this borrows its key only, never the
            // value word the returned pointer writes.
            let k: &[u8] = unsafe { &(*entry).0 };
            if k == key {
                // SAFETY: a raw place projection to the value word, with the
                // buffer's write provenance and no intermediate reference.
                let val_ptr = unsafe { &raw mut (*entry).1 };
                return NonNull::new(val_ptr);
            }
        }
        None
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None`
    /// if absent — the compat `JudyHSGet` convention. Valid until the
    /// next structural mutation.
    #[must_use]
    pub fn get_value_slot(&mut self, key: &[u8]) -> Option<NonNull<u64>> {
        self.get_slot_ptr(key)
    }

    fn insert_slot_inner(&mut self, key: &[u8], init_val: u64) -> (NonNull<u64>, Option<u64>) {
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
            let bucket: Bucket = vec![(key.into(), init_val)];
            let raw = Box::into_raw(Box::new(bucket));
            // SAFETY: the slot stays valid until the next structural
            // trie mutation; none happens between `ins_slot` and here.
            unsafe { *slot.as_ptr() = raw as u64 };
            self.len += 1;
            self.extra_bytes += BUCKET_OVERHEAD + key.len() + ENTRY_OVERHEAD;
            // SAFETY: freshly allocated above; entry 0 exists.
            let fresh: &mut Bucket = unsafe { &mut *raw };
            return (NonNull::from(&mut fresh[0].1), None);
        }
        let old = word as *mut Bucket;
        // SAFETY: live bucket owned by this map; read-only search.
        if let Some(at) = unsafe { &*old }.iter().position(|(k, _)| &**k == key) {
            // Existing key: only its value word will mutate, in place —
            // the one mutation published buckets allow.
            // SAFETY: as above; the slot pointer stays valid until the
            // next structural mutation.
            let live: &mut Bucket = unsafe { &mut *old };
            let old_val = live[at].1;
            return (NonNull::from(&mut live[at].1), Some(old_val));
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
        bucket.push((key.into(), init_val));
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
        (NonNull::from(&mut fresh[old_len].1), None)
    }

    /// Inserts `key` with value 0 if absent — an existing value is kept
    /// untouched — and returns a **writable pointer to its value slot**:
    /// the compat `JudyHSIns` contract. Valid until the next structural
    /// mutation.
    pub fn ins_slot(&mut self, key: &[u8]) -> NonNull<u64> {
        self.insert_slot_inner(key, 0).0
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        let (slot, prev) = self.insert_slot_inner(key, val);
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
            self.extra_bytes = self.extra_bytes.saturating_sub(BUCKET_OVERHEAD);
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
            let bucket = unsafe { &*(word as *const Bucket) };
            for (k, v) in bucket {
                count += 1;
                if !f(k, *v) {
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
            dispose_bucket(word as *mut Bucket, true, defer.as_ref());
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
}
