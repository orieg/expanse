//! Chunked slab/arena allocator and high-level blob map.
//!
//! Stores variable-length byte payloads associated with 64-bit keys. Small
//! payloads (up to 7 bytes) are stored directly inside 64-bit value slots
//! ([`crate::slot::ValueSlot`]) with zero heap allocation. Larger payloads
//! are bump-allocated in contiguous 16-byte aligned slabs managed by [`BlobArena`].
//!
//! # Capacity limits
//!
//! Arena payloads are addressed by one of two value-slot encodings, chosen per
//! blob by where it lands:
//!
//! * **`ArenaShort`** packs a **24-bit** global byte offset alongside 32-bit
//!   hot metadata. It addresses the first **16 MiB** (`0x0100_0000`) of arena.
//! * **`ArenaLong`** packs a **16-bit chunk id** (the arena chunk index) plus a
//!   **40-bit intra-chunk offset**. Once a blob's global offset would cross the
//!   24-bit `ArenaShort` ceiling, [`BlobArena::alloc_blob`] emits an `ArenaLong`
//!   locator instead, so the live arena can grow past 16 MiB.
//!
//! The encoding ceiling is therefore `65536 * chunk_size` (16-bit chunk id ×
//! `chunk_size`), e.g. 128 GiB at the default 2 MiB chunk. A shipped safety cap
//! ([`MAX_ARENA_CAPACITY`], 1 GiB) bounds actual arena growth and the aggregate
//! capacity a loaded image may declare; [`BlobArena::alloc_blob`] returns
//! [`ArenaError::OffsetOverflow`] once growth would cross that cap or the 16-bit
//! chunk-id space ([`MAX_ARENA_CHUNKS`]). A single payload must still fit in one
//! chunk, so its length is bounded by `chunk_size - 8` (each record carries an
//! 8-byte [`BlobRecordHeader`]). The `External` slot encoding remains reserved.
//!
//! # Inline / `ArenaLong` metadata
//!
//! Two slot encodings do not carry the 32-bit hot-metadata word, so `insert`'s
//! `hot_meta` argument is ignored for them and `get`/`scan_filtered` report
//! their metadata as `0`:
//!
//! * **Inline** payloads (`<= 7` bytes): bits `63:32` hold payload bytes.
//! * **`ArenaLong`** payloads: all 56 non-tag bits address the chunk/offset,
//!   leaving no room for metadata. A blob that resides in (or relocates into,
//!   during compaction) an `ArenaLong` slot therefore loses any hot metadata it
//!   would have carried as `ArenaShort`.

use crate::map::ExpanseMap;
use crate::slot::{SlotTag, ValueSlot};
use crate::types::Key;
use core::ptr::NonNull;

/// Packed 8-byte record header preceding every arena payload.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlobRecordHeader {
    /// Payload length in bytes. Bounded in practice by `chunk_size - 8`
    /// (a payload must fit in one chunk); the `u32` width is not the limit.
    pub len: u32,
    /// Generation counter for ABA protection and compaction validation.
    pub generation: u32,
}

/// Magic identifier for Expanse binary image files ("EXPANSE\0").
pub const EXPANSE_MAGIC: [u8; 8] = *b"EXPANSE\0";
/// Current format version for relocatable ExpanseBlobMap images.
pub const EXPANSE_FORMAT_VERSION: u32 = 1;

/// Relocatable 64-byte binary image file header.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlobMapFileHeader {
    /// Magic string `EXPANSE\0`.
    pub magic: [u8; 8],
    /// Format version (`1`).
    pub version: u32,
    /// Format flags (reserved, 0).
    pub flags: u32,
    /// Total number of entries in the index.
    pub entry_count: u64,
    /// Byte offset where the index/entries section begins.
    pub index_offset: u64,
    /// Byte offset where the arena slab section begins.
    pub arena_offset: u64,
    /// Total file/image size in bytes.
    pub total_size: u64,
    /// Chunk size used by the arena.
    pub chunk_size: u64,
    /// Number of arena chunks.
    pub chunk_count: u64,
}

/// A typed view of a retrieved value payload.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlobView<'a> {
    /// Inlined value (<= 7 bytes) borrowing directly from leaf value slot memory.
    Inline(&'a [u8]),
    /// Arena-allocated value borrowing directly from an arena slab.
    Arena(&'a [u8]),
}

impl<'a> BlobView<'a> {
    /// Returns the underlying byte slice.
    #[inline(always)]
    #[must_use]
    pub fn as_bytes(&self) -> &'a [u8] {
        match self {
            BlobView::Inline(slice) => slice,
            BlobView::Arena(slice) => slice,
        }
    }

    /// Returns the length of the payload in bytes.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns `true` if the payload is empty (0 bytes).
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if the payload is stored inline in the value slot.
    #[inline(always)]
    #[must_use]
    pub fn is_inline(&self) -> bool {
        matches!(self, BlobView::Inline(_))
    }

    /// Returns `true` if the payload is stored in the slab arena.
    #[inline(always)]
    #[must_use]
    pub fn is_arena(&self) -> bool {
        matches!(self, BlobView::Arena(_))
    }
}

impl<'a> core::ops::Deref for BlobView<'a> {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl<'a> AsRef<[u8]> for BlobView<'a> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'a> PartialEq<[u8]> for BlobView<'a> {
    #[inline(always)]
    fn eq(&self, other: &[u8]) -> bool {
        self.as_bytes() == other
    }
}

impl<'a> PartialEq<BlobView<'a>> for [u8] {
    #[inline(always)]
    fn eq(&self, other: &BlobView<'a>) -> bool {
        self == other.as_bytes()
    }
}

/// Error conditions during blob arena operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaError {
    /// Arena memory allocation failed.
    AllocationFailed,
    /// Arena growth would exceed the addressable/allowed ceiling: the shipped
    /// safety cap ([`MAX_ARENA_CAPACITY`]) or the 16-bit chunk-id space
    /// ([`MAX_ARENA_CHUNKS`]), or a locator could not be encoded.
    OffsetOverflow,
    /// Invalid arena offset was provided.
    InvalidOffset,
    /// Blob generation mismatch (ABA detected).
    GenerationMismatch,
    /// Corrupted record header encountered.
    CorruptedHeader,
}

impl core::fmt::Display for ArenaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "Arena memory allocation failed"),
            Self::OffsetOverflow => {
                write!(f, "Arena growth exceeded the addressable/allowed ceiling")
            }
            Self::InvalidOffset => write!(f, "Invalid arena offset"),
            Self::GenerationMismatch => write!(f, "Blob generation mismatch (ABA detected)"),
            Self::CorruptedHeader => write!(f, "Corrupted blob record header"),
        }
    }
}

impl std::error::Error for ArenaError {}

/// Summary statistics from an in-place arena garbage collection and compaction run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompactionStats {
    /// Active live payload bytes before compaction.
    pub live_bytes_before: usize,
    /// Active live payload bytes after compaction.
    pub live_bytes_after: usize,
    /// Total arena capacity allocated before compaction.
    pub total_allocated_before: usize,
    /// Total arena capacity allocated after compaction.
    pub total_allocated_after: usize,
    /// Number of chunks before compaction.
    pub chunks_before: usize,
    /// Number of chunks after compaction.
    pub chunks_after: usize,
    /// Number of live records relocated.
    pub live_records_moved: usize,
}

/// A single contiguous 16-byte aligned bump-allocated slab chunk.
pub struct ArenaChunk {
    ptr: NonNull<u8>,
    capacity: usize,
    cursor: usize,
    generation: u32,
}

impl ArenaChunk {
    /// Maximum allowed chunk capacity (1 GiB) to prevent corrupted images from causing OOM.
    pub const MAX_CHUNK_CAPACITY: usize = 1024 * 1024 * 1024;

    /// Creates a new arena chunk of given capacity and initial generation.
    pub fn new(capacity: usize, generation: u32) -> Result<Self, ArenaError> {
        if capacity == 0 || capacity > Self::MAX_CHUNK_CAPACITY {
            return Err(ArenaError::AllocationFailed);
        }
        let layout = std::alloc::Layout::from_size_align(capacity, 16)
            .map_err(|_| ArenaError::AllocationFailed)?;
        // SAFETY: Allocating memory with 16-byte alignment.
        let raw = unsafe { std::alloc::alloc_zeroed(layout) };
        let ptr = NonNull::new(raw).ok_or(ArenaError::AllocationFailed)?;
        Ok(Self {
            ptr,
            capacity,
            cursor: 0,
            generation,
        })
    }

