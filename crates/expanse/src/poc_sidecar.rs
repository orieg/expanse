//! Experimental POC (#295): scalar metadata sidecar + inverted index.
//!
//! **This module is a research POC, gated behind the non-default
//! `poc-meta-sidecar` feature, and is NOT part of the stable public API.** It
//! exists to answer the question parked in issue #295: *which `AGENTS.md` §2.2
//! compliant sidecar shape covers the wide (`>24`-bit) / multi-column / `>64`
//! GiB blob-metadata capacity axes that the shipped Phase-1 in-slot
//! [`ArenaMeta`](crate::slot::SlotTag::ArenaMeta) encoding cannot?* The shipped
//! [`ExpanseBlobMap`](crate::blobmap::ExpanseBlobMap) is untouched — this module
//! only adds decoupled parallel structures beside the same core engine.
//!
//! # Why a *decoupled* sidecar (Judy §2.2 alignment)
//!
//! `AGENTS.md` §2.2 forbids widening the trie value word (Fat Slots) and
//! forbids complecting columnar attributes into the trie index; it *mandates*
//! that auxiliary/columnar metadata live in **decoupled columnar sidecars**.
//! Both POC structures here honour that: the trie value word
//! ([`ExpanseMap`]'s `u64`) never widens — it carries a compact dense record
//! **handle** ([`RecordId`]) only — and every metadata column lives in a
//! separate parallel array, so `ValueSlot` stays exactly one machine word and
//! the leaf cache-line density (8 slots / 64 B line) is preserved.
//!
//! # Part 1 — [`SidecarBlobMap`]
//!
//! A blob map whose trie stores, per key, a dense [`RecordId`] handle (not an
//! arena locator, not metadata). Two decoupled parallel arrays, indexed by that
//! handle, hold everything the shipped in-slot encoding packs into the 64-bit
//! value word:
//!
//! * `offsets[rid]: u64` — the arena address of the payload. Because it is a
//!   full `u64` sidecar entry, it is **not** bounded by the shipped
//!   32-bit-locator 64 GiB ceiling (axis *c*).
//! * `meta[rid]: [u32; K]` — `K` full **32-bit** metadata columns (axes *a* and
//!   *b*): a single 32-bit timestamp (`K = 1`), or `ts | tenant | status`
//!   (`K = 3`), etc.
//!
//! During a key-ordered range scan the predicate reads `meta[rid]` — a warm,
//! dense, L2/L3-resident array access — instead of the cold arena payload, so
//! the scan keeps Phase-1's key order *and* its cold-payload-skip speedup while
//! lifting all three capacity limits. Compaction rewrites only the dense
//! `offsets` array; the trie index is never touched (record handles are
//! stable), which is strictly less work than the Phase-1 in-slot compaction
//! that rewrites a value slot per live record via a random-access trie descent.
//!
//! # Part 2 — [`InvertedIndex`]
//!
//! For *low-cardinality discrete* attributes (tenant, status), a classic
//! inverted index — one [`ExpanseSet`] posting list of keys per attribute value
//! — answers multi-attribute equality queries by **native set intersection**
//! ([`ExpanseSet::intersection`] / [`ExpanseSet::intersection_len`], #339/#348)
//! rather than a scan. Its weak point is *high-cardinality / continuous* fields
//! (timestamps): one posting list per distinct value explodes the list count,
//! which coarse **bucketing** mitigates at the cost of a residual exact filter.

use crate::blobmap::{ArenaError, BlobArena, DEFAULT_CHUNK_SIZE};
use crate::map::ExpanseMap;
use crate::set::ExpanseSet;
use crate::types::Key;
use std::collections::BTreeMap;

/// Dense record handle stored in the trie value word of a [`SidecarBlobMap`].
///
/// A monotonic ordinal assigned at insertion (dead handles are recycled through
/// a free list), it indexes the decoupled `offsets` and `meta` sidecar arrays.
/// It is deliberately a compact `u32` — 4.29 G live records — because the wide
/// arena address it *replaces* in the value word now lives in the `u64`
/// `offsets` sidecar, so the trie word need not be wide at all.
pub type RecordId = u32;

// ---------------------------------------------------------------------------
// Wide-locator encoding (axis c) — production-shape alternative
// ---------------------------------------------------------------------------

/// Bit width of the wide arena locator that the freed 24 metadata bits enable
/// when metadata is evicted from the slot into the sidecar.
pub const WIDE_LOCATOR_BITS: u32 = 56;

/// Mask for a [`WIDE_LOCATOR_BITS`]-bit locator.
pub const WIDE_LOCATOR_MASK: u64 = (1u64 << WIDE_LOCATOR_BITS) - 1;

/// Global-offset envelope of a 56-bit locator addressing 16-byte units:
/// `2^56 × 16 = 2^60` bytes = **1 EiB** (comfortably beyond the issue's
/// stated 64 PiB, which assumed a raw-byte 56-bit locator). Contrast the
/// shipped 32-bit `ArenaMeta` locator's `2^32 × 16 =` **64 GiB**
/// ([`crate::blobmap::ARENA_META_CEILING`]).
pub const WIDE_LOCATOR_CEILING: u128 = (1u128 << WIDE_LOCATOR_BITS) * 16;

