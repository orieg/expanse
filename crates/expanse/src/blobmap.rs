//! Chunked slab/arena allocator and high-level blob map.
//!
//! Stores variable-length byte payloads associated with 64-bit keys. Small
//! payloads (up to 7 bytes) are stored directly inside 64-bit value slots
//! ([`crate::slot::ValueSlot`]) with zero heap allocation. Larger payloads
//! are bump-allocated in contiguous 16-byte aligned slabs managed by [`BlobArena`].
//!
//! # Capacity limits
//!
//! Every arena payload is addressed by a single uniform value-slot encoding,
//! [`ArenaMeta`](crate::slot::SlotTag::ArenaMeta): `[hot_meta (24 bits) |
//! locator (32 bits) | tag]`. The locator is the record's global byte offset
//! divided by 16 (records are 16-byte aligned), so it addresses `2^32` 16-byte
//! units = **64 GiB** of arena — the `CompactInSlot` layout (#282/#285). Because
//! metadata rides in the same word as the locator, **every** arena blob carries
//! filterable 24-bit hot metadata; there is no metadata-less spill.
//!
//! A capacity cap on allocated chunk bytes bounds actual arena growth: the
//! caller's, or [`DEFAULT_ARENA_CAPACITY`] (1 GiB), clamped to the 64 GiB
//! locator envelope. [`BlobArena::alloc_blob`] returns
//! [`ArenaError::OffsetOverflow`] once growth would cross that cap (or the
//! [`MAX_ARENA_CHUNKS`] chunk-count sanity limit), and [`ArenaError::MetaOverflow`]
//! if `hot_meta` exceeds the 24-bit field. A single payload must still fit in one
//! chunk, so its length is bounded by `chunk_size - 8` (each record carries an
//! 8-byte [`BlobRecordHeader`]). The `External` slot encoding remains reserved.
//!
//! # Inline metadata
//!
//! Inline (`<= 7` byte) payloads live entirely in the value-slot word (bits
//! `63:8` hold payload bytes), so they carry no separate hot-metadata field:
//! `insert`'s `hot_meta` argument is ignored for them and `get`/`scan_filtered`
//! report their metadata as `0`. Their payload is already resident in the slot,
//! so a metadata predicate never needs a cold-DRAM fetch for them regardless.

use crate::map::ExpanseMap;
#[cfg(feature = "std")]
use crate::occ::Collector;
use crate::slot::{SlotTag, ValueSlot};
use crate::types::Key;
use core::alloc::Layout;
use core::ptr::NonNull;
#[cfg(feature = "std")]
use core::sync::atomic::AtomicPtr;
#[cfg(feature = "std")]
use core::sync::atomic::Ordering;
#[cfg(feature = "std")]
use core_alloc::alloc::{alloc, handle_alloc_error};
use core_alloc::alloc::{alloc_zeroed, dealloc};
#[cfg(feature = "std")]
use core_alloc::sync::Arc;
use core_alloc::vec::Vec;
#[cfg(feature = "std")]
use std::sync::OnceLock;

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

/// Parses the record at `off` within a chunk whose first `bound` bytes are
/// readable, expecting records stamped `generation`; returns the payload's
/// base pointer and length, or `None` when anything fails to check out
/// (offset past `bound`, generation mismatch, length past `bound`).
///
/// **The one definition of the record wire format on the read side.** The
/// single-threaded path ([`ArenaChunk::get_slice`]) calls it with
/// `bound = cursor`; the optimistic path ([`resolve_meta_in_table`]) calls it
/// with `bound = capacity`, relying on zeroed unwritten bytes failing the
/// generation check (generation 0 is never live).
///
/// # Safety
///
/// `base .. base + bound` must be readable bytes of one live allocation.
#[inline(always)]
unsafe fn read_record(
    base: *const u8,
    off: usize,
    bound: usize,
    generation: u32,
) -> Option<(*const u8, usize)> {
    if off.checked_add(8)? > bound {
        return None;
    }
    // SAFETY: `off + 8 <= bound`, readable per this function's contract. The
    // loaded bytes may be torn/stale on the optimistic path — every use is
    // range-checked here and discarded by that caller unless its seqlock
    // snapshot validates.
    let header = unsafe { core::ptr::read_unaligned(base.add(off).cast::<BlobRecordHeader>()) };
    if header.generation != generation {
        return None;
    }
    let len = header.len as usize;
    if off.checked_add(8)?.checked_add(len)? > bound {
        return None;
    }
    // SAFETY: `off + 8 + len <= bound` — in-bounds of the allocation.
    Some((unsafe { base.add(off + 8) }, len))
}

/// Magic identifier for Expanse binary image files ("EXPANSE\0").
pub const EXPANSE_MAGIC: [u8; 8] = *b"EXPANSE\0";
/// Current format version for relocatable ExpanseBlobMap images.
pub const EXPANSE_FORMAT_VERSION: u32 = 2;

/// Relocatable 64-byte binary image file header.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlobMapFileHeader {
    /// Magic string `EXPANSE\0`.
    pub magic: [u8; 8],
    /// Format version (`2`).
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
    /// Inlined uncompressed value (<= 7 bytes) borrowing directly from leaf value slot memory.
    Inline(&'a [u8]),
    /// Arena-allocated value borrowing directly from an arena slab.
    Arena(&'a [u8]),
    /// Compressed inlined value decoded into a fixed 16-byte buffer.
    CompressedInline {
        /// Decompressed byte buffer.
        buf: [u8; 16],
        /// Length of valid decompressed bytes.
        len: u8,
    },
}

impl<'a> BlobView<'a> {
    /// Returns the underlying byte slice.
    #[inline(always)]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            BlobView::Inline(slice) => slice,
            BlobView::Arena(slice) => slice,
            BlobView::CompressedInline { buf, len } => &buf[..*len as usize],
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

    /// Returns `true` if the payload is stored inline in the value slot (raw or compressed).
    #[inline(always)]
    #[must_use]
    pub fn is_inline(&self) -> bool {
        matches!(
            self,
            BlobView::Inline(_) | BlobView::CompressedInline { .. }
        )
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
#[non_exhaustive]
pub enum ArenaError {
    /// Arena memory allocation failed.
    AllocationFailed,
    /// Arena growth would exceed the addressable/allowed ceiling: the arena's
    /// capacity cap (see [`DEFAULT_ARENA_CAPACITY`]), the [`MAX_ARENA_CHUNKS`]
    /// chunk-count limit, or the 64 GiB `ArenaMeta` locator envelope
    /// (`global_offset / 16` no longer fits a `u32`); or a loaded image declares
    /// more chunk bytes than the loader's cap.
    ///
    /// From an insert, it means the cap refused the record's chunk and **no
    /// compaction ran in that call**: the reclaim rule declined, or it is
    /// switched off. The arena may still hold dead bytes; compare
    /// [`BlobArena::live_bytes`] with [`BlobArena::mem_used`] (allocated chunk
    /// bytes) to see how many a [`ExpanseBlobMap::compact`] could free. An
    /// insert that compacted and was still refused returns
    /// [`Self::ArenaFull`] instead.
    OffsetOverflow,
    /// An insert compacted the arena under the reclaim rule and the record
    /// still did not fit under the capacity cap (#1300): the cap is filled by
    /// live records and the tails of their chunks, so a further compaction
    /// frees nothing. Only a removal or a larger cap makes room. The map's
    /// contents are unchanged, but every arena payload has moved.
    ArenaFull,
    /// `hot_meta` exceeds the 24-bit `ArenaMeta` field
    /// ([`ValueSlot::ARENA_META_MAX`]). Rejected rather than silently truncated.
    MetaOverflow,
    /// Invalid arena offset was provided.
    InvalidOffset,
    /// Blob generation mismatch (ABA detected).
    GenerationMismatch,
    /// Corrupted record header encountered.
    CorruptedHeader,
    /// The image was written by a different [`EXPANSE_FORMAT_VERSION`]. Images
    /// are not a cross-version format: a map saved by one release loads only
    /// in releases that share its format version, and no migration is
    /// attempted. `found` is the version in the image, `supported` the one
    /// this build reads and writes.
    UnsupportedFormatVersion {
        /// Format version recorded in the image header.
        found: u32,
        /// Format version this build reads and writes.
        supported: u32,
    },
}

impl core::fmt::Display for ArenaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "Arena memory allocation failed"),
            Self::OffsetOverflow => {
                write!(f, "Arena growth exceeded the addressable/allowed ceiling")
            }
            Self::ArenaFull => write!(
                f,
                "Arena full: the record does not fit under the capacity cap after compacting"
            ),
            Self::MetaOverflow => write!(f, "hot_meta exceeds the 24-bit ArenaMeta field"),
            Self::InvalidOffset => write!(f, "Invalid arena offset"),
            Self::GenerationMismatch => write!(f, "Blob generation mismatch (ABA detected)"),
            Self::CorruptedHeader => write!(f, "Corrupted blob record header"),
            Self::UnsupportedFormatVersion { found, supported } => write!(
                f,
                "Unsupported image format version {found} (this build reads and writes version {supported})"
            ),
        }
    }
}

impl core::error::Error for ArenaError {}

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

/// A compaction whose copy has run and whose index rewrites have not
/// ([`BlobArena::prepare_compaction`], #1300 item 2): the compacted arena and
/// each relocated key's new slot word. Applying it is infallible; dropping it
/// frees the copy and changes nothing.
pub(crate) struct PreparedCompaction {
    new_arena: BlobArena,
    rewrites: Vec<(Key, u64)>,
    source_generation: u32,
    source_total_allocated: usize,
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
        let layout =
            Layout::from_size_align(capacity, 16).map_err(|_| ArenaError::AllocationFailed)?;
        // SAFETY: Allocating memory with 16-byte alignment.
        let raw = unsafe { alloc_zeroed(layout) };
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
        // SAFETY: `ptr .. ptr + cursor` is the initialized prefix of this
        // chunk's live allocation (`cursor <= capacity`).
        let (payload, len) = unsafe {
            read_record(
                self.ptr.as_ptr(),
                offset_in_chunk,
                self.cursor,
                self.generation,
            )
        }?;
        // SAFETY: `read_record` bounds the payload within the allocation.
        Some(unsafe { core::slice::from_raw_parts(payload, len) })
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

    /// Phase 7 (issue #219): hands this chunk's allocation to the epoch
    /// collector for deferred freeing instead of dropping it — pinned readers
    /// may still hold pointers into it. Consumes the chunk without running its
    /// `Drop` (the collector frees the memory after the grace period).
    #[cfg(feature = "std")]
    pub(crate) fn retire_into(self, collector: &Collector) {
        let ptr = self.ptr;
        let capacity = self.capacity;
        core::mem::forget(self);
        // SAFETY: `ptr` is this chunk's own allocation, made by the global
        // allocator with `(capacity, 16)`; `forget` gave up the only owner, so
        // it is retired once. The caller unlinked the chunk from the published
        // table, so only readers pinned before that can still hold it.
        unsafe { collector.retire(ptr, capacity, 16) };
    }
}

impl Drop for ArenaChunk {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity, 16).unwrap();
        // SAFETY: self.ptr was allocated with this exact layout.
        unsafe {
            dealloc(self.ptr.as_ptr(), layout);
        }
    }
}

// SAFETY: ArenaChunk exclusively owns its heap allocation.
unsafe impl Send for ArenaChunk {}
// SAFETY: ArenaChunk memory is immutable across concurrent threads unless uniquely borrowed.
unsafe impl Sync for ArenaChunk {}

/// Default chunk capacity: 2 MiB.
pub const DEFAULT_CHUNK_SIZE: usize = 2 * 1024 * 1024;

/// Arena payload alignment (bytes). Every record starts on a 16-byte boundary
/// (see [`ArenaChunk::alloc`]), so a global byte offset is always a multiple of
/// this and the `ArenaMeta` locator `global / ARENA_ALIGN` is exact.
pub const ARENA_ALIGN: usize = 16;

/// Global-offset envelope of the 32-bit `ArenaMeta` locator: `2^32 * 16` =
/// **64 GiB**. A record whose global byte offset reaches this bound can no
/// longer be encoded (`global / 16` would not fit a `u32`).
pub const ARENA_META_CEILING: u64 = (1u64 << 32) * (ARENA_ALIGN as u64);

/// Chunk-count sanity cap (`2^16`). The `ArenaMeta` locator no longer carries a
/// chunk id — chunk/offset are recovered arithmetically from the global offset —
/// but the arena still limits the number of chunks it will allocate or accept
/// from a loaded image, as a corruption guard. Beside it, a loaded image may
/// declare at most [`ARENA_META_CEILING`] of chunk bytes, the locator envelope.
/// Effective growth is bounded far lower by the arena's capacity cap
/// ([`DEFAULT_ARENA_CAPACITY`] unless the caller sets one).
pub const MAX_ARENA_CHUNKS: usize = 1 << 16;

/// Default capacity cap on an arena's allocated chunk bytes (**1 GiB**): the
/// runtime growth budget of an arena built without an explicit cap
/// ([`BlobArena::new`], [`ExpanseBlobMap::new`], [`ExpanseBlobMap::from_bytes_slice`]).
/// A caller picks another with
/// [`ExpanseBlobMap::with_chunk_size_and_max_capacity`], clamped to
/// `[chunk_size, ARENA_META_CEILING]`.
///
/// The cap counts allocated chunk bytes, dead and live, not the process's
/// memory: a compaction holds the old and the new chunk sets at once, plus a
/// relocation list of 16 bytes per index entry, and on
/// `SyncExpanseBlobMap` the old set stays allocated until the
/// epoch collector frees it (#1290).
///
/// A budget, not a structural limit: the structural limits are
/// [`MAX_ARENA_CHUNKS`] and the 64 GiB `ArenaMeta` locator envelope
/// ([`ARENA_META_CEILING`]), which also bound what a loaded image may declare.
/// 1 GiB comfortably exceeds any single-socket last-level cache (what the RFC
/// §10.3 cold-DRAM predicate-scan regime requires) while bounding a runaway
/// workload's growth.
pub const DEFAULT_ARENA_CAPACITY: usize = 1 << 30;

/// The former name of [`DEFAULT_ARENA_CAPACITY`], which was both the default
/// growth budget and the image loader's bound on declared capacity (#1300).
/// The loader's bound is now the caller's cap
/// ([`ExpanseBlobMap::from_bytes_slice_with_max_capacity`]) within the
/// structural limits; this constant is only the default budget.
#[deprecated(
    note = "use DEFAULT_ARENA_CAPACITY (the default growth budget); the image loader's bound is the caller's cap"
)]
pub const MAX_ARENA_CAPACITY: usize = DEFAULT_ARENA_CAPACITY;

/// The reclaim rule's copy budget (#1290, `docs/design/large-values.md`
/// §6.3.1). An insert that the cap refuses a new chunk compacts the arena once
/// and retries once, and only if the live bytes the compaction copies are at
/// most this many times the bytes freeable since the previous compaction.
/// `scripts/blob_reclaim_bounds.py` bounds what the rule sustains and copies.
pub(crate) const RECLAIM_COPY_PER_GROWTH: usize = 1;

#[cfg(all(test, feature = "std"))]
std::thread_local! {
    /// The capacity `compact_with_index` last reserved for its relocation list
    /// on this thread (#1290 G1.10).
    static LAST_RELOCATION_RESERVE: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    /// Runs once at the start of the next [`BlobArena::prepare_compaction`] on
    /// this thread: the tests that pin where the copy runs relative to the
    /// tree bracket read the map from another thread inside it (#1300 item 2).
    pub(crate) static PREPARE_HOOK: core::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { core::cell::RefCell::new(None) };
}

/// An insert's retry after the reclaim rule compacted the arena: a refusal
/// there is [`ArenaError::ArenaFull`], since a compaction just ran and freed
/// what it could (#1300). Every other error passes through.
#[cold]
fn refused_after_compaction(e: ArenaError) -> ArenaError {
    match e {
        ArenaError::OffsetOverflow => ArenaError::ArenaFull,
        e => e,
    }
}

