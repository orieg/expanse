package io.github.orieg.expanse;

import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.MemorySegment;
import java.util.function.LongPredicate;

/**
 * Multithreaded concurrent ordered set with lock-free readers (OCC).
 * <p>
 * Supports multiple concurrent reader threads without acquiring read locks or stalling writers.
 */
public final class SyncExpanseSet implements AutoCloseable, LongPredicate {

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * A registered reader handle for a thread.
     * <p>
     * Reusing a {@link Reader} across hot loops avoids creating/destroying throwaway readers.
     */
    public final class Reader implements AutoCloseable, LongPredicate {
        private MemorySegment readerHandle;
        private boolean readerClosed = false;

        private Reader(MemorySegment readerHandle) {
            this.readerHandle = readerHandle;
        }

        /**
         * Lock-free membership test.
         *
         * @param key key to test
         * @return true if key exists
         */
        public boolean contains(long key) {
            if (readerClosed || readerHandle.equals(MemorySegment.NULL)) {
                throw new IllegalStateException("Reader has been closed");
            }
            checkOpen();
            try {
                return (boolean) ExpanseNative.MH_expanse_sync_set_reader_contains.invokeExact(readerHandle, key);
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
        }

        @Override
        public boolean test(long value) {
            return contains(value);
        }

        @Override
        public void close() {
            if (!readerClosed && !readerHandle.equals(MemorySegment.NULL)) {
                try {
                    ExpanseNative.MH_expanse_sync_set_reader_free.invokeExact(readerHandle);
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
     * Creates a new empty concurrent {@link SyncExpanseSet}.
     */
    public SyncExpanseSet() {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_sync_set_new.invokeExact();
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native expanse_sync_set_t");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating SyncExpanseSet", t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("SyncExpanseSet has been closed");
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
            MemorySegment r = (MemorySegment) ExpanseNative.MH_expanse_sync_set_reader_new.invokeExact(handle);
            if (r.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating SyncExpanseSet reader");
            }
            return new Reader(r);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts a key. Writers serialize internally.
     *
     * @param key key to insert
     * @return true if key was newly inserted
     */
    public boolean insert(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_sync_set_insert.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes a key.
     *
     * @param key key to remove
     * @return true if key was present and removed
     */
    public boolean remove(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_sync_set_remove.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * One-shot membership test.
     *
     * @param key key to check
     * @return true if present
     */
    public boolean contains(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_sync_set_contains.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    @Override
    public boolean test(long value) {
        return contains(value);
    }

    /**
     * Number of keys in the set.
     *
     * @return key count
     */
    public long size() {
        return len();
    }

    /**
     * Number of keys in the set.
     *
     * @return key count
     */
    public long len() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_sync_set_len.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_sync_set_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException("Failed to free SyncExpanseSet", t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
            }
        }
    }
}