    /// Returns the capacity of this chunk in bytes.
    #[inline(always)]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the current allocation cursor within this chunk.
    #[inline(always)]
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns remaining unused bytes in this chunk.
    #[inline(always)]
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.cursor)
    }

    /// Returns `true` if a payload of `data_len` bytes fits in this chunk.
    #[inline(always)]
    #[must_use]
    pub fn can_fit(&self, data_len: usize) -> bool {
        let needed = 8 + data_len;
        self.cursor + needed <= self.capacity
    }

    /// Allocates a record in this chunk, returning the byte offset of the header.
    pub fn alloc(&mut self, data: &[u8]) -> Result<usize, ArenaError> {
        let needed = 8 + data.len();
        if self.cursor + needed > self.capacity {
            return Err(ArenaError::AllocationFailed);
        }
        let record_offset = self.cursor;
        let header = BlobRecordHeader {
            len: data.len() as u32,
            generation: self.generation,
        };
        // SAFETY: record_offset + needed <= capacity, pointer is valid and memory is owned.
        unsafe {
            let base = self.ptr.as_ptr().add(record_offset);
            core::ptr::write_unaligned(base.cast::<BlobRecordHeader>(), header);
            if !data.is_empty() {
                core::ptr::copy_nonoverlapping(data.as_ptr(), base.add(8), data.len());
            }
        }
        let next_cursor = record_offset + needed;
        // Align to 16 bytes for next record
        self.cursor = (next_cursor + 15) & !15;
        Ok(record_offset)
    }

    /// Reads payload slice from offset within chunk.
    #[must_use]
    pub fn get_slice(&self, offset_in_chunk: usize) -> Option<&[u8]> {
        if offset_in_chunk.checked_add(8)? > self.cursor {
            return None;
        }
        // SAFETY: offset_in_chunk + 8 <= cursor <= capacity, valid allocated memory.
        unsafe {
            let base = self.ptr.as_ptr().add(offset_in_chunk);
            let header = core::ptr::read_unaligned(base.cast::<BlobRecordHeader>());
            if header.generation != self.generation {
                return None;
            }
            let len = header.len as usize;
            if offset_in_chunk.checked_add(8)?.checked_add(len)? > self.cursor {
                return None;
            }
            Some(core::slice::from_raw_parts(base.add(8), len))
        }
    }

    /// Returns the generation counter.
    #[inline(always)]
    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Returns the live bump-allocated slice of this chunk.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        if self.cursor == 0 {
            &[]
        } else {
            // SAFETY: cursor <= capacity, ptr is allocated and valid.
            unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.cursor) }
        }
    }

    /// Returns the raw allocated slice up to the cursor.
    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        self.as_bytes()
    }

    /// Creates an arena chunk pre-populated from a raw data slice.
    pub fn from_raw_parts(
        capacity: usize,
        cursor: usize,
        generation: u32,
        data: &[u8],
    ) -> Result<Self, ArenaError> {
        if capacity == 0
            || capacity > Self::MAX_CHUNK_CAPACITY
            || cursor > capacity
            || data.len() > capacity
        {
            return Err(ArenaError::InvalidOffset);
        }
        let mut chunk = Self::new(capacity, generation)?;
        if !data.is_empty() {
            // SAFETY: destination chunk.ptr has at least capacity bytes, data is valid.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    chunk.ptr.as_ptr(),
                    data.len().min(cursor),
                );
            }
        }
        chunk.cursor = cursor;
        Ok(chunk)
    }
}

impl Drop for ArenaChunk {
    fn drop(&mut self) {
        let layout = std::alloc::Layout::from_size_align(self.capacity, 16).unwrap();
        // SAFETY: self.ptr was allocated with this exact layout.
        unsafe {
            std::alloc::dealloc(self.ptr.as_ptr(), layout);
        }
    }
}

// SAFETY: ArenaChunk exclusively owns its heap allocation.
unsafe impl Send for ArenaChunk {}
// SAFETY: ArenaChunk memory is immutable across concurrent threads unless uniquely borrowed.
unsafe impl Sync for ArenaChunk {}

/// Default chunk capacity: 2 MiB.
pub const DEFAULT_CHUNK_SIZE: usize = 2 * 1024 * 1024;

/// Global-offset ceiling of the 24-bit `ArenaShort` locator (16 MiB). Blobs
/// whose global byte offset is at or beyond this value are addressed with an
/// `ArenaLong` slot ((chunk id, intra-chunk offset)) instead.
pub const ARENA_SHORT_CEILING: usize = (ValueSlot::ARENA_OFFSET_MASK as usize) + 1;

/// Maximum number of arena chunks addressable by a 16-bit `ArenaLong` chunk id
/// (`2^16`). The encoding ceiling of the arena is `MAX_ARENA_CHUNKS *
/// chunk_size`.
pub const MAX_ARENA_CHUNKS: usize = 1 << 16;

/// Shipped safety cap on total arena capacity (**1 GiB**).
///
/// The `ArenaLong` encoding could address `MAX_ARENA_CHUNKS * chunk_size`
/// (128 GiB at the default 2 MiB chunk), but growth is bounded to this cap so a
/// runaway workload — or a crafted image declaring a huge `chunk_count *
/// chunk_size` — cannot drive an unbounded `alloc_zeroed`. 1 GiB is 64× the old
/// 16 MiB `ArenaShort` ceiling and comfortably exceeds any single-socket
/// last-level cache, which is what the RFC §10.3 cold-DRAM predicate-scan
/// regime requires. Raise this constant to lift the shipped cap toward the
/// encoding ceiling.
pub const MAX_ARENA_CAPACITY: usize = 1 << 30;

/// Where a freshly allocated blob landed, and therefore which value-slot
/// encoding addresses it. Returned by [`BlobArena::alloc_blob`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobLoc {
    /// The global byte offset fits the 24-bit `ArenaShort` locator (< 16 MiB).
    Short(u32),
    /// Beyond 16 MiB: addressed by chunk id + intra-chunk byte offset via an
    /// `ArenaLong` slot.
    Long {
        /// Arena chunk index (the 16-bit `ArenaLong` chunk id).
        chunk_id: u16,
        /// Byte offset of the record header within that chunk (40-bit field).
        offset_in_chunk: u64,
    },
}

/// Builds the arena-backed [`ValueSlot`] addressing `loc`, attaching `hot_meta`
/// only to the `ArenaShort` encoding (`ArenaLong` has no metadata field).
#[inline]
fn slot_from_loc(loc: BlobLoc, hot_meta: u32) -> Result<ValueSlot, ArenaError> {
    match loc {
        BlobLoc::Short(offset) => {
            ValueSlot::new_arena_short(hot_meta, offset).ok_or(ArenaError::OffsetOverflow)
        }
        BlobLoc::Long {
            chunk_id,
            offset_in_chunk,
        } => ValueSlot::new_arena_long(chunk_id, offset_in_chunk).ok_or(ArenaError::OffsetOverflow),
    }
}

/// Chunked slab allocator for variable-length payload storage.
pub struct BlobArena {
    chunks: Vec<ArenaChunk>,
    active_chunk: Option<usize>,
    chunk_size: usize,
    total_allocated: usize,
    live_bytes: usize,
    /// Current generation, stamped into every chunk allocated by this arena
    /// and bumped on each [`compact_with_index`](Self::compact_with_index) so
    /// that an arena offset held across a compaction fails the generation
    /// check in [`ArenaChunk::get_slice`] instead of resolving to unrelated
    /// bytes. Serialized per chunk so save/load preserves it.
    generation: u32,
    /// Total-capacity ceiling in bytes. Allocating a new chunk fails with
    /// [`ArenaError::OffsetOverflow`] once `total_allocated + chunk_size` would
    /// cross it. Defaults to [`MAX_ARENA_CAPACITY`]; a compaction inherits the
    /// source arena's cap. Not serialized (it is a growth policy, not data).
    max_capacity: usize,
}