/// Builds the uniform [`ArenaMeta`](SlotTag::ArenaMeta) [`ValueSlot`] for a blob
/// at flat `global_offset` carrying `hot_meta`.
///
/// The locator is `global_offset / 16` (records are 16-byte aligned). Returns
/// [`ArenaError::OffsetOverflow`] if the offset is unaligned or beyond the 64 GiB
/// envelope, and [`ArenaError::MetaOverflow`] if `hot_meta` exceeds 24 bits —
/// never silently truncating either field.
#[inline]
pub(crate) fn slot_from_global(global_offset: u64, hot_meta: u32) -> Result<ValueSlot, ArenaError> {
    if !global_offset.is_multiple_of(ARENA_ALIGN as u64) || global_offset >= ARENA_META_CEILING {
        return Err(ArenaError::OffsetOverflow);
    }
    if hot_meta > ValueSlot::ARENA_META_MAX {
        return Err(ArenaError::MetaOverflow);
    }
    let locator = (global_offset / (ARENA_ALIGN as u64)) as u32;
    ValueSlot::new_arena_meta(hot_meta, locator).ok_or(ArenaError::MetaOverflow)
}

/// Phase 7 (issue #219): one entry of the RCU-published chunk table — the raw
/// geometry an optimistic reader needs to resolve a record inside one chunk.
/// Entries are immutable once published.
#[repr(C)]
#[derive(Clone, Copy)]
struct ChunkRef {
    /// Base of the chunk allocation.
    ptr: *const u8,
    /// Chunk capacity in bytes (reader bound; the cursor moves under the
    /// writer, so readers bound by capacity and rely on the record
    /// generation check — unwritten chunk bytes are zeroed and generation 0
    /// is never a live generation).
    capacity: usize,
    /// Generation stamped on the chunk's records.
    generation: u32,
}

/// Phase 7 (issue #219): header of the RCU-published chunk table. `len`
/// [`ChunkRef`] entries trail this header in the same allocation, so a single
/// pointer publishes a self-describing, immutable snapshot — readers never
/// touch the arena's `Vec<ArenaChunk>`, whose buffer the global allocator
/// frees on growth (no grace period). Superseded tables are retired through
/// the epoch collector.
#[repr(C)]
pub(crate) struct ChunkTable {
    /// Number of trailing [`ChunkRef`] entries.
    len: usize,
    /// The arena's fixed chunk size (immutable after construction), so a
    /// reader can split a global offset without touching the arena struct.
    chunk_size: usize,
}

/// Allocation size of a chunk table with `len` entries.
#[inline]
fn table_bytes(len: usize) -> usize {
    core::mem::size_of::<ChunkTable>() + len * core::mem::size_of::<ChunkRef>()
}

/// Allocation layout of a chunk table with `len` entries. The alignment
/// (8) deliberately matches no collector size class ([`crate::alloc::RAW_ALIGN`]
/// is 16), so retired tables always go through a plain deferred `dealloc`.
#[inline]
fn table_layout(len: usize) -> Layout {
    Layout::from_size_align(table_bytes(len), core::mem::align_of::<ChunkTable>())
        .expect("valid chunk table layout")
}

/// Resolves an `ArenaMeta` `locator` (`global / 16` in 16-byte units) through
/// a published chunk table to the payload's base pointer and length.
///
/// Returns `None` for anything that does not resolve cleanly — a null table,
/// an out-of-range chunk or offset, a generation mismatch, or a length beyond
/// the chunk. The loaded bytes may be torn or stale (this is the optimistic
/// read path): the caller MUST validate its seqlock snapshot before using the
/// result, which disambiguates a genuinely dangling locator (validated `None`)
/// from a racing writer (validation fails → retry).
///
/// # Safety
///
/// `table` must be null or a table published by a [`BlobArena`] in deferred
/// mode, and the caller must hold an epoch pin taken before loading it: the
/// table and every chunk it references are then EBR-live, so all reads stay
/// within live allocations even when the table has been superseded.
#[cfg(feature = "std")]
pub(crate) unsafe fn resolve_meta_in_table(
    table: *const ChunkTable,
    locator: u32,
) -> Option<(*const u8, usize)> {
    if table.is_null() {
        return None;
    }
    // SAFETY: non-null published table, EBR-live under the caller's pin.
    let (len, chunk_size) = unsafe { ((*table).len, (*table).chunk_size) };
    let offset = usize::try_from((locator as u64) * (ARENA_ALIGN as u64)).ok()?;
    let idx = offset / chunk_size;
    let off = offset % chunk_size;
    if idx >= len {
        return None;
    }
    // SAFETY: `idx < len` entries trail the header in the same allocation.
    let entry = unsafe {
        *table
            .cast::<u8>()
            .add(core::mem::size_of::<ChunkTable>())
            .cast::<ChunkRef>()
            .add(idx)
    };
    // SAFETY: `entry.ptr .. entry.ptr + capacity` is one chunk allocation,
    // EBR-live under the caller's pin; the shared parser range-checks every
    // access against `capacity`, and the caller discards the result unless
    // its seqlock snapshot validates.
    unsafe { read_record(entry.ptr, off, entry.capacity, entry.generation) }
}

/// Multi-writer private arena (Refs #929, AGENTS.md §2.7): the geometry of a
/// chunk granted to one writer slot by [`BlobArena::grant_private_chunk`].
#[cfg(all(
    not(feature = "ablation-blob-shared-arena"),
    not(feature = "ablation-blob-serial-writers"),
    feature = "std"
))]
#[derive(Clone, Copy)]
pub(crate) struct PrivateChunk {
    /// Base of the chunk allocation.
    pub(crate) base: NonNull<u8>,
    /// Index of the chunk in the arena's chunk set (and published table).
    pub(crate) index: usize,
    /// Chunk capacity in bytes.
    pub(crate) capacity: usize,
    /// Generation to stamp on records written into the chunk.
    pub(crate) generation: u32,
}

/// Multi-writer private arena (Refs #929): writes one record — header then
/// payload — at `off` in a privately owned chunk. The write-side twin of
/// [`read_record`], and byte-for-byte what [`ArenaChunk::alloc`] writes.
///
/// # Safety
///
/// `base + off .. base + off + 8 + data.len()` must lie inside one live chunk
/// allocation, and no other thread may read or write those bytes until the
/// caller publishes a locator to them.
#[cfg(all(
    not(feature = "ablation-blob-shared-arena"),
    not(feature = "ablation-blob-serial-writers"),
    feature = "std"
))]
#[inline]
pub(crate) unsafe fn write_record(base: *mut u8, off: usize, generation: u32, data: &[u8]) {
    let header = BlobRecordHeader {
        len: data.len() as u32,
        generation,
    };
    // SAFETY: the range is in bounds of one live allocation and unshared, per
    // this function's contract; the header is packed, hence `write_unaligned`.
    unsafe {
        let at = base.add(off);
        core::ptr::write_unaligned(at.cast::<BlobRecordHeader>(), header);
        if !data.is_empty() {
            core::ptr::copy_nonoverlapping(data.as_ptr(), at.add(8), data.len());
        }
    }
}

/// Chunked slab allocator for variable-length payload storage.
pub struct BlobArena {
    /// The chunk set. **Invariant (Phase 7):** any mutation of this set must
    /// call [`Self::republish_table`] before any dropped chunk is disposed —
    /// readers resolve through the published table, and a block must be
    /// unreachable before it enters the collector's grace period. Current
    /// mutation sites: `alloc_blob` (growth), `push_chunk`, `clear`,
    /// `compact_with_index`.
    chunks: Vec<ArenaChunk>,
    active_chunk: Option<usize>,
    /// Fixed record-addressing granularity:
    /// `global_offset = idx * chunk_size + offset_in_chunk`.
    /// **Immutable after construction** — every published chunk table and
    /// every `ArenaMeta` locator in an index bakes this split in, so
    /// mutating it on a live arena would misresolve them all.
    chunk_size: usize,
    total_allocated: usize,
    live_bytes: usize,
    /// `total_allocated` and `live_bytes` right after the last compaction,
    /// zero since construction or [`Self::clear`]: the reclaim rule's baseline
    /// ([`Self::reclaim_allowed`]).
    compacted_total: usize,
    compacted_live: usize,
    /// Current generation, stamped into every chunk allocated by this arena
    /// and bumped on each [`compact_with_index`](Self::compact_with_index) so
    /// that an arena offset held across a compaction fails the generation
    /// check in [`ArenaChunk::get_slice`] instead of resolving to unrelated
    /// bytes. Serialized per chunk so save/load preserves it.
    generation: u32,
    /// Total-capacity ceiling in bytes. Allocating a new chunk fails with
    /// [`ArenaError::OffsetOverflow`] once `total_allocated + chunk_size` would
    /// cross it. Defaults to [`DEFAULT_ARENA_CAPACITY`]; a compaction inherits the
    /// source arena's cap. Not serialized (it is a growth policy, not data).
    max_capacity: usize,
    /// Phase 7 (issue #219): when set, dead chunks and superseded chunk
    /// tables are retired to the collector instead of freed — concurrent
    /// readers may still hold pointers into them — and the reader table is
    /// published. Mirrors [`crate::alloc::NodeAlloc`]'s deferred mode.
    #[cfg(feature = "std")]
    deferred: OnceLock<Arc<ArenaDeferred>>,
}

/// A deferred arena's shared state, on the heap rather than in the arena
/// (#1086): the collector, and the reader table the concurrent wrapper's
/// readers load. A wrapper writer that holds `&mut BlobArena` (a shared-path
/// allocation) covers the arena's own bytes, so a reader that loaded the
/// table through the arena would access memory that reference asserts no
/// one else touches. Through this cell it does not.
#[cfg(feature = "std")]
pub(crate) struct ArenaDeferred {
    collector: Arc<Collector>,
    /// RCU-published [`ChunkTable`] for optimistic readers; null while the
    /// arena has no chunks. Republished whole on every chunk-set change,
    /// always *before* the chunks it dropped are retired (a block must be
    /// unreachable before it enters the grace period).
    reader_table: AtomicPtr<ChunkTable>,
}

#[cfg(feature = "std")]
impl ArenaDeferred {
    /// Current published chunk table (null when the arena has no chunks).
    /// Readers must hold an epoch pin taken before this load — see
    /// [`resolve_meta_in_table`].
    #[inline(always)]
    pub(crate) fn reader_table(&self) -> *const ChunkTable {
        self.reader_table.load(Ordering::Acquire)
    }
}

impl BlobArena {
    /// Creates a new `BlobArena` with the specified chunk size.
    ///
    /// `chunk_size` is clamped into `[4096, ArenaChunk::MAX_CHUNK_CAPACITY]`
    /// (1 GiB upper bound) — a chunk larger than a chunk allocation can ever be
    /// would make every arena insert fail — then rounded **up** to a multiple of
    /// [`ARENA_ALIGN`] (16) so a chunk boundary lands on a 16-byte-aligned global
    /// offset. Combined with 16-byte-aligned records, that keeps every record's
    /// global offset a multiple of 16, so the `ArenaMeta` locator (`global / 16`)
    /// is exact.
    #[must_use]
    pub fn new(chunk_size: usize) -> Self {
        let clamped = chunk_size.clamp(4096, ArenaChunk::MAX_CHUNK_CAPACITY);
        let aligned = (clamped + ARENA_ALIGN - 1) & !(ARENA_ALIGN - 1);
        Self {
            chunks: Vec::new(),
            active_chunk: None,
            chunk_size: aligned,
            total_allocated: 0,
            live_bytes: 0,
            compacted_total: 0,
            compacted_live: 0,
            generation: 1,
            max_capacity: DEFAULT_ARENA_CAPACITY,
            #[cfg(feature = "std")]
            deferred: OnceLock::new(),
        }
    }

    /// Creates a new `BlobArena` with the specified chunk size and maximum capacity ceiling.
    ///
    /// `chunk_size` is clamped into `[4096, ArenaChunk::MAX_CHUNK_CAPACITY]`.
    /// `max_capacity` is clamped to at least `chunk_size` (ensuring that the arena can allocate
    /// at least one initial chunk) and at most [`ARENA_META_CEILING`] (64 GiB, the structural
    /// ceiling of the 36-bit chunk locator address space; capped at `usize::MAX` on 32-bit targets).
    ///
    /// ## Behavior at Capacity Ceiling
    ///
    /// When total chunk allocations reach `max_capacity`, attempting to allocate an additional chunk
    /// fails with [`ArenaError::OffsetOverflow`], and the arena is unchanged. The arena never
    /// reclaims on its own; [`ExpanseBlobMap::insert`] may compact it under the reclaim rule
    /// before failing (see there).
    ///
    /// Note on 32-bit targets: [`ExpanseBlobMap32`](crate::blobmap32::ExpanseBlobMap32) uses a fixed
    /// 12-bit addressable slab (at most 4095 entries) optimized for embedded systems per
    /// `docs/design/32-bit-embedded.md`, where capacity is bounded by the fixed slab structure rather
    /// than dynamic multi-chunk arena expansion.
    #[must_use]
    pub fn with_chunk_size_and_max_capacity(chunk_size: usize, max_capacity: usize) -> Self {
        let mut arena = Self::new(chunk_size);
        let max_allowed = if (ARENA_META_CEILING as u128) > (usize::MAX as u128) {
            usize::MAX
        } else {
            ARENA_META_CEILING as usize
        };
        arena.max_capacity = max_capacity.clamp(arena.chunk_size, max_allowed);
        arena
    }

    /// Switches this arena to deferred reclamation through `collector`,
    /// permanently (Phase 7 concurrent wrappers call this once at
    /// construction, alongside the index's `NodeAlloc::defer_to`): dead
    /// chunks and superseded chunk tables then wait out the epoch grace
    /// period instead of being freed, and a [`ChunkTable`] snapshot is
    /// published for optimistic readers. Idempotent for the same collector;
    /// a second call with a different collector is a bug and panics.
    ///
    /// `pub(crate)` deliberately: only the `sync` wrappers drive a
    /// collector's epochs. A caller deferring a standalone arena through
    /// the public [`ExpanseBlobMap::arena`] accessor would retire whole
    /// chunks into bins nothing ever advances (unbounded growth), and the
    /// different-collector panic would be reachable from safe code.
    #[cfg(feature = "std")]
    pub(crate) fn defer_to(&self, collector: Arc<Collector>) {
        let stored = self.deferred.get_or_init(|| {
            Arc::new(ArenaDeferred {
                collector: Arc::clone(&collector),
                reader_table: AtomicPtr::new(core::ptr::null_mut()),
            })
        });
        assert!(
            Arc::ptr_eq(&stored.collector, &collector),
            "BlobArena already deferred to a different collector"
        );
        self.republish_table();
    }

    /// Current published chunk table (null when not in deferred mode or
    /// when the arena has no chunks), for tests; the concurrent wrapper
    /// loads it through [`Self::deferred_cell`].
    #[cfg(all(feature = "std", test))]
    pub(crate) fn reader_table(&self) -> *const ChunkTable {
        self.deferred
            .get()
            .map_or(core::ptr::null(), |d| d.reader_table())
    }

    /// The deferred arena's shared cell, for a concurrent wrapper to hold
    /// and load the reader table through without reaching into the arena
    /// (#1086). `None` until [`Self::defer_to`].
    #[cfg(feature = "std")]
    pub(crate) fn deferred_cell(&self) -> Option<Arc<ArenaDeferred>> {
        self.deferred.get().cloned()
    }

