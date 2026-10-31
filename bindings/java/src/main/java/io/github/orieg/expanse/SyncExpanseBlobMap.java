package io.github.orieg.expanse;

import static io.github.orieg.expanse.internal.ExpanseNative.*;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.Set;

/**
 * Multithreaded concurrent map from 64-bit keys to byte payloads with 24-bit hot metadata, with
 * optimistic readers (OCC) beside a serialized writer: the shared form of {@link ExpanseBlobMap}.
 * <p>
 * Values are copied, never borrowed: every read returns a fresh {@code byte[]}, so nothing a
 * caller holds is invalidated by a later insert, remove or compaction. A read into a native
 * buffer that proves too small is retried inside the wrapper with a buffer of the length the
 * native read reported; as the retry is a new read, it observes whatever a writer left in
 * between (a longer or shorter value, or none), and the value returned is the one that read saw.
 * A payload larger than {@code Integer.MAX_VALUE - 8} bytes cannot be returned as an array and
 * raises {@link IllegalStateException}.
 * <p>
 * Hot metadata is the 24-bit field of an arena-stored value. A value is stored in its slot word,
 * without that field, only when {@code hotMeta} is 0 (a payload of 7 bytes or fewer, or one the
 * map compresses into the slot word), and reads back metadata 0; a payload of any length inserted
 * with non-zero {@code hotMeta} is arena-stored and reads it back, and a {@code hotMeta} wider
 * than 24 bits is refused at every length (issue #1548).
 * <p>
 * Protocol, as for {@link SyncExpanseMap}: the map is safe from any number of threads at once
 * except {@link #close()}; a {@link Reader} is used by one thread at a time and is closed before
 * its map (closing the map closes every reader still open).
 */
public final class SyncExpanseBlobMap implements AutoCloseable {

    private static final int ST_OK = 0;
    private static final int ST_NOT_FOUND = 1;
    private static final int ST_BUFFER_TOO_SMALL = 2;
    private static final int ST_MISMATCH = 3;
    private static final int ST_META_OVERFLOW = 16;
    private static final int ST_ALLOCATION_FAILED = 17;
    private static final int ST_CAP_REFUSED = 18;
    private static final int ST_ARENA_FULL = 19;
    private static final int ST_NO_META_FIELD = 20;
    private static final int ST_INVALID_ARGUMENT = 32;

    private static final int INITIAL_BUF = 256;
    private static final int MAX_ARRAY = Integer.MAX_VALUE - 8;

    /**
     * A stored value: its payload and its hot metadata.
     *
     * @param data payload bytes
     * @param hotMeta the 24-bit hot metadata (0 for a payload stored in its slot word)
     */
    public record Blob(byte[] data, int hotMeta) {
        /** Validates that the payload is present. */
        public Blob {
            Objects.requireNonNull(data, "data must not be null");
        }
    }

    /** Why a compare-and-swap or get-or-insert did, or did not, change the map. */
    public enum ExchangeStatus {
        /** The new value was stored (or, for a compare-and-swap to absent, the key removed). */
        OK,
        /** Nothing stored: the current value was not the expected one (or the key was present). */
        MISMATCH,
        /** The current value was the expected one, but the new metadata is wider than 24 bits. */
        META_OVERFLOW,
        /** The current value was the expected one, but the new payload could not be allocated. */
        ALLOCATION_FAILED,
        /** The current value was the expected one, but the capacity cap refused a chunk. */
        CAP_REFUSED,
        /** The current value was the expected one, but the arena is full after a compaction. */
        ARENA_FULL,
        /** Metadata swap only: the value has no metadata field to swap. */
        NO_META_FIELD;

        static ExchangeStatus of(int code) {
            return switch (code) {
                case ST_OK -> OK;
                case ST_MISMATCH -> MISMATCH;
                case ST_META_OVERFLOW -> ExchangeStatus.META_OVERFLOW;
                case ST_ALLOCATION_FAILED -> ExchangeStatus.ALLOCATION_FAILED;
                case ST_CAP_REFUSED -> ExchangeStatus.CAP_REFUSED;
                case ST_ARENA_FULL -> ExchangeStatus.ARENA_FULL;
                case ST_NO_META_FIELD -> ExchangeStatus.NO_META_FIELD;
                case ST_INVALID_ARGUMENT -> throw new IllegalArgumentException("invalid argument rejected by the native map");
                default -> throw new IllegalStateException("Unexpected expanse_blob_status_t: " + code);
            };
        }
    }