/// Packs a 16-byte-aligned `global_offset` into a 56-bit locator (`offset / 16`)
/// laid beside an 8-bit tag: `[locator(56) | tag(8)]`. This is the
/// *production-shape* alternative to storing a dense [`RecordId`]: it keeps the
/// wide address in the slot but still evicts metadata to the sidecar. Returns
/// `None` if the offset is unaligned or beyond [`WIDE_LOCATOR_CEILING`].
///
/// The POC [`SidecarBlobMap`] itself uses the dense-handle form instead (the
/// value word carries a `RecordId`, and *both* address and metadata are
/// sidecars) — this helper exists to make the 56-bit-locator capacity claim
/// concrete and test-checked.
#[must_use]
pub fn pack_wide_locator(global_offset: u128, tag: u8) -> Option<u64> {
    if global_offset % 16 != 0 || global_offset >= WIDE_LOCATOR_CEILING {
        return None;
    }
    let locator = (global_offset / 16) as u64;
    Some((locator << 8) | (tag as u64))
}

/// Recovers `(global_offset, tag)` from a value packed by [`pack_wide_locator`].
#[must_use]
pub fn unpack_wide_locator(raw: u64) -> (u128, u8) {
    let tag = (raw & 0xFF) as u8;
    let locator = (raw >> 8) & WIDE_LOCATOR_MASK;
    ((locator as u128) * 16, tag)
}

// ---------------------------------------------------------------------------
// Part 1 — scalar metadata sidecar
// ---------------------------------------------------------------------------

/// A blob map with a decoupled scalar metadata sidecar and `K` full-32-bit
/// metadata columns. See the [module docs](self) for the design rationale.
///
/// `K` is the number of metadata columns: `1` for a single 32-bit field
/// (timestamp), `3` for `ts | tenant | status`, etc. All payloads are stored in
/// the arena (the POC measures the metadata sidecar, so it does not reproduce
/// the shipped inline-≤7-byte packing, which is orthogonal and preserved in
/// Phase 1).
pub struct SidecarBlobMap<const K: usize> {
    /// Trie: `key -> RecordId` (the handle, stored in the `u64` value word).
    index: ExpanseMap,
    /// Payload backing store (reused unchanged from the shipped engine).
    arena: BlobArena,
    /// `RecordId -> arena global byte offset`. A `u64` column, so it addresses
    /// beyond the shipped 32-bit-locator 64 GiB ceiling (axis *c*).
    offsets: Vec<u64>,
    /// `RecordId -> K` full-32-bit metadata columns (axes *a*, *b*).
    meta: Vec<[u32; K]>,
    /// `RecordId -> live?`. A dead handle's payload is reclaimed at the next
    /// [`compact`](Self::compact); the handle itself is recycled via `free`.
    live_flag: Vec<bool>,
    /// Recycled dead handles, reused before growing the sidecar arrays.
    free: Vec<RecordId>,
    /// Live payload + header bytes (drives compaction fragmentation ratio).
    live_payload_bytes: usize,
    /// Live entry count.
    live_count: u64,
}

impl<const K: usize> SidecarBlobMap<K> {
    /// Creates an empty sidecar blob map with the default 2 MiB arena chunks.
    #[must_use]
    pub fn new() -> Self {
        Self::with_chunk_size(DEFAULT_CHUNK_SIZE)
    }