    /// Rebuilds and publishes the reader chunk table from the current chunk
    /// set, retiring the superseded table through the collector. No-op
    /// unless deferred mode is on. Publishing an empty chunk set stores
    /// null. Must run *before* any chunk dropped by the change is retired.
    fn republish_table(&self) {
        #[cfg(feature = "std")]
        {
            let Some(deferred) = self.deferred.get() else {
                return;
            };
            let collector = &deferred.collector;
            let new_table: *mut ChunkTable = if self.chunks.is_empty() {
                core::ptr::null_mut()
            } else {
                let len = self.chunks.len();
                let layout = table_layout(len);
                // SAFETY: non-zero-size layout; every byte is initialized below.
                let raw = unsafe { alloc(layout) };
                let Some(table) = NonNull::new(raw.cast::<ChunkTable>()) else {
                    handle_alloc_error(layout)
                };
                // SAFETY: fresh allocation of `table_bytes(len)` bytes: header
                // first, then `len` ChunkRef entries.
                unsafe {
                    table.as_ptr().write(ChunkTable {
                        len,
                        chunk_size: self.chunk_size,
                    });
                    let entries = table
                        .as_ptr()
                        .cast::<u8>()
                        .add(core::mem::size_of::<ChunkTable>())
                        .cast::<ChunkRef>();
                    for (i, chunk) in self.chunks.iter().enumerate() {
                        entries.add(i).write(ChunkRef {
                            ptr: chunk.ptr.as_ptr(),
                            capacity: chunk.capacity,
                            generation: chunk.generation,
                        });
                    }
                }
                table.as_ptr()
            };
            let old = deferred.reader_table.swap(new_table, Ordering::AcqRel);
            if let Some(old) = NonNull::new(old) {
                // SAFETY: `old` was published by this arena; its `len` header
                // field is immutable, giving back the exact allocation size.
                let bytes = table_bytes(unsafe { (*old.as_ptr()).len });
                // SAFETY: `old` was allocated by the global allocator with
                // `(table_bytes(len), align_of::<ChunkTable>())` when it was
                // published; the swap above unlinked it, so no new reader can
                // load it, and this is the only thread that got it back.
                unsafe {
                    collector.retire(old.cast::<u8>(), bytes, core::mem::align_of::<ChunkTable>());
                }
            }
        }
    }

    /// Disposes of chunks no longer referenced by the arena: retired
    /// through the collector in deferred mode (pinned readers may still
    /// hold pointers into them), dropped (freed immediately) otherwise.
    /// In deferred mode the caller must have republished the reader table
    /// first, so no new reader can reach these chunks.
    fn dispose_chunks(&self, _chunks: Vec<ArenaChunk>) {
        #[cfg(feature = "std")]
        if let Some(deferred) = self.deferred.get() {
            for chunk in _chunks {
                chunk.retire_into(&deferred.collector);
            }
        }
        // Not deferred: dropping the Vec frees each chunk immediately.
    }

    /// Returns the arena's current generation counter.
    #[inline(always)]
    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Flat global byte offset of a record at `(chunk index, intra-chunk offset)`.
    /// This is the address [`Self::get_blob_slice`] recovers by division and the
    /// value the `ArenaMeta` locator encodes as `global / 16`.
    #[inline]
    fn global_offset(&self, idx: usize, offset_in_chunk: usize) -> u64 {
        (idx as u64) * (self.chunk_size as u64) + (offset_in_chunk as u64)
    }

    /// Allocates a blob payload in the arena, returning its flat **global byte
    /// offset** (the caller encodes it into an `ArenaMeta` [`ValueSlot`] via
    /// [`slot_from_global`]).
    ///
    /// Fails with [`ArenaError::OffsetOverflow`] once growing the arena would
    /// cross the [`MAX_ARENA_CHUNKS`] chunk-count cap or the arena's capacity
    /// cap ([`Self::max_capacity`]), and with [`ArenaError::AllocationFailed`]
    /// if a single record cannot fit one chunk (`8 + data.len() > chunk_size`).
    /// The arena cannot compact without the index, so this never reclaims;
    /// [`ExpanseBlobMap::insert`] does, under the reclaim rule.
    ///
    /// Inlined, so a caller's fast path (the active chunk has room) makes no
    /// call; opening a chunk is `alloc_blob_in_new_chunk`, out of line.
    #[inline]
    pub fn alloc_blob(&mut self, data: &[u8]) -> Result<u64, ArenaError> {
        let needed = 8 + data.len();
        if needed > self.chunk_size {
            return Err(ArenaError::AllocationFailed);
        }

        if let Some(idx) = self.active_chunk
            && self.chunks[idx].can_fit(data.len())
        {
            let offset_in_chunk = self.chunks[idx].alloc(data)?;
            self.live_bytes += needed;
            return Ok(self.global_offset(idx, offset_in_chunk));
        }
        self.alloc_blob_in_new_chunk(data)
    }

    /// [`Self::alloc_blob`]'s growth path: the active chunk cannot fit the
    /// record, so a new chunk is opened, within the caps.
    #[inline(never)]
    fn alloc_blob_in_new_chunk(&mut self, data: &[u8]) -> Result<u64, ArenaError> {
        let needed = 8 + data.len();
        // A new chunk is required — enforce the chunk-count and total capacity
        // caps before allocating anything.
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
        // Phase 7: optimistic readers resolve through the published table, so
        // the grown chunk set must be republished for the record to become
        // reachable (readers on the old table simply retry).
        self.republish_table();
        Ok(self.global_offset(idx, offset_in_chunk))
    }

    /// Prepares a [`ValueSlot`] for `data` and `hot_meta`: inline if `<= 7` bytes,
    /// compressed inline if compressible with `hot_meta == 0`, or allocated in the
    /// arena returning an `ArenaMeta` slot.
    #[inline(always)]
    pub(crate) fn prepare_slot(
        &mut self,
        data: &[u8],
        hot_meta: u32,
    ) -> Result<ValueSlot, ArenaError> {
        if data.len() <= 7 {
            ValueSlot::new_inline(data).ok_or(ArenaError::AllocationFailed)
        } else if hot_meta == 0
            && let Some(slot) = crate::codec::try_compress_inline(data)
        {
            Ok(slot)
        } else {
            // Validate the metadata envelope *before* allocating arena bytes, so a
            // rejected insert leaves no orphaned payload behind.
            if hot_meta > ValueSlot::ARENA_META_MAX {
                return Err(ArenaError::MetaOverflow);
            }
            let global = self.alloc_blob(data)?;
            slot_from_global(global, hot_meta)
        }
    }

    /// Returns a slice of the blob payload at flat `global_offset`. The chunk is
    /// recovered by `global_offset / chunk_size`. Returns `None` (never UB) for
    /// an out-of-range chunk or offset, so a crafted image resolves cleanly.
    #[inline]
    #[must_use]
    pub fn get_blob_slice(&self, global_offset: u64) -> Option<&[u8]> {
        let offset = usize::try_from(global_offset).ok()?;
        let chunk_idx = offset / self.chunk_size;
        let offset_in_chunk = offset % self.chunk_size;
        self.chunks.get(chunk_idx)?.get_slice(offset_in_chunk)
    }

    /// Resolves an `ArenaMeta` `locator` (a `global / 16` address in 16-byte
    /// units) to its payload slice, or `None` if it does not resolve.
    #[inline]
    #[must_use]
    pub fn resolve_meta(&self, locator: u32) -> Option<&[u8]> {
        self.get_blob_slice((locator as u64) * (ARENA_ALIGN as u64))
    }

    /// Records that the blob at flat `global_offset` was deleted/overwritten,
    /// decrementing the live-byte accounting used to decide compaction.
    pub fn record_deleted(&mut self, global_offset: u64) {
        // Resolve the length and drop the borrow before mutating `live_bytes`.
        let len = self.get_blob_slice(global_offset).map(<[u8]>::len);
        if let Some(len) = len {
            self.live_bytes = self.live_bytes.saturating_sub(8 + len);
        }
    }

    /// Records deletion for an arena-backed `slot` (no-op for inline / non-arena
    /// slots, which own no arena bytes).
    pub fn record_deleted_slot(&mut self, slot: ValueSlot) {
        if slot.tag() == SlotTag::ArenaMeta {
            self.record_deleted((slot.arena_meta_locator() as u64) * (ARENA_ALIGN as u64));
        }
    }

    /// Copying compaction: every live `ArenaMeta` payload the index references
    /// is copied into a fresh arena, the index's `ValueSlot` locators are then
    /// rewritten to the new offsets, and the old chunk set is swapped out and
    /// disposed of (freed, or retired to the collector in deferred mode).
    /// Nothing is compacted in place.
    ///
    /// Two phases, all-or-nothing: every live payload is copied into the fresh
    /// arena *before* any index slot is rewritten. If any copy fails (e.g.
    /// [`ArenaError::AllocationFailed`] / [`ArenaError::OffsetOverflow`]) the
    /// method returns `Err` with both `self` and `index` left untouched — the
    /// half-built new arena is dropped and no index slot points into it.
    /// The new arena's generation is bumped so a **retired** chunk — one a
    /// pinned reader still holds a payload borrow into — fails the
    /// [`ArenaChunk::get_slice`] generation check rather than resolving to
    /// relocated bytes.
    ///
    /// That is the whole of what the bump buys, and it is narrower than it
    /// looks (#763). `self.chunks` is replaced wholesale here and every record
    /// in the new set carries the *new* generation, so an offset held from
    /// before the compaction and resolved against the compacted arena reads a
    /// header stamped with the current generation and **passes** the check. It
    /// then yields `None`, or another key's payload where a 16-byte-aligned
    /// stale offset lands on a re-packed record. Nothing unsound — every read
    /// is bounds-checked against `cursor` — but not the "fails closed" the
    /// bump suggests on its own.
    ///
    /// Callers never see that, because the only supported path,
    /// [`ExpanseBlobMap::compact`], rewrites every index slot in the same call
    /// and leaves no stale offset behind.
    pub fn compact_with_index(
        &mut self,
        index: &mut ExpanseMap,
    ) -> Result<CompactionStats, ArenaError> {
        let prepared = self.prepare_compaction(index)?;
        Ok(self.apply_compaction(index, prepared))
    }

    /// [`Self::compact_with_index`]'s first half: collects every `ArenaMeta`
    /// entry of `index` and copies its payload into a fresh arena that nothing
    /// else can reach (#1300 item 2). It writes nothing a reader of `self` or
    /// `index` loads, which is why it takes both by shared reference: a
    /// concurrent map runs it with writers excluded and readers admitted.
    /// A failure returns before anything observable has changed.
    pub(crate) fn prepare_compaction(
        &self,
        index: &ExpanseMap,
    ) -> Result<PreparedCompaction, ArenaError> {
        #[cfg(all(test, feature = "std"))]
        if let Some(hook) = PREPARE_HOOK.with(|h| h.borrow_mut().take()) {
            hook();
        }
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

        // Collect every arena-backed (`ArenaMeta`) entry into one list,
        // reserved once for the index's entry count (#1290 §28a): a failed
        // reservation returns before anything is changed, where a growing
        // vector would abort the process on allocation failure.
        let entries = usize::try_from(index.len()).map_err(|_| ArenaError::AllocationFailed)?;
        let mut rewrites: Vec<(Key, u64)> = Vec::new();
        rewrites
            .try_reserve_exact(entries)
            .map_err(|_| ArenaError::AllocationFailed)?;
        #[cfg(all(test, feature = "std"))]
        LAST_RELOCATION_RESERVE.with(|c| c.set(rewrites.capacity()));
        rewrites.extend(index.iter().filter_map(|(key, raw_slot)| {
            (ValueSlot::from_raw(raw_slot).tag() == SlotTag::ArenaMeta).then_some((key, raw_slot))
        }));

        // Phase 1: relocate every live payload into the new arena, rewriting
        // each entry in place to its new raw slot and keeping only the ones
        // relocated. A failure here returns before any index slot is touched,
        // so `self`/`index` stay consistent. The blob's 24-bit hot metadata
        // rides along; only its locator changes with the new location.
        let mut kept = 0;
        for i in 0..rewrites.len() {
            let (key, raw) = rewrites[i];
            let slot = ValueSlot::from_raw(raw);
            if let Some(payload) = self.resolve_meta(slot.arena_meta_locator()) {
                let global = new_arena.alloc_blob(payload)?;
                let new_slot = slot_from_global(global, slot.arena_meta_meta())?;
                rewrites[kept] = (key, new_slot.to_raw());
                kept += 1;
            }
        }
        rewrites.truncate(kept);

        Ok(PreparedCompaction {
            new_arena,
            rewrites,
            source_generation: self.generation,
            source_total_allocated: self.total_allocated,
        })
    }

    /// [`Self::compact_with_index`]'s second half: rewrites every relocated
    /// key's index slot, installs the compacted chunk set, republishes the
    /// reader table and disposes of the old chunks. Infallible: everything
    /// that could fail ran in [`Self::prepare_compaction`], which must have
    /// been taken from this arena with nothing changed since.
    pub(crate) fn apply_compaction(
        &mut self,
        index: &mut ExpanseMap,
        prepared: PreparedCompaction,
    ) -> CompactionStats {
        let PreparedCompaction {
            mut new_arena,
            rewrites,
            source_generation,
            source_total_allocated,
        } = prepared;
        debug_assert!(
            self.generation == source_generation && self.total_allocated == source_total_allocated,
            "a prepared compaction applied to an arena that changed after it was prepared"
        );
        let live_bytes_before = self.live_bytes;
        let total_allocated_before = self.total_allocated;
        let chunks_before = self.chunks.len();
        let live_records_moved = rewrites.len();

        // Phase 2: every relocation succeeded — apply the index rewrites.
        for (key, raw) in rewrites {
            if let Some(slot_ptr) = index.get_value_slot(key) {
                // SAFETY: slot_ptr points to the live slot of key in the index,
                // valid until the next structural mutation (none happens here).
                unsafe {
                    // A shared map's readers load this word concurrently
                    // (#1086): an atomic store, the same instruction as a
                    // plain one, and no `&mut` over the published word.
                    #[cfg(all(target_pointer_width = "64", feature = "std"))]
                    core::sync::atomic::AtomicU64::from_ptr(slot_ptr.as_ptr())
                        .store(raw, core::sync::atomic::Ordering::Relaxed);
                    #[cfg(not(all(target_pointer_width = "64", feature = "std")))]
                    {
                        *slot_ptr.as_ptr() = raw;
                    }
                }
            }
        }

        let live_bytes_after = new_arena.live_bytes;
        let total_allocated_after = new_arena.total_allocated;
        let chunks_after = new_arena.chunks.len();

        // Install the compacted chunk set piecewise (not `*self = new_arena`:
        // that would drop the old chunks immediately and discard the deferred
        // handle). In deferred mode the new reader table must be published —
        // making the old chunks unreachable to new readers — BEFORE those
        // chunks enter the collector's grace period; pinned readers holding
        // pre-compaction payload borrows keep reading the retired bytes,
        // which are never rewritten.
        let old_chunks =
            core::mem::replace(&mut self.chunks, core::mem::take(&mut new_arena.chunks));
        self.active_chunk = new_arena.active_chunk;
        self.total_allocated = new_arena.total_allocated;
        self.live_bytes = new_arena.live_bytes;
        self.reset_reclaim_baseline();
        self.generation = new_arena.generation;
        self.republish_table();
        self.dispose_chunks(old_chunks);

        CompactionStats {
            live_bytes_before,
            live_bytes_after,
            total_allocated_before,
            total_allocated_after,
            chunks_before,
            chunks_after,
            live_records_moved,
        }
    }

