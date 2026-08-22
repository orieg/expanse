package io.github.orieg.expanse;

import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.OptionalLong;

/**
 * Multithreaded concurrent ordered map with lock-free readers (OCC).
 * <p>
 * Allows concurrent optimistic reader threads while a single writer serializes updates.
 */
public final class SyncExpanseMap implements AutoCloseable {

    private static final ThreadLocal<MemorySegment> SCRATCH =
            ThreadLocal.withInitial(() -> Arena.ofAuto().allocate(ValueLayout.JAVA_LONG, 2));

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * A registered reader handle for a thread.
     */
    public final class Reader implements AutoCloseable {
        private MemorySegment readerHandle;
        private boolean readerClosed = false;

        private Reader(MemorySegment readerHandle) {
            this.readerHandle = readerHandle;
        }

        /**
         * Lock-free lookup.
         *
         * @param key search key
         * @return OptionalLong containing value if present
         */
        public OptionalLong get(long key) {
            if (readerClosed || readerHandle.equals(MemorySegment.NULL)) {
                throw new IllegalStateException("Reader has been closed");
            }
            checkOpen();
            MemorySegment scratch = SCRATCH.get();
            try {
                boolean found = (boolean) ExpanseNative.MH_expanse_sync_map_reader_get.invokeExact(
                        readerHandle, key, scratch);
                return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
        }

        /**
         * Checks if key is present.
         *
         * @param key search key
         * @return true if present
         */
        public boolean containsKey(long key) {
            return get(key).isPresent();
        }

        @Override
        public void close() {
            if (!readerClosed && !readerHandle.equals(MemorySegment.NULL)) {
                try {
                    ExpanseNative.MH_expanse_sync_map_reader_free.invokeExact(readerHandle);
                } catch (Throwable t) {
                    throw new RuntimeException(t);
                } finally {
                    readerHandle = MemorySegment.NULL;
                    readerClosed = true;
                }
            }
        }
    }

    /**
     * Creates a new empty concurrent {@link SyncExpanseMap}.
     */
    public SyncExpanseMap() {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_sync_map_new.invokeExact();
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native expanse_sync_map_t");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating SyncExpanseMap", t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("SyncExpanseMap has been closed");
        }
    }

    /**
     * Registers and returns a new {@link Reader} handle for the current thread.
     *
     * @return new Reader instance
     */
    public Reader reader() {
        checkOpen();
        try {
            MemorySegment r = (MemorySegment) ExpanseNative.MH_expanse_sync_map_reader_new.invokeExact(handle);
            if (r.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating SyncExpanseMap reader");
            }
            return new Reader(r);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts or updates a key-value mapping.
     *
     * @param key 64-bit key
     * @param value 64-bit value
     * @return true if key was newly inserted
     */
    public boolean insert(long key, long value) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_sync_map_insert.invokeExact(
                    handle, key, value, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts or updates a key-value mapping.
     *
     * @param key 64-bit key
     * @param value 64-bit value
     * @return true if key was newly inserted
     */
    public boolean put(long key, long value) {
        return insert(key, value);
    }

    /**
     * Removes the mapping for {@code key}.
     *
     * @param key key to remove
     * @return true if key was present and removed
     */
    public boolean remove(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_sync_map_remove.invokeExact(
                    handle, key, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * One-shot lookup (for hot loops prefer a {@link #reader()}).
     *
     * @param key search key
     * @return OptionalLong containing value if present
     */
    public OptionalLong get(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_sync_map_get.invokeExact(handle, key, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks if key exists.
     *
     * @param key search key
     * @return true if present
     */
    public boolean containsKey(long key) {
        return get(key).isPresent();
    }

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
            return (long) ExpanseNative.MH_expanse_sync_map_len.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_sync_map_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException("Failed to free SyncExpanseMap", t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
            }
        }
    }
}