    /// Creates an empty sidecar blob map with a custom arena chunk size.
    #[must_use]
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            index: ExpanseMap::new(),
            arena: BlobArena::new(chunk_size),
            offsets: Vec::new(),
            meta: Vec::new(),
            live_flag: Vec::new(),
            free: Vec::new(),
            live_payload_bytes: 0,
            live_count: 0,
        }
    }

    /// Number of live entries.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.live_count
    }

    /// Returns `true` if the map contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live_count == 0
    }

    /// Returns `true` if `key` is present.
    #[must_use]
    pub fn contains_key(&self, key: Key) -> bool {
        self.index.contains_key(key)
    }

    /// Total heap bytes: trie index + arena + the two sidecar arrays.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.index.mem_used()
            + self.arena.mem_used()
            + self.offsets.capacity() * core::mem::size_of::<u64>()
            + self.meta.capacity() * core::mem::size_of::<[u32; K]>()
            + self.live_flag.capacity()
            + self.free.capacity() * core::mem::size_of::<RecordId>()
    }

    /// Bytes the two decoupled sidecar arrays occupy per allocated record slot
    /// (`offsets` + `meta` + `live_flag`), the quantity that must stay
    /// L2/L3-resident for the warm-metadata scan to hold. Handle recycling keeps
    /// the allocated slot count near the live count.
    #[must_use]
    pub const fn sidecar_bytes_per_record() -> usize {
        core::mem::size_of::<u64>() + core::mem::size_of::<[u32; K]>() + 1
    }

    /// Live payload + header bytes currently reachable.
    #[must_use]
    pub fn live_payload_bytes(&self) -> usize {
        self.live_payload_bytes
    }

    /// Number of allocated record slots (live + not-yet-recycled dead).
    #[must_use]
    pub fn record_slots(&self) -> usize {
        self.offsets.len()
    }

    /// Reference to the backing arena.
    #[must_use]
    pub fn arena(&self) -> &BlobArena {
        &self.arena
    }

    /// Inserts or overwrites `key` with `data` and its `K` metadata columns.
    ///
    /// Overwriting an existing key reuses its record handle in place (the old
    /// payload becomes dead until the next [`compact`](Self::compact)). Errors
    /// with [`ArenaError`] only on arena allocation failure (a payload larger
    /// than one chunk, or the arena capacity cap) — metadata is a full 32-bit
    /// column, so there is no `MetaOverflow`.
    pub fn insert(&mut self, key: Key, data: &[u8], cols: [u32; K]) -> Result<(), ArenaError> {
        let needed = 8 + data.len();
        if let Some(rid_raw) = self.index.get(key) {
            // Overwrite: allocate the new payload first (so a failure leaves the
            // old entry intact), then retarget the existing handle.
            let rid = rid_raw as usize;
            let old_len = self
                .arena
                .get_blob_slice(self.offsets[rid])
                .map_or(0, |p| 8 + p.len());
            let off = self.arena.alloc_blob(data)?;
            self.offsets[rid] = off;
            self.meta[rid] = cols;
            self.live_payload_bytes = self.live_payload_bytes - old_len + needed;
            Ok(())
        } else {
            let off = self.arena.alloc_blob(data)?;
            let rid = if let Some(rid) = self.free.pop() {
                let r = rid as usize;
                self.offsets[r] = off;
                self.meta[r] = cols;
                self.live_flag[r] = true;
                rid
            } else {
                let rid = self.offsets.len() as RecordId;
                self.offsets.push(off);
                self.meta.push(cols);
                self.live_flag.push(true);
                rid
            };
            self.index.insert(key, rid as u64);
            self.live_payload_bytes += needed;
            self.live_count += 1;
            Ok(())
        }
    }

    /// Point lookup: returns the payload slice and its metadata columns.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<(&[u8], &[u32; K])> {
        let rid = self.index.get(key)? as usize;
        let payload = self.arena.get_blob_slice(self.offsets[rid])?;
        Some((payload, &self.meta[rid]))
    }

    /// Metadata columns for `key` without touching the payload.
    #[must_use]
    pub fn meta_of(&self, key: Key) -> Option<&[u32; K]> {
        let rid = self.index.get(key)? as usize;
        Some(&self.meta[rid])
    }

    /// Removes `key`, returning `true` if it was present. The payload is
    /// reclaimed at the next [`compact`](Self::compact); the handle is recycled.
    pub fn remove(&mut self, key: Key) -> bool {
        if let Some(rid_raw) = self.index.remove(key) {
            let rid = rid_raw as usize;
            if self.live_flag[rid] {
                let dead = self
                    .arena
                    .get_blob_slice(self.offsets[rid])
                    .map_or(0, |p| 8 + p.len());
                self.live_payload_bytes = self.live_payload_bytes.saturating_sub(dead);
                self.live_flag[rid] = false;
                self.free.push(rid as RecordId);
                self.live_count -= 1;
            }
            true
        } else {
            false
        }
    }

    /// Key-ordered range scan with the predicate evaluated against the **warm
    /// sidecar** metadata columns before any cold payload fetch.
    ///
    /// For every key in `range` (ascending trie order) the predicate sees the
    /// key and its `[u32; K]` columns; only on a match is the cold payload
    /// resolved and passed to the callback. Returning `false` from the callback
    /// stops the scan.
    pub fn scan_filtered<P, F>(
        &self,
        range: core::ops::RangeInclusive<Key>,
        mut predicate: P,
        mut callback: F,
    ) where
        P: FnMut(Key, &[u32; K]) -> bool,
        F: FnMut(Key, &[u8], &[u32; K]) -> bool,
    {
        for (key, rid_raw) in self.index.range(range) {
            let rid = rid_raw as usize;
            let cols = &self.meta[rid];
            if predicate(key, cols) {
                if let Some(payload) = self.arena.get_blob_slice(self.offsets[rid]) {
                    if !callback(key, payload, cols) {
                        break;
                    }
                }
            }
        }
    }

    /// Fragmentation ratio `(allocated − live) / allocated` of the arena, the
    /// same trigger the shipped engine uses to decide compaction.
    #[must_use]
    pub fn fragmentation(&self) -> f64 {
        let allocated = self.arena.mem_used();
        if allocated == 0 {
            return 0.0;
        }
        (allocated - self.live_payload_bytes.min(allocated)) as f64 / allocated as f64
    }

    /// In-place compaction: relocates every live payload into a fresh arena and
    /// rewrites only the dense `offsets` sidecar. **The trie index is never
    /// touched** — record handles are stable across compaction — so this does
    /// strictly less work than the shipped in-slot compaction, which rewrites a
    /// trie value slot per live record via a random-access descent.
    ///
    /// All-or-nothing: if any relocation fails the map is left unchanged.
    /// Returns the number of live records relocated.
    pub fn compact(&mut self) -> Result<usize, ArenaError> {
        let mut new_arena = BlobArena::new(self.arena.chunk_size());
        // Relocate into fresh offsets first; commit only if every alloc succeeds.
        let mut new_offsets = self.offsets.clone();
        let mut moved = 0usize;
        for (rid, new_off_slot) in new_offsets.iter_mut().enumerate() {
            if !self.live_flag[rid] {
                continue;
            }
            let Some(payload) = self.arena.get_blob_slice(self.offsets[rid]) else {
                continue;
            };
            *new_off_slot = new_arena.alloc_blob(payload)?;
            moved += 1;
        }
        self.arena = new_arena;
        self.offsets = new_offsets;
        Ok(moved)
    }
}

