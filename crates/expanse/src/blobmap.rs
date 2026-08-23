//! Chunked slab/arena allocator and high-level blob map.
//!
//! Stores variable-length byte payloads associated with 64-bit keys. Small
//! payloads (up to 7 bytes) are stored directly inside 64-bit value slots
//! ([`crate::slot::ValueSlot`]) with zero heap allocation. Larger payloads
//! are bump-allocated in contiguous 16-byte aligned slabs managed by [`BlobArena`].

use crate::map::ExpanseMap;
use crate::slot::{SlotTag, ValueSlot};
use crate::types::Key;
use core::ptr::NonNull;

/// Packed 8-byte record header preceding every arena payload.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlobRecordHeader {
    /// Payload length in bytes (up to 4 GB).
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
    /// Arena byte offset exceeded 24-bit limit (16 MiB).
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
            Self::OffsetOverflow => write!(f, "Arena offset overflow (> 24-bit limit)"),
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
    /// Creates a new arena chunk of given capacity and initial generation.
    pub fn new(capacity: usize, generation: u32) -> Result<Self, ArenaError> {
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
        if offset_in_chunk + 8 > self.cursor {
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
            if offset_in_chunk + 8 + len > self.cursor {
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

    /// Returns the raw allocated slice up to the cursor.
    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        if self.cursor == 0 {
            &[]
        } else {
            // SAFETY: cursor <= capacity, ptr is allocated and valid.
            unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.cursor) }
        }
    }

    /// Creates an arena chunk pre-populated from a raw data slice.
    pub fn from_raw_parts(
        capacity: usize,
        cursor: usize,
        generation: u32,
        data: &[u8],
    ) -> Result<Self, ArenaError> {
        let mut chunk = Self::new(capacity, generation)?;
        if cursor > capacity || data.len() > capacity {
            return Err(ArenaError::InvalidOffset);
        }
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

/// Chunked slab allocator for variable-length payload storage.
pub struct BlobArena {
    chunks: Vec<ArenaChunk>,
    active_chunk: Option<usize>,
    chunk_size: usize,
    total_allocated: usize,
    live_bytes: usize,
}

impl BlobArena {
    /// Creates a new `BlobArena` with the specified chunk size.
    #[must_use]
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunks: Vec::new(),
            active_chunk: None,
            chunk_size: if chunk_size < 4096 { 4096 } else { chunk_size },
            total_allocated: 0,
            live_bytes: 0,
        }
    }

    /// Allocates a blob payload in the arena, returning its 24-bit global offset.
    pub fn alloc_blob(&mut self, data: &[u8]) -> Result<u32, ArenaError> {
        let needed = 8 + data.len();
        if needed > self.chunk_size {
            return Err(ArenaError::AllocationFailed);
        }

        if let Some(idx) = self.active_chunk {
            if self.chunks[idx].can_fit(data.len()) {
                let offset_in_chunk = self.chunks[idx].alloc(data)?;
                let global_offset = (idx * self.chunk_size + offset_in_chunk) as u32;
                if global_offset > 0x00FF_FFFF {
                    return Err(ArenaError::OffsetOverflow);
                }
                self.live_bytes += needed;
                return Ok(global_offset);
            }
        }

        // Allocate a new chunk
        let next_generation = 1u32;
        let mut new_chunk = ArenaChunk::new(self.chunk_size, next_generation)?;
        let offset_in_chunk = new_chunk.alloc(data)?;
        let idx = self.chunks.len();
        let global_offset = (idx * self.chunk_size + offset_in_chunk) as u32;
        if global_offset > 0x00FF_FFFF {
            return Err(ArenaError::OffsetOverflow);
        }
        self.chunks.push(new_chunk);
        self.total_allocated += self.chunk_size;
        self.active_chunk = Some(idx);
        self.live_bytes += needed;
        Ok(global_offset)
    }

    /// Returns a slice of the blob payload at the given 24-bit arena offset.
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

    /// Records that a blob at `global_offset` has been deleted or overwritten.
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

    /// In-place mark-compact GC consolidating live payloads into fresh chunk(s),
    /// updating `ValueSlot` arena offsets directly in the trie index, and freeing dead chunks.
    pub fn compact_with_index(
        &mut self,
        index: &mut ExpanseMap,
    ) -> Result<CompactionStats, ArenaError> {
        let live_bytes_before = self.live_bytes;
        let total_allocated_before = self.total_allocated;
        let chunks_before = self.chunks.len();

        let mut new_arena = BlobArena::new(self.chunk_size);
        let mut live_records_moved = 0usize;

        let live_entries: Vec<(Key, u32, u32)> = index
            .iter()
            .filter_map(|(key, raw_slot)| {
                let slot = ValueSlot::from_raw(raw_slot);
                if slot.tag() == SlotTag::ArenaShort {
                    Some((key, slot.arena_offset(), slot.hot_meta()))
                } else {
                    None
                }
            })
            .collect();

        for (key, old_offset, meta) in live_entries {
            if let Some(payload) = self.get_blob_slice(old_offset) {
                let new_offset = new_arena.alloc_blob(payload)?;
                let new_slot = ValueSlot::new_arena_short(meta, new_offset)
                    .ok_or(ArenaError::OffsetOverflow)?;
                if let Some(mut slot_ptr) = index.get_value_slot(key) {
                    // SAFETY: slot_ptr points to the live slot of key in the index
                    unsafe {
                        *slot_ptr.as_mut() = new_slot.to_raw();
                    }
                }
                live_records_moved += 1;
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
    pub fn insert(&mut self, key: Key, data: &[u8], hot_meta: u32) -> Result<(), ArenaError> {
        let old_slot = self.index.get(key).map(ValueSlot::from_raw);
        if data.len() <= 7 {
            let slot = ValueSlot::new_inline(data).ok_or(ArenaError::AllocationFailed)?;
            self.index.insert(key, slot.to_raw());
            if let Some(old) = old_slot {
                if old.tag() == SlotTag::ArenaShort {
                    self.arena.record_deleted(old.arena_offset());
                }
            }
            Ok(())
        } else {
            let offset = self.arena.alloc_blob(data)?;
            let slot =
                ValueSlot::new_arena_short(hot_meta, offset).ok_or(ArenaError::OffsetOverflow)?;
            self.index.insert(key, slot.to_raw());
            if let Some(old) = old_slot {
                if old.tag() == SlotTag::ArenaShort {
                    self.arena.record_deleted(old.arena_offset());
                }
            }
            Ok(())
        }
    }

    /// Point lookup returning a zero-copy [`BlobView`] and the 32-bit hot metadata word.
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
            _ => None,
        }
    }

    /// Removes a key from the map, returning `true` if the key was present.
    pub fn remove(&mut self, key: Key) -> bool {
        if let Some(raw_val) = self.index.remove(key) {
            let slot = ValueSlot::from_raw(raw_val);
            if slot.tag() == SlotTag::ArenaShort {
                self.arena.record_deleted(slot.arena_offset());
            }
            true
        } else {
            false
        }
    }

    /// Executes a range scan with a predicate evaluated against hot metadata
    /// before dereferencing cold payload cache lines.
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
            let meta = slot.hot_meta();
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

        let header = BlobMapFileHeader {
            magic: EXPANSE_MAGIC,
            version: EXPANSE_FORMAT_VERSION,
            flags: 0,
            entry_count,
            index_offset,
            arena_offset,
            total_size,
            chunk_size: self.arena.chunk_size as u64,
            chunk_count,
        };

        // SAFETY: BlobMapFileHeader is 64 bytes repr(C), safe to cast to bytes slice.
        let header_bytes = unsafe {
            core::slice::from_raw_parts((&header as *const BlobMapFileHeader).cast::<u8>(), 64)
        };
        writer.write_all(header_bytes)?;

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

        // SAFETY: bytes has length >= 64.
        let header: BlobMapFileHeader =
            unsafe { core::ptr::read_unaligned(bytes.as_ptr().cast::<BlobMapFileHeader>()) };

        if header.magic != EXPANSE_MAGIC || header.version != EXPANSE_FORMAT_VERSION {
            return Err(ArenaError::CorruptedHeader);
        }

        if header.total_size as usize > bytes.len() {
            return Err(ArenaError::CorruptedHeader);
        }

        let mut map = Self::with_chunk_size(header.chunk_size as usize);

        // Read arena chunks
        let mut arena_pos = header.arena_offset as usize;
        for _ in 0..header.chunk_count {
            if arena_pos + 24 > bytes.len() {
                return Err(ArenaError::CorruptedHeader);
            }
            let cap =
                u64::from_le_bytes(bytes[arena_pos..arena_pos + 8].try_into().unwrap()) as usize;
            let cur = u64::from_le_bytes(bytes[arena_pos + 8..arena_pos + 16].try_into().unwrap())
                as usize;
            let generation =
                u32::from_le_bytes(bytes[arena_pos + 16..arena_pos + 20].try_into().unwrap());
            arena_pos += 24;

            if arena_pos + cur > bytes.len() {
                return Err(ArenaError::CorruptedHeader);
            }

            let chunk_data = &bytes[arena_pos..arena_pos + cur];
            let chunk = ArenaChunk::from_raw_parts(cap, cur, generation, chunk_data)?;
            map.arena.push_chunk(chunk);

            let aligned_cur = (cur + 15) & !15;
            arena_pos += aligned_cur;
        }

        // Read index entries
        let mut idx_pos = header.index_offset as usize;
        for _ in 0..header.entry_count {
            if idx_pos + 16 > bytes.len() {
                return Err(ArenaError::CorruptedHeader);
            }
            let key = u64::from_le_bytes(bytes[idx_pos..idx_pos + 8].try_into().unwrap());
            let raw_slot = u64::from_le_bytes(bytes[idx_pos + 8..idx_pos + 16].try_into().unwrap());
            idx_pos += 16;
            map.index.insert(key, raw_slot);

            // Recompute live_bytes if ArenaShort
            let slot = ValueSlot::from_raw(raw_slot);
            if slot.tag() == SlotTag::ArenaShort {
                let offset = slot.arena_offset();
                if let Some(payload) = map.arena.get_blob_slice(offset) {
                    map.arena.live_bytes += 8 + payload.len();
                }
            }
        }

        Ok(map)
    }

    /// Loads a blob map from a binary image file at `path`.
    pub fn mmap_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ArenaError> {
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
    fn mmap_file_save_and_load_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("expanse_test_mmap.bin");

        let mut map = ExpanseBlobMap::with_chunk_size(64 * 1024);
        for i in 0..50u64 {
            let payload = format!("test-payload-record-{i}");
            map.insert(i * 10, payload.as_bytes(), i as u32).unwrap();
        }

        map.save_to_file(&path).unwrap();

        let loaded = ExpanseBlobMap::mmap_file(&path).unwrap();
        assert_eq!(loaded.len(), 50);

        for i in 0..50u64 {
            let (view, meta) = loaded.get(i * 10).unwrap();
            let expected = format!("test-payload-record-{i}");
            assert_eq!(view.as_bytes(), expected.as_bytes());
            assert_eq!(meta, i as u32);
        }

        let _ = std::fs::remove_file(path);
    }
}
