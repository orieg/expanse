package io.github.orieg.expanse;

import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.OptionalLong;
import java.util.Set;
import java.util.function.UnaryOperator;

/**
 * Multithreaded concurrent ordered map with lock-free readers (OCC).
 * <p>
 * Allows concurrent optimistic reader threads while a single writer serializes updates.
 */
public final class SyncExpanseMap implements AutoCloseable {

    private static final ThreadLocal<MemorySegment> SCRATCH =
            ThreadLocal.withInitial(() -> Arena.ofAuto().allocate(ValueLayout.JAVA_LONG, 2));

    private MemorySegment handle;
    // volatile so a concurrent reader thread observes closure promptly and refuses.
    private volatile boolean closed = false;

    // Live readers borrow the map's native storage (the native reader is a
    // 'static-transmuted borrow of the map), so a reader outliving the map is a
    // use-after-free. We track every live reader and free them all — before the map
    // itself — when the map closes. Guarded by synchronization on `this`.
    private final Set<Reader> liveReaders = Collections.newSetFromMap(new IdentityHashMap<>());

    /**
     * A registered reader handle for a thread.
     */
    public final class Reader implements AutoCloseable {
        private MemorySegment readerHandle;
        // volatile: set from the map's close() (another thread) as well as this reader's close().
        private volatile boolean readerClosed = false;

        private Reader(MemorySegment readerHandle) {
            this.readerHandle = readerHandle;
        }

        /**
         * Frees the native reader. Must be called while holding the lock on the owning
         * {@link SyncExpanseMap} so it is ordered before the map's own free. Idempotent.
         */
        private void invalidate() {
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
            synchronized (SyncExpanseMap.this) {
                liveReaders.remove(this);
                invalidate();
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
    public synchronized Reader reader() {
        checkOpen();
        try {
            MemorySegment r = (MemorySegment) ExpanseNative.MH_expanse_sync_map_reader_new.invokeExact(handle);
            if (r.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating SyncExpanseMap reader");
            }
            Reader reader = new Reader(r);
            liveReaders.add(reader);
            return reader;
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
     * The outcome of {@link #compareExchange}.
     *
     * @param stored true when the new word was stored
     * @param observed the word the compare saw (equal to the expected word when {@code stored});
     *                 empty if the key was absent
     */
    public record Exchange(boolean stored, OptionalLong observed) {}

    /**
     * Atomic compare-and-swap on one key, as one linearizable step: stores {@code newValue}
     * only if the key's current word is {@code expected}. An empty {@link OptionalLong} is
     * "absent" on both sides, so every {@code long} stays a legal value:
     * {@code (empty, v)} inserts if absent and {@code (e, empty)} removes if equal.
     * The result reports the word the compare saw on both outcomes, so a failed call hands
     * back the word to retry against.
     *
     * @param key 64-bit key
     * @param expected the word the key must currently hold, or empty for "only if absent"
     * @param newValue the word to store, or empty to remove the key
     * @return whether the store happened and the word the compare saw
     */
    public Exchange compareExchange(long key, OptionalLong expected, OptionalLong newValue) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean stored = (boolean) ExpanseNative.MH_expanse_sync_map_compare_exchange.invokeExact(
                    handle, key,
                    expected.isPresent(), expected.orElse(0L),
                    newValue.isPresent(), newValue.orElse(0L),
                    scratch.asSlice(8, 1), scratch);
            boolean seenPresent = scratch.get(ValueLayout.JAVA_BYTE, 8) != 0;
            return new Exchange(stored,
                    seenPresent ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty());
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Stores {@code value} under {@code key} only if the key is absent, as one linearizable
     * step. Of any number of callers racing on one absent key, exactly one stores.
     *
     * @param key 64-bit key
     * @param value 64-bit value
     * @return the value already present (left untouched), or empty when {@code value} was stored
     */
    public OptionalLong getOrInsert(long key, long value) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean stored = (boolean) ExpanseNative.MH_expanse_sync_map_get_or_insert.invokeExact(
                    handle, key, value, scratch);
            return stored ? OptionalLong.empty() : OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0));
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Replaces the value for {@code key} with {@code fn(current)}: a non-empty result stores
     * that value and an empty result removes the key. {@code current} is empty if the key is
     * absent.
     * <p>
     * This is a loop over {@link #compareExchange} written in Java, as
     * {@code SyncExpanseMap::update} is in Rust (the C ABI exports no callback form):
     * {@code fn} runs again with the observed word whenever a concurrent writer wins, so it
     * may run several times for one call and must be free of side effects. An exception
     * thrown by {@code fn} propagates and leaves the key unchanged.
     *
     * @param key 64-bit key
     * @param fn maps the current word to the word to store
     * @return the value that was replaced, or empty if the key was absent
     */
    public OptionalLong update(long key, UnaryOperator<OptionalLong> fn) {
        OptionalLong expected = get(key);
        while (true) {
            OptionalLong next = fn.apply(expected);
            Exchange x = compareExchange(key, expected, next);
            if (x.stored()) {
                return x.observed();
            }
            expected = x.observed();
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

    /**
     * Returns the off-heap bytes this map holds from the system allocator:
     * its tree's own share plus the blocks its epoch collector keeps for
     * reuse or is waiting to reclaim. Read with writers excluded, so writers
     * wait for it.
     *
     * @return bytes of native heap memory held
     */
    public long memHeld() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_sync_map_mem_held.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the freed blocks this map's epoch collector keeps for reuse
     * to the system allocator; {@link #memHeld()} falls by exactly the bytes
     * released. Runs beside readers and writers; blocks still in their grace
     * period stay held until a later call.
     *
     * @return bytes released
     */
    public long shrinkToFit() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_sync_map_shrink_to_fit.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    @Override
    public synchronized void close() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            return; // idempotent: a second close (or a close/close race) is a no-op.
        }
        // Free every live reader BEFORE the map: the native reader borrows the map,
        // so using or freeing a reader after the map is gone would be a use-after-free.
        for (Reader reader : liveReaders) {
            reader.invalidate();
        }
        liveReaders.clear();
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