impl<const K: usize> Default for SidecarBlobMap<K> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Part 2 — inverted index for low-cardinality discrete attributes
// ---------------------------------------------------------------------------

/// Cardinality / memory characterization of one inverted-index column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColumnStats {
    /// Number of distinct attribute values (posting lists).
    pub posting_lists: usize,
    /// Total postings summed across all lists (= number of indexed keys).
    pub total_postings: u64,
    /// Heap bytes across all posting-list sets.
    pub mem_bytes: usize,
}

/// A two-column inverted index (`tenant`, `status`) plus a timestamp column
/// maintained in both an exact and a bucketed form, to characterize the
/// high-cardinality weak point.
///
/// `tenant` and `status` are the intended low-cardinality discrete use case:
/// multi-attribute equality is answered by native [`ExpanseSet`] intersection.
/// The timestamp columns exist only to measure the failure mode: `ts_exact`
/// keeps one posting list per distinct timestamp (which explodes toward one
/// list per key as timestamps approach unique), while `ts_bucketed` coarsens
/// timestamps by [`Self::bucket_shift`] to bound the list count.
pub struct InvertedIndex {
    tenant: BTreeMap<u32, ExpanseSet>,
    status: BTreeMap<u32, ExpanseSet>,
    ts_exact: BTreeMap<u32, ExpanseSet>,
    ts_bucketed: BTreeMap<u32, ExpanseSet>,
    bucket_shift: u32,
    keys: u64,
}

impl InvertedIndex {
    /// Creates an inverted index whose timestamp bucketing coarsens by
    /// `bucket_shift` low bits (bucket width `2^bucket_shift`).
    #[must_use]
    pub fn new(bucket_shift: u32) -> Self {
        Self {
            tenant: BTreeMap::new(),
            status: BTreeMap::new(),
            ts_exact: BTreeMap::new(),
            ts_bucketed: BTreeMap::new(),
            bucket_shift,
            keys: 0,
        }
    }

    /// Timestamp bucket-shift (bucket width is `2^shift`).
    #[must_use]
    pub fn bucket_shift(&self) -> u32 {
        self.bucket_shift
    }

    /// Indexes `key` under `tenant`, `status`, and `ts` (in both ts forms).
    pub fn insert(&mut self, key: Key, tenant: u32, status: u32, ts: u32) {
        self.tenant.entry(tenant).or_default().insert(key);
        self.status.entry(status).or_default().insert(key);
        self.ts_exact.entry(ts).or_default().insert(key);
        self.ts_bucketed
            .entry(ts >> self.bucket_shift)
            .or_default()
            .insert(key);
        self.keys += 1;
    }

    /// Number of indexed keys.
    #[must_use]
    pub fn keys(&self) -> u64 {
        self.keys
    }

    /// Materializes the key set matching `tenant = t AND status = s` via native
    /// set intersection ([`ExpanseSet::intersection`]).
    #[must_use]
    pub fn query_tenant_status(&self, t: u32, s: u32) -> ExpanseSet {
        match (self.tenant.get(&t), self.status.get(&s)) {
            (Some(a), Some(b)) => a.intersection(b),
            _ => ExpanseSet::new(),
        }
    }

    /// Counts `tenant = t AND status = s` via structural
    /// [`ExpanseSet::intersection_len`] — no result set is materialized.
    #[must_use]
    pub fn count_tenant_status(&self, t: u32, s: u32) -> u64 {
        match (self.tenant.get(&t), self.status.get(&s)) {
            (Some(a), Some(b)) => a.intersection_len(b),
            _ => 0,
        }
    }

    /// Exact-timestamp range query `[lo, hi]` as the union of every exact
    /// posting list in range — the high-cardinality weak point: it visits one
    /// list per *distinct timestamp* in `[lo, hi]`.
    #[must_use]
    pub fn query_ts_range_exact(&self, lo: u32, hi: u32) -> ExpanseSet {
        let mut out = ExpanseSet::new();
        for (_ts, set) in self.ts_exact.range(lo..=hi) {
            for k in set.iter() {
                out.insert(k);
            }
        }
        out
    }