    /// Total allocated heap bytes in arena chunks: what the capacity cap
    /// ([`Self::max_capacity`]) counts.
    #[inline(always)]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.total_allocated
    }

    /// The capacity cap on allocated chunk bytes ([`Self::mem_used`]):
    /// [`DEFAULT_ARENA_CAPACITY`] unless the arena was built with
    /// [`Self::with_chunk_size_and_max_capacity`], as clamped there. Growth
    /// that would cross it fails with [`ArenaError::OffsetOverflow`].
    #[inline]
    #[must_use]
    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    /// Active live bytes: each live record's payload plus its 8-byte header.
    /// [`Self::mem_used`] minus this is what a compaction could free, less the
    /// unused tails of the chunks it packs the records into.
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
        self.republish_table();
    }

    /// Multi-writer private arena (Refs #929, AGENTS.md §2.7): appends a fresh
    /// chunk that one optimistic writer slot will bump-allocate into privately,
    /// and republishes the reader table so records written into it resolve.
    ///
    /// The chunk is an ordinary member of the chunk set — a locator into it
    /// is `index * chunk_size + offset` like any other — with two differences
    /// while it is privately owned: it never becomes `active_chunk`, so
    /// [`Self::alloc_blob`] never writes into it, and its `cursor` is parked at
    /// `capacity`, because the owner's bump pointer lives outside the arena.
    /// The single-threaded resolver ([`ArenaChunk::get_slice`]) therefore bounds
    /// by capacity exactly as the optimistic resolver does, and the same
    /// argument covers it: unwritten bytes are zero and generation 0 is never
    /// live. `total_allocated` is charged here; `live_bytes` is charged by the
    /// owner's per-slot delta ([`Self::fold_live_delta`]).
    ///
    /// The same caps as [`Self::alloc_blob`]'s growth path apply.
    #[cfg(all(
        not(feature = "ablation-blob-shared-arena"),
        not(feature = "ablation-blob-serial-writers"),
        feature = "std"
    ))]
    pub(crate) fn grant_private_chunk(&mut self) -> Result<PrivateChunk, ArenaError> {
        let index = self.chunks.len();
        if index >= MAX_ARENA_CHUNKS {
            return Err(ArenaError::OffsetOverflow);
        }
        if self.total_allocated.saturating_add(self.chunk_size) > self.max_capacity {
            return Err(ArenaError::OffsetOverflow);
        }
        let mut chunk = ArenaChunk::new(self.chunk_size, self.generation)?;
        chunk.cursor = chunk.capacity;
        let grant = PrivateChunk {
            base: chunk.ptr,
            index,
            capacity: chunk.capacity,
            generation: chunk.generation,
        };
        self.chunks.push(chunk);
        self.total_allocated += self.chunk_size;
        // The table must name the chunk before any record in it is published
        // through the index: the owner writes records only after this returns.
        self.republish_table();
        Ok(grant)
    }

    /// Multi-writer private arena (Refs #929): folds one writer slot's
    /// signed live-byte delta into `live_bytes`. A slot's delta can be negative
    /// (it overwrote more bytes than it allocated); the folded total cannot.
    /// A negative total is an accounting defect and is not clamped to the
    /// clean value (AGENTS.md §8.22.3): debug builds panic, and a release
    /// build wraps to a conspicuous value instead of reporting zero.
    #[cfg(all(
        not(feature = "ablation-blob-shared-arena"),
        not(feature = "ablation-blob-serial-writers"),
        feature = "std"
    ))]
    pub(crate) fn fold_live_delta(&mut self, delta: isize) {
        let sum = (self.live_bytes as isize).wrapping_add(delta);
        debug_assert!(sum >= 0, "arena live_bytes folded below zero: {sum}");
        self.live_bytes = sum as usize;
    }

    /// Whether the reclaim rule lets an insert that the cap refused compact
    /// the arena (#1290, `docs/benchmarks/concurrency/METHODOLOGY.md` §28a):
    ///
    /// - the copy budget: the live bytes a compaction copies are at most
    ///   [`RECLAIM_COPY_PER_GROWTH`] times the chunk bytes grown plus the live
    ///   bytes dropped since the baseline. Every compaction resets the
    ///   baseline, so no workload triggers two without an insert or a removal
    ///   in between;
    /// - the waste guard: the live bytes are under half the allocated chunk
    ///   bytes. An arena filled with live records never passes it, so a bulk
    ///   load that reaches the cap fails without a copy that frees nothing,
    ///   and no automatic copy reaches half the cap.
    ///
    /// On a concurrent map `live_bytes` is exact only once the writers'
    /// private deltas are folded, so the wrapper reads this after folding.
    #[must_use]
    pub(crate) fn reclaim_allowed(&self) -> bool {
        let grown = self.total_allocated.saturating_sub(self.compacted_total);
        let dropped = self.compacted_live.saturating_sub(self.live_bytes);
        self.live_bytes <= RECLAIM_COPY_PER_GROWTH.saturating_mul(grown.saturating_add(dropped))
            && self.live_bytes.saturating_mul(2) < self.total_allocated
    }

    /// Whether [`Self::alloc_blob`] would refuse a `len`-byte payload at the
    /// capacity cap or the chunk-count limit ([`ArenaError::OffsetOverflow`]):
    /// the active chunk cannot fit it, and opening another chunk would cross a
    /// limit. Reads only; a payload larger than a chunk is not a cap refusal.
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    pub(crate) fn alloc_would_refuse_at_cap(&self, len: usize) -> bool {
        if 8 + len > self.chunk_size {
            return false;
        }
        if let Some(idx) = self.active_chunk
            && self.chunks[idx].can_fit(len)
        {
            return false;
        }
        self.chunks.len() >= MAX_ARENA_CHUNKS
            || self.total_allocated.saturating_add(self.chunk_size) > self.max_capacity
    }

    /// Moves the reclaim rule's baseline to the arena's current state: after a
    /// compaction, a `clear`, or a compaction that failed and must not be
    /// retried until the arena grows or loses live bytes.
    fn reset_reclaim_baseline(&mut self) {
        self.compacted_total = self.total_allocated;
        self.compacted_live = self.live_bytes;
    }

    /// Resets and frees all arena chunks (retired through the collector in
    /// deferred mode — pinned readers may still hold payload borrows).
    pub fn clear(&mut self) {
        let old_chunks = core::mem::take(&mut self.chunks);
        self.active_chunk = None;
        self.total_allocated = 0;
        self.live_bytes = 0;
        self.reset_reclaim_baseline();
        // Unpublish (null table) before the chunks enter the grace period.
        self.republish_table();
        self.dispose_chunks(old_chunks);
    }
}

impl Drop for BlobArena {
    fn drop(&mut self) {
        // Dropping the arena proves exclusive ownership (concurrent wrappers
        // hand out readers only for the arena's lifetime), so the published
        // table is freed directly; chunks still owned by `self.chunks` free
        // via `ArenaChunk::drop`, and already-retired ones drain with the
        // collector.
        #[cfg(feature = "std")]
        let Some(deferred) = self.deferred.get() else {
            return;
        };
        // A concurrent wrapper may hold the cell past this drop; leave it
        // null rather than naming the freed table.
        #[cfg(feature = "std")]
        let table = deferred
            .reader_table
            .swap(core::ptr::null_mut(), Ordering::AcqRel);
        #[cfg(not(feature = "std"))]
        let table: *mut ChunkTable = core::ptr::null_mut();
        if let Some(table) = NonNull::new(table) {
            // SAFETY: `table` was allocated by `republish_table` with
            // `table_layout(len)`; its `len` header field is immutable.
            unsafe {
                let layout = table_layout((*table.as_ptr()).len);
                dealloc(table.cast::<u8>().as_ptr(), layout);
            }
        }
    }
}