impl BlobArena {
    /// Creates a new `BlobArena` with the specified chunk size.
    ///
    /// `chunk_size` is clamped into `[4096, ArenaChunk::MAX_CHUNK_CAPACITY]`
    /// (1 GiB upper bound): a chunk larger than a chunk allocation can ever be
    /// would make every arena insert fail.
    #[must_use]
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunks: Vec::new(),
            active_chunk: None,
            chunk_size: chunk_size.clamp(4096, ArenaChunk::MAX_CHUNK_CAPACITY),
            total_allocated: 0,
            live_bytes: 0,
            generation: 1,
            max_capacity: MAX_ARENA_CAPACITY,
        }
    }

    /// Returns the arena's current generation counter.
    #[inline(always)]
    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Classifies where a record at `(chunk index, intra-chunk offset)` sits and
    /// picks the value-slot encoding: `ArenaShort` while its flat global offset
    /// still fits the 24-bit locator, `ArenaLong` once it crosses 16 MiB.
    ///
    /// The flat global offset `idx * chunk_size + offset_in_chunk` is the same
    /// address [`Self::get_blob_slice`] recovers by division, so the two paths
    /// resolve to the identical chunk/offset regardless of which encoding a
    /// given blob uses.
    #[inline]
    fn classify_loc(&self, idx: usize, offset_in_chunk: usize) -> BlobLoc {
        let global = idx * self.chunk_size + offset_in_chunk;
        if global <= (ValueSlot::ARENA_OFFSET_MASK as usize) {
            BlobLoc::Short(global as u32)
        } else {
            BlobLoc::Long {
                chunk_id: idx as u16,
                offset_in_chunk: offset_in_chunk as u64,
            }
        }
    }

    /// Allocates a blob payload in the arena, returning its [`BlobLoc`] (the
    /// caller turns it into an `ArenaShort` or `ArenaLong` [`ValueSlot`]).
    ///
    /// Fails with [`ArenaError::OffsetOverflow`] once growing the arena would
    /// cross the 16-bit chunk-id space ([`MAX_ARENA_CHUNKS`]) or the shipped
    /// [`MAX_ARENA_CAPACITY`] safety cap, and with [`ArenaError::AllocationFailed`]
    /// if a single record cannot fit one chunk (`8 + data.len() > chunk_size`).
    pub fn alloc_blob(&mut self, data: &[u8]) -> Result<BlobLoc, ArenaError> {
        let needed = 8 + data.len();
        if needed > self.chunk_size {
            return Err(ArenaError::AllocationFailed);
        }

        if let Some(idx) = self.active_chunk {
            if self.chunks[idx].can_fit(data.len()) {
                let offset_in_chunk = self.chunks[idx].alloc(data)?;
                self.live_bytes += needed;
                return Ok(self.classify_loc(idx, offset_in_chunk));
            }
        }

        // A new chunk is required — enforce the chunk-id space and the total
        // capacity cap before allocating anything.
        let idx = self.chunks.len();
        if idx >= MAX_ARENA_CHUNKS {
            return Err(ArenaError::OffsetOverflow);
        }
        if self.total_allocated.saturating_add(self.chunk_size) > self.max_capacity {
            return Err(ArenaError::OffsetOverflow);
        }

        // Allocate a new chunk stamped with the arena's current generation.
        let mut new_chunk = ArenaChunk::new(self.chunk_size, self.generation)?;
        let offset_in_chunk = new_chunk.alloc(data)?;
        self.chunks.push(new_chunk);
        self.total_allocated += self.chunk_size;
        self.active_chunk = Some(idx);
        self.live_bytes += needed;
        Ok(self.classify_loc(idx, offset_in_chunk))
    }

    /// Returns a slice of the blob payload at the given 24-bit `ArenaShort`
    /// global offset. The chunk is recovered by `offset / chunk_size`, so this
    /// path is only valid for offsets that fit the `ArenaShort` locator; use
    /// [`Self::get_blob_slice_long`] for `ArenaLong` locators.
    #[inline]
    #[must_use]
    pub fn get_blob_slice(&self, global_offset: u32) -> Option<&[u8]> {
        let offset = global_offset as usize;
        let chunk_idx = offset / self.chunk_size;
        let offset_in_chunk = offset % self.chunk_size;
        if chunk_idx < self.chunks.len() {
            self.chunks[chunk_idx].get_slice(offset_in_chunk)
        } else {
            None
        }
    }

    /// Returns a slice of the blob payload addressed by an `ArenaLong` locator
    /// `(chunk_id, offset_in_chunk)`. Returns `None` (never UB) for an
    /// out-of-range chunk id or offset, so a crafted image resolves cleanly.
    #[inline]
    #[must_use]
    pub fn get_blob_slice_long(&self, chunk_id: u16, offset_in_chunk: u64) -> Option<&[u8]> {
        let idx = chunk_id as usize;
        let offset = usize::try_from(offset_in_chunk).ok()?;
        self.chunks.get(idx)?.get_slice(offset)
    }

    /// Records that an `ArenaShort` blob at `global_offset` was deleted/overwritten.
    pub fn record_deleted(&mut self, global_offset: u32) {
        let offset = global_offset as usize;
        let chunk_idx = offset / self.chunk_size;
        let offset_in_chunk = offset % self.chunk_size;
        if chunk_idx < self.chunks.len() {
            if let Some(slice) = self.chunks[chunk_idx].get_slice(offset_in_chunk) {
                let needed = 8 + slice.len();
                self.live_bytes = self.live_bytes.saturating_sub(needed);
            }
        }
    }

    /// Records that an `ArenaLong` blob at `(chunk_id, offset_in_chunk)` was
    /// deleted/overwritten.
    pub fn record_deleted_long(&mut self, chunk_id: u16, offset_in_chunk: u64) {
        // Resolve the length and drop the borrow before mutating `live_bytes`.
        let len = self
            .get_blob_slice_long(chunk_id, offset_in_chunk)
            .map(<[u8]>::len);
        if let Some(len) = len {
            self.live_bytes = self.live_bytes.saturating_sub(8 + len);
        }
    }

    /// Records deletion for whichever arena encoding `slot` uses (no-op for
    /// inline / non-arena slots).
    pub fn record_deleted_slot(&mut self, slot: ValueSlot) {
        match slot.tag() {
            SlotTag::ArenaShort => self.record_deleted(slot.arena_offset()),
            SlotTag::ArenaLong => {
                let (chunk_id, offset_in_chunk) = slot.arena_long_loc();
                self.record_deleted_long(chunk_id, offset_in_chunk);
            }
            _ => {}
        }
    }

    /// In-place mark-compact GC consolidating live payloads into fresh chunk(s),
    /// updating `ValueSlot` arena offsets directly in the trie index, and freeing dead chunks.
    ///
    /// All-or-nothing: every live payload is relocated into a fresh arena
    /// *before* any index slot is rewritten. If any relocation fails (e.g.
    /// [`ArenaError::AllocationFailed`] / [`ArenaError::OffsetOverflow`]) the
    /// method returns `Err` with both `self` and `index` left untouched — the
    /// half-built new arena is dropped and no index slot points into it. The
    /// new arena's generation is bumped so any arena offset still held from
    /// before the compaction fails the [`ArenaChunk::get_slice`] generation
    /// check rather than resolving to relocated bytes.
    pub fn compact_with_index(
        &mut self,
        index: &mut ExpanseMap,
    ) -> Result<CompactionStats, ArenaError> {
        let live_bytes_before = self.live_bytes;
        let total_allocated_before = self.total_allocated;
        let chunks_before = self.chunks.len();

        let mut new_arena = BlobArena::new(self.chunk_size);
        // Inherit the source arena's capacity cap so the compacted arena is held
        // to the same ceiling.
        new_arena.max_capacity = self.max_capacity;
        // Bump the generation for the compacted arena so stale offsets fail
        // the generation check instead of aliasing relocated records. Skip 0
        // so zero-initialized (unwritten) arena bytes never match a live gen.
        new_arena.generation = {
            let g = self.generation.wrapping_add(1);
            if g == 0 { 1 } else { g }
        };

        // Collect every arena-backed entry (both `ArenaShort` and `ArenaLong`).
        let live_entries: Vec<(Key, ValueSlot)> = index
            .iter()
            .filter_map(|(key, raw_slot)| {
                let slot = ValueSlot::from_raw(raw_slot);
                matches!(slot.tag(), SlotTag::ArenaShort | SlotTag::ArenaLong)
                    .then_some((key, slot))
            })
            .collect();

        // Phase 1: relocate every live payload into the new arena, collecting
        // the (key, new raw slot) rewrites. A failure here returns before any
        // index slot is touched, so `self`/`index` stay consistent. A blob's
        // encoding is re-derived from its *new* location, so a record may switch
        // between `ArenaShort` and `ArenaLong` across a compaction; hot metadata
        // is preserved only on the `ArenaShort` side (`ArenaLong` has none).
        let mut rewrites: Vec<(Key, u64)> = Vec::with_capacity(live_entries.len());
        for (key, slot) in live_entries {
            let (payload, meta) = match slot.tag() {
                SlotTag::ArenaShort => (self.get_blob_slice(slot.arena_offset()), slot.hot_meta()),
                SlotTag::ArenaLong => {
                    let (chunk_id, offset_in_chunk) = slot.arena_long_loc();
                    (self.get_blob_slice_long(chunk_id, offset_in_chunk), 0u32)
                }
                // filter above admits only the two arena tags.
                _ => (None, 0u32),
            };
            if let Some(payload) = payload {
                let loc = new_arena.alloc_blob(payload)?;
                let new_slot = slot_from_loc(loc, meta)?;
                rewrites.push((key, new_slot.to_raw()));
            }
        }

        let live_records_moved = rewrites.len();

        // Phase 2: every relocation succeeded — apply the index rewrites.
        for (key, raw) in rewrites {
            if let Some(mut slot_ptr) = index.get_value_slot(key) {
                // SAFETY: slot_ptr points to the live slot of key in the index,
                // valid until the next structural mutation (none happens here).
                unsafe {
                    *slot_ptr.as_mut() = raw;
                }
            }
        }

        let live_bytes_after = new_arena.live_bytes;
        let total_allocated_after = new_arena.total_allocated;
        let chunks_after = new_arena.chunks.len();

        *self = new_arena;

        Ok(CompactionStats {
            live_bytes_before,
            live_bytes_after,
            total_allocated_before,
            total_allocated_after,
            chunks_before,
            chunks_after,
            live_records_moved,
        })
    }

    /// Total allocated heap bytes in arena chunks.
    #[inline(always)]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.total_allocated
    }

    /// Active live payload bytes.
    #[inline(always)]
    #[must_use]
    pub fn live_bytes(&self) -> usize {
        self.live_bytes
    }

    /// Number of allocated chunks.
    #[inline(always)]
    #[must_use]
    pub fn chunks_count(&self) -> usize {
        self.chunks.len()
    }

    /// Returns a slice of the arena chunks.
    #[inline(always)]
    #[must_use]
    pub fn chunks(&self) -> &[ArenaChunk] {
        &self.chunks
    }

    /// Returns the chunk capacity in bytes.
    #[inline(always)]
    #[must_use]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Appends a pre-populated chunk to the arena.
    pub fn push_chunk(&mut self, chunk: ArenaChunk) {
        self.total_allocated += chunk.capacity();
        self.chunks.push(chunk);
        self.active_chunk = Some(self.chunks.len() - 1);
    }

    /// Resets and frees all arena chunks.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.active_chunk = None;
        self.total_allocated = 0;
        self.live_bytes = 0;
    }
}

