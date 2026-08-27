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
//! A shipped safety cap ([`MAX_ARENA_CAPACITY`], 1 GiB) bounds actual arena
//! growth and the aggregate capacity a loaded image may declare — far below the
//! 64 GiB locator envelope. [`BlobArena::alloc_blob`] returns
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
use crate::occ::Collector;
use crate::slot::{SlotTag, ValueSlot};
use crate::types::Key;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, OnceLock};

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
/// `bound = cursor`; the lock-free path ([`resolve_meta_in_table`]) calls it
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
    // loaded bytes may be torn/stale on the lock-free path — every use is
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
    /// safety cap ([`MAX_ARENA_CAPACITY`]), the [`MAX_ARENA_CHUNKS`] chunk-count
    /// limit, or the 64 GiB `ArenaMeta` locator envelope (`global_offset / 16`
    /// no longer fits a `u32`).
    OffsetOverflow,
    /// `hot_meta` exceeds the 24-bit `ArenaMeta` field
    /// ([`ValueSlot::ARENA_META_MAX`]). Rejected rather than silently truncated.
    MetaOverflow,
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
            Self::MetaOverflow => write!(f, "hot_meta exceeds the 24-bit ArenaMeta field"),
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
    pub(crate) fn retire_into(self, collector: &Collector) {
        let ptr = self.ptr;
        let capacity = self.capacity;
        core::mem::forget(self);
        collector.retire(ptr, capacity, 16);
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
/// from a loaded image, as a corruption guard. Effective growth is bounded far
/// lower by [`MAX_ARENA_CAPACITY`].
pub const MAX_ARENA_CHUNKS: usize = 1 << 16;

/// Shipped safety cap on total arena capacity (**1 GiB**).
///
/// Growth is bounded to this cap so a runaway workload — or a crafted image
/// declaring a huge `chunk_count * chunk_size` — cannot drive an unbounded
/// `alloc_zeroed`. 1 GiB comfortably exceeds any single-socket last-level cache
/// (what the RFC §10.3 cold-DRAM predicate-scan regime requires) while staying
/// well under the 64 GiB `ArenaMeta` locator envelope ([`ARENA_META_CEILING`]),
/// so a locator overflow cannot occur under the shipped cap. Raise this constant
/// to lift the shipped cap toward that envelope.
pub const MAX_ARENA_CAPACITY: usize = 1 << 30;

/// Builds the uniform [`ArenaMeta`](SlotTag::ArenaMeta) [`ValueSlot`] for a blob
/// at flat `global_offset` carrying `hot_meta`.
///
/// The locator is `global_offset / 16` (records are 16-byte aligned). Returns
/// [`ArenaError::OffsetOverflow`] if the offset is unaligned or beyond the 64 GiB
/// envelope, and [`ArenaError::MetaOverflow`] if `hot_meta` exceeds 24 bits —
/// never silently truncating either field.
#[inline]
fn slot_from_global(global_offset: u64, hot_meta: u32) -> Result<ValueSlot, ArenaError> {
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
/// geometry a lock-free reader needs to resolve a record inside one chunk.
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
fn table_layout(len: usize) -> std::alloc::Layout {
    std::alloc::Layout::from_size_align(table_bytes(len), core::mem::align_of::<ChunkTable>())
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
    /// Phase 7 (issue #219): when set, dead chunks and superseded chunk
    /// tables are retired to the collector instead of freed — concurrent
    /// readers may still hold pointers into them. Mirrors
    /// [`crate::alloc::NodeAlloc`]'s deferred mode.
    deferred: OnceLock<Arc<Collector>>,
    /// RCU-published [`ChunkTable`] for lock-free readers; null unless
    /// deferred mode has published one. Republished whole on every
    /// chunk-set change, always *before* the chunks it dropped are retired
    /// (a block must be unreachable before it enters the grace period).
    reader_table: AtomicPtr<ChunkTable>,
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
            generation: 1,
            max_capacity: MAX_ARENA_CAPACITY,
            deferred: OnceLock::new(),
            reader_table: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    /// Switches this arena to deferred reclamation through `collector`,
    /// permanently (Phase 7 concurrent wrappers call this once at
    /// construction, alongside the index's `NodeAlloc::defer_to`): dead
    /// chunks and superseded chunk tables then wait out the epoch grace
    /// period instead of being freed, and a [`ChunkTable`] snapshot is
    /// published for lock-free readers. Idempotent for the same collector;
    /// a second call with a different collector is a bug and panics.
    ///
    /// `pub(crate)` deliberately: only the `sync` wrappers drive a
    /// collector's epochs. A caller deferring a standalone arena through
    /// the public [`ExpanseBlobMap::arena`] accessor would retire whole
    /// chunks into bins nothing ever advances (unbounded growth), and the
    /// different-collector panic would be reachable from safe code.
    pub(crate) fn defer_to(&self, collector: Arc<Collector>) {
        let stored = self.deferred.get_or_init(|| Arc::clone(&collector));
        assert!(
            Arc::ptr_eq(stored, &collector),
            "BlobArena already deferred to a different collector"
        );
        self.republish_table();
    }

    /// Current published chunk table (null when not in deferred mode or
    /// when the arena has no chunks). Readers must hold an epoch pin taken
    /// before this load — see [`resolve_meta_in_table`].
    pub(crate) fn reader_table(&self) -> *const ChunkTable {
        self.reader_table.load(Ordering::Acquire)
    }

    /// Rebuilds and publishes the reader chunk table from the current chunk
    /// set, retiring the superseded table through the collector. No-op
    /// unless deferred mode is on. Publishing an empty chunk set stores
    /// null. Must run *before* any chunk dropped by the change is retired.
    fn republish_table(&self) {
        let Some(collector) = self.deferred.get() else {
            return;
        };
        let new_table: *mut ChunkTable = if self.chunks.is_empty() {
            core::ptr::null_mut()
        } else {
            let len = self.chunks.len();
            let layout = table_layout(len);
            // SAFETY: non-zero-size layout; every byte is initialized below.
            let raw = unsafe { std::alloc::alloc(layout) };
            let Some(table) = NonNull::new(raw.cast::<ChunkTable>()) else {
                std::alloc::handle_alloc_error(layout)
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
        let old = self.reader_table.swap(new_table, Ordering::AcqRel);
        if let Some(old) = NonNull::new(old) {
            // SAFETY: `old` was published by this arena; its `len` header
            // field is immutable, giving back the exact allocation size.
            let bytes = table_bytes(unsafe { (*old.as_ptr()).len });
            collector.retire(old.cast::<u8>(), bytes, core::mem::align_of::<ChunkTable>());
        }
    }

    /// Disposes of chunks no longer referenced by the arena: retired
    /// through the collector in deferred mode (pinned readers may still
    /// hold pointers into them), dropped (freed immediately) otherwise.
    /// In deferred mode the caller must have republished the reader table
    /// first, so no new reader can reach these chunks.
    fn dispose_chunks(&self, chunks: Vec<ArenaChunk>) {
        if let Some(collector) = self.deferred.get() {
            for chunk in chunks {
                chunk.retire_into(collector);
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
    /// cross the [`MAX_ARENA_CHUNKS`] chunk-count cap or the shipped
    /// [`MAX_ARENA_CAPACITY`] safety cap, and with [`ArenaError::AllocationFailed`]
    /// if a single record cannot fit one chunk (`8 + data.len() > chunk_size`).
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
        // Phase 7: lock-free readers resolve through the published table, so
        // the grown chunk set must be republished for the record to become
        // reachable (readers on the old table simply retry).
        self.republish_table();
        Ok(self.global_offset(idx, offset_in_chunk))
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

        // Collect every arena-backed (`ArenaMeta`) entry.
        let live_entries: Vec<(Key, ValueSlot)> = index
            .iter()
            .filter_map(|(key, raw_slot)| {
                let slot = ValueSlot::from_raw(raw_slot);
                (slot.tag() == SlotTag::ArenaMeta).then_some((key, slot))
            })
            .collect();

        // Phase 1: relocate every live payload into the new arena, collecting
        // the (key, new raw slot) rewrites. A failure here returns before any
        // index slot is touched, so `self`/`index` stay consistent. The blob's
        // 24-bit hot metadata rides along; only its locator changes with the new
        // location.
        let mut rewrites: Vec<(Key, u64)> = Vec::with_capacity(live_entries.len());
        for (key, slot) in live_entries {
            let meta = slot.arena_meta_meta();
            let payload = self.resolve_meta(slot.arena_meta_locator());
            if let Some(payload) = payload {
                let global = new_arena.alloc_blob(payload)?;
                let new_slot = slot_from_global(global, meta)?;
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
        self.generation = new_arena.generation;
        self.republish_table();
        self.dispose_chunks(old_chunks);

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
        self.republish_table();
    }

    /// Resets and frees all arena chunks (retired through the collector in
    /// deferred mode — pinned readers may still hold payload borrows).
    pub fn clear(&mut self) {
        let old_chunks = core::mem::take(&mut self.chunks);
        self.active_chunk = None;
        self.total_allocated = 0;
        self.live_bytes = 0;
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
        let table = *self.reader_table.get_mut();
        if let Some(table) = NonNull::new(table) {
            // SAFETY: `table` was allocated by `republish_table` with
            // `table_layout(len)`; its `len` header field is immutable.
            unsafe {
                let layout = table_layout((*table.as_ptr()).len);
                std::alloc::dealloc(table.cast::<u8>().as_ptr(), layout);
            }
        }
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
    /// Note: inline (`<= 7` byte) payloads store their bytes in the slot word and
    /// carry no metadata field, so `hot_meta` is ignored for them and later reads
    /// report their metadata as `0`. Arena payloads (`> 7` bytes) all carry the
    /// 24-bit metadata; `hot_meta` exceeding 24 bits returns
    /// [`ArenaError::MetaOverflow`] rather than being truncated.
    pub fn insert(&mut self, key: Key, data: &[u8], hot_meta: u32) -> Result<(), ArenaError> {
        let old_slot = self.index.get(key).map(ValueSlot::from_raw);
        if data.len() <= 7 {
            let slot = ValueSlot::new_inline(data).ok_or(ArenaError::AllocationFailed)?;
            self.index.insert(key, slot.to_raw());
        } else {
            // Validate the metadata envelope *before* allocating arena bytes, so a
            // rejected insert leaves no orphaned payload behind.
            if hot_meta > ValueSlot::ARENA_META_MAX {
                return Err(ArenaError::MetaOverflow);
            }
            let global = self.arena.alloc_blob(data)?;
            let slot = slot_from_global(global, hot_meta)?;
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
            SlotTag::ArenaMeta => {
                let meta = slot.arena_meta_meta();
                let slice = self.arena.resolve_meta(slot.arena_meta_locator())?;
                Some((BlobView::Arena(slice), meta))
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
                // Non-inline / non-arena tags carry no payload — skipped, matching
                // `get`'s `_ => None` arm.
                _ => continue,
            };
            if !callback(key, view, meta) {
                break;
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

    /// Phase 7 (issue #219): replaces the index with a copy rebuilt through
    /// an allocator deferred to `collector` **before any node is
    /// allocated**. Sharing a populated map requires this because a
    /// single-threaded index holds slab-carved node memory, which must
    /// never be retired to the collector (see `NodeAlloc::defer_to`); the
    /// old index (and its slab pages, wholesale) is freed here. Arena
    /// payloads are untouched — the raw `ValueSlot` words carry over.
    pub(crate) fn rebuild_index_deferred(&mut self, collector: &Arc<Collector>) {
        let fresh = ExpanseMap::new();
        fresh.occ_root().1.defer_to(Arc::clone(collector));
        let mut fresh = fresh;
        for (key, raw_slot) in self.index.iter() {
            fresh.insert(key, raw_slot);
        }
        self.index = fresh;
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

    /// Phase 7 (issue #219): deferred-mode round trip — single-threaded and
    /// Miri-clean. Chunks dropped by compaction and `clear` are retired
    /// through the epoch collector (a pinned reader keeps reading the old
    /// bytes), the RCU chunk table tracks every chunk-set change, and
    /// everything drains without leaks or double frees.
    #[test]
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

    /// A live arena never stamps generation 0 (`BlobArena::new` starts at 1;
    /// compaction skips 0), and both read paths treat "generation 0" as
    /// never-live to reject zeroed unwritten bytes — the lock-free resolver
    /// bounds by capacity and depends on it. A crafted image declaring
    /// generation 0 must therefore be rejected at load.
    #[test]
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
}