/// A high-level map from 64-bit keys to arbitrary-length byte blobs backed by
/// inline value slots and chunked arena slabs.
pub struct ExpanseBlobMap {
    index: ExpanseMap,
    pub(crate) arena: BlobArena,
    /// Whether an insert that the capacity cap refuses may compact the arena
    /// under the reclaim rule ([`Self::set_reclaim_at_cap`]).
    reclaim_at_cap: bool,
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
            reclaim_at_cap: true,
        }
    }

    /// Creates an empty blob map with custom arena chunk size and maximum capacity ceiling.
    ///
    /// `chunk_size` is clamped into `[4096, ArenaChunk::MAX_CHUNK_CAPACITY]`
    /// (see [`BlobArena::new`]), and `max_capacity` is clamped to at least `chunk_size`
    /// and at most [`ARENA_META_CEILING`] (capped at `usize::MAX` on 32-bit platforms).
    ///
    /// ## Behavior at Capacity Ceiling
    ///
    /// When total chunk allocations reach `max_capacity`, an [`insert`](Self::insert) that needs
    /// another chunk may compact the arena under the reclaim rule and retry once (#1290); if it
    /// still cannot be satisfied it fails with [`ArenaError::OffsetOverflow`] when no compaction
    /// ran, [`ArenaError::ArenaFull`] when one did (or [`ArenaError::AllocationFailed`] if the
    /// compaction cannot allocate). The map's contents are then unchanged, but after a compaction
    /// every arena payload has moved.
    ///
    /// Note on 32-bit targets: [`ExpanseBlobMap32`](crate::blobmap32::ExpanseBlobMap32) uses a fixed
    /// 12-bit addressable slab (at most 4095 entries) per `docs/design/32-bit-embedded.md`, where
    /// capacity is bounded by the fixed slab structure rather than dynamic chunk expansion.
    #[must_use]
    pub fn with_chunk_size_and_max_capacity(chunk_size: usize, max_capacity: usize) -> Self {
        Self {
            index: ExpanseMap::new(),
            arena: BlobArena::with_chunk_size_and_max_capacity(chunk_size, max_capacity),
            reclaim_at_cap: true,
        }
    }

    /// The index's allocator, reached through a raw pointer to the map
    /// without a reference to the whole map (#1086; see
    /// [`ExpanseMap::alloc_of`]).
    ///
    /// # Safety
    ///
    /// `this` points to a live map for `'a`, and no `&mut` to it exists
    /// meanwhile.
    #[cfg(feature = "std")]
    #[cfg_attr(feature = "ablation-blob-serial-writers", allow(dead_code))]
    #[inline(always)]
    pub(crate) unsafe fn alloc_of<'a>(this: *const Self) -> &'a crate::alloc::NodeAlloc {
        // SAFETY: caller contract; the field is projected through the raw
        // pointer, so no reference to the whole map is formed.
        unsafe { ExpanseMap::alloc_of(core::ptr::addr_of!((*this).index)) }
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

    #[inline(always)]
    #[cfg(feature = "std")]
    pub(crate) unsafe fn root_top_ptr(&self) -> *mut crate::node::Edge {
        // SAFETY: forwarded contract from ExpanseMap::root_top_ptr.
        unsafe { self.index.root_top_ptr() }
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn root_is_tree(&self) -> bool {
        self.index.root_is_tree()
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn clear_path(&self) {
        self.index.clear_path();
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn set_tree_pop(&mut self, pop: u64) {
        self.index.set_tree_pop(pop);
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn prepare_slot(
        &mut self,
        data: &[u8],
        hot_meta: u32,
    ) -> Result<ValueSlot, ArenaError> {
        self.arena.prepare_slot(data, hot_meta)
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn insert_slot(&mut self, key: Key, slot: ValueSlot) {
        if let Some(old_raw) = self.index.insert(key, slot.to_raw()) {
            let old = ValueSlot::from_raw(old_raw);
            self.arena.record_deleted_slot(old);
        }
    }

    /// [`Self::insert_slot`] for the concurrent wrapper (#1086): the index's
    /// shared entry, whose root-leaf stores are atomic words.
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers")
    ))]
    pub(crate) fn insert_slot_shared(&mut self, key: Key, slot: ValueSlot) {
        if let Some(old_raw) = self.index.insert_shared(key, slot.to_raw()) {
            let old = ValueSlot::from_raw(old_raw);
            self.arena.record_deleted_slot(old);
        }
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[allow(dead_code)]
    pub(crate) fn record_deleted_slot(&mut self, slot: ValueSlot) {
        self.arena.record_deleted_slot(slot);
    }

    /// Multi-writer private arena (Refs #929): [`BlobArena::grant_private_chunk`]
    /// through a raw map pointer, borrowing only the `arena` field.
    ///
    /// # Safety
    ///
    /// `this` must point to a live map, and the caller must exclude every other
    /// mutation of the arena's chunk set for the call (the wrapper's
    /// `arena_write` mutex inside the writer gate, or quiescence).
    #[cfg(all(
        not(feature = "ablation-blob-shared-arena"),
        not(feature = "ablation-blob-serial-writers"),
        feature = "std"
    ))]
    pub(crate) unsafe fn grant_private_chunk_raw(
        this: *mut Self,
    ) -> Result<PrivateChunk, ArenaError> {
        // SAFETY: `this` is live and the chunk set is exclusively the caller's
        // for the call, per this function's contract. The borrow covers the
        // `arena` field alone, not the index other writers are descending.
        unsafe {
            let arena_ptr = core::ptr::addr_of_mut!((*this).arena);
            (*arena_ptr).grant_private_chunk()
        }
    }

    /// Multi-writer private arena (Refs #929): see [`BlobArena::fold_live_delta`].
    #[cfg(all(
        not(feature = "ablation-blob-shared-arena"),
        not(feature = "ablation-blob-serial-writers"),
        feature = "std"
    ))]
    pub(crate) fn fold_arena_live_delta(&mut self, delta: isize) {
        self.arena.fold_live_delta(delta);
    }

    // No `arena_mut`. The index stores flat arena offsets, so handing out
    // `&mut BlobArena` is a licence to change one half of a two-part invariant
    // whose other half the holder cannot see (#763). Every mutation reachable
    // through it desynchronises the two: `clear` frees the chunks while the
    // index keeps its slots, `record_deleted` double-counts against the
    // `record_deleted_slot` calls `insert`/`remove` already make, and
    // `alloc_blob`/`push_chunk` grow the arena behind the index's back.
    //
    // The one arena mutation that preserves the invariant, `compact_with_index`,
    // was never reachable through it anyway: it needs `&mut` to both halves and
    // `index()` is shared-only, so the only call that compiled passed a foreign
    // index. `ExpanseBlobMap::compact()` is that operation, done correctly, and
    // it reached past this accessor to the field rather than using it.
    //
    // `arena()` stays: shared access to `mem_used`/`live_bytes`/`generation`
    // has an obvious use and no hazard. If a narrow need appears, add a method
    // on the map named for the intent (`reserve_chunks`), not a hole in the
    // encapsulation named for the field.

    /// Inserts a key-blob pair with 32-bit hot metadata.
    ///
    /// Payloads `<= 7 bytes` are stored inline with zero heap allocation.
    /// Payloads `> 7 bytes` are allocated in the slab arena.
    ///
    /// Note: inline (`<= 7` byte) payloads store their bytes in the slot word and
    /// carry no metadata field, so `hot_meta` is ignored for them and later reads
    /// report their metadata as `0`. Arena payloads (`> 7` bytes) all carry the
    /// 24-bit metadata; `hot_meta` exceeding 24 bits returns
    /// [`ArenaError::MetaOverflow`] rather than being truncated.
    ///
    /// When the arena's capacity cap refuses the chunk an arena payload needs,
    /// the insert runs one [`Self::compact`] and retries once, if the reclaim
    /// rule allows: the live bytes the compaction copies are at most the chunk
    /// bytes grown plus the live bytes dropped since the previous compaction,
    /// and under half the allocated chunk bytes (#1290,
    /// `docs/design/large-values.md` §6.3.1; [`Self::set_reclaim_at_cap`]
    /// turns it off). That insert pays for copying every live payload, under
    /// half the cap, and holds the old and the new chunk sets at once while it
    /// does.
    ///
    /// When the rule declines (or is switched off), the insert fails with
    /// [`ArenaError::OffsetOverflow`] and compacts nothing; the arena may still
    /// hold dead bytes, which [`BlobArena::live_bytes`] and
    /// [`BlobArena::mem_used`] show. When the arena is still full after
    /// compacting, it fails with [`ArenaError::ArenaFull`]; if the compaction
    /// itself cannot allocate, with [`ArenaError::AllocationFailed`]. Either
    /// way the map's contents are unchanged, but after a compaction its layout
    /// is not: every arena payload has moved, the arena generation has
    /// advanced and [`Self::mem_used`] reports the compacted size.
    pub fn insert(&mut self, key: Key, data: &[u8], hot_meta: u32) -> Result<(), ArenaError> {
        let slot = if data.len() <= 7 {
            ValueSlot::new_inline(data).ok_or(ArenaError::AllocationFailed)?
        } else if hot_meta == 0
            && let Some(slot) = crate::codec::try_compress_inline(data)
        {
            slot
        } else {
            // Validate the metadata envelope *before* allocating arena bytes, so a
            // rejected insert leaves no orphaned payload behind.
            if hot_meta > ValueSlot::ARENA_META_MAX {
                return Err(ArenaError::MetaOverflow);
            }
            let global = self.alloc_payload(data)?;
            slot_from_global(global, hot_meta)?
        };

        if let Some(old_raw) = self.index.insert(key, slot.to_raw()) {
            self.arena.record_deleted_slot(ValueSlot::from_raw(old_raw));
        }
        Ok(())
    }

    /// [`Self::insert`]'s arena allocation, out of line so `insert` makes one
    /// call and holds nothing across it, as it did before the reclaim rule. The
    /// retry needs `data` after a refusal, and only [`BlobArena::alloc_blob`]'s
    /// growth call can refuse; with the fast path inlined here, `data` stays live
    /// across that cold call alone.
    #[inline(never)]
    fn alloc_payload(&mut self, data: &[u8]) -> Result<u64, ArenaError> {
        match self.arena.alloc_blob(data) {
            Err(ArenaError::OffsetOverflow) => self.alloc_after_reclaim(data),
            res => res,
        }
    }

    /// The cap refused [`Self::insert`] a new chunk: compact once if the
    /// reclaim rule allows, then allocate again (#1290). The index is untouched
    /// until the allocation succeeds, so the compaction relocates only records
    /// that are already indexed.
    #[cold]
    #[inline(never)]
    fn alloc_after_reclaim(&mut self, data: &[u8]) -> Result<u64, ArenaError> {
        if !self.reclaim_for_insert()? {
            return Err(ArenaError::OffsetOverflow);
        }
        self.arena
            .alloc_blob(data)
            .map_err(refused_after_compaction)
    }

    /// Compacts once if the reclaim rule allows (#1290), returning whether it
    /// did. A compaction that fails leaves the map untouched, and the rule's
    /// baseline moves to the current state so the next refused insert does not
    /// repeat it before the arena grows or loses live bytes.
    fn reclaim_for_insert(&mut self) -> Result<bool, ArenaError> {
        if !self.reclaim_at_cap || !self.arena.reclaim_allowed() {
            return Ok(false);
        }
        match self.compact() {
            Ok(_) => Ok(true),
            Err(e) => {
                self.arena.reset_reclaim_baseline();
                Err(e)
            }
        }
    }

    /// [`Self::insert_shared`], compacting once under the reclaim rule if the
    /// cap refuses its allocation (#1290). Returns the insert's result and
    /// whether a compaction ran, which the concurrent wrapper needs in order
    /// to reset its writers' private chunks. The caller must exclude every
    /// other writer and have folded their live-byte deltas.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn insert_shared_reclaiming(
        &mut self,
        key: Key,
        data: &[u8],
        hot_meta: u32,
    ) -> (Result<(), ArenaError>, bool) {
        match self.insert_shared(key, data, hot_meta) {
            Err(ArenaError::OffsetOverflow) => match self.reclaim_for_insert() {
                Ok(true) => (
                    self.insert_shared(key, data, hot_meta)
                        .map_err(refused_after_compaction),
                    true,
                ),
                Ok(false) => (Err(ArenaError::OffsetOverflow), false),
                Err(e) => (Err(e), false),
            },
            res => (res, false),
        }
    }

    /// The half of a compaction that runs with readers admitted (#1300 item 2):
    /// the copy into a fresh arena. [`Self::apply_compaction`] finishes it.
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    pub(crate) fn prepare_compaction(&self) -> Result<PreparedCompaction, ArenaError> {
        self.arena.prepare_compaction(&self.index)
    }

    /// The half of a compaction that changes what readers load: the index
    /// rewrites, the chunk-set swap and the table republish.
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    pub(crate) fn apply_compaction(&mut self, prepared: PreparedCompaction) -> CompactionStats {
        self.arena.apply_compaction(&mut self.index, prepared)
    }

    /// Whether [`Self::insert_shared`] of `data` would be refused at the
    /// capacity cap with the reclaim rule allowing a compaction: the payload
    /// goes to the arena (not inline, not compressed inline, metadata in
    /// range), the arena would refuse its chunk, and the rule admits a copy.
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    fn insert_needs_reclaim(&self, data: &[u8], hot_meta: u32) -> bool {
        let to_arena = data.len() > 7
            && hot_meta <= ValueSlot::ARENA_META_MAX
            && !(hot_meta == 0 && crate::codec::try_compress_inline(data).is_some());
        self.reclaim_at_cap
            && to_arena
            && self.arena.alloc_would_refuse_at_cap(data.len())
            && self.arena.reclaim_allowed()
    }

    /// The part of [`Self::insert_shared_reclaiming`] that may run before the
    /// concurrent wrapper opens the tree bracket (#1300 item 2): when the
    /// insert would be refused at the cap and the rule allows a compaction,
    /// the copy. `None` when no compaction is foreseen. A failed copy moves
    /// the rule's baseline, as a failed compaction does, and is returned for
    /// [`Self::insert_shared_after_prepare`] to report.
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    pub(crate) fn prepare_reclaim_for_insert(
        &mut self,
        data: &[u8],
        hot_meta: u32,
    ) -> Option<Result<PreparedCompaction, ArenaError>> {
        if !self.insert_needs_reclaim(data, hot_meta) {
            return None;
        }
        let prepared = self.arena.prepare_compaction(&self.index);
        if prepared.is_err() {
            self.arena.reset_reclaim_baseline();
        }
        Some(prepared)
    }

    /// [`Self::insert_shared_reclaiming`] with the copy already prepared by
    /// [`Self::prepare_reclaim_for_insert`]. The insert is attempted first, so
    /// the outcome is the one the unprepared path would give: a prepared copy
    /// is applied only if the cap refuses the insert, and dropped otherwise.
    /// With nothing prepared it is [`Self::insert_shared_reclaiming`].
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    pub(crate) fn insert_shared_after_prepare(
        &mut self,
        key: Key,
        data: &[u8],
        hot_meta: u32,
        prepared: Option<Result<PreparedCompaction, ArenaError>>,
    ) -> (Result<(), ArenaError>, bool) {
        let Some(prepared) = prepared else {
            return self.insert_shared_reclaiming(key, data, hot_meta);
        };
        match self.insert_shared(key, data, hot_meta) {
            Err(ArenaError::OffsetOverflow) => match prepared {
                Ok(p) => {
                    self.arena.apply_compaction(&mut self.index, p);
                    (
                        self.insert_shared(key, data, hot_meta)
                            .map_err(refused_after_compaction),
                        true,
                    )
                }
                Err(e) => (Err(e), false),
            },
            res => (res, false),
        }
    }

    /// [`Self::insert`] for the concurrent wrapper; see [`Self::insert_slot_shared`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn insert_shared(
        &mut self,
        key: Key,
        data: &[u8],
        hot_meta: u32,
    ) -> Result<(), ArenaError> {
        let slot = if data.len() <= 7 {
            ValueSlot::new_inline(data).ok_or(ArenaError::AllocationFailed)?
        } else if hot_meta == 0
            && let Some(slot) = crate::codec::try_compress_inline(data)
        {
            slot
        } else {
            // Validate the metadata envelope *before* allocating arena bytes, so a
            // rejected insert leaves no orphaned payload behind.
            if hot_meta > ValueSlot::ARENA_META_MAX {
                return Err(ArenaError::MetaOverflow);
            }
            let global = self.arena.alloc_blob(data)?;
            slot_from_global(global, hot_meta)?
        };

        if let Some(old_raw) = self.index.insert_shared(key, slot.to_raw()) {
            self.arena.record_deleted_slot(ValueSlot::from_raw(old_raw));
        }
        Ok(())
    }

    /// Point lookup returning a zero-copy [`BlobView`] and the 32-bit hot metadata word.
    ///
    /// Inline (`<= 7` byte raw or compressed) payloads do not store metadata; their returned
    /// `hot_meta` is always `0`.
    #[must_use]
    pub fn get<'a>(&'a self, key: Key) -> Option<(BlobView<'a>, u32)> {
        let slot_ptr = self.index.get_slot_ptr(key)?;
        // SAFETY: slot_ptr points to the live 64-bit value slot inside self.index.
        let raw_slot = unsafe { *slot_ptr.as_ptr() };
        let slot = ValueSlot::from_raw(raw_slot);
        let tag = slot.tag();
        match tag {
            SlotTag::Inline0
            | SlotTag::Inline1
            | SlotTag::Inline2
            | SlotTag::Inline3
            | SlotTag::Inline4
            | SlotTag::Inline5
            | SlotTag::Inline6
            | SlotTag::Inline7 => {
                let len = tag as u8 as usize;
                // SAFETY: In little-endian representation, byte offsets 1..=len contain
                // the inline payload bytes. The slot memory is owned by self.index and
                // valid for lifetime 'a.
                let slice = unsafe {
                    let bytes = slot_ptr.as_ptr().cast::<u8>();
                    core::slice::from_raw_parts(bytes.add(1), len)
                };
                Some((BlobView::Inline(slice), 0))
            }
            SlotTag::ArenaMeta => {
                let meta = slot.arena_meta_meta();
                let slice = self.arena.resolve_meta(slot.arena_meta_locator())?;
                Some((BlobView::Arena(slice), meta))
            }
            _ if tag.is_compressed_inline() => {
                let mut buf = [0u8; 16];
                let decoded_len = crate::codec::decompress_inline(slot, &mut buf)?;
                Some((
                    BlobView::CompressedInline {
                        buf,
                        len: decoded_len as u8,
                    },
                    0,
                ))
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

    /// [`Self::remove`] for the concurrent wrapper; see [`Self::insert_slot_shared`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn remove_shared(&mut self, key: Key) -> bool {
        if let Some(raw_val) = self.index.remove_shared(key) {
            self.arena.record_deleted_slot(ValueSlot::from_raw(raw_val));
            true
        } else {
            false
        }
    }

    /// Compare-and-swap the 24-bit hot metadata stored for `key`.
    pub fn compare_exchange_meta(
        &mut self,
        key: Key,
        expected: Option<u32>,
        new: Option<u32>,
    ) -> Result<Option<u32>, Option<u32>> {
        let cur_slot_raw = self.index.get(key);
        let cur_meta = cur_slot_raw.map(|raw| {
            let slot = ValueSlot::from_raw(raw);
            if slot.tag() == SlotTag::ArenaMeta {
                slot.arena_meta_meta()
            } else {
                0
            }
        });
        if cur_meta != expected {
            return Err(cur_meta);
        }
        match (cur_slot_raw, new) {
            (None, None) => Ok(None),
            (None, Some(_)) => Err(None),
            (Some(raw), None) => {
                let slot = ValueSlot::from_raw(raw);
                self.index.remove(key);
                self.arena.record_deleted_slot(slot);
                Ok(cur_meta)
            }
            (Some(raw), Some(new_m)) => {
                let slot = ValueSlot::from_raw(raw);
                if slot.tag() == SlotTag::ArenaMeta {
                    if let Some(new_slot) = slot.with_arena_meta_meta(new_m) {
                        self.index.insert(key, new_slot.to_raw());
                        Ok(cur_meta)
                    } else {
                        Err(cur_meta)
                    }
                } else if new_m == 0 {
                    Ok(cur_meta)
                } else {
                    Err(cur_meta)
                }
            }
        }
    }

    /// Compare-and-swap the 24-bit hot metadata stored for `key` (shared index).
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn compare_exchange_meta_shared(
        &mut self,
        key: Key,
        expected: Option<u32>,
        new: Option<u32>,
    ) -> Result<Option<u32>, Option<u32>> {
        let cur_slot_raw = self.index.get(key);
        let cur_meta = cur_slot_raw.map(|raw| {
            let slot = ValueSlot::from_raw(raw);
            if slot.tag() == SlotTag::ArenaMeta {
                slot.arena_meta_meta()
            } else {
                0
            }
        });
        if cur_meta != expected {
            return Err(cur_meta);
        }
        match (cur_slot_raw, new) {
            (None, None) => Ok(None),
            (None, Some(_)) => Err(None),
            (Some(raw), None) => {
                let slot = ValueSlot::from_raw(raw);
                self.index.remove_shared(key);
                self.arena.record_deleted_slot(slot);
                Ok(cur_meta)
            }
            (Some(raw), Some(new_m)) => {
                let slot = ValueSlot::from_raw(raw);
                if slot.tag() == SlotTag::ArenaMeta {
                    if let Some(new_slot) = slot.with_arena_meta_meta(new_m) {
                        self.index.insert_shared(key, new_slot.to_raw());
                        Ok(cur_meta)
                    } else {
                        Err(cur_meta)
                    }
                } else if new_m == 0 {
                    Ok(cur_meta)
                } else {
                    Err(cur_meta)
                }
            }
        }
    }

    /// Compare-and-swap the payload and metadata stored for `key`.
    ///
    /// # Allocation Note on Failure
    /// On mismatch, returning `Err(Some((Vec<u8>, u32)))` allocates a fresh `Vec<u8>`
    /// to return the observed payload bytes. In high-contention loops where CAS operations
    /// retry frequently, allocating on failure introduces heap overhead.
    ///
    /// For workloads where synchronization is governed by metadata (e.g. sequence numbers,
    /// version counters, or status tags in the 24-bit metadata field), callers should prefer
    /// [`Self::compare_exchange_meta`], which executes directly on the in-slot `hot_meta`
    /// word with zero heap allocation and zero arena access on both success and failure.
    #[allow(clippy::type_complexity)]
    pub fn compare_exchange(
        &mut self,
        key: Key,
        expected: Option<(&[u8], u32)>,
        new: Option<(&[u8], u32)>,
    ) -> Result<Option<(Vec<u8>, u32)>, Option<(Vec<u8>, u32)>> {
        let cur = self.get(key).map(|(v, m)| (v.as_bytes().to_vec(), m));
        let matches = match (&cur, expected) {
            (None, None) => true,
            (Some((cur_b, cur_m)), Some((exp_b, exp_m))) => cur_b == exp_b && *cur_m == exp_m,
            _ => false,
        };
        if !matches {
            return Err(cur);
        }
        match new {
            Some((data, meta)) => {
                if self.insert(key, data, meta).is_ok() {
                    Ok(cur)
                } else {
                    Err(cur)
                }
            }
            None => {
                self.remove(key);
                Ok(cur)
            }
        }
    }

    /// Compare-and-swap the payload and metadata stored for `key` (shared index).
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[allow(clippy::type_complexity)]
    pub(crate) fn compare_exchange_shared(
        &mut self,
        key: Key,
        expected: Option<(&[u8], u32)>,
        new: Option<(&[u8], u32)>,
    ) -> Result<Option<(Vec<u8>, u32)>, Option<(Vec<u8>, u32)>> {
        let cur = self.get(key).map(|(v, m)| (v.as_bytes().to_vec(), m));
        let matches = match (&cur, expected) {
            (None, None) => true,
            (Some((cur_b, cur_m)), Some((exp_b, exp_m))) => cur_b == exp_b && *cur_m == exp_m,
            _ => false,
        };
        if !matches {
            return Err(cur);
        }
        match new {
            Some((data, meta)) => {
                if self.insert_shared(key, data, meta).is_ok() {
                    Ok(cur)
                } else {
                    Err(cur)
                }
            }
            None => {
                self.remove_shared(key);
                Ok(cur)
            }
        }
    }

    /// Executes a range scan with a predicate evaluated against hot metadata
    /// before dereferencing cold payload cache lines.
    ///
    /// Inline (`<= 7` byte raw or compressed) payloads have no stored metadata; the predicate and
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
            let tag = slot.tag();
            // Inline slots store payload bytes in the slot word, not metadata;
            // only `ArenaMeta` slots carry a real hot-metadata field. Reading it
            // on an inline slot would feed payload garbage to the predicate, so
            // report inline metadata as 0.
            let meta = if tag == SlotTag::ArenaMeta {
                slot.arena_meta_meta()
            } else {
                0
            };
            if !predicate(key, meta) {
                continue;
            }
            // Resolve the payload directly from the slot the range walk already
            // holds — no per-match `get(key)` re-descent through the trie (#355).
            // `le_bytes` must outlive the match so an inline `BlobView` can borrow
            // the slot word's payload bytes; keep it bound in the loop body.
            let le_bytes = raw_slot.to_le_bytes();
            let view = match tag {
                SlotTag::Inline0
                | SlotTag::Inline1
                | SlotTag::Inline2
                | SlotTag::Inline3
                | SlotTag::Inline4
                | SlotTag::Inline5
                | SlotTag::Inline6
                | SlotTag::Inline7 => {
                    // Little-endian: `ValueSlot::new_inline` writes payload byte
                    // `i` at slot byte `i + 1`, so bytes `1..=len` are the payload.
                    let len = tag as u8 as usize;
                    BlobView::Inline(&le_bytes[1..1 + len])
                }
                SlotTag::ArenaMeta => match self.arena.resolve_meta(slot.arena_meta_locator()) {
                    Some(slice) => BlobView::Arena(slice),
                    // A slot whose locator no longer resolves (e.g. stale after a
                    // compaction) is skipped, exactly as `get` would return `None`.
                    None => continue,
                },
                _ if tag.is_compressed_inline() => {
                    let mut buf = [0u8; 16];
                    let Some(decoded_len) = crate::codec::decompress_inline(slot, &mut buf) else {
                        continue;
                    };
                    BlobView::CompressedInline {
                        buf,
                        len: decoded_len as u8,
                    }
                }
                // Non-inline / non-arena tags carry no payload — skipped, matching
                // `get`'s `_ => None` arm.
                _ => continue,
            };
            if !callback(key, view, meta) {
                break;
            }
        }
    }

    /// Whether an insert that the capacity cap refuses a chunk may compact the
    /// arena under the reclaim rule (#1290, `docs/design/large-values.md`
    /// §6.3.1). On by default.
    #[must_use]
    pub fn reclaim_at_cap(&self) -> bool {
        self.reclaim_at_cap
    }

    /// Turns the reclaim at the capacity cap on or off. Off, an insert that the
    /// cap refuses fails with [`ArenaError::OffsetOverflow`] and compacts
    /// nothing, as before #1290: a caller that needs every insert's cost
    /// bounded by its own payload calls [`Self::compact`] itself.
    pub fn set_reclaim_at_cap(&mut self, on: bool) {
        self.reclaim_at_cap = on;
    }

    /// Runs in-place garbage collection and compaction.
    pub fn compact(&mut self) -> Result<CompactionStats, ArenaError> {
        self.arena.compact_with_index(&mut self.index)
    }

    /// Returns a reference to the internal index.
    ///
    /// Its values are raw [`ValueSlot`] words. Hot metadata exists only on
    /// [`SlotTag::ArenaMeta`] slots; an inline slot holds payload bytes in the
    /// same bits. Filter by metadata with [`Self::scan_filtered`], which
    /// decodes the tag, rather than by reading bits of the index words.
    #[must_use]
    pub fn index(&self) -> &ExpanseMap {
        &self.index
    }

    /// Phase 7 (issue #219): replaces the index with a copy rebuilt through
    /// an allocator deferred to `collector` **before any node is
    /// allocated**. Sharing a populated map requires this because a
    /// single-threaded index holds slab-carved node memory, which must
    /// never be retired to the collector (see `NodeAlloc::defer_to`); the
    /// old index (and its slab pages, wholesale) is freed here. Arena
    /// payloads are untouched — the raw `ValueSlot` words carry over.
    /// Binds the wrapper's tree-level version word to the index trie's
    /// allocator (#568 PR 3; see `NodeAlloc::bind_tree_word`).
    ///
    /// # Safety
    ///
    /// As `NodeAlloc::bind_tree_word`: `word` outlives every operation on
    /// this map.
    #[cfg(feature = "std")]
    pub(crate) unsafe fn bind_tree_word(&self, word: *const crate::occ::SeqVersion) {
        // SAFETY: forwarded contract.
        unsafe { self.index.occ_root().1.bind_tree_word(word) };
    }

    #[cfg(feature = "std")]
    pub(crate) fn rebuild_index_deferred(&mut self, collector: &Arc<Collector>) {
        let fresh = ExpanseMap::new();
        fresh.occ_root().1.defer_to(Arc::clone(collector));
        let mut fresh = fresh;
        for (key, raw_slot) in self.index.iter() {
            fresh.insert(key, raw_slot);
        }
        fresh.clear_path();
        self.index = fresh;
    }

    /// Serializes the blob map to a writer in relocatable binary image format.
    #[cfg(feature = "std")]
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
    #[cfg(feature = "std")]
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<usize> {
        let mut file = std::fs::File::create(path)?;
        self.save_to_writer(&mut file)
    }

    /// Deserializes a relocatable binary image from a byte slice, into a map
    /// whose arena capacity cap is [`DEFAULT_ARENA_CAPACITY`]. An image that
    /// declares more chunk bytes than that fails with
    /// [`ArenaError::OffsetOverflow`]; load it with
    /// [`Self::from_bytes_slice_with_max_capacity`] and a cap that holds it.
    pub fn from_bytes_slice(bytes: &[u8]) -> Result<Self, ArenaError> {
        Self::from_bytes_slice_with_max_capacity(bytes, DEFAULT_ARENA_CAPACITY)
    }

    /// Deserializes a relocatable binary image from a byte slice, into a map
    /// whose arena capacity cap is `max_capacity`, clamped as by
    /// [`Self::with_chunk_size_and_max_capacity`] with the image's chunk size.
    /// The cap is a growth policy and is not stored in the image, so the
    /// loader takes it from the caller.
    ///
    /// The image's declared chunk bytes (`chunk_count * chunk_size`) are
    /// bounded twice before anything is allocated: by the structural limits a
    /// valid image never crosses ([`MAX_ARENA_CHUNKS`] and the 64 GiB locator
    /// envelope, [`ARENA_META_CEILING`]), which fail with
    /// [`ArenaError::CorruptedHeader`], and by the clamped cap, which fails
    /// with [`ArenaError::OffsetOverflow`]. The second bound is what limits the
    /// allocation a small crafted image can drive, so a caller loading
    /// untrusted bytes keeps `max_capacity` at what it is prepared to allocate.
    pub fn from_bytes_slice_with_max_capacity(
        bytes: &[u8],
        max_capacity: usize,
    ) -> Result<Self, ArenaError> {
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

        if header.magic != EXPANSE_MAGIC {
            return Err(ArenaError::CorruptedHeader);
        }
        if header.version != EXPANSE_FORMAT_VERSION {
            return Err(ArenaError::UnsupportedFormatVersion {
                found: header.version,
                supported: EXPANSE_FORMAT_VERSION,
            });
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

        // Chunk-count sanity cap ([`MAX_ARENA_CHUNKS`]); a larger declared count
        // is treated as corruption and rejected.
        if header.chunk_count > MAX_ARENA_CHUNKS as u64 {
            return Err(ArenaError::CorruptedHeader);
        }
        // `ArenaMeta` locators are `global / 16`, so a chunk boundary must land on
        // a 16-byte-aligned global offset: reject an unaligned declared chunk size
        // rather than let it desync locator decoding.
        if !header.chunk_size.is_multiple_of(ARENA_ALIGN as u64) {
            return Err(ArenaError::CorruptedHeader);
        }

        // Bound the aggregate declared arena capacity twice. Structurally here:
        // no arena addresses more than the 64 GiB locator envelope, so a larger
        // declaration is corruption. Then, below, by the caller's cap, the
        // growth budget the loaded map keeps: a small crafted header could
        // otherwise declare a huge `chunk_count * chunk_size` and drive
        // `alloc_zeroed` into an OOM/DoS.
        let declared_capacity = header
            .chunk_count
            .checked_mul(header.chunk_size)
            .ok_or(ArenaError::CorruptedHeader)?;
        if declared_capacity > ARENA_META_CEILING {
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

        // Every structural check has passed: an image over the caller's cap
        // is valid but too large for this map, not corrupt.
        let mut map =
            Self::with_chunk_size_and_max_capacity(header.chunk_size as usize, max_capacity);
        if declared_capacity > map.arena.max_capacity as u64 {
            return Err(ArenaError::OffsetOverflow);
        }

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
            // point at the wrong chunk. Reject it. Generation 0 is likewise
            // never written by a live arena (`BlobArena::new` starts at 1
            // and compaction skips 0) and the read paths rely on "generation
            // 0 is never live" to reject zeroed unwritten bytes — a crafted
            // image declaring it must not load.
            if cap == 0
                || cap > ArenaChunk::MAX_CHUNK_CAPACITY
                || cap != header.chunk_size as usize
                || cur > cap
                || generation == 0
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

            // Recompute live_bytes for arena-backed slots.
            let slot = ValueSlot::from_raw(raw_slot);
            let payload_len = if slot.tag() == SlotTag::ArenaMeta {
                map.arena
                    .resolve_meta(slot.arena_meta_locator())
                    .map(<[u8]>::len)
            } else {
                None
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
    ///
    /// The loaded map's capacity cap is [`DEFAULT_ARENA_CAPACITY`], as for
    /// [`Self::from_bytes_slice`]; [`Self::load_from_file_with_max_capacity`]
    /// takes another.
    #[cfg(feature = "std")]
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ArenaError> {
        Self::load_from_file_with_max_capacity(path, DEFAULT_ARENA_CAPACITY)
    }

    /// [`Self::load_from_file`] into a map whose capacity cap is
    /// `max_capacity`, as [`Self::from_bytes_slice_with_max_capacity`] loads.
    #[cfg(feature = "std")]
    pub fn load_from_file_with_max_capacity<P: AsRef<std::path::Path>>(
        path: P,
        max_capacity: usize,
    ) -> Result<Self, ArenaError> {
        let bytes = std::fs::read(path).map_err(|_| ArenaError::CorruptedHeader)?;
        Self::from_bytes_slice_with_max_capacity(&bytes, max_capacity)
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
    use core_alloc::format;
    use core_alloc::vec;
    use core_alloc::vec::Vec;

    /// The #1290 reclaim tests' arena (METHODOLOGY §28.3): 4 KiB chunks
    /// under a 64 KiB cap, so 16 chunks, and `hot_meta = 1` on every insert
    /// so no payload is stored inline.
    const RECLAIM_CHUNK: usize = 4096;
    const RECLAIM_CAP: usize = 64 * 1024;

    fn reclaim_payload(key: u64, round: u64, len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| (key as u8) ^ ((round as u8).wrapping_mul(31)) ^ (i as u8))
            .collect()
    }

    /// Overwrites `live` keys round-robin `overwrites` times on a
    /// [`RECLAIM_CAP`] arena, returning the refused inserts and the
    /// compactions, counted by the arena generation, after the live set.
    fn overwrite_at_cap(live: u64, len: usize, overwrites: u64) -> (u64, u32) {
        let mut m = ExpanseBlobMap::with_chunk_size_and_max_capacity(RECLAIM_CHUNK, RECLAIM_CAP);
        for k in 0..live {
            m.insert(k, &reclaim_payload(k, 0, len), 1)
                .expect("the live set fits the arena");
        }
        let generation = m.arena().generation();
        let mut last = vec![0u64; live as usize];
        let mut refused = 0;
        for i in 0..overwrites {
            let (k, round) = (i % live, i / live + 1);
            match m.insert(k, &reclaim_payload(k, round, len), 1) {
                Ok(()) => last[k as usize] = round,
                Err(ArenaError::OffsetOverflow) => refused += 1,
                Err(e) => panic!("overwrite {i} failed with {e:?}"),
            }
        }
        for k in 0..live {
            let (view, meta) = m.get(k).expect("a live key went missing");
            assert_eq!(
                view.as_bytes(),
                &reclaim_payload(k, last[k as usize], len)[..]
            );
            assert_eq!(meta, 1);
        }
        let stats = m.compact().expect("compaction");
        assert_eq!(
            stats.live_bytes_before, stats.live_bytes_after,
            "the charged live bytes disagree with the index's"
        );
        (refused, m.arena().generation().wrapping_sub(generation) - 1)
    }

    /// #1290 G1.1-G1.4: the engine against `simulate` in
    /// `scripts/blob_reclaim_bounds.py`, which gives these counts: at the
    /// largest sustained live set no insert is refused, one record more and
    /// the rule compacts once, then declines.
    #[test]
    fn reclaim_matches_the_model_at_the_cap() {
        // 16 chunks hold 448 records of 128 B (28 per chunk); 4 x 448 overwrites.
        assert_eq!(overwrite_at_cap(224, 128, 1792), (0, 7), "G1.1");
        assert_eq!(overwrite_at_cap(225, 128, 1792), (1346, 1), "G1.2");
        // 9 B records: 17 bytes charged, a 32-byte stride, 128 per chunk.
        assert_eq!(overwrite_at_cap(1280, 9, 8192), (0, 10), "G1.3");
        assert_eq!(overwrite_at_cap(1281, 9, 8192), (6658, 1), "G1.4");
    }

    /// #1290 G1.5a (METHODOLOGY §28a), `simulate_fill_then_remove` in the
    /// bounds script: distinct keys until the cap refuses one. The arena is
    /// then more than half live, so the waste guard declines a compaction that
    /// would free nothing, and the next insert is refused the same way. After
    /// half the keys are removed, one compaction admits it.
    #[test]
    fn reclaim_after_removals_at_a_full_arena() {
        let mut m = ExpanseBlobMap::with_chunk_size_and_max_capacity(RECLAIM_CHUNK, RECLAIM_CAP);
        let g0 = m.arena().generation();
        let mut n = 0u64;
        loop {
            match m.insert(n, &reclaim_payload(n, 0, 128), 1) {
                Ok(()) => n += 1,
                Err(ArenaError::OffsetOverflow) => break,
                Err(e) => panic!("insert {n} failed with {e:?}"),
            }
        }
        assert_eq!(n, 448, "16 chunks of 28 records");
        assert_eq!(
            m.arena().generation(),
            g0,
            "no compaction of an arena more than half live"
        );
        assert_eq!(
            m.insert(n, &reclaim_payload(n, 0, 128), 1),
            Err(ArenaError::OffsetOverflow)
        );
        assert_eq!(m.arena().generation(), g0, "nothing changed since");
        for k in 0..n / 2 {
            assert!(m.remove(k));
        }
        m.insert(n, &reclaim_payload(n, 0, 128), 1)
            .expect("the removals pay for a compaction");
        assert_eq!(m.arena().generation() - g0, 1);
        for k in n / 2..=n {
            assert_eq!(
                m.get(k).unwrap().0.as_bytes(),
                &reclaim_payload(k, 0, 128)[..]
            );
        }
        assert!(m.get(0).is_none());
    }

    /// #1290 G1.8 (METHODOLOGY §28a): G1.1's workload with the reclaim off.
    /// 224 live records fill 8 of the 16 chunks, so 224 overwrites fit and the
    /// other 1,568 are refused, and the arena is never compacted.
    #[test]
    fn reclaim_off_refuses_at_the_cap_without_compacting() {
        let mut m = ExpanseBlobMap::with_chunk_size_and_max_capacity(RECLAIM_CHUNK, RECLAIM_CAP);
        m.set_reclaim_at_cap(false);
        assert!(!m.reclaim_at_cap());
        for k in 0..224u64 {
            m.insert(k, &reclaim_payload(k, 0, 128), 1).unwrap();
        }
        let g0 = m.arena().generation();
        let mut refused = 0u64;
        let mut first_refusal = None;
        for i in 0..1792u64 {
            let k = i % 224;
            match m.insert(k, &reclaim_payload(k, i / 224 + 1, 128), 1) {
                Ok(()) => assert!(
                    first_refusal.is_none(),
                    "an insert succeeded after a refusal"
                ),
                Err(ArenaError::OffsetOverflow) => {
                    refused += 1;
                    first_refusal.get_or_insert(i);
                }
                Err(e) => panic!("overwrite {i} failed with {e:?}"),
            }
        }
        assert_eq!((first_refusal, refused), (Some(224), 1568));
        assert_eq!(m.arena().generation(), g0, "the reclaim is off");
        m.set_reclaim_at_cap(true);
        m.insert(0, &reclaim_payload(0, 99, 128), 1)
            .expect("turned back on, the rule compacts");
        assert_eq!(m.arena().generation() - g0, 1);
    }

    /// #1290 G1.10 (METHODOLOGY §28a C4): the relocation list is reserved once
    /// for the index's entry count, inline entries included, and the
    /// compaction relocates exactly the arena entries.
    #[test]
    #[cfg(feature = "std")]
    fn compaction_reserves_its_relocation_list_once() {
        let mut m = ExpanseBlobMap::with_chunk_size(RECLAIM_CHUNK);
        for k in 0..1_000u64 {
            m.insert(k, &[1, 2, 3], 0).unwrap();
        }
        for k in 1_000..1_010u64 {
            m.insert(k, &reclaim_payload(k, 0, 128), 1).unwrap();
        }
        let stats = m.compact().unwrap();
        assert_eq!(LAST_RELOCATION_RESERVE.with(core::cell::Cell::get), 1_010);
        assert_eq!(stats.live_records_moved, 10);
        for k in 1_000..1_010u64 {
            assert_eq!(
                m.get(k).unwrap().0.as_bytes(),
                &reclaim_payload(k, 0, 128)[..]
            );
        }
    }

    /// Phase 7 (issue #219): deferred-mode round trip — single-threaded and
    /// Miri-clean. Chunks dropped by compaction and `clear` are retired
    /// through the epoch collector (a pinned reader keeps reading the old
    /// bytes), the RCU chunk table tracks every chunk-set change, and
    /// everything drains without leaks or double frees.
    #[test]
    #[cfg(feature = "std")]
    fn deferred_arena_retires_chunks_and_tables() {
        let collector = Arc::new(Collector::new());
        let mut arena = BlobArena::new(4096);
        arena.defer_to(Arc::clone(&collector));
        assert!(arena.reader_table().is_null(), "no chunks, no table");

        let payload = [7u8; 100];
        let global = arena.alloc_blob(&payload).unwrap();
        let locator = (global / ARENA_ALIGN as u64) as u32;

        // Contract order: the pin must be taken BEFORE the table pointer is
        // loaded (see `resolve_meta_in_table`'s safety section).
        let reader = collector.register();
        let pin = reader.pin();
        let table = arena.reader_table();
        assert!(!table.is_null(), "first chunk publishes a table");
        // SAFETY: pinned, freshly published table.
        let (ptr, len) = unsafe { resolve_meta_in_table(table, locator) }.expect("resolves");
        // SAFETY: in-bounds of the live chunk (per resolve contract).
        let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
        assert_eq!(bytes, &payload[..]);

        // Compact with an index referencing the record: the old chunk is
        // retired, not freed — the pinned pointer stays readable.
        let mut index = ExpanseMap::new();
        let slot = slot_from_global(global, 5).unwrap();
        index.insert(42, slot.to_raw());
        let stats = arena.compact_with_index(&mut index).unwrap();
        assert_eq!(stats.live_records_moved, 1);
        // SAFETY: the pin taken before the compaction keeps the retired
        // chunk EBR-live; retired chunk bytes are never rewritten.
        let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
        assert_eq!(bytes, &payload[..]);

        // The relocated record resolves through the republished table with
        // its hot metadata intact.
        let new_slot = ValueSlot::from_raw(index.get(42).unwrap());
        assert_eq!(new_slot.arena_meta_meta(), 5);
        // SAFETY: pinned, freshly published table.
        let (p2, l2) =
            unsafe { resolve_meta_in_table(arena.reader_table(), new_slot.arena_meta_locator()) }
                .expect("relocated record resolves");
        // SAFETY: in-bounds of the live compacted chunk.
        let bytes = unsafe { core::slice::from_raw_parts(p2, l2) };
        assert_eq!(bytes, &payload[..]);

        drop(pin);
        collector.try_advance();
        collector.try_advance();
        collector.try_advance(); // frees the retired chunk + superseded tables

        // `clear` retires the remaining chunks and unpublishes the table.
        arena.clear();
        assert!(arena.reader_table().is_null());
        drop(arena);
        drop(reader);
        drop(collector); // drains anything still queued
    }

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

    /// Hot metadata is decoded only from `ArenaMeta` slots. An inline slot
    /// stores payload bytes in bits 63:8, so a decoder that reads bits 63:40
    /// without checking the tag — the removed `ExpanseMap::scan_filtered` /
    /// `range_filtered` did, over `index()` — reports payload bytes as
    /// metadata. The map's own scan reports 0 for the inline slot.
    #[test]
    fn scan_filtered_reads_metadata_only_from_arena_slots() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        map.insert(1, &[0xFF; 7], 0x00AB_CDEF).unwrap();
        map.insert(2, b"a payload stored in the slab arena", 0x0012_3456)
            .unwrap();

        // Precondition: the inline word's bits 63:40 are non-zero, so an
        // untagged decode of this index word yields a non-zero "metadata".
        let inline_raw = map.index().get(1).expect("inline key present");
        assert_eq!(ValueSlot::from_raw(inline_raw).tag(), SlotTag::Inline7);
        assert_ne!((inline_raw >> 40) & ValueSlot::ARENA_META_MASK, 0);

        let mut seen = Vec::new();
        map.scan_filtered(
            0..=10,
            |_k, meta| meta != 0,
            |k, view, meta| {
                seen.push((k, view.as_bytes().to_vec(), meta));
                true
            },
        );
        assert_eq!(
            seen,
            vec![(
                2,
                b"a payload stored in the slab arena".to_vec(),
                0x0012_3456
            )]
        );

        let mut metas = Vec::new();
        map.scan_filtered(
            0..=10,
            |_k, _meta| true,
            |k, _view, meta| {
                metas.push((k, meta));
                true
            },
        );
        assert_eq!(metas, vec![(1, 0), (2, 0x0012_3456)]);
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
    #[cfg(feature = "std")]
    fn test_binary_serialization_and_deserialization_roundtrip() {
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
    #[cfg(all(not(miri), feature = "std"))]
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
    #[cfg(feature = "std")]
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

    /// A live arena never stamps generation 0 (`BlobArena::new` starts at 1;
    /// compaction skips 0), and both read paths treat "generation 0" as
    /// never-live to reject zeroed unwritten bytes — the optimistic resolver
    /// bounds by capacity and depends on it. A crafted image declaring
    /// generation 0 must therefore be rejected at load.
    #[test]
    #[cfg(feature = "std")]
    fn corrupted_image_generation_zero_rejected() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        map.insert(1, &[0xCD; 1000], 7).unwrap();
        let mut buf = Vec::new();
        map.save_to_writer(&mut buf).unwrap();
        assert!(ExpanseBlobMap::from_bytes_slice(&buf).is_ok());

        // The first chunk header at arena_offset is cap(8) | cursor(8) |
        // generation(4) | pad(4); zero the generation.
        let arena_off = u64::from_le_bytes(buf[32..40].try_into().unwrap()) as usize;
        buf[arena_off + 16..arena_off + 20].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            ExpanseBlobMap::from_bytes_slice(&buf),
            Err(ArenaError::CorruptedHeader)
        ));
    }

    /// A header declaring `chunk_count` chunks of 1 GiB, with no chunk data.
    fn huge_capacity_header(chunk_count: u64) -> Vec<u8> {
        let mut buf = vec![0u8; 2048];
        buf[0..8].copy_from_slice(&EXPANSE_MAGIC);
        buf[8..12].copy_from_slice(&EXPANSE_FORMAT_VERSION.to_le_bytes());
        // flags[12..16] = 0, entry_count[16..24] = 0
        buf[24..32].copy_from_slice(&64u64.to_le_bytes()); // index_offset
        buf[32..40].copy_from_slice(&64u64.to_le_bytes()); // arena_offset
        buf[40..48].copy_from_slice(&2048u64.to_le_bytes()); // total_size
        buf[48..56].copy_from_slice(&(1024u64 * 1024 * 1024).to_le_bytes()); // chunk_size = 1 GiB
        buf[56..64].copy_from_slice(&chunk_count.to_le_bytes());
        buf
    }

    #[test]
    fn corrupted_image_huge_aggregate_capacity_rejected() {
        // A small file that declares chunk_count * chunk_size = 2 GiB, above
        // the default cap (DEFAULT_ARENA_CAPACITY, 1 GiB). chunk_count (2)
        // passes both the MAX_ARENA_CHUNKS and bytes.len()/24 bounds, and
        // chunk_size (1 GiB) is exactly MAX_CHUNK_CAPACITY, so the
        // aggregate-capacity check against the loader's cap is the one that
        // fires, before any chunk is allocated: it guards the multi-GiB
        // alloc_zeroed DoS. The image is not corrupt, only over the cap.
        let buf = huge_capacity_header(2);
        assert!(matches!(
            ExpanseBlobMap::from_bytes_slice(&buf),
            Err(ArenaError::OffsetOverflow)
        ));
        // Under a cap that holds it, the declaration passes and the missing
        // chunk headers are what is rejected (a zero `cap`), still before any
        // chunk is allocated.
        assert!(matches!(
            ExpanseBlobMap::from_bytes_slice_with_max_capacity(&buf, 2 << 30),
            Err(ArenaError::CorruptedHeader)
        ));
        // Past the 64 GiB locator envelope no cap admits it: that is corrupt.
        let buf = huge_capacity_header(65);
        assert!(matches!(
            ExpanseBlobMap::from_bytes_slice_with_max_capacity(&buf, usize::MAX),
            Err(ArenaError::CorruptedHeader)
        ));
    }

    /// #1300 item 4's prerequisite: the loader's bound on declared capacity is
    /// the caller's cap, which the loaded map keeps, not a shipped constant.
    #[test]
    #[cfg(feature = "std")]
    fn image_loads_under_the_callers_cap_and_keeps_it() {
        let mut map = ExpanseBlobMap::with_chunk_size_and_max_capacity(4096, 4 * 4096);
        for k in 0..12u64 {
            map.insert(k, &[k as u8; 1000], 1).unwrap(); // 4 records per chunk
        }
        assert_eq!(map.arena().mem_used(), 3 * 4096);
        let mut buf = Vec::new();
        map.save_to_writer(&mut buf).unwrap();

        assert!(matches!(
            ExpanseBlobMap::from_bytes_slice_with_max_capacity(&buf, 2 * 4096),
            Err(ArenaError::OffsetOverflow)
        ));
        let loaded = ExpanseBlobMap::from_bytes_slice_with_max_capacity(&buf, 4 * 4096).unwrap();
        assert_eq!(loaded.arena().max_capacity(), 4 * 4096);
        assert_eq!(loaded.len(), 12);
        let default = ExpanseBlobMap::from_bytes_slice(&buf).unwrap();
        assert_eq!(default.arena().max_capacity(), DEFAULT_ARENA_CAPACITY);
    }

    /// #1300 item 3: an insert the cap refuses with no compaction is
    /// `OffsetOverflow`; one refused after its own compaction is `ArenaFull`.
    /// The arena holds one 4 KiB chunk (a 6000-byte cap admits no second), so
    /// compacting packs the live record at the chunk's start and a record
    /// larger than the remaining tail still does not fit.
    /// #1300 item 2 (METHODOLOGY §30, H2/H3): an insert through
    /// `prepare_reclaim_for_insert` and `insert_shared_after_prepare` has the
    /// outcome `insert_shared_reclaiming` gives on an identical map, at every
    /// step of a run that reaches the cap many times; a copy is prepared
    /// exactly when that insert compacts, and every key reads its last payload.
    #[test]
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    fn prepared_reclaiming_insert_matches_the_unprepared_one() {
        let new = || {
            let mut m = ExpanseBlobMap::with_chunk_size_and_max_capacity(4096, 20 * 1024);
            for k in 0..16u64 {
                m.insert_shared(k, &[k as u8; 128], 1).unwrap();
            }
            m
        };
        let (mut staged, mut plain) = (new(), new());
        let mut compactions = 0;
        for i in 0..2_000u64 {
            let (k, len) = (i % 16, 100 + (i % 7) as usize * 40);
            let data = vec![(i % 251) as u8; len];
            let prepared = staged.prepare_reclaim_for_insert(&data, 1);
            let foreseen = matches!(prepared, Some(Ok(_)));
            let got = staged.insert_shared_after_prepare(k, &data, 1, prepared);
            let want = plain.insert_shared_reclaiming(k, &data, 1);
            assert_eq!(got, want, "insert {i}");
            assert_eq!(
                foreseen, want.1,
                "insert {i}: the copy was prepared {foreseen}, the insert compacted {}",
                want.1
            );
            compactions += usize::from(want.1);
        }
        assert!(compactions >= 10, "{compactions} compactions");
        for k in 0..16u64 {
            assert_eq!(
                staged.get(k).map(|(v, m)| (v.as_bytes().to_vec(), m)),
                plain.get(k).map(|(v, m)| (v.as_bytes().to_vec(), m))
            );
        }
        assert_eq!(staged.arena().generation(), plain.arena().generation());
        assert_eq!(staged.arena().live_bytes(), plain.arena().live_bytes());
    }

    /// The fallbacks of `insert_shared_after_prepare` (§30, H3): a prepared
    /// copy is dropped, and nothing compacted, when the insert fits after
    /// all; a failed copy is reported only if the cap refuses the insert.
    #[test]
    #[cfg(all(
        target_pointer_width = "64",
        feature = "std",
        not(feature = "ablation-blob-serial-writers"),
        not(feature = "ablation-blob-shared-arena")
    ))]
    fn prepared_insert_fallbacks() {
        let mut m = ExpanseBlobMap::with_chunk_size_and_max_capacity(4096, 8192);
        m.insert_shared(1, &[1; 1000], 1).unwrap();
        m.insert_shared(2, &[2; 1000], 1).unwrap();
        let g0 = m.arena().generation();
        // The insert fits: a prepared copy is dropped and the arena keeps its generation.
        let p = m.prepare_compaction().unwrap();
        assert_eq!(
            m.insert_shared_after_prepare(3, &[3; 1000], 1, Some(Ok(p))),
            (Ok(()), false)
        );
        assert_eq!(m.arena().generation(), g0);
        // The insert fits: a failed copy is not reported.
        assert_eq!(
            m.insert_shared_after_prepare(4, &[4; 100], 1, Some(Err(ArenaError::AllocationFailed))),
            (Ok(()), false)
        );
        // Fill the cap: a refused insert reports the failed copy and compacts nothing.
        while m.insert_shared(9, &[9; 3000], 1).is_ok() {
            assert!(m.remove(9));
        }
        assert_eq!(
            m.insert_shared_after_prepare(
                9,
                &[9; 3000],
                1,
                Some(Err(ArenaError::AllocationFailed))
            ),
            (Err(ArenaError::AllocationFailed), false)
        );
        assert_eq!(m.arena().generation(), g0);
        assert_eq!(m.get(1).unwrap().0.as_bytes(), &[1u8; 1000][..]);
    }

    #[test]
    fn arena_full_after_compaction_is_distinct_from_a_declined_reclaim() {
        let mut m = ExpanseBlobMap::with_chunk_size_and_max_capacity(4096, 6000);
        for k in 0..4u64 {
            m.insert(k, &[k as u8; 1000], 1).unwrap(); // 1008 B each, 4032 in the chunk
        }
        assert_eq!(m.arena().chunks_count(), 1);

        // More than half the chunk is live: the waste guard declines, so the
        // insert is refused without compacting.
        let g0 = m.arena().generation();
        assert_eq!(m.insert(9, &[9; 3100], 1), Err(ArenaError::OffsetOverflow));
        assert_eq!(m.arena().generation(), g0, "no compaction ran");
        assert!(m.arena().live_bytes() * 2 >= m.arena().mem_used());

        // Three removals leave 1008 live bytes: the rule allows a compaction,
        // after which 3088 bytes remain in the chunk, short of the 3108 the
        // record needs.
        for k in 1..4u64 {
            assert!(m.remove(k));
        }
        assert_eq!(m.insert(9, &[9; 3100], 1), Err(ArenaError::ArenaFull));
        assert_ne!(m.arena().generation(), g0, "the insert compacted");
        assert_eq!(m.len(), 1);
        assert_eq!(m.get(0).unwrap().0.as_bytes(), &[0u8; 1000][..]);

        // A record that fits the compacted tail is admitted.
        assert_eq!(m.insert(9, &[9; 3000], 1), Ok(()));

        // Switched off, a refusal never compacts, so it is never `ArenaFull`.
        let mut off = ExpanseBlobMap::with_chunk_size_and_max_capacity(4096, 6000);
        off.set_reclaim_at_cap(false);
        off.insert(0, &[0; 1000], 1).unwrap();
        off.insert(1, &[1; 2000], 1).unwrap();
        assert!(off.remove(1));
        assert_eq!(
            off.insert(9, &[9; 3100], 1),
            Err(ArenaError::OffsetOverflow)
        );
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
    fn arena_meta_uniform_and_metadata_survives_past_16mib() {
        // One record fills a 1 MiB chunk exactly, so blob k lands at the start of
        // chunk k, global offset = k * 1 MiB. Keys 16..=19 sit at/past 16 MiB —
        // exactly where the old encoding spilled to metadata-less `ArenaLong`.
        // Under the uniform `ArenaMeta` encoding every key is `ArenaMeta` and
        // *every* key keeps its hot metadata (this is the #285 Phase 1 fix).
        let chunk = 1024 * 1024; // 1 MiB
        let mut map = ExpanseBlobMap::with_chunk_size(chunk);
        for k in 0..20u64 {
            let payload = vec![(0xA0 + k) as u8; chunk - 8];
            map.insert(k, &payload, 1000 + k as u32)
                .expect("insert past 16 MiB");
        }

        for k in 0..20u64 {
            let raw = map.index.get(k).expect("key present");
            assert_eq!(
                ValueSlot::from_raw(raw).tag(),
                SlotTag::ArenaMeta,
                "k={k} must be ArenaMeta"
            );
            let (view, meta) = map.get(k).expect("value present");
            assert!(view.is_arena());
            assert_eq!(view.as_bytes(), &vec![(0xA0 + k) as u8; chunk - 8][..]);
            // Metadata is preserved for ALL keys, including those past 16 MiB.
            assert_eq!(meta, 1000 + k as u32, "k={k} meta must survive past 16 MiB");
        }
        // The arena genuinely grew past the old 16 MiB ArenaShort ceiling.
        assert!(map.arena().mem_used() > 16 * 1024 * 1024);
    }

    #[test]
    fn insert_rejects_metadata_beyond_24_bits() {
        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        // Max 24-bit metadata is accepted...
        assert!(
            map.insert(1, &[0u8; 100], ValueSlot::ARENA_META_MAX)
                .is_ok()
        );
        assert_eq!(map.get(1).unwrap().1, ValueSlot::ARENA_META_MAX);
        // ...anything above it is rejected, never truncated, and inserts nothing.
        assert!(matches!(
            map.insert(2, &[0u8; 100], ValueSlot::ARENA_META_MAX + 1),
            Err(ArenaError::MetaOverflow)
        ));
        assert!(map.get(2).is_none(), "rejected insert must leave no entry");
        // Inline payloads ignore metadata entirely (no envelope check needed).
        assert!(map.insert(3, &[1, 2, 3], u32::MAX).is_ok());
        assert_eq!(map.get(3).unwrap().1, 0);
    }

    #[test]
    fn custom_max_capacity_is_honored_and_inherited() {
        let chunk = 64 * 1024;
        let cap = 128 * 1024; // Allows exactly 2 chunks
        let mut map = ExpanseBlobMap::with_chunk_size_and_max_capacity(chunk, cap);
        assert_eq!(map.arena.max_capacity, cap);

        // Clamping checks: minimum bound is chunk_size
        let small = ExpanseBlobMap::with_chunk_size_and_max_capacity(64 * 1024, 1024);
        assert_eq!(small.arena.max_capacity, 64 * 1024);

        // Clamping checks: maximum bound is ARENA_META_CEILING (64 GiB) or usize::MAX
        let high = ExpanseBlobMap::with_chunk_size_and_max_capacity(64 * 1024, usize::MAX);
        let expected_ceiling = if (ARENA_META_CEILING as u128) > (usize::MAX as u128) {
            usize::MAX
        } else {
            ARENA_META_CEILING as usize
        };
        assert_eq!(
            high.arena.max_capacity, expected_ceiling,
            "max_capacity must be clamped to ARENA_META_CEILING"
        );

        let payload = vec![0x42; chunk - 8];
        // 1st chunk
        assert!(map.insert(1, &payload, 0).is_ok());
        assert_eq!(map.len(), 1);
        // 2nd chunk
        assert!(map.insert(2, &payload, 0).is_ok());
        assert_eq!(map.len(), 2);

        // 3rd chunk exceeds 128 KiB cap -> OffsetOverflow
        assert!(matches!(
            map.insert(3, &payload, 0),
            Err(ArenaError::OffsetOverflow)
        ));

        // Invariant: failed insert leaves digital tree index untouched
        assert_eq!(map.len(), 2, "failed insert must leave map len untouched");
        assert!(
            map.get(3).is_none(),
            "rejected key must not be present in index"
        );
        assert!(
            map.get(1).is_some(),
            "previously inserted key 1 must remain intact"
        );
        assert!(
            map.get(2).is_some(),
            "previously inserted key 2 must remain intact"
        );

        // Compaction inherits the custom capacity cap
        let stats = map.compact().expect("compaction within capacity succeeds");
        assert_eq!(map.arena.max_capacity, cap);
        assert_eq!(stats.chunks_after, 2);
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
    #[cfg(feature = "std")]
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
        // Locator of a high key, which lives beyond the first chunk.
        let raw = map.index.get(19).unwrap();
        let stale = ValueSlot::from_raw(raw).arena_meta_locator();
        assert!(map.arena().resolve_meta(stale).is_some());
        assert!(
            (stale as usize) * ARENA_ALIGN >= chunk,
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
        // The locator held across the compaction no longer resolves.
        assert!(map.arena().resolve_meta(stale).is_none());
        for i in 0..2u64 {
            assert_eq!(map.get(i).unwrap().0.as_bytes(), &[i as u8; 4000][..]);
        }
    }

    // ---------------------------------------------------------------------
    // Multi-chunk `ArenaMeta` tests. A small (kilobyte) multi-chunk arena keeps
    // these Miri-safe while still exercising the cross-chunk locator math: with
    // ~2 KiB payloads in 4 KiB chunks, keys land in chunks 0, 1, 2, ... and the
    // `ArenaMeta` locator (`global / 16`) must resolve each to the right record.
    // ---------------------------------------------------------------------

    #[test]
    fn arena_meta_resolves_across_chunks_on_small_arena() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..6u64 {
            let payload = vec![0x11 * (k as u8 + 1); 2000];
            map.insert(k, &payload, 700 + k as u32).unwrap();
        }
        // Key 5 lives past the first chunk (locator * 16 >= chunk_size).
        let raw = map.index.get(5).unwrap();
        let slot = ValueSlot::from_raw(raw);
        assert_eq!(slot.tag(), SlotTag::ArenaMeta);
        assert!(
            (slot.arena_meta_locator() as usize) * ARENA_ALIGN >= map.arena().chunk_size(),
            "key 5 should live past the first chunk"
        );

        for k in 0..6u64 {
            let (view, meta) = map.get(k).expect("value present");
            assert!(view.is_arena());
            assert_eq!(view.as_bytes(), &vec![0x11 * (k as u8 + 1); 2000][..]);
            assert_eq!(
                meta,
                700 + k as u32,
                "k={k} metadata preserved across chunks"
            );
        }
    }

    #[test]
    fn arena_meta_bad_locator_reads_none() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..4u64 {
            map.insert(k, &vec![0x33; 2000], 0).unwrap();
        }
        // A crafted index slot with an out-of-range locator resolves to None via
        // `get` (clean, no UB — Miri validates the pointer math).
        let bad = ValueSlot::new_arena_meta(0, u32::MAX).unwrap();
        map.index.insert(0, bad.to_raw());
        assert!(map.get(0).is_none());
        // The low-level resolver agrees and never faults.
        assert!(map.arena().resolve_meta(u32::MAX).is_none());
    }

    #[test]
    #[cfg(feature = "std")]
    fn arena_meta_save_load_roundtrip_multi_chunk() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..6u64 {
            let payload = vec![0x20 + k as u8; 2000];
            map.insert(k, &payload, 300 + k as u32).unwrap();
        }
        let mut buf = Vec::new();
        map.save_to_writer(&mut buf).unwrap();
        let restored = ExpanseBlobMap::from_bytes_slice(&buf).unwrap();
        assert_eq!(restored.len(), 6);

        for k in 0..6u64 {
            let (view, meta) = restored.get(k).expect("entry present after reload");
            assert_eq!(view.as_bytes(), &vec![0x20 + k as u8; 2000][..]);
            assert_eq!(
                ValueSlot::from_raw(restored.index().get(k).unwrap()).tag(),
                SlotTag::ArenaMeta
            );
            // Metadata survives the image roundtrip for every key, including
            // those living past the first chunk.
            assert_eq!(meta, 300 + k as u32);
        }
    }

    #[test]
    fn arena_meta_compaction_relocates_multi_chunk() {
        let mut map = ExpanseBlobMap::with_chunk_size(4096);
        for k in 0..8u64 {
            let payload = vec![0x40 + k as u8; 2000];
            map.insert(k, &payload, 900 + k as u32).unwrap();
        }
        // Churn away all but two keys that live past the first chunk.
        for k in 0..6u64 {
            assert!(map.remove(k));
        }
        assert_eq!(map.len(), 2);

        let stats = map.compact().unwrap();
        assert_eq!(stats.live_records_moved, 2);

        for k in 6..8u64 {
            let (view, meta) = map.get(k).expect("entry survives compaction");
            assert_eq!(view.as_bytes(), &vec![0x40 + k as u8; 2000][..]);
            // Metadata rides along through compaction relocation.
            assert_eq!(meta, 900 + k as u32, "k={k} meta preserved by compaction");
        }
    }

    /// `scan_filtered` must visit exactly the same `(key, meta, payload)`
    /// sequence a `BTreeMap` reference does when filtered by the same predicate —
    /// across inline (`<= 7` B) and arena payloads, at low/zero/full selectivity,
    /// over full and partial windows, and after a compaction relocates arena
    /// records. This pins the #355 change (resolve from the held slot, no
    /// per-match `get` re-descent) to byte-identical output.
    #[test]
    fn scan_filtered_matches_btreemap_reference() {
        use std::collections::BTreeMap;

        type Pred = dyn Fn(u64, u32) -> bool;

        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        // Reference model: key -> (effective_meta, payload). `effective_meta`
        // mirrors `scan_filtered`: 0 for inline (no metadata field), the hot
        // metadata for arena payloads.
        let mut model: BTreeMap<u64, (u32, Vec<u8>)> = BTreeMap::new();

        let n = 400u64;
        for i in 0..n {
            let (payload, hot_meta): (Vec<u8>, u32) = if i % 3 == 0 {
                // Inline payload: 0..=7 bytes (metadata ignored / reported as 0).
                let len = (i % 8) as usize;
                (
                    (0..len).map(|b| (i as u8).wrapping_add(b as u8)).collect(),
                    0,
                )
            } else {
                // Arena payload: >7 bytes, carries 24-bit hot metadata.
                let len = 8 + (i % 40) as usize;
                (
                    (0..len).map(|b| (i as u8).wrapping_add(b as u8)).collect(),
                    (i as u32) & ValueSlot::ARENA_META_MAX,
                )
            };
            map.insert(i, &payload, hot_meta).unwrap();
            let effective_meta = if payload.len() <= 7 { 0 } else { hot_meta };
            model.insert(i, (effective_meta, payload));
        }

        // Collect the scan_filtered output and the identically-filtered model
        // slice, then compare — as a single assertion per (predicate, window).
        fn check(
            map: &ExpanseBlobMap,
            model: &BTreeMap<u64, (u32, Vec<u8>)>,
            lo: u64,
            hi: u64,
            pred: &Pred,
            label: &str,
        ) {
            let mut seen: Vec<(u64, u32, Vec<u8>)> = Vec::new();
            map.scan_filtered(lo..=hi, pred, |k, view, m| {
                seen.push((k, m, view.as_bytes().to_vec()));
                true
            });
            let expected: Vec<(u64, u32, Vec<u8>)> = model
                .range(lo..=hi)
                .filter(|(k, (m, _))| pred(**k, *m))
                .map(|(k, (m, p))| (*k, *m, p.clone()))
                .collect();
            assert_eq!(seen, expected, "{label}");
        }

        // sigma = 1.0 (all match), 0.0 (none), 0.05 (exactly 1/20 of keys, chosen
        // odd so the subset survives the even-key churn below).
        let preds: [(&str, &Pred); 3] = [
            ("sigma=1.0", &|_k, _m| true),
            ("sigma=0.0", &|_k, _m| false),
            ("sigma=0.05", &|k, _m| k % 20 == 1),
        ];

        for (label, pred) in preds {
            check(&map, &model, 0, n - 1, pred, &format!("{label} full-range"));
            check(&map, &model, 50, 349, pred, &format!("{label} sub-range"));
        }

        // Churn away every even key, compact (arena records relocate to fresh
        // chunks; inline payloads stay in-slot), then re-verify against the model.
        for i in (0..n).step_by(2) {
            assert!(map.remove(i));
            model.remove(&i);
        }
        map.compact().unwrap();

        for (label, pred) in preds {
            check(
                &map,
                &model,
                0,
                n - 1,
                pred,
                &format!("{label} post-compaction"),
            );
        }
    }

    #[test]
    fn insert_single_descent_replacement_and_error_invariants() {
        let mut map = ExpanseBlobMap::new();
        // Fresh inline payload
        map.insert(1, b"inline", 0).unwrap();
        assert_eq!(map.len(), 1);
        let (view, meta) = map.get(1).unwrap();
        assert_eq!(view.as_bytes(), b"inline");
        assert_eq!(meta, 0);

        // Overwrite inline with arena payload
        let arena_payload = b"this is a larger payload > 7 bytes";
        map.insert(1, arena_payload, 0x1234).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.arena.live_bytes, 8 + arena_payload.len());
        let (view, meta) = map.get(1).unwrap();
        assert_eq!(view.as_bytes(), arena_payload);
        assert_eq!(meta, 0x1234);

        // Overwrite arena with arena, checking space accounting
        let arena_payload_2 = b"another large payload for replacement";
        map.insert(1, arena_payload_2, 0x5678).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.arena.live_bytes, 8 + arena_payload_2.len());
        let (view, meta) = map.get(1).unwrap();
        assert_eq!(view.as_bytes(), arena_payload_2);
        assert_eq!(meta, 0x5678);

        // Error path: MetaOverflow rejected without touching index or allocating arena bytes
        let allocated_at_err = map.arena.total_allocated;
        let overflow_meta = ValueSlot::ARENA_META_MAX + 1;
        assert_eq!(
            map.insert(2, arena_payload, overflow_meta),
            Err(ArenaError::MetaOverflow)
        );
        // Key 2 must not exist in map, and arena must not have grown
        assert!(map.get(2).is_none());
        assert_eq!(map.arena.total_allocated, allocated_at_err);
        assert_eq!(map.len(), 1);

        // Overwrite existing key with invalid meta must leave existing key intact
        assert_eq!(
            map.insert(1, arena_payload, overflow_meta),
            Err(ArenaError::MetaOverflow)
        );
        let (view, meta) = map.get(1).unwrap();
        assert_eq!(view.as_bytes(), arena_payload_2);
        assert_eq!(meta, 0x5678);
        assert_eq!(map.arena.total_allocated, allocated_at_err);
    }
}
