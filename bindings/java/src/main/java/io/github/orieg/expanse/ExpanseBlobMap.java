package io.github.orieg.expanse;

import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.Objects;
import java.util.Optional;

/**
 * High-performance off-heap map from 64-bit keys to arbitrary-length byte payloads
 * backed by polymorphic 64-bit value slots and chunked slab arenas.
 */
public final class ExpanseBlobMap implements AutoCloseable {

    /**
     * A retrieved blob record containing byte payload, 32-bit hot metadata, and storage tag.
     *
     * @param data payload byte array
     * @param hotMeta 32-bit hot metadata word
     * @param isInline true if payload was stored inline in the value slot (<= 7 bytes)
     */
    public record BlobRecord(byte[] data, int hotMeta, boolean isInline) {}

    /**
     * Outcome of {@link #insertStatus}, one per {@code expanse_blob_status_t} value.
     */
    public enum InsertStatus {
        /** The value is stored. */
        OK,
        /** Refused: hot metadata wider than 24 bits. */
        META_OVERFLOW,
        /** Refused: payload larger than a chunk, or allocation failure. */
        ALLOCATION_FAILED,
        /** Refused: the capacity cap refused a chunk and nothing was compacted. */
        CAP_REFUSED,
        /** Refused: this insert compacted the arena and the record still does not fit. */
        ARENA_FULL,
        /** Usage error reported by the native call. */
        INVALID_ARGUMENT,
        /** Any other native status. */
        ERROR;

        static InsertStatus of(int code) {
            return switch (code) {
                case 0 -> OK;
                case 16 -> META_OVERFLOW;
                case 17 -> ALLOCATION_FAILED;
                case 18 -> CAP_REFUSED;
                case 19 -> ARENA_FULL;
                case 32 -> INVALID_ARGUMENT;
                default -> ERROR;
            };
        }
    }

    /**
     * Arena accounting returned by {@link #arenaStats()}.
     *
     * @param liveBytes live payload bytes plus an 8-byte header per record
     * @param allocatedBytes allocated chunk bytes, dead and live: what the cap counts
     * @param maxCapacity the capacity cap, as clamped
     * @param chunkSize the arena's chunk size
     * @param reclaimAtCap whether an insert the cap refuses may compact the arena
     */
    public record ArenaStats(long liveBytes, long allocatedBytes, long maxCapacity, long chunkSize,
            boolean reclaimAtCap) {}

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * Creates a new empty {@link ExpanseBlobMap} with default 2 MiB slab chunks.
     */
    public ExpanseBlobMap() {
        this(0);
    }