    /// Bucketed-timestamp range query `[lo, hi]`: unions the covering buckets,
    /// then applies an exact residual filter on the two boundary buckets (whose
    /// keys may fall outside `[lo, hi]`). Interior buckets are fully contained,
    /// so they need no residual check. `ts_of` supplies a key's exact timestamp
    /// for the residual filter.
    #[must_use]
    pub fn query_ts_range_bucketed<T: Fn(Key) -> u32>(
        &self,
        lo: u32,
        hi: u32,
        ts_of: T,
    ) -> ExpanseSet {
        let lo_b = lo >> self.bucket_shift;
        let hi_b = hi >> self.bucket_shift;
        let mut out = ExpanseSet::new();
        for (&bucket, set) in self.ts_bucketed.range(lo_b..=hi_b) {
            let boundary = bucket == lo_b || bucket == hi_b;
            for k in set.iter() {
                if boundary {
                    let ts = ts_of(k);
                    if ts < lo || ts > hi {
                        continue;
                    }
                }
                out.insert(k);
            }
        }
        out
    }

    /// Characterizes the `tenant` column.
    #[must_use]
    pub fn tenant_stats(&self) -> ColumnStats {
        Self::column_stats(&self.tenant)
    }

    /// Characterizes the `status` column.
    #[must_use]
    pub fn status_stats(&self) -> ColumnStats {
        Self::column_stats(&self.status)
    }

    /// Characterizes the exact-timestamp column (the weak point).
    #[must_use]
    pub fn ts_exact_stats(&self) -> ColumnStats {
        Self::column_stats(&self.ts_exact)
    }

    /// Characterizes the bucketed-timestamp column (the mitigation).
    #[must_use]
    pub fn ts_bucketed_stats(&self) -> ColumnStats {
        Self::column_stats(&self.ts_bucketed)
    }