/// A high-level map from 64-bit keys to arbitrary-length byte blobs backed by
/// inline value slots and chunked arena slabs.
pub struct ExpanseBlobMap {
    index: ExpanseMap,
    arena: BlobArena,
}

impl ExpanseBlobMap {
    /// Creates an empty blob map with default 2 MiB arena chunk slabs.
    #[must_use]
    pub fn new() -> Self {
        Self::with_chunk_size(DEFAULT_CHUNK_SIZE)
    }

    /// Creates an empty blob map with custom arena chunk size.
    ///
    /// `chunk_size` is clamped into `[4096, ArenaChunk::MAX_CHUNK_CAPACITY]`
    /// (see [`BlobArena::new`]); a value above the 1 GiB chunk cap would
    /// otherwise yield a map in which every arena insert fails.
    #[must_use]
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            index: ExpanseMap::new(),
            arena: BlobArena::new(chunk_size),
        }
    }

    /// Number of entries in the blob map.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.index.len()
    }

    /// Returns `true` if the map contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Returns `true` if the map contains the specified key.
    #[must_use]
    pub fn contains_key(&self, key: Key) -> bool {
        self.index.contains_key(key)
    }

    /// Total heap memory used by the index and the blob arena.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.index.mem_used() + self.arena.mem_used()
    }

    /// Returns a reference to the backing blob arena.
    #[must_use]
    pub fn arena(&self) -> &BlobArena {
        &self.arena
    }

    /// Returns a mutable reference to the backing blob arena.
    pub fn arena_mut(&mut self) -> &mut BlobArena {
        &mut self.arena
    }

    /// Inserts a key-blob pair with 32-bit hot metadata.
    ///
    /// Payloads `<= 7 bytes` are stored inline with zero heap allocation.
    /// Payloads `> 7 bytes` are allocated in the slab arena.
    ///
    /// Note: inline (`<= 7` byte) payloads and `ArenaLong` payloads (those past
    /// the 16 MiB `ArenaShort` range) do not store `hot_meta` — their slot bits
    /// carry payload / locator instead — so `hot_meta` is ignored for them and
    /// later reads report their metadata as `0`.
    pub fn insert(&mut self, key: Key, data: &[u8], hot_meta: u32) -> Result<(), ArenaError> {
        let old_slot = self.index.get(key).map(ValueSlot::from_raw);
        if data.len() <= 7 {
            let slot = ValueSlot::new_inline(data).ok_or(ArenaError::AllocationFailed)?;
            self.index.insert(key, slot.to_raw());
        } else {
            let loc = self.arena.alloc_blob(data)?;
            let slot = slot_from_loc(loc, hot_meta)?;
            self.index.insert(key, slot.to_raw());
        }
        if let Some(old) = old_slot {
            self.arena.record_deleted_slot(old);
        }
        Ok(())
    }

    /// Point lookup returning a zero-copy [`BlobView`] and the 32-bit hot metadata word.
    ///
    /// Inline (`<= 7` byte) payloads do not store metadata; their returned
    /// `hot_meta` is always `0`.
    #[must_use]
    pub fn get<'a>(&'a self, key: Key) -> Option<(BlobView<'a>, u32)> {
        let slot_ptr = self.index.get_slot_ptr(key)?;
        // SAFETY: slot_ptr points to the live 64-bit value slot inside self.index.
        let raw_slot = unsafe { *slot_ptr.as_ptr() };
        let slot = ValueSlot::from_raw(raw_slot);
        match slot.tag() {
            SlotTag::Inline0
            | SlotTag::Inline1
            | SlotTag::Inline2
            | SlotTag::Inline3
            | SlotTag::Inline4
            | SlotTag::Inline5
            | SlotTag::Inline6
            | SlotTag::Inline7 => {
                let len = slot.tag() as u8 as usize;
                // SAFETY: In little-endian representation, byte offsets 1..=len contain
                // the inline payload bytes. The slot memory is owned by self.index and
                // valid for lifetime 'a.
                let slice = unsafe {
                    let bytes = slot_ptr.as_ptr().cast::<u8>();
                    core::slice::from_raw_parts(bytes.add(1), len)
                };
                Some((BlobView::Inline(slice), 0))
            }
            SlotTag::ArenaShort => {
                let offset = slot.arena_offset();
                let meta = slot.hot_meta();
                let slice = self.arena.get_blob_slice(offset)?;
                Some((BlobView::Arena(slice), meta))
            }
            SlotTag::ArenaLong => {
                let (chunk_id, offset_in_chunk) = slot.arena_long_loc();
                let slice = self.arena.get_blob_slice_long(chunk_id, offset_in_chunk)?;
                // `ArenaLong` carries no hot-metadata word; report 0.
                Some((BlobView::Arena(slice), 0))
            }
            _ => None,
        }
    }

    /// Removes a key from the map, returning `true` if the key was present.
    pub fn remove(&mut self, key: Key) -> bool {
        if let Some(raw_val) = self.index.remove(key) {
            self.arena.record_deleted_slot(ValueSlot::from_raw(raw_val));
            true
        } else {
            false
        }
    }

    /// Executes a range scan with a predicate evaluated against hot metadata
    /// before dereferencing cold payload cache lines.
    ///
    /// Inline (`<= 7` byte) payloads have no stored metadata; the predicate and
    /// callback see `hot_meta == 0` for them.
    pub fn scan_filtered<P, F>(
        &self,
        range: core::ops::RangeInclusive<Key>,
        mut predicate: P,
        mut callback: F,
    ) where
        P: FnMut(Key, u32) -> bool,
        F: FnMut(Key, BlobView<'_>, u32) -> bool,
    {
        for (key, raw_slot) in self.index.range(range) {
            let slot = ValueSlot::from_raw(raw_slot);
            // Inline slots store payload bytes in bits 63:32, not metadata;
            // only arena slots carry a real hot-metadata word. Reading
            // `hot_meta()` on an inline slot would feed payload garbage to the
            // predicate, so report inline metadata as 0 (mirrors blobmap32).
            let meta = if slot.tag() == SlotTag::ArenaShort {
                slot.hot_meta()
            } else {
                0
            };
            if predicate(key, meta) {
                if let Some((view, _)) = self.get(key) {
                    if !callback(key, view, meta) {
                        break;
                    }
                }
            }
        }
    }

    /// Runs in-place garbage collection and compaction.
    pub fn compact(&mut self) -> Result<CompactionStats, ArenaError> {
        self.arena.compact_with_index(&mut self.index)
    }

    /// Returns a reference to the internal index.
    #[must_use]
    pub fn index(&self) -> &ExpanseMap {
        &self.index
    }

    /// Serializes the blob map to a writer in relocatable binary image format.
    pub fn save_to_writer<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<usize> {
        let entry_count = self.index.len();
        let index_offset = 64u64;
        let index_size = entry_count * 16;
        let arena_offset = index_offset + index_size;

        let chunk_count = self.arena.chunks.len() as u64;
        let mut total_arena_size = 0u64;
        for chunk in &self.arena.chunks {
            let cursor = chunk.cursor();
            let aligned_cursor = (cursor + 15) & !15;
            total_arena_size += 24 + aligned_cursor as u64;
        }

        let total_size = arena_offset + total_arena_size;

        // Serialize the 64-byte header field-by-field in explicit
        // little-endian, matching the field order/offsets in
        // `BlobMapFileHeader` and the field-by-field parse in
        // `from_bytes_slice` (portable, endianness-independent):
        //   magic[8] | version(u32) | flags(u32) | entry_count(u64)
        //   | index_offset(u64) | arena_offset(u64) | total_size(u64)
        //   | chunk_size(u64) | chunk_count(u64) = 64 bytes.
        writer.write_all(&EXPANSE_MAGIC)?;
        writer.write_all(&EXPANSE_FORMAT_VERSION.to_le_bytes())?;
        writer.write_all(&0u32.to_le_bytes())?; // flags (reserved)
        writer.write_all(&entry_count.to_le_bytes())?;
        writer.write_all(&index_offset.to_le_bytes())?;
        writer.write_all(&arena_offset.to_le_bytes())?;
        writer.write_all(&total_size.to_le_bytes())?;
        writer.write_all(&(self.arena.chunk_size as u64).to_le_bytes())?;
        writer.write_all(&chunk_count.to_le_bytes())?;

        // Write index entries (key: u64, raw_slot: u64)
        for (key, raw_slot) in self.index.iter() {
            writer.write_all(&key.to_le_bytes())?;
            writer.write_all(&raw_slot.to_le_bytes())?;
        }

        // Write arena chunks
        for chunk in &self.arena.chunks {
            let cap = chunk.capacity() as u64;
            let cur = chunk.cursor() as u64;
            let generation = chunk.generation;
            writer.write_all(&cap.to_le_bytes())?;
            writer.write_all(&cur.to_le_bytes())?;
            writer.write_all(&generation.to_le_bytes())?;
            writer.write_all(&0u32.to_le_bytes())?; // 4-byte padding

            let chunk_data = chunk.raw_bytes();
            writer.write_all(chunk_data)?;
            let pad_len = ((cur as usize + 15) & !15) - cur as usize;
            if pad_len > 0 {
                writer.write_all(&[0u8; 16][..pad_len])?;
            }
        }

        Ok(total_size as usize)
    }

    /// Saves the blob map to a file at the given path.
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<usize> {
        let mut file = std::fs::File::create(path)?;
        self.save_to_writer(&mut file)
    }

    /// Deserializes a relocatable binary image from a byte slice.
    pub fn from_bytes_slice(bytes: &[u8]) -> Result<Self, ArenaError> {
        if bytes.len() < 64 {
            return Err(ArenaError::CorruptedHeader);
        }

        // Parse the 64-byte header field-by-field in explicit little-endian,
        // mirroring `save_to_writer` (portable; no unaligned struct cast).
        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[0..8]);
        let header = BlobMapFileHeader {
            magic,
            version: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            flags: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            entry_count: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            index_offset: u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
            arena_offset: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
            total_size: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            chunk_size: u64::from_le_bytes(bytes[48..56].try_into().unwrap()),
            chunk_count: u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
        };

        if header.magic != EXPANSE_MAGIC || header.version != EXPANSE_FORMAT_VERSION {
            return Err(ArenaError::CorruptedHeader);
        }

        if header.total_size as usize > bytes.len() || (header.total_size as usize) < 64 {
            return Err(ArenaError::CorruptedHeader);
        }

        // A valid image is always saved from an arena whose chunk size was
        // clamped into [4096, MAX_CHUNK_CAPACITY]; reject anything outside
        // that range so the clamp in `with_chunk_size` can never disagree with
        // the per-chunk `cap` (which would corrupt `get_blob_slice` indexing).
        if header.chunk_size < 4096 || header.chunk_size > ArenaChunk::MAX_CHUNK_CAPACITY as u64 {
            return Err(ArenaError::CorruptedHeader);
        }

        // The 16-bit `ArenaLong` chunk id can address at most `MAX_ARENA_CHUNKS`
        // chunks; a larger count is unrepresentable and rejected.
        if header.chunk_count > MAX_ARENA_CHUNKS as u64 {
            return Err(ArenaError::CorruptedHeader);
        }

        // Bound the aggregate declared arena capacity to the shipped safety cap
        // (`MAX_ARENA_CAPACITY`). A small crafted header could otherwise declare
        // a huge `chunk_count * chunk_size` and drive `alloc_zeroed` into an
        // OOM/DoS; a legitimately saved arena never crosses this ceiling.
        let declared_capacity = header
            .chunk_count
            .checked_mul(header.chunk_size)
            .ok_or(ArenaError::CorruptedHeader)?;
        if declared_capacity > MAX_ARENA_CAPACITY as u64 {
            return Err(ArenaError::CorruptedHeader);
        }

        if header.chunk_count > (bytes.len() / 24) as u64 {
            return Err(ArenaError::CorruptedHeader);
        }

        if header.entry_count > (bytes.len() / 16) as u64 {
            return Err(ArenaError::CorruptedHeader);
        }

        if (header.arena_offset as usize) > bytes.len()
            || (header.index_offset as usize) > bytes.len()
        {
            return Err(ArenaError::CorruptedHeader);
        }

        let mut map = Self::with_chunk_size(header.chunk_size as usize);
        // Track the generation stamped on loaded chunks so future allocs and
        // compactions continue from a consistent value.
        let mut loaded_generation: Option<u32> = None;

        // Read arena chunks
        let mut arena_pos = header.arena_offset as usize;
        for _ in 0..header.chunk_count {
            let chunk_header_bytes = bytes
                .get(
                    arena_pos
                        ..arena_pos
                            .checked_add(24)
                            .ok_or(ArenaError::CorruptedHeader)?,
                )
                .ok_or(ArenaError::CorruptedHeader)?;
            let cap = u64::from_le_bytes(chunk_header_bytes[0..8].try_into().unwrap()) as usize;
            let cur = u64::from_le_bytes(chunk_header_bytes[8..16].try_into().unwrap()) as usize;
            let generation = u32::from_le_bytes(chunk_header_bytes[16..20].try_into().unwrap());
            arena_pos = arena_pos
                .checked_add(24)
                .ok_or(ArenaError::CorruptedHeader)?;

            // Every chunk in a valid image has `cap == chunk_size`;
            // `get_blob_slice` maps a global offset to a chunk via
            // `offset / chunk_size`, so a non-uniform `cap` would silently
            // point at the wrong chunk. Reject it.
            if cap == 0
                || cap > ArenaChunk::MAX_CHUNK_CAPACITY
                || cap != header.chunk_size as usize
                || cur > cap
            {
                return Err(ArenaError::CorruptedHeader);
            }

            let chunk_end = arena_pos
                .checked_add(cur)
                .ok_or(ArenaError::CorruptedHeader)?;
            let chunk_data = bytes
                .get(arena_pos..chunk_end)
                .ok_or(ArenaError::CorruptedHeader)?;
            let chunk = ArenaChunk::from_raw_parts(cap, cur, generation, chunk_data)?;
            map.arena.push_chunk(chunk);
            loaded_generation = Some(generation);

            let aligned_cur = (cur.checked_add(15).ok_or(ArenaError::CorruptedHeader)?) & !15;
            arena_pos = arena_pos
                .checked_add(aligned_cur)
                .ok_or(ArenaError::CorruptedHeader)?;
        }

        // Adopt the loaded chunks' generation so later allocs/compactions
        // stay consistent with the records already in the arena.
        if let Some(g) = loaded_generation {
            map.arena.generation = g;
        }

        // Read index entries
        let mut idx_pos = header.index_offset as usize;
        for _ in 0..header.entry_count {
            let entry_bytes = bytes
                .get(idx_pos..idx_pos.checked_add(16).ok_or(ArenaError::CorruptedHeader)?)
                .ok_or(ArenaError::CorruptedHeader)?;
            let key = u64::from_le_bytes(entry_bytes[0..8].try_into().unwrap());
            let raw_slot = u64::from_le_bytes(entry_bytes[8..16].try_into().unwrap());
            idx_pos = idx_pos.checked_add(16).ok_or(ArenaError::CorruptedHeader)?;
            map.index.insert(key, raw_slot);

            // Recompute live_bytes for arena-backed slots (both encodings).
            let slot = ValueSlot::from_raw(raw_slot);
            let payload_len = match slot.tag() {
                SlotTag::ArenaShort => map
                    .arena
                    .get_blob_slice(slot.arena_offset())
                    .map(<[u8]>::len),
                SlotTag::ArenaLong => {
                    let (chunk_id, offset_in_chunk) = slot.arena_long_loc();
                    map.arena
                        .get_blob_slice_long(chunk_id, offset_in_chunk)
                        .map(<[u8]>::len)
                }
                _ => None,
            };
            if let Some(len) = payload_len {
                map.arena.live_bytes += 8 + len;
            }
        }

        Ok(map)
    }

    /// Loads a blob map from a binary image file at `path`.
    ///
    /// This reads the whole file into memory (`std::fs::read`) and rebuilds the
    /// index entry-by-entry — it is not a memory map, hence `load_from_file`
    /// rather than the former `mmap_file` name.
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ArenaError> {
        let bytes = std::fs::read(path).map_err(|_| ArenaError::CorruptedHeader)?;
        Self::from_bytes_slice(&bytes)
    }

    /// Removes all entries from the map and frees all arena slabs.
    pub fn clear(&mut self) {
        self.index.clear();
        self.arena.clear();
    }
}