    /**
     * The outcome of {@link #compareExchange} or {@link #getOrInsert}.
     *
     * @param status what happened
     * @param observed for a compare-and-swap, the value the compare saw (empty if absent; equal to
     *                 the expected value when the comparison held, i.e. on {@code OK} and on every
     *                 refusal); for get-or-insert, the value already present on {@code MISMATCH}
     *                 and empty otherwise
     */
    public record Exchange(ExchangeStatus status, Optional<Blob> observed) {
        /**
         * Whether the new value was stored.
         *
         * @return true on {@link ExchangeStatus#OK}
         */
        public boolean stored() {
            return status == ExchangeStatus.OK;
        }
    }

    /**
     * The outcome of {@link #compareExchangeMeta}.
     *
     * @param status what happened
     * @param observed the metadata the compare saw on {@code MISMATCH} (empty if the key is
     *                 absent); the expected metadata otherwise
     */
    public record MetaExchange(ExchangeStatus status, OptionalInt observed) {
        /**
         * Whether the metadata was swapped.
         *
         * @return true on {@link ExchangeStatus#OK}
         */
        public boolean stored() {
            return status == ExchangeStatus.OK;
        }
    }

    private MemorySegment handle;
    // volatile so a concurrent thread observes closure promptly and refuses.
    private volatile boolean closed = false;

    // Live readers borrow the map's native storage; they are freed, before the map, when it
    // closes. Guarded by synchronization on `this`.
    private final Set<Reader> liveReaders = Collections.newSetFromMap(new IdentityHashMap<>());

    /**
     * A registered reader handle: optimistic reads that never take the writer's lock. Used by
     * one thread at a time and closed before its map (closing the map closes it too).
     */
    public final class Reader implements AutoCloseable {
        private MemorySegment readerHandle;
        private volatile boolean readerClosed = false;

        private Reader(MemorySegment readerHandle) {
            this.readerHandle = readerHandle;
        }

        private void invalidate() {
            if (!readerClosed && !readerHandle.equals(MemorySegment.NULL)) {
                try {
                    MH_expanse_sync_blob_map_reader_free.invokeExact(readerHandle);
                } catch (Throwable t) {
                    throw rethrow(t);
                } finally {
                    readerHandle = MemorySegment.NULL;
                    readerClosed = true;
                }
            }
        }

        private MemorySegment live() {
            if (readerClosed || readerHandle.equals(MemorySegment.NULL)) {
                throw new IllegalStateException("Reader has been closed");
            }
            checkOpen();
            return readerHandle;
        }

        /**
         * Optimistic lookup of the payload and metadata.
         *
         * @param key 64-bit key
         * @return the value, or empty if the key is absent
         */
        public Optional<Blob> get(long key) {
            MemorySegment r = live();
            try (Arena a = Arena.ofConfined()) {
                Read read = readInto(a, (buf, cap, out) -> (int) MH_expanse_sync_blob_map_reader_get_into.invokeExact(
                        r, key, buf, cap, out, out.asSlice(8, 4)));
                return found(read);
            } catch (Throwable t) {
                throw rethrow(t);
            }
        }

        /**
         * Optimistic metadata lookup, copying no payload.
         *
         * @param key 64-bit key
         * @return the metadata, or empty if the key is absent
         */
        public OptionalInt getMeta(long key) {
            MemorySegment r = live();
            try (Arena a = Arena.ofConfined()) {
                MemorySegment out = a.allocate(ValueLayout.JAVA_INT);
                boolean found = (boolean) MH_expanse_sync_blob_map_reader_get_meta.invokeExact(r, key, out);
                return found ? OptionalInt.of(out.get(ValueLayout.JAVA_INT, 0)) : OptionalInt.empty();
            } catch (Throwable t) {
                throw rethrow(t);
            }
        }

        /**
         * Checks if a key is present.
         *
         * @param key 64-bit key
         * @return true if present
         */
        public boolean containsKey(long key) {
            MemorySegment r = live();
            try {
                return (boolean) MH_expanse_sync_blob_map_reader_contains.invokeExact(r, key);
            } catch (Throwable t) {
                throw rethrow(t);
            }
        }