    fn column_stats(col: &BTreeMap<u32, ExpanseSet>) -> ColumnStats {
        let mut total = 0u64;
        let mut mem = 0usize;
        for set in col.values() {
            total += set.len();
            mem += set.mem_used();
        }
        ColumnStats {
            posting_lists: col.len(),
            total_postings: total,
            mem_bytes: mem,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap as RefMap;

    // Deterministic xorshift, matching the bench harness generator.
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

    // ---- Part 1: scalar metadata sidecar ------------------------------------

    #[test]
    fn point_lookup_and_meta_roundtrip() {
        let mut map = SidecarBlobMap::<1>::with_chunk_size(64 * 1024);
        map.insert(10, b"", [0]).unwrap();
        map.insert(11, b"a-short-blob", [42]).unwrap();
        map.insert(12, &vec![0xAB; 500], [u32::MAX]).unwrap();

        assert_eq!(map.len(), 3);
        assert_eq!(map.get(10).unwrap().0, b"");
        assert_eq!(map.get(11).unwrap().0, b"a-short-blob");
        assert_eq!(map.get(11).unwrap().1, &[42]);
        // Full 32-bit metadata — impossible in the shipped 24-bit in-slot form.
        assert_eq!(map.get(12).unwrap().1, &[u32::MAX]);
        assert_eq!(map.get(12).unwrap().0, &vec![0xAB; 500][..]);
        assert!(map.get(99).is_none());
    }

    #[test]
    fn axis_a_full_32bit_timestamp_range_scan_matches_btreemap() {
        // Timestamps spanning the full u32 range, including values > 2^24 that
        // the shipped 24-bit in-slot metadata cannot represent.
        let mut map = SidecarBlobMap::<1>::with_chunk_size(1 << 20);
        let mut reference: RefMap<u64, (Vec<u8>, u32)> = RefMap::new();
        let mut rng = XorShift(0xA11CE);
        for k in 0..2000u64 {
            let ts = (rng.next() & 0xFFFF_FFFF) as u32; // full 32-bit
            let payload = format!("ts-payload-{k}-{ts}").into_bytes();
            map.insert(k, &payload, [ts]).unwrap();
            reference.insert(k, (payload, ts));
        }

        // Predicate: ts in a window that straddles the 24-bit boundary.
        let (lo, hi) = (0x00A0_0000u32, 0xF000_0000u32);
        let mut got: Vec<(u64, u32)> = Vec::new();
        map.scan_filtered(
            0..=u64::MAX,
            |_k, cols| (lo..=hi).contains(&cols[0]),
            |k, _payload, cols| {
                got.push((k, cols[0]));
                true
            },
        );

        let mut want: Vec<(u64, u32)> = reference
            .iter()
            .filter(|(_, (_, ts))| (lo..=hi).contains(ts))
            .map(|(k, (_, ts))| (*k, *ts))
            .collect();
        want.sort_unstable();
        // Scan output must already be key-ordered (trie walk); assert directly.
        assert_eq!(got, want);
        assert!(
            got.windows(2).all(|w| w[0].0 < w[1].0),
            "scan must be key-ordered"
        );
    }

    #[test]
    fn axis_b_three_attribute_predicate_matches_btreemap() {
        // ts | tenant | status, each full 32-bit — the multi-column axis.
        let mut map = SidecarBlobMap::<3>::with_chunk_size(1 << 20);
        let mut reference: RefMap<u64, [u32; 3]> = RefMap::new();
        let mut rng = XorShift(0xB0B);
        for k in 0..5000u64 {
            let ts = (rng.next() & 0xFFFF_FFFF) as u32;
            let tenant = (rng.next() % 8) as u32;
            let status = (rng.next() % 4) as u32;
            map.insert(k, b"row-payload-XXXXXXXX", [ts, tenant, status])
                .unwrap();
            reference.insert(k, [ts, tenant, status]);
        }

        // ts BETWEEN lo AND hi  AND  tenant = 3  AND  status = 1
        let (lo, hi, tenant, status) = (0x1000_0000u32, 0xE000_0000u32, 3u32, 1u32);
        let pred = |c: &[u32; 3]| (lo..=hi).contains(&c[0]) && c[1] == tenant && c[2] == status;

        let mut got: Vec<u64> = Vec::new();
        map.scan_filtered(
            0..=u64::MAX,
            |_k, c| pred(c),
            |k, _p, _c| {
                got.push(k);
                true
            },
        );

        let mut want: Vec<u64> = reference
            .iter()
            .filter(|(_, c)| pred(c))
            .map(|(k, _)| *k)
            .collect();
        want.sort_unstable();
        assert_eq!(got, want);
    }

    #[test]
    fn overwrite_updates_payload_and_meta_reusing_handle() {
        let mut map = SidecarBlobMap::<2>::with_chunk_size(64 * 1024);
        map.insert(1, b"original-value", [10, 20]).unwrap();
        let slots_before = map.record_slots();
        map.insert(1, b"replacement-value-longer", [11, 21])
            .unwrap();
        assert_eq!(map.len(), 1);
        // Overwrite reuses the handle: no new record slot allocated.
        assert_eq!(map.record_slots(), slots_before);
        assert_eq!(map.get(1).unwrap().0, b"replacement-value-longer");
        assert_eq!(map.get(1).unwrap().1, &[11, 21]);
    }

    #[test]
    fn remove_recycles_handle_and_compaction_preserves_data() {
        let mut map = SidecarBlobMap::<1>::with_chunk_size(64 * 1024);
        for k in 0..300u64 {
            map.insert(k, &[(k & 0xFF) as u8; 200], [k as u32]).unwrap();
        }
        // Delete 200, churning the arena.
        for k in 0..200u64 {
            assert!(map.remove(k));
        }
        assert_eq!(map.len(), 100);
        // A recycled handle keeps record_slots from growing unboundedly.
        for k in 300..340u64 {
            map.insert(k, &[0x11u8; 200], [k as u32]).unwrap();
        }
        assert!(map.record_slots() <= 300, "freed handles must be recycled");

        let moved = map.compact().unwrap();
        assert_eq!(moved as u64, map.len());

        // Every surviving entry intact and correct after compaction (no trie
        // rewrite happened — handles are stable).
        for k in 200..300u64 {
            assert_eq!(map.get(k).unwrap().0, &vec![(k & 0xFF) as u8; 200][..]);
            assert_eq!(map.get(k).unwrap().1, &[k as u32]);
        }
        for k in 300..340u64 {
            assert_eq!(map.get(k).unwrap().1, &[k as u32]);
        }
    }

    #[test]
    fn full_differential_insert_remove_get_against_btreemap() {
        let mut map = SidecarBlobMap::<1>::with_chunk_size(1 << 16);
        let mut reference: RefMap<u64, (Vec<u8>, u32)> = RefMap::new();
        let mut rng = XorShift(0xC0FFEE);
        for _ in 0..8000 {
            let k = rng.next() % 1500;
            let op = rng.next() % 3;
            if op == 0 {
                assert_eq!(map.remove(k), reference.remove(&k).is_some());
            } else {
                let len = (rng.next() % 40) as usize;
                let payload: Vec<u8> = (0..len).map(|i| (i as u64 ^ k) as u8).collect();
                let m = (rng.next() & 0xFFFF_FFFF) as u32;
                map.insert(k, &payload, [m]).unwrap();
                reference.insert(k, (payload, m));
            }
            assert_eq!(map.len(), reference.len() as u64);
        }
        // Occasionally compact, then re-verify the whole map.
        map.compact().unwrap();
        for (k, (payload, m)) in &reference {
            let (got_p, got_m) = map.get(*k).unwrap();
            assert_eq!(got_p, &payload[..]);
            assert_eq!(got_m, &[*m]);
        }
        // Absent keys stay absent.
        for k in 1500..1520 {
            assert!(map.get(k).is_none());
        }
    }

    // ---- Axis c: >64 GiB capacity (encoding proxy) --------------------------

    #[test]
    fn axis_c_wide_locator_encodes_beyond_64gib() {
        use crate::blobmap::ARENA_META_CEILING; // 64 GiB
        // The shipped 32-bit ArenaMeta locator ceiling.
        assert_eq!(ARENA_META_CEILING, (1u64 << 32) * 16); // 64 GiB

        // An offset the shipped in-slot encoding cannot represent (100 GiB),
        // but the 56-bit sidecar locator can, round-tripping exactly.
        let off_100gib: u128 = 100 * 1024 * 1024 * 1024;
        assert!(off_100gib > ARENA_META_CEILING as u128);
        let packed = pack_wide_locator(off_100gib, 0x11).expect("56-bit locator holds 100 GiB");
        let (back, tag) = unpack_wide_locator(packed);
        assert_eq!(back, off_100gib);
        assert_eq!(tag, 0x11);

        // The shipped 32-bit locator would overflow at this offset.
        assert!((off_100gib / 16) > u32::MAX as u128);

        // Ceiling boundary: 1 EiB is out of range, one unit below is in range.
        assert!(pack_wide_locator(WIDE_LOCATOR_CEILING, 0).is_none());
        assert!(pack_wide_locator(WIDE_LOCATOR_CEILING - 16, 0).is_some());
        // Unaligned offsets are rejected, never silently truncated.
        assert!(pack_wide_locator(24, 0).is_none());
    }

    #[test]
    fn sidecar_bytes_per_record_math() {
        // Pins the Step-0 residency arithmetic in code (§5.6 pre-registration).
        // K=1: 8 (offset u64) + 4 (meta u32) + 1 (live flag) = 13 B/record.
        assert_eq!(SidecarBlobMap::<1>::sidecar_bytes_per_record(), 13);
        // K=3: 8 + 12 + 1 = 21 B/record.
        assert_eq!(SidecarBlobMap::<3>::sidecar_bytes_per_record(), 21);
    }

    // ---- Part 2: inverted index --------------------------------------------

    #[test]
    fn inverted_index_intersection_matches_reference() {
        let mut idx = InvertedIndex::new(8);
        let mut rng = XorShift(0xD00D);
        // Reference: brute-force (tenant, status) per key.
        let mut ref_ts: RefMap<u64, (u32, u32, u32)> = RefMap::new();
        for k in 0..6000u64 {
            let tenant = (rng.next() % 5) as u32;
            let status = (rng.next() % 3) as u32;
            let ts = (rng.next() & 0xFFFF_FFFF) as u32;
            idx.insert(k, tenant, status, ts);
            ref_ts.insert(k, (tenant, status, ts));
        }
        assert_eq!(idx.keys(), 6000);

        for t in 0..5u32 {
            for s in 0..3u32 {
                let got = idx.query_tenant_status(t, s);
                let want: Vec<u64> = ref_ts
                    .iter()
                    .filter(|(_, (tt, ss, _))| *tt == t && *ss == s)
                    .map(|(k, _)| *k)
                    .collect();
                let got_keys: Vec<u64> = got.iter().collect();
                assert_eq!(got_keys, want, "intersection set mismatch t={t} s={s}");
                // intersection_len must agree with the materialized set.
                assert_eq!(idx.count_tenant_status(t, s), want.len() as u64);
            }
        }
    }

    #[test]
    fn inverted_index_ts_range_exact_and_bucketed_agree() {
        let mut idx = InvertedIndex::new(10); // 1024-wide buckets
        let mut rng = XorShift(0xFEED);
        let mut ref_ts: RefMap<u64, u32> = RefMap::new();
        for k in 0..8000u64 {
            let ts = (rng.next() % 200_000) as u32;
            idx.insert(k, 0, 0, ts);
            ref_ts.insert(k, ts);
        }
        let ts_of = |k: Key| *ref_ts.get(&k).unwrap();

        for &(lo, hi) in &[
            (0u32, 1023u32),
            (500, 50_000),
            (99_000, 101_000),
            (0, 200_000),
        ] {
            let exact = idx.query_ts_range_exact(lo, hi);
            let bucketed = idx.query_ts_range_bucketed(lo, hi, ts_of);
            let mut want: Vec<u64> = ref_ts
                .iter()
                .filter(|(_, ts)| (lo..=hi).contains(ts))
                .map(|(k, _)| *k)
                .collect();
            want.sort_unstable();
            assert_eq!(
                exact.iter().collect::<Vec<_>>(),
                want,
                "exact ts range [{lo},{hi}]"
            );
            assert_eq!(
                bucketed.iter().collect::<Vec<_>>(),
                want,
                "bucketed ts range [{lo},{hi}] (residual filter)"
            );
        }
    }

    #[test]
    fn ts_bucketing_reduces_posting_list_count() {
        // Near-unique timestamps: the exact column approaches one list per key;
        // bucketing bounds it. This is the characterization the POC reports.
        let mut idx = InvertedIndex::new(12); // 4096-wide buckets
        for k in 0..10_000u64 {
            idx.insert(k, 0, 0, k as u32); // unique ts == k
        }
        let exact = idx.ts_exact_stats();
        let bucketed = idx.ts_bucketed_stats();
        assert_eq!(exact.posting_lists, 10_000, "unique ts => one list per key");
        // 10_000 / 4096 -> 3 buckets.
        assert!(
            bucketed.posting_lists <= 4,
            "bucketing collapses the list count"
        );
        assert_eq!(exact.total_postings, bucketed.total_postings);
    }
}

/// Arch-independent characterization harness (#295 POC). Emits the structural
/// tables (memory overhead, inverted-index cardinality/memory, compaction work)
/// that do not require a timing host. Run with:
/// `cargo test -p expanse-trie --features poc-meta-sidecar characterize_poc_295 -- --nocapture --ignored`
#[cfg(test)]
mod characterize {
    use super::*;

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

    const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

    #[test]
    #[ignore = "characterization output; run explicitly with --nocapture --ignored"]
    fn characterize_poc_295() {
        // Match the cold-DRAM bench dataset: N = 262_144, 1 KiB payloads.
        const N: u64 = 262_144;
        const PAYLOAD: usize = 1024;

        // ---- Sidecar memory overhead (K=1 and K=3) --------------------------
        eprintln!("\n=== §5.6 characterization (arch-independent) — N={N}, payload={PAYLOAD}B ===");
        let mut s1 = SidecarBlobMap::<1>::with_chunk_size(64 * 1024 * 1024);
        let mut s3 = SidecarBlobMap::<3>::with_chunk_size(64 * 1024 * 1024);
        for k in 0..N {
            let m = (k.wrapping_mul(GOLDEN) % 10_000) as u32;
            let payload = [(k & 0xFF) as u8; PAYLOAD];
            s1.insert(k, &payload, [m]).unwrap();
            s3.insert(k, &payload, [m, (k % 8) as u32, (k % 4) as u32])
                .unwrap();
        }
        eprintln!(
            "sidecar bytes/record: K=1 => {} B, K=3 => {} B",
            SidecarBlobMap::<1>::sidecar_bytes_per_record(),
            SidecarBlobMap::<3>::sidecar_bytes_per_record()
        );
        let sc1 = SidecarBlobMap::<1>::sidecar_bytes_per_record() * N as usize;
        let sc3 = SidecarBlobMap::<3>::sidecar_bytes_per_record() * N as usize;
        eprintln!(
            "sidecar array footprint @N: K=1 => {:.2} MiB, K=3 => {:.2} MiB (must stay L2/L3-resident for the warm scan)",
            sc1 as f64 / 1048576.0,
            sc3 as f64 / 1048576.0
        );
        eprintln!(
            "residency @262k: K=1 {:.2} MiB vs ref L2=1.25 MiB/core, L3=30 MiB; K=3 {:.2} MiB",
            sc1 as f64 / 1048576.0,
            sc3 as f64 / 1048576.0
        );
        eprintln!(
            "sidecar map total mem_used: K=1 => {:.1} MiB, K=3 => {:.1} MiB (incl. arena+trie)",
            s1.mem_used() as f64 / 1048576.0,
            s3.mem_used() as f64 / 1048576.0
        );

        // ---- Compaction work: trie writes avoided ---------------------------
        // Delete half, then compact; report live records relocated (= dense
        // offset writes) and trie value-slot writes avoided vs Phase 1 (= same
        // count, since Phase 1 rewrites one trie slot per live record).
        for k in 0..(N / 2) {
            s1.remove(k);
        }
        let moved = s1.compact().unwrap();
        eprintln!(
            "compaction @50%-delete: relocated {moved} live records => {moved} dense offset[] writes, {moved} random-access trie value-slot writes AVOIDED vs Phase 1",
        );

        // ---- Inverted index cardinality + memory ----------------------------
        // Low-cardinality tenant/status + high-cardinality ts.
        let mut idx = InvertedIndex::new(12); // 4096-wide ts buckets
        let mut rng = XorShift(0x5EED);
        for k in 0..N {
            let tenant = (rng.next() % 64) as u32; // 64 tenants
            let status = (rng.next() % 4) as u32; // 4 statuses
            let ts = (rng.next() % 2_000_000) as u32; // ~2M distinct ts
            idx.insert(k, tenant, status, ts);
        }
        let t = idx.tenant_stats();
        let st = idx.status_stats();
        let tse = idx.ts_exact_stats();
        let tsb = idx.ts_bucketed_stats();
        eprintln!("\n-- inverted index @N={N} --");
        eprintln!(
            "tenant (64 vals): lists={} postings={} mem={:.2} MiB",
            t.posting_lists,
            t.total_postings,
            t.mem_bytes as f64 / 1048576.0
        );
        eprintln!(
            "status (4 vals):  lists={} postings={} mem={:.2} MiB",
            st.posting_lists,
            st.total_postings,
            st.mem_bytes as f64 / 1048576.0
        );
        eprintln!(
            "ts EXACT:    lists={} postings={} mem={:.2} MiB  <- high-cardinality weak point",
            tse.posting_lists,
            tse.total_postings,
            tse.mem_bytes as f64 / 1048576.0
        );
        eprintln!(
            "ts BUCKETED(2^12): lists={} postings={} mem={:.2} MiB  <- {:.0}x fewer lists",
            tsb.posting_lists,
            tsb.total_postings,
            tsb.mem_bytes as f64 / 1048576.0,
            tse.posting_lists as f64 / tsb.posting_lists.max(1) as f64
        );
        // A representative intersection cardinality.
        let c = idx.count_tenant_status(3, 1);
        eprintln!("count(tenant=3 AND status=1) = {c} (via intersection_len, no materialization)");
        eprintln!("=== end characterization ===\n");
    }
}