impl Default for ExpanseBlobMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_inline_and_arena_blobs() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);

        // Inline 0..=7 bytes
        map.insert(10, b"", 0).unwrap();
        map.insert(11, b"a", 0).unwrap();
        map.insert(12, b"hello", 0).unwrap();
        map.insert(13, b"1234567", 0).unwrap();

        // Arena blobs (>= 8 bytes)
        map.insert(20, b"12345678", 100).unwrap();
        map.insert(21, b"a long blob that is stored in the slab arena!", 200)
            .unwrap();

        assert_eq!(map.len(), 6);

        let (v10, _) = map.get(10).unwrap();
        assert!(v10.is_inline());
        assert_eq!(v10.as_bytes(), b"");

        let (v11, _) = map.get(11).unwrap();
        assert!(v11.is_inline());
        assert_eq!(v11.as_bytes(), b"a");

        let (v12, _) = map.get(12).unwrap();
        assert!(v12.is_inline());
        assert_eq!(v12.as_bytes(), b"hello");

        let (v13, _) = map.get(13).unwrap();
        assert!(v13.is_inline());
        assert_eq!(v13.as_bytes(), b"1234567");

        let (v20, meta20) = map.get(20).unwrap();
        assert!(v20.is_arena());
        assert_eq!(v20.as_bytes(), b"12345678");
        assert_eq!(meta20, 100);

        let (v21, meta21) = map.get(21).unwrap();
        assert!(v21.is_arena());
        assert_eq!(
            v21.as_bytes(),
            b"a long blob that is stored in the slab arena!"
        );
        assert_eq!(meta21, 200);
    }

    #[test]
    fn scan_filtered_selects_correct_blobs() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        for i in 0..100u64 {
            let payload = format!("payload-data-value-{}", i);
            let hot_meta = (i * 10) as u32;
            map.insert(i, payload.as_bytes(), hot_meta).unwrap();
        }

        // Scan keys in 10..=30 with meta in 150..=250 (keys 15..=25)
        let mut seen = Vec::new();
        map.scan_filtered(
            10..=30,
            |_key, meta| (150..=250).contains(&meta),
            |key, view, meta| {
                seen.push((key, view.as_bytes().to_vec(), meta));
                true
            },
        );

        assert_eq!(seen.len(), 11);
        for (idx, &(k, _, m)) in seen.iter().enumerate() {
            let expected_key = 15 + idx as u64;
            assert_eq!(k, expected_key);
            assert_eq!(m, (expected_key * 10) as u32);
        }
    }

    #[test]
    fn compaction_reclaims_dead_space() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);

        // Insert 200 blobs
        for i in 0..200u64 {
            let payload = vec![0xAB; 256];
            map.insert(i, &payload, i as u32).unwrap();
        }

        let live_before = map.arena.live_bytes();

        // Delete 150 blobs (churn)
        for i in 0..150u64 {
            assert!(map.remove(i));
        }

        assert_eq!(map.len(), 50);
        let live_after_deletes = map.arena.live_bytes();
        assert!(live_after_deletes < live_before);

        // Run compaction
        let stats = map.compact().unwrap();
        assert_eq!(stats.live_records_moved, 50);

        // Verify remaining 50 blobs still intact
        for i in 150..200u64 {
            let (view, meta) = map.get(i).unwrap();
            assert_eq!(meta, i as u32);
            assert_eq!(view.len(), 256);
            assert_eq!(view.as_bytes(), &vec![0xAB; 256][..]);
        }
    }

    #[test]
    fn relocatable_binary_image_roundtrip() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);

        // Mix of inline (0..7 bytes) and arena payloads (>7 bytes)
        map.insert(1, b"", 0).unwrap();
        map.insert(2, b"a", 0).unwrap();
        map.insert(3, b"1234567", 0).unwrap();
        map.insert(4, b"12345678", 10).unwrap();
        map.insert(5, b"large payload in arena chunk memory", 20)
            .unwrap();
        map.insert(6, &vec![0x42; 1024], 30).unwrap();

        let mut buffer = Vec::new();
        let bytes_written = map.save_to_writer(&mut buffer).unwrap();
        assert_eq!(bytes_written, buffer.len());

        let restored = ExpanseBlobMap::from_bytes_slice(&buffer).unwrap();
        assert_eq!(restored.len(), 6);

        let (v1, _) = restored.get(1).unwrap();
        assert!(v1.is_inline());
        assert_eq!(v1.as_bytes(), b"");

        let (v2, _) = restored.get(2).unwrap();
        assert!(v2.is_inline());
        assert_eq!(v2.as_bytes(), b"a");

        let (v3, _) = restored.get(3).unwrap();
        assert!(v3.is_inline());
        assert_eq!(v3.as_bytes(), b"1234567");

        let (v4, m4) = restored.get(4).unwrap();
        assert!(v4.is_arena());
        assert_eq!(v4.as_bytes(), b"12345678");
        assert_eq!(m4, 10);

        let (v5, m5) = restored.get(5).unwrap();
        assert!(v5.is_arena());
        assert_eq!(v5.as_bytes(), b"large payload in arena chunk memory");
        assert_eq!(m5, 20);

        let (v6, m6) = restored.get(6).unwrap();
        assert!(v6.is_arena());
        assert_eq!(v6.as_bytes(), &vec![0x42; 1024][..]);
        assert_eq!(m6, 30);
    }

    #[test]
    #[cfg(not(miri))]
    fn load_from_file_save_and_load_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("expanse_test_load_from_file.bin");

        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        for i in 0..50u64 {
            let payload = format!("test-payload-record-{i}");
            map.insert(i * 10, payload.as_bytes(), i as u32).unwrap();
        }

        map.save_to_file(&path).unwrap();

        let loaded = ExpanseBlobMap::load_from_file(&path).unwrap();
        assert_eq!(loaded.len(), 50);

        for i in 0..50u64 {
            let (view, meta) = loaded.get(i * 10).unwrap();
            let expected = format!("test-payload-record-{i}");
            assert_eq!(view.as_bytes(), expected.as_bytes());
            assert_eq!(meta, i as u32);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_corrupted_image_rejection() {
        // Truncated input
        assert!(ExpanseBlobMap::from_bytes_slice(&[0u8; 10]).is_err());

        // Invalid magic
        let mut bad_magic = vec![0u8; 64];
        bad_magic[0..8].copy_from_slice(b"BADMAGIC");
        assert!(ExpanseBlobMap::from_bytes_slice(&bad_magic).is_err());

        // Huge chunk size attack input
        let mut huge_chunk = vec![0u8; 64];
        huge_chunk[0..8].copy_from_slice(b"EXPANSE\0");
        huge_chunk[8..16].copy_from_slice(&1u64.to_le_bytes()); // version
        huge_chunk[16..24].copy_from_slice(&64u64.to_le_bytes()); // total size
        huge_chunk[40..48].copy_from_slice(&0x45534e41505845u64.to_le_bytes()); // huge chunk_size
        assert!(ExpanseBlobMap::from_bytes_slice(&huge_chunk).is_err());

        // Zero chunk size
        huge_chunk[40..48].copy_from_slice(&0u64.to_le_bytes());
        assert!(ExpanseBlobMap::from_bytes_slice(&huge_chunk).is_err());

        // Fuzzer regression unit: offset overflow in header offsets
        let fuzzer_crash = [
            69, 88, 80, 65, 78, 83, 69, 0, 1, 0, 0, 0, 0, 1, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 69, 78, 80, 1, 83, 0, 0, 0, 0,
            0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 110, 46, 110, 110, 110, 0, 0,
            0, 0, 1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 65, 0, 0, 0, 0, 0, 0, 0, 110, 83, 69, 0, 69, 78,
            80, 1, 83, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 110,
            46, 110, 59, 110, 255, 255, 255, 255, 255, 255, 191, 255, 255, 255, 255, 255, 255, 255,
            255, 201, 255, 255, 255, 255, 255, 255, 255, 255, 1, 255, 0, 0, 0, 0, 110, 0, 0,
        ];
        assert!(ExpanseBlobMap::from_bytes_slice(&fuzzer_crash).is_err());
    }

    #[test]
    fn corrupted_image_non_uniform_chunk_cap_rejected() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        map.insert(1, &[0xCD; 1000], 7).unwrap();
        let mut buf = Vec::new();
        map.save_to_writer(&mut buf).unwrap();
        // Baseline: the untouched image parses.
        assert!(ExpanseBlobMap::from_bytes_slice(&buf).is_ok());

        // arena_offset lives at header bytes[32..40]; the first chunk header's
        // `cap` field is the first 8 bytes there. Set it to a valid-range value
        // that differs from chunk_size — get_blob_slice would then map offsets
        // to the wrong chunk, so it must be rejected.
        let arena_off = u64::from_le_bytes(buf[32..40].try_into().unwrap()) as usize;
        let bad_cap = (64u64 * 1024) + 16;
        buf[arena_off..arena_off + 8].copy_from_slice(&bad_cap.to_le_bytes());
        assert!(matches!(
            ExpanseBlobMap::from_bytes_slice(&buf),
            Err(ArenaError::CorruptedHeader)
        ));
    }

    #[test]
    fn corrupted_image_huge_aggregate_capacity_rejected() {
        // A ~512-byte file that declares chunk_count * chunk_size = 2 GiB, above
        // the shipped MAX_ARENA_CAPACITY (1 GiB) safety cap. chunk_count (2)
        // passes both the MAX_ARENA_CHUNKS and bytes.len()/24 bounds, and
        // chunk_size (1 GiB) is exactly MAX_CHUNK_CAPACITY, so the
        // aggregate-capacity check is the one that fires — proving it guards the
        // multi-GiB alloc_zeroed DoS at the new ceiling.
        let mut buf = vec![0u8; 512];
        buf[0..8].copy_from_slice(&EXPANSE_MAGIC);
        buf[8..12].copy_from_slice(&EXPANSE_FORMAT_VERSION.to_le_bytes());
        // flags[12..16] = 0, entry_count[16..24] = 0
        buf[24..32].copy_from_slice(&64u64.to_le_bytes()); // index_offset
        buf[32..40].copy_from_slice(&64u64.to_le_bytes()); // arena_offset
        buf[40..48].copy_from_slice(&512u64.to_le_bytes()); // total_size
        buf[48..56].copy_from_slice(&(1024u64 * 1024 * 1024).to_le_bytes()); // chunk_size = 1 GiB
        buf[56..64].copy_from_slice(&2u64.to_le_bytes()); // chunk_count = 2 -> 2 GiB
        assert!(matches!(
            ExpanseBlobMap::from_bytes_slice(&buf),
            Err(ArenaError::CorruptedHeader)
        ));
    }

    #[test]
    fn payload_larger_than_chunk_rejected() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        // A payload equal to chunk_size cannot fit alongside the 8-byte header.
        assert!(matches!(
            map.insert(9, &[0u8; 4096], 0),
            Err(ArenaError::AllocationFailed)
        ));
        // And one strictly larger than chunk_size.
        assert!(matches!(
            map.insert(9, &[0u8; 5000], 0),
            Err(ArenaError::AllocationFailed)
        ));
    }

    #[test]
    #[cfg(not(miri))]
    fn arena_long_produced_at_16mib_boundary() {
        // One record fills a 1 MiB chunk exactly, so blob k lands at the start
        // of chunk k, global offset = k * 1 MiB. The 24-bit `ArenaShort` locator
        // covers global < 16 MiB, i.e. chunks 0..=15; chunk 16 begins at exactly
        // 16 MiB (0x0100_0000) and must switch to `ArenaLong`.
        let chunk = 1024 * 1024; // 1 MiB
        let mut map = ExpanseBlobMap::with_chunk_size(chunk);
        for k in 0..20u64 {
            // Distinct payload per key so read-back can't alias.
            let payload = vec![(0xA0 + k) as u8; chunk - 8];
            map.insert(k, &payload, k as u32)
                .expect("insert past 16 MiB");
        }

        for k in 0..20u64 {
            let raw = map.index.get(k).expect("key present");
            let tag = ValueSlot::from_raw(raw).tag();
            if k < 16 {
                assert_eq!(tag, SlotTag::ArenaShort, "k={k} should be ArenaShort");
            } else {
                assert_eq!(tag, SlotTag::ArenaLong, "k={k} should be ArenaLong");
            }
            let (view, meta) = map.get(k).expect("value present");
            assert!(view.is_arena());
            assert_eq!(view.as_bytes(), &vec![(0xA0 + k) as u8; chunk - 8][..]);
            // ArenaLong carries no hot metadata (reported as 0); ArenaShort does.
            let expected_meta = if k < 16 { k as u32 } else { 0 };
            assert_eq!(meta, expected_meta, "k={k} meta");
        }
        // The arena grew past the old 16 MiB ceiling.
        assert!(map.arena().mem_used() > ARENA_SHORT_CEILING);
    }

    #[test]
    #[cfg(not(miri))]
    fn failed_compaction_leaves_map_intact() {
        let chunk = 1024 * 1024;
        let mut map = ExpanseBlobMap::with_chunk_size(chunk);
        // Pin the arena's capacity cap to 16 MiB so a 17th 1 MiB chunk overflows
        // cheaply (the compacted arena inherits this cap), exercising the
        // all-or-nothing failure path without allocating gigabytes.
        map.arena.max_capacity = 16 * 1024 * 1024;
        let payload = vec![0x5A; chunk - 8];
        map.insert(0, &payload, 42).unwrap();
        let raw = map.index.get(0).expect("key 0 present");
        // 16 extra index entries aliasing the single arena record at offset 0.
        // Compaction copies each into its own record, overflowing the 16 MiB
        // cap partway through phase 1 -> Err, with self left untouched.
        for k in 1..=16u64 {
            map.index.insert(k, raw);
        }
        assert_eq!(map.len(), 17);
        let gen_before = map.arena().generation();

        assert!(matches!(map.compact(), Err(ArenaError::OffsetOverflow)));

        // Every entry, the count, and the generation survive the failure.
        assert_eq!(map.len(), 17);
        assert_eq!(map.arena().generation(), gen_before);
        for k in 0..=16u64 {
            let (view, _) = map.get(k).expect("entry survives failed compaction");
            assert_eq!(view.len(), chunk - 8);
        }
    }

    #[test]
    fn save_load_roundtrip_preserves_generation() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        for i in 0..40u64 {
            map.insert(i, &[i as u8; 50], i as u32).unwrap();
        }
        // Advance the generation with a real compaction.
        for i in 0..20u64 {
            assert!(map.remove(i));
        }
        map.compact().unwrap();
        let generation = map.arena().generation();
        assert!(
            generation >= 2,
            "generation should advance past the initial value"
        );

        let mut buf = Vec::new();
        map.save_to_writer(&mut buf).unwrap();
        let restored = ExpanseBlobMap::from_bytes_slice(&buf).unwrap();

        assert_eq!(restored.len(), 20);
        assert_eq!(
            restored.arena().generation(),
            generation,
            "generation must survive save/load"
        );
        for i in 20..40u64 {
            let (view, meta) = restored.get(i).expect("entry present after reload");
            assert_eq!(view.as_bytes(), &[i as u8; 50][..]);
            assert_eq!(meta, i as u32);
        }
    }

    #[test]
    fn stale_arena_offset_after_compact_yields_none() {
        let chunk = 8192;
        let mut map = ExpanseBlobMap::with_chunk_size(chunk);
        for i in 0..20u64 {
            map.insert(i, &[i as u8; 4000], i as u32).unwrap();
        }
        // Offset of a high key, which lives beyond the first chunk.
        let raw = map.index.get(19).unwrap();
        let stale = ValueSlot::from_raw(raw).arena_offset();
        assert!(map.arena().get_blob_slice(stale).is_some());
        assert!(
            stale as usize >= chunk,
            "high key should live past the first chunk"
        );

        let gen_before = map.arena().generation();
        for i in 2..20u64 {
            assert!(map.remove(i));
        }
        map.compact().unwrap();

        assert!(
            map.arena().generation() > gen_before,
            "generation advances on compact"
        );
        // The offset held across the compaction no longer resolves.
        assert!(map.arena().get_blob_slice(stale).is_none());
        for i in 0..2u64 {
            assert_eq!(map.get(i).unwrap().0.as_bytes(), &[i as u8; 4000][..]);
        }
    }

    // ---------------------------------------------------------------------
    // ArenaLong tests. Reaching global offset > 16 MiB by normal inserts
    // requires a >16 MiB arena (too heavy for Miri), so the Miri-safe tests
    // build a tiny multi-chunk arena and rewrite an index slot into the
    // *equivalent* `ArenaLong` locator ((offset / chunk_size, offset %
    // chunk_size)) pointing at the same physical record. `get` then dispatches
    // on the tag and takes the multi-chunk `get_blob_slice_long` path, so the
    // exact chunk/offset pointer math runs under Miri on kilobytes of arena.
    // ---------------------------------------------------------------------

    /// Rewrites key `k`'s `ArenaShort` slot into the equivalent `ArenaLong`
    /// locator addressing the same physical record, and returns it.
    fn rewrite_short_to_long(map: &mut ExpanseBlobMap, k: Key) -> (u16, u64) {
        let raw = map.index.get(k).expect("key present");
        let slot = ValueSlot::from_raw(raw);
        assert_eq!(slot.tag(), SlotTag::ArenaShort, "precondition: short slot");
        let off = slot.arena_offset() as usize;
        let chunk_size = map.arena.chunk_size();
        let chunk_id = (off / chunk_size) as u16;
        let offset_in_chunk = (off % chunk_size) as u64;
        let long = ValueSlot::new_arena_long(chunk_id, offset_in_chunk).expect("encodable");
        map.index.insert(k, long.to_raw());
        (chunk_id, offset_in_chunk)
    }

    #[test]
    fn arena_long_read_resolves_on_small_arena() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        // ~2 KiB payloads -> two records per 4 KiB chunk, so keys spread across
        // several chunks (chunk ids > 0 exist).
        for k in 0..6u64 {
            let payload = vec![0x11 * (k as u8 + 1); 2000];
            map.insert(k, &payload, 700 + k as u32).unwrap();
        }
        let (cid, coff) = rewrite_short_to_long(&mut map, 5);
        assert!(cid >= 1, "key 5 should live past the first chunk");

        // Read via the ArenaLong path.
        let (view, meta) = map.get(5).expect("value present via ArenaLong");
        assert!(view.is_arena());
        assert_eq!(view.as_bytes(), &vec![0x11 * 6u8; 2000][..]);
        assert_eq!(meta, 0, "ArenaLong carries no hot metadata");

        // The low-level resolver agrees.
        assert_eq!(
            map.arena().get_blob_slice_long(cid, coff),
            Some(&vec![0x11 * 6u8; 2000][..])
        );
        // Other keys still resolve as ArenaShort with their metadata.
        let (v0, m0) = map.get(0).unwrap();
        assert_eq!(v0.as_bytes(), &vec![0x11u8; 2000][..]);
        assert_eq!(m0, 700);
    }

    #[test]
    fn arena_long_out_of_range_reads_none() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..4u64 {
            map.insert(k, &vec![0x33; 2000], 0).unwrap();
        }
        // Out-of-range chunk id -> None (no chunk 9999).
        assert!(map.arena().get_blob_slice_long(9999, 0).is_none());
        // Valid chunk, offset past the cursor -> None.
        assert!(map.arena().get_blob_slice_long(0, 1 << 20).is_none());

        // A crafted index slot with a bad ArenaLong locator resolves to None via
        // `get` (clean, no UB — Miri validates the pointer math).
        let bad = ValueSlot::new_arena_long(9999, 4096).unwrap();
        map.index.insert(0, bad.to_raw());
        assert!(map.get(0).is_none());
    }

    #[test]
    fn arena_long_save_load_roundtrip_small() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..6u64 {
            let payload = vec![0x20 + k as u8; 2000];
            map.insert(k, &payload, k as u32).unwrap();
        }
        // Turn two keys into ArenaLong locators.
        rewrite_short_to_long(&mut map, 4);
        rewrite_short_to_long(&mut map, 5);

        let mut buf = Vec::new();
        map.save_to_writer(&mut buf).unwrap();
        let restored = ExpanseBlobMap::from_bytes_slice(&buf).unwrap();
        assert_eq!(restored.len(), 6);

        for k in 0..6u64 {
            let (view, meta) = restored.get(k).expect("entry present after reload");
            assert_eq!(view.as_bytes(), &vec![0x20 + k as u8; 2000][..]);
            let raw = restored.index().get(k).unwrap();
            if k >= 4 {
                assert_eq!(ValueSlot::from_raw(raw).tag(), SlotTag::ArenaLong);
                assert_eq!(meta, 0);
            } else {
                assert_eq!(ValueSlot::from_raw(raw).tag(), SlotTag::ArenaShort);
                assert_eq!(meta, k as u32);
            }
        }
    }

    #[test]
    fn arena_long_compaction_relocates() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..8u64 {
            let payload = vec![0x40 + k as u8; 2000];
            map.insert(k, &payload, k as u32).unwrap();
        }
        // Make keys 6 and 7 ArenaLong, then churn away the rest.
        rewrite_short_to_long(&mut map, 6);
        rewrite_short_to_long(&mut map, 7);
        for k in 0..6u64 {
            assert!(map.remove(k));
        }
        assert_eq!(map.len(), 2);

        // Compaction must read the surviving ArenaLong records (via the Long
        // path) and relocate them all-or-nothing.
        let stats = map.compact().unwrap();
        assert_eq!(stats.live_records_moved, 2);

        for k in 6..8u64 {
            let (view, _meta) = map.get(k).expect("ArenaLong entry survives compaction");
            assert_eq!(view.as_bytes(), &vec![0x40 + k as u8; 2000][..]);
        }
    }

    #[test]
    #[cfg(not(miri))]
    fn multi_chunk_over_16mib_save_load_roundtrip() {
        // A genuinely >16 MiB arena with real `ArenaLong` slots produced by
        // normal inserts, round-tripped through the image format.
        let chunk = 1024 * 1024; // 1 MiB
        let mut map = ExpanseBlobMap::with_chunk_size(chunk);
        for k in 0..20u64 {
            let payload = vec![0x50 + k as u8; chunk - 8];
            map.insert(k, &payload, k as u32).unwrap();
        }
        assert!(map.arena().mem_used() > ARENA_SHORT_CEILING);

        let mut buf = Vec::new();
        let n = map.save_to_writer(&mut buf).unwrap();
        assert_eq!(n, buf.len());
        assert!(buf.len() > ARENA_SHORT_CEILING, "image exceeds 16 MiB");

        let restored = ExpanseBlobMap::from_bytes_slice(&buf).unwrap();
        assert_eq!(restored.len(), 20);

        let mut saw_long = false;
        for k in 0..20u64 {
            let (view, _meta) = restored.get(k).expect("entry present after reload");
            assert_eq!(view.as_bytes(), &vec![0x50 + k as u8; chunk - 8][..]);
            if ValueSlot::from_raw(restored.index().get(k).unwrap()).tag() == SlotTag::ArenaLong {
                saw_long = true;
            }
        }
        assert!(saw_long, "a >16 MiB arena must contain ArenaLong slots");
    }
}