        /** Frees the reader. Idempotent; must precede the map's own close. */
        @Override
        public void close() {
            synchronized (SyncExpanseBlobMap.this) {
                liveReaders.remove(this);
                invalidate();
            }
        }
    }

    /** Creates a new empty concurrent blob map with the default chunk size and capacity cap. */
    public SyncExpanseBlobMap() {
        this(0, -1);
    }

    /**
     * Creates a new empty concurrent blob map with a custom slab chunk size.
     *
     * @param chunkSize slab chunk size in bytes (0: the default 2 MiB)
     */
    public SyncExpanseBlobMap(long chunkSize) {
        this(chunkSize, -1);
    }

    /**
     * Creates a new empty concurrent blob map whose arena capacity cap is {@code maxCapacity}
     * bytes of allocated chunks, clamped to {@code [chunkSize, 64 GiB]}.
     *
     * @param chunkSize slab chunk size in bytes (0: the default 2 MiB)
     * @param maxCapacity capacity cap in bytes (0: the default 1 GiB)
     */
    public SyncExpanseBlobMap(long chunkSize, long maxCapacity) {
        try {
            this.handle = maxCapacity < 0
                    ? (MemorySegment) MH_expanse_sync_blob_map_new.invokeExact(chunkSize)
                    : (MemorySegment) MH_expanse_sync_blob_map_new_with_capacity.invokeExact(chunkSize, maxCapacity);
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating SyncExpanseBlobMap", t);
        }
        if (handle.equals(MemorySegment.NULL)) {
            throw new OutOfMemoryError("Failed to allocate native expanse_sync_blob_map_t");
        }
    }