    /**
     * Creates a new empty {@link ExpanseBlobMap} with custom slab chunk capacity in bytes.
     *
     * @param chunkSize slab chunk size in bytes
     */
    public ExpanseBlobMap(long chunkSize) {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_blob_map_new.invokeExact(chunkSize);
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native ExpanseBlobMap");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating ExpanseBlobMap", t);
        }
    }

    /**
     * Creates a new empty {@link ExpanseBlobMap} whose arena capacity cap is
     * {@code maxCapacity} bytes of allocated chunks (0: the default 1 GiB), clamped to
     * {@code [chunkSize, 64 GiB]}.
     *
     * @param chunkSize slab chunk size in bytes (0: the default 2 MiB)
     * @param maxCapacity capacity cap in bytes (0: the default 1 GiB)
     */
    public ExpanseBlobMap(long chunkSize, long maxCapacity) {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_blob_map_new_with_capacity.invokeExact(
                    chunkSize, maxCapacity);
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native ExpanseBlobMap");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating ExpanseBlobMap", t);
        }
    }

    /**
     * Turns the reclaim at the capacity cap on (the default) or off. Off, an insert the
     * cap refuses compacts nothing.
     *
     * @param on whether the reclaim is on
     */
    public void setReclaimAtCap(boolean on) {
        checkOpen();
        try {
            ExpanseNative.MH_expanse_blob_map_set_reclaim_at_cap.invokeExact(handle, on);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the arena's accounting.
     *
     * @return live bytes, allocated chunk bytes, the cap, the chunk size and the switch
     */
    public ArenaStats arenaStats() {
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(ValueLayout.JAVA_LONG, 5);
            int rc = (int) ExpanseNative.MH_expanse_blob_map_arena_stats.invokeExact(handle, seg, 40L);
            if (rc != 0) {
                throw new IllegalStateException("expanse_blob_map_arena_stats returned " + rc);
            }
            return new ArenaStats(
                    seg.getAtIndex(ValueLayout.JAVA_LONG, 0),
                    seg.getAtIndex(ValueLayout.JAVA_LONG, 1),
                    seg.getAtIndex(ValueLayout.JAVA_LONG, 2),
                    seg.getAtIndex(ValueLayout.JAVA_LONG, 3),
                    seg.getAtIndex(ValueLayout.JAVA_LONG, 4) != 0);
        } catch (RuntimeException e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts like {@link #insert(long, byte[], int)}, returning why a refused insert was
     * refused: {@link InsertStatus#CAP_REFUSED} when nothing was compacted (dead bytes may
     * remain; see {@link #arenaStats()}), {@link InsertStatus#ARENA_FULL} when this insert
     * compacted and the record still does not fit.
     *
     * @param key 64-bit unsigned key
     * @param data payload byte array
     * @param hotMeta 32-bit hot metadata
     * @return the insert's status
     */
    public InsertStatus insertStatus(long key, byte[] data, int hotMeta) {
        Objects.requireNonNull(data, "data must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = MemorySegment.NULL;
            if (data.length > 0) {
                seg = arena.allocate(ValueLayout.JAVA_BYTE, data.length);
                MemorySegment.copy(data, 0, seg, ValueLayout.JAVA_BYTE, 0, data.length);
            }
            int rc = (int) ExpanseNative.MH_expanse_blob_map_insert_ex.invokeExact(
                    handle, key, seg, (long) data.length, hotMeta);
            return InsertStatus.of(rc);
        } catch (RuntimeException e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("ExpanseBlobMap has been closed");
        }
    }

    /**
     * Inserts a key-blob mapping with default hot metadata (0).
     *
     * @param key 64-bit unsigned key
     * @param data payload byte array
     * @return true on success
     */
    public boolean insert(long key, byte[] data) {
        return insert(key, data, 0);
    }

    /**
     * Inserts a key-blob mapping with 32-bit hot metadata.
     *
     * @param key 64-bit unsigned key
     * @param data payload byte array
     * @param hotMeta 32-bit hot metadata
     * @return true on success
     */
    public boolean insert(long key, byte[] data, int hotMeta) {
        Objects.requireNonNull(data, "data must not be null");
        checkOpen();
        if (data.length == 0) {
            return insert(key, MemorySegment.NULL, 0, hotMeta);
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(ValueLayout.JAVA_BYTE, data.length);
            MemorySegment.copy(data, 0, seg, ValueLayout.JAVA_BYTE, 0, data.length);
            return insert(key, seg, data.length, hotMeta);
        }
    }

    /**
     * Inserts a key-blob mapping from raw native memory segment.
     *
     * @param key 64-bit key
     * @param dataSegment memory segment pointing to payload bytes
     * @param len byte length
     * @param hotMeta 32-bit hot metadata
     * @return true on success
     */
    public boolean insert(long key, MemorySegment dataSegment, long len, int hotMeta) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_blob_map_insert.invokeExact(
                    handle, key, dataSegment, len, hotMeta);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Retrieves the blob record for the given key.
     *
     * @param key 64-bit key
     * @return Optional containing BlobRecord if key is present
     */
    public Optional<BlobRecord> get(long key) {
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            // Allocate 24-byte struct ExpanseBlobView: ptr(8), len(8), hot_meta(4), is_inline(1), pad(3)
            MemorySegment outView = arena.allocate(24, 8);
            boolean found = (boolean) ExpanseNative.MH_expanse_blob_map_get.invokeExact(handle, key, outView);
            if (!found) {
                return Optional.empty();
            }

            MemorySegment ptr = outView.get(ValueLayout.ADDRESS, 0);
            long len = outView.get(ValueLayout.JAVA_LONG, 8);
            int hotMeta = outView.get(ValueLayout.JAVA_INT, 16);
            boolean isInline = outView.get(ValueLayout.JAVA_BOOLEAN, 20);

            byte[] bytes;
            if (len == 0 || ptr.equals(MemorySegment.NULL)) {
                bytes = new byte[0];
            } else {
                // A Java array cannot exceed Integer.MAX_VALUE elements. Guard
                // explicitly: without this, `(int) len` for len >= 2^31 would either
                // go negative (NegativeArraySizeException) or silently truncate the
                // payload. Fail loud rather than corrupt.
                if (len > Integer.MAX_VALUE) {
                    throw new IllegalStateException(
                            "Blob payload of " + Long.toUnsignedString(len)
                            + " bytes exceeds the maximum Java array size ("
                            + Integer.MAX_VALUE + "); use a streaming/segment API instead");
                }
                bytes = new byte[(int) len];
                MemorySegment dataSeg = ptr.reinterpret(len);
                MemorySegment.copy(dataSeg, ValueLayout.JAVA_BYTE, 0, bytes, 0, (int) len);
            }

            return Optional.of(new BlobRecord(bytes, hotMeta, isInline));
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Retrieves only the payload bytes for the given key.
     *
     * @param key 64-bit key
     * @return byte array if present, null otherwise
     */
    public byte[] getBytes(long key) {
        return get(key).map(BlobRecord::data).orElse(null);
    }

    /**
     * Removes a key from the map.
     *
     * @param key 64-bit key
     * @return true if key was present and removed
     */
    public boolean remove(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_blob_map_remove.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks if key exists in the map.
     *
     * @param key 64-bit key
     * @return true if key is present
     */
    public boolean contains(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_blob_map_contains.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks if key exists in the map.
     *
     * @param key 64-bit key
     * @return true if key is present
     */
    public boolean containsKey(long key) {
        return contains(key);
    }

    /**
     * Returns the number of entries in the map.
     *
     * @return count of stored keys
     */
    public long len() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            return 0;
        }
        try {
            return (long) ExpanseNative.MH_expanse_blob_map_len.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns true if map is empty.
     *
     * @return true if empty
     */
    public boolean isEmpty() {
        return len() == 0;
    }

    /**
     * Returns total off-heap bytes used by index and slab arena.
     *
     * @return heap memory in bytes
     */
    public long memUsed() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            return 0;
        }
        try {
            return (long) ExpanseNative.MH_expanse_blob_map_mem_used.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Runs in-place arena garbage collection and compaction.
     *
     * @return true if compaction succeeded
     */
    public boolean compact() {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_blob_map_compact.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes all entries and resets the slab arena.
     */
    public void clear() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            return;
        }
        try {
            ExpanseNative.MH_expanse_blob_map_clear.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_blob_map_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException(t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
            }
        }
    }
}