    private static RuntimeException rethrow(Throwable t) {
        return SyncKeyedMap.rethrow(t);
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("SyncExpanseBlobMap has been closed");
        }
    }

    /**
     * Registers and returns a new {@link Reader}. Close it before the map, or let the map's
     * {@link #close()} close it.
     *
     * @return new Reader instance
     */
    public synchronized Reader reader() {
        checkOpen();
        try {
            MemorySegment r = (MemorySegment) MH_expanse_sync_blob_map_reader_new.invokeExact(handle);
            if (r.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating SyncExpanseBlobMap reader");
            }
            Reader reader = new Reader(r);
            liveReaders.add(reader);
            return reader;
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    // ---- the caller-buffer retry loop -------------------------------------------------------

    /** One native call into a caller buffer; the length goes to out[0..8) and the metadata to out[8..12). */
    @FunctionalInterface
    private interface BufferCall {
        int run(MemorySegment buf, long cap, MemorySegment out) throws Throwable;
    }

    /** A native call's status with the payload it copied, valid while its arena is open. */
    private record Read(int status, boolean present, MemorySegment buf, long len, int meta) {
        Blob blob() {
            return new Blob(buf.asSlice(0, len).toArray(ValueLayout.JAVA_BYTE), meta);
        }
    }

    /**
     * Runs {@code call} with a scratch buffer, growing it to the length the native call reports
     * for as long as the status is BUFFER_TOO_SMALL. Each retry is a new native call, so it
     * reports the state of the map at that call. The buffer lives in {@code arena}.
     */
    private static Read readInto(Arena arena, BufferCall call) throws Throwable {
        MemorySegment out = arena.allocate(16, 8); // length at 0, metadata at 8, observed-present at 12
        long cap = INITIAL_BUF;
        while (true) {
            MemorySegment buf = arena.allocate(cap);
            int status = call.run(buf, cap, out);
            if (status == ST_BUFFER_TOO_SMALL) {
                long need = out.get(ValueLayout.JAVA_LONG, 0);
                if (need < 0 || need > MAX_ARRAY) {
                    throw new IllegalStateException("Blob payload of " + Long.toUnsignedString(need)
                            + " bytes exceeds the maximum Java array size (" + MAX_ARRAY + ")");
                }
                cap = Math.max(need, 1);
                continue;
            }
            return new Read(status, out.get(ValueLayout.JAVA_BYTE, 12) != 0, buf,
                    out.get(ValueLayout.JAVA_LONG, 0), out.get(ValueLayout.JAVA_INT, 8));
        }
    }

    /** The value of a finished read: present on OK, absent on NOT_FOUND, an error otherwise. */
    private static Optional<Blob> found(Read read) {
        return switch (read.status()) {
            case ST_OK -> Optional.of(read.blob());
            case ST_NOT_FOUND -> Optional.empty();
            case ST_INVALID_ARGUMENT -> throw new IllegalArgumentException("invalid argument rejected by the native map");
            default -> throw new IllegalStateException("Unexpected expanse_blob_status_t: " + read.status());
        };
    }

    private static MemorySegment payload(byte[] data, Arena arena) {
        if (data.length == 0) {
            return MemorySegment.NULL;
        }
        MemorySegment seg = arena.allocate(data.length);
        MemorySegment.copy(data, 0, seg, ValueLayout.JAVA_BYTE, 0, data.length);
        return seg;
    }

    // ---- point operations -------------------------------------------------------------------

    /**
     * Inserts a key-blob mapping with hot metadata 0.
     *
     * @param key 64-bit key
     * @param data payload bytes
     * @return true if stored; false if refused (see {@link #insertStatus})
     */
    public boolean insert(long key, byte[] data) {
        return insertStatus(key, data, 0) == ExpanseBlobMap.InsertStatus.OK;
    }

    /**
     * Inserts a key-blob mapping with 24-bit hot metadata.
     *
     * @param key 64-bit key
     * @param data payload bytes
     * @param hotMeta hot metadata (at most 24 bits)
     * @return true if stored; false if refused (see {@link #insertStatus})
     */
    public boolean insert(long key, byte[] data, int hotMeta) {
        return insertStatus(key, data, hotMeta) == ExpanseBlobMap.InsertStatus.OK;
    }

    /**
     * Inserts like {@link #insert(long, byte[], int)}, returning why a refused insert was
     * refused. A refusal leaves the map unchanged.
     *
     * @param key 64-bit key
     * @param data payload bytes
     * @param hotMeta hot metadata (at most 24 bits)
     * @return the insert's status
     */
    public ExpanseBlobMap.InsertStatus insertStatus(long key, byte[] data, int hotMeta) {
        Objects.requireNonNull(data, "data must not be null");
        checkOpen();
        try (Arena a = Arena.ofConfined()) {
            int rc = (int) MH_expanse_sync_blob_map_insert.invokeExact(
                    handle, key, payload(data, a), (long) data.length, hotMeta);
            return ExpanseBlobMap.InsertStatus.of(rc);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Removes the mapping for {@code key}.
     *
     * @param key 64-bit key
     * @return true if the key was present and removed
     */
    public boolean remove(long key) {
        checkOpen();
        try {
            return (boolean) MH_expanse_sync_blob_map_remove.invokeExact(handle, key);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * One-shot lookup of the payload and metadata (for hot loops prefer a {@link #reader()}).
     *
     * @param key 64-bit key
     * @return the value, or empty if the key is absent
     */
    public Optional<Blob> get(long key) {
        checkOpen();
        try (Arena a = Arena.ofConfined()) {
            Read read = readInto(a, (buf, cap, out) -> (int) MH_expanse_sync_blob_map_get_into.invokeExact(
                    handle, key, buf, cap, out, out.asSlice(8, 4)));
            return found(read);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Retrieves only the payload bytes for the given key.
     *
     * @param key 64-bit key
     * @return byte array if present, null otherwise
     */
    public byte[] getBytes(long key) {
        return get(key).map(Blob::data).orElse(null);
    }

    /**
     * Metadata lookup, copying no payload.
     *
     * @param key 64-bit key
     * @return the metadata, or empty if the key is absent
     */
    public OptionalInt getMeta(long key) {
        checkOpen();
        try (Arena a = Arena.ofConfined()) {
            MemorySegment out = a.allocate(ValueLayout.JAVA_INT);
            boolean found = (boolean) MH_expanse_sync_blob_map_get_meta.invokeExact(handle, key, out);
            return found ? OptionalInt.of(out.get(ValueLayout.JAVA_INT, 0)) : OptionalInt.empty();
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Checks if a key is present.
     *
     * @param key 64-bit key
     * @return true if present
     */
    public boolean containsKey(long key) {
        checkOpen();
        try {
            return (boolean) MH_expanse_sync_blob_map_contains.invokeExact(handle, key);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    // ---- read-modify-write ------------------------------------------------------------------

    /**
     * Atomic compare-and-swap on one key, as one linearizable step. A value is its payload bytes
     * and its metadata, and both are compared. An empty {@link Optional} is "absent" on both
     * sides: {@code (empty, v)} inserts only if absent and {@code (e, empty)} removes if equal.
     * Nothing is stored unless the comparison held; a refusal to store a value that was expected
     * (see {@link ExchangeStatus}) leaves the map unchanged.
     *
     * @param key 64-bit key
     * @param expected the value the key must currently hold, or empty for "only if absent"
     * @param newValue the value to store, or empty to remove the key
     * @return the outcome, with the value the compare saw
     */
    public Exchange compareExchange(long key, Optional<Blob> expected, Optional<Blob> newValue) {
        Objects.requireNonNull(expected, "expected must not be null");
        Objects.requireNonNull(newValue, "newValue must not be null");
        checkOpen();
        try (Arena a = Arena.ofConfined()) {
            MemorySegment expData = expected.map(b -> payload(b.data(), a)).orElse(MemorySegment.NULL);
            long expLen = expected.map(b -> (long) b.data().length).orElse(0L);
            int expMeta = expected.map(Blob::hotMeta).orElse(0);
            MemorySegment newData = newValue.map(b -> payload(b.data(), a)).orElse(MemorySegment.NULL);
            long newLen = newValue.map(b -> (long) b.data().length).orElse(0L);
            int newMeta = newValue.map(Blob::hotMeta).orElse(0);
            Read read = readInto(a, (buf, cap, out) -> (int) MH_expanse_sync_blob_map_compare_exchange.invokeExact(
                    handle, key, expected.isPresent(), expData, expLen, expMeta,
                    newValue.isPresent(), newData, newLen, newMeta,
                    out.asSlice(12, 1), buf, cap, out, out.asSlice(8, 4)));
            ExchangeStatus status = ExchangeStatus.of(read.status());
            Optional<Blob> observed = status == ExchangeStatus.MISMATCH
                    ? (read.present() ? Optional.of(read.blob()) : Optional.empty())
                    : expected;
            return new Exchange(status, observed);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Stores {@code data} under {@code key} only if the key is absent, as one linearizable step.
     * Of any number of callers racing on one absent key, exactly one stores.
     *
     * @param key 64-bit key
     * @param data payload to store if the key is absent
     * @param hotMeta hot metadata to store with it (at most 24 bits)
     * @return {@code OK} when stored; {@code MISMATCH} with the value already present (left
     *         untouched); or a refusal ({@code META_OVERFLOW}, {@code ALLOCATION_FAILED},
     *         {@code CAP_REFUSED}, {@code ARENA_FULL}) when the key was absent but the value
     *         cannot be stored, with the map unchanged
     */
    public Exchange getOrInsert(long key, byte[] data, int hotMeta) {
        Objects.requireNonNull(data, "data must not be null");
        checkOpen();
        try (Arena a = Arena.ofConfined()) {
            MemorySegment seg = payload(data, a);
            Read read = readInto(a, (buf, cap, out) -> (int) MH_expanse_sync_blob_map_get_or_insert.invokeExact(
                    handle, key, seg, (long) data.length, hotMeta, buf, cap, out, out.asSlice(8, 4)));
            ExchangeStatus status = ExchangeStatus.of(read.status());
            return new Exchange(status,
                    status == ExchangeStatus.MISMATCH ? Optional.of(read.blob()) : Optional.empty());
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Atomic compare-and-swap of the 24-bit metadata alone, with no payload copy and no
     * allocation: the form for a retry loop whose state lives in the metadata. An empty
     * {@link OptionalInt} is "absent" on both sides; metadata alone cannot create a key, so
     * expecting absent and storing a value reports {@code NO_META_FIELD}.
     *
     * @param key 64-bit key
     * @param expected the metadata the key must hold, or empty for "only if absent"
     * @param newMeta the metadata to store, or empty to remove the key
     * @return the outcome: {@code OK}, {@code MISMATCH} with the observed metadata,
     *         {@code META_OVERFLOW}, or {@code NO_META_FIELD} (the comparison held but the value
     *         has no metadata field)
     */
    public MetaExchange compareExchangeMeta(long key, OptionalInt expected, OptionalInt newMeta) {
        Objects.requireNonNull(expected, "expected must not be null");
        Objects.requireNonNull(newMeta, "newMeta must not be null");
        checkOpen();
        try (Arena a = Arena.ofConfined()) {
            MemorySegment out = a.allocate(8, 4); // observed metadata at 0, observed-present at 4
            int rc = (int) MH_expanse_sync_blob_map_compare_exchange_meta.invokeExact(
                    handle, key, expected.isPresent(), expected.orElse(0),
                    newMeta.isPresent(), newMeta.orElse(0), out.asSlice(4, 1), out);
            ExchangeStatus status = ExchangeStatus.of(rc);
            OptionalInt observed = status == ExchangeStatus.MISMATCH
                    ? (out.get(ValueLayout.JAVA_BYTE, 4) != 0 ? OptionalInt.of(out.get(ValueLayout.JAVA_INT, 0))
                            : OptionalInt.empty())
                    : expected;
            return new MetaExchange(status, observed);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    // ---- accounting and maintenance ---------------------------------------------------------

    /**
     * Number of entries.
     *
     * @return entry count
     */
    public long size() {
        return len();
    }

    /**
     * Number of entries.
     *
     * @return entry count
     */
    public long len() {
        checkOpen();
        try {
            return (long) MH_expanse_sync_blob_map_len.invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Off-heap bytes in use. Reads with writers excluded and waits for a running compaction.
     *
     * @return bytes
     */
    public long memUsed() {
        checkOpen();
        try {
            return (long) MH_expanse_sync_blob_map_mem_used.invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * An estimate of the off-heap bytes in use that takes no lock and waits for nothing.
     *
     * @return bytes
     */
    public long memUsedEstimate() {
        checkOpen();
        try {
            return (long) MH_expanse_sync_blob_map_mem_used_estimate.invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Off-heap bytes held from the system allocator, including blocks the epoch collector keeps
     * for reuse. Reads with writers excluded and waits for a running compaction.
     *
     * @return bytes
     */
    public long memHeld() {
        checkOpen();
        try {
            return (long) MH_expanse_sync_blob_map_mem_held.invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * An estimate of {@link #memHeld()} that takes no map lock and waits for nothing; it may lag
     * a running compaction or a busy collector freelist.
     *
     * @return bytes
     */
    public long memHeldEstimate() {
        checkOpen();
        try {
            return (long) MH_expanse_sync_blob_map_mem_held_estimate.invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Returns the collector's freed index blocks to the system allocator.
     *
     * @return bytes released
     */
    public long shrinkToFit() {
        checkOpen();
        try {
            return (long) MH_expanse_sync_blob_map_shrink_to_fit.invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Turns the reclaim at the capacity cap on (the default) or off. Off, an insert the cap
     * refuses compacts nothing. The compare-and-swap and get-or-insert calls never run it.
     *
     * @param on whether the reclaim is on
     */
    public void setReclaimAtCap(boolean on) {
        checkOpen();
        try {
            MH_expanse_sync_blob_map_set_reclaim_at_cap.invokeExact(handle, on);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Compacts the arena, freeing dead bytes. Readers keep reading while it copies.
     *
     * @return {@code OK}, or the refusal
     */
    public ExpanseBlobMap.InsertStatus compact() {
        checkOpen();
        try {
            return ExpanseBlobMap.InsertStatus.of((int) MH_expanse_sync_blob_map_compact.invokeExact(handle));
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Closes the map, first closing every reader still open. Idempotent. No call may be in
     * progress on the map or on one of its readers.
     */
    @Override
    public synchronized void close() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            return;
        }
        for (Reader reader : liveReaders) {
            reader.invalidate();
        }
        liveReaders.clear();
        try {
            MH_expanse_sync_blob_map_free.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException("Failed to free SyncExpanseBlobMap", t);
        } finally {
            handle = MemorySegment.NULL;
            closed = true;
        }
    }
}
