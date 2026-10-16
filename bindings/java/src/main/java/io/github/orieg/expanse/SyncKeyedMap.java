package io.github.orieg.expanse;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.Set;
import java.util.function.Function;
import java.util.function.UnaryOperator;

/**
 * Shared implementation of the three concurrent {@code key -> long} maps over the C handles
 * {@code expanse_sync_strmap_t}, {@code expanse_sync_bytesmap_t} and
 * {@code expanse_sync_ordered_bytesmap_t}. The public classes
 * ({@link SyncExpanseStrMap}, {@link SyncExpanseBytesMap}, {@link SyncExpanseOrderedBytesMap})
 * fix the key type and add what is specific to their map.
 * <p>
 * Protocol, as for {@link SyncExpanseMap}: the map is safe from any number of threads at once
 * except {@link #close()}; a {@code Reader} is used by one thread at a time and is closed before
 * its map (closing the map closes every reader still open). A native call must not be in
 * progress on a map or reader being closed.
 *
 * @param <K> the key type
 */
abstract class SyncKeyedMap<K> implements AutoCloseable {

    /** The native entry points of one map kind, all in the (map, key, length, ...) shape. */
    record Handles(
            MethodHandle free,
            MethodHandle insert,
            MethodHandle get,
            MethodHandle contains,
            MethodHandle remove,
            MethodHandle len,
            MethodHandle memUsed,
            MethodHandle memHeld,
            MethodHandle shrinkToFit,
            MethodHandle compareExchange,
            MethodHandle getOrInsert,
            MethodHandle readerNew,
            MethodHandle readerFree,
            MethodHandle readerGet,
            MethodHandle readerContains) {

        /**
         * Adapts a set of entry points that take a NUL-terminated key (no length) to the
         * (key, length) shape; the added length argument is ignored.
         */
        Handles ignoringLengths() {
            return new Handles(free, skip(insert), skip(get), skip(contains), skip(remove), len, memUsed, memHeld,
                    shrinkToFit, skip(compareExchange), skip(getOrInsert), readerNew, readerFree, skip(readerGet),
                    skip(readerContains));
        }
    }

    /** Inserts an ignored {@code long} length after the (target, key) arguments. */
    static MethodHandle skip(MethodHandle mh) {
        return MethodHandles.dropArguments(mh, 2, long.class);
    }

    /** Inserts ignored (key, length) arguments after the target, for first/last navigation. */
    static MethodHandle noKey(MethodHandle mh) {
        return MethodHandles.dropArguments(mh, 1, MemorySegment.class, long.class);
    }

    static final int NAV_OK = 0;
    static final int NAV_NOT_FOUND = 1;
    static final int NAV_BUFFER_TOO_SMALL = 2;

    private static final int NAV_INITIAL_BUF = 64;

    private final Handles h;
    private final boolean cstr;
    private MemorySegment handle;
    // volatile so a concurrent thread observes closure promptly and refuses.
    private volatile boolean closed = false;

    // Live readers borrow the map's native storage, so a reader outliving the map is a
    // use-after-free. Every live reader is freed, before the map itself, when the map closes.
    // Guarded by synchronization on `this`.
    private final Set<BaseReader<K>> liveReaders = Collections.newSetFromMap(new IdentityHashMap<>());

    /**
     * @param h the native entry points
     * @param newMap the constructor entry point, {@code () -> handle}
     * @param what the map's name, for error messages
     * @param cstr true if keys are passed as NUL-terminated strings
     */
    SyncKeyedMap(Handles h, MethodHandle newMap, String what, boolean cstr) {
        this.h = h;
        this.cstr = cstr;
        try {
            this.handle = (MemorySegment) newMap.invokeExact();
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating " + what, t);
        }
        if (handle.equals(MemorySegment.NULL)) {
            throw new OutOfMemoryError("Failed to allocate native " + what);
        }
    }

    /** The key's bytes, without any terminator. Rejects keys the native map cannot hold. */
    abstract byte[] keyBytes(K key);

    final MemorySegment keySegment(byte[] kb, Arena arena) {
        if (cstr) {
            MemorySegment seg = arena.allocate(kb.length + 1L);
            MemorySegment.copy(kb, 0, seg, ValueLayout.JAVA_BYTE, 0, kb.length);
            return seg;
        }
        if (kb.length == 0) {
            return MemorySegment.NULL;
        }
        MemorySegment seg = arena.allocate(kb.length);
        MemorySegment.copy(kb, 0, seg, ValueLayout.JAVA_BYTE, 0, kb.length);
        return seg;
    }

    static RuntimeException rethrow(Throwable t) {
        if (t instanceof RuntimeException e) {
            return e;
        }
        if (t instanceof Error e) {
            throw e;
        }
        return new RuntimeException(t);
    }

    final void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException(getClass().getSimpleName() + " has been closed");
        }
    }

    /** The native map handle, for the subclasses' navigation calls. */
    final MemorySegment nativeHandle() {
        checkOpen();
        return handle;
    }

    /**
     * A registered reader handle. Used by one thread at a time; closed before its map.
     *
     * @param <K> the key type
     */
    abstract static class BaseReader<K> implements AutoCloseable {
        private final SyncKeyedMap<K> owner;
        private MemorySegment readerHandle;
        // volatile: set from the map's close() (another thread) as well as this reader's close().
        private volatile boolean readerClosed = false;

        BaseReader(SyncKeyedMap<K> owner, MemorySegment readerHandle) {
            this.owner = owner;
            this.readerHandle = readerHandle;
        }

        /** Frees the native reader. Called holding the lock on the owning map. Idempotent. */
        final void invalidate() {
            if (!readerClosed && !readerHandle.equals(MemorySegment.NULL)) {
                try {
                    owner.h.readerFree().invokeExact(readerHandle);
                } catch (Throwable t) {
                    throw rethrow(t);
                } finally {
                    readerHandle = MemorySegment.NULL;
                    readerClosed = true;
                }
            }
        }

        /** The native reader handle, after checking that neither the reader nor its map is closed. */
        final MemorySegment live() {
            if (readerClosed || readerHandle.equals(MemorySegment.NULL)) {
                throw new IllegalStateException("Reader has been closed");
            }
            owner.checkOpen();
            return readerHandle;
        }

        final SyncKeyedMap<K> owner() {
            return owner;
        }

        /**
         * Optimistic lookup.
         *
         * @param key search key
         * @return the value, or empty if the key is absent
         */
        public OptionalLong get(K key) {
            MemorySegment r = live();
            byte[] kb = owner.keyBytes(key);
            try (Arena a = Arena.ofConfined()) {
                MemorySegment out = a.allocate(ValueLayout.JAVA_LONG);
                boolean found = (boolean) owner.h.readerGet().invokeExact(
                        r, owner.keySegment(kb, a), (long) kb.length, out);
                return found ? OptionalLong.of(out.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
            } catch (Throwable t) {
                throw rethrow(t);
            }
        }

        /**
         * Checks if a key is present.
         *
         * @param key search key
         * @return true if present
         */
        public boolean containsKey(K key) {
            MemorySegment r = live();
            byte[] kb = owner.keyBytes(key);
            try (Arena a = Arena.ofConfined()) {
                return (boolean) owner.h.readerContains().invokeExact(
                        r, owner.keySegment(kb, a), (long) kb.length);
            } catch (Throwable t) {
                throw rethrow(t);
            }
        }

        /** Frees the reader. Idempotent; must precede the map's own close. */
        @Override
        public void close() {
            synchronized (owner) {
                owner.liveReaders.remove(this);
                invalidate();
            }
        }
    }

    /** Registers a new reader built by {@code make} from the native reader handle. */
    final synchronized <R extends BaseReader<K>> R register(Function<MemorySegment, R> make) {
        checkOpen();
        try {
            MemorySegment r = (MemorySegment) h.readerNew().invokeExact(handle);
            if (r.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating reader");
            }
            R reader = make.apply(r);
            liveReaders.add(reader);
            return reader;
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Inserts or updates a key-value mapping.
     *
     * @param key the key
     * @param value 64-bit value
     * @return true if the key was newly inserted
     */
    public boolean insert(K key, long value) {
        checkOpen();
        byte[] kb = keyBytes(key);
        try (Arena a = Arena.ofConfined()) {
            return (boolean) h.insert().invokeExact(
                    handle, keySegment(kb, a), (long) kb.length, value, MemorySegment.NULL);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Inserts or updates a key-value mapping.
     *
     * @param key the key
     * @param value 64-bit value
     * @return true if the key was newly inserted
     */
    public boolean put(K key, long value) {
        return insert(key, value);
    }

    /**
     * Inserts or updates a key-value mapping, returning the replaced value.
     *
     * @param key the key
     * @param value 64-bit value
     * @return the previous value, or empty if the key was new
     */
    public OptionalLong putAndGetOld(K key, long value) {
        checkOpen();
        byte[] kb = keyBytes(key);
        try (Arena a = Arena.ofConfined()) {
            MemorySegment out = a.allocate(ValueLayout.JAVA_LONG);
            boolean isNew = (boolean) h.insert().invokeExact(
                    handle, keySegment(kb, a), (long) kb.length, value, out);
            return isNew ? OptionalLong.empty() : OptionalLong.of(out.get(ValueLayout.JAVA_LONG, 0));
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * One-shot lookup (for hot loops prefer a reader).
     *
     * @param key search key
     * @return the value, or empty if the key is absent
     */
    public OptionalLong get(K key) {
        checkOpen();
        byte[] kb = keyBytes(key);
        try (Arena a = Arena.ofConfined()) {
            MemorySegment out = a.allocate(ValueLayout.JAVA_LONG);
            boolean found = (boolean) h.get().invokeExact(handle, keySegment(kb, a), (long) kb.length, out);
            return found ? OptionalLong.of(out.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Checks if a key is present.
     *
     * @param key search key
     * @return true if present
     */
    public boolean containsKey(K key) {
        checkOpen();
        byte[] kb = keyBytes(key);
        try (Arena a = Arena.ofConfined()) {
            return (boolean) h.contains().invokeExact(handle, keySegment(kb, a), (long) kb.length);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Removes the mapping for {@code key}.
     *
     * @param key the key
     * @return true if the key was present and removed
     */
    public boolean remove(K key) {
        checkOpen();
        byte[] kb = keyBytes(key);
        try (Arena a = Arena.ofConfined()) {
            return (boolean) h.remove().invokeExact(
                    handle, keySegment(kb, a), (long) kb.length, MemorySegment.NULL);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Atomic compare-and-swap on one key, as one linearizable step: stores {@code newValue}
     * only if the key's current word is {@code expected}. An empty {@link OptionalLong} is
     * "absent" on both sides, so every {@code long} stays a legal value. The result reports the
     * word the compare saw on both outcomes.
     *
     * @param key the key
     * @param expected the word the key must currently hold, or empty for "only if absent"
     * @param newValue the word to store, or empty to remove the key
     * @return whether the store happened and the word the compare saw
     */
    public SyncExpanseMap.Exchange compareExchange(K key, OptionalLong expected, OptionalLong newValue) {
        checkOpen();
        byte[] kb = keyBytes(key);
        try (Arena a = Arena.ofConfined()) {
            MemorySegment out = a.allocate(16, 8); // observed word at 0, observed-present flag at 8
            boolean stored = (boolean) h.compareExchange().invokeExact(
                    handle, keySegment(kb, a), (long) kb.length,
                    expected.isPresent(), expected.orElse(0L),
                    newValue.isPresent(), newValue.orElse(0L),
                    out.asSlice(8, 1), out);
            boolean seenPresent = out.get(ValueLayout.JAVA_BYTE, 8) != 0;
            return new SyncExpanseMap.Exchange(stored,
                    seenPresent ? OptionalLong.of(out.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty());
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Stores {@code value} under {@code key} only if the key is absent, as one linearizable
     * step. Of any number of callers racing on one absent key, exactly one stores.
     *
     * @param key the key
     * @param value 64-bit value
     * @return the value already present (left untouched), or empty when {@code value} was stored
     */
    public OptionalLong getOrInsert(K key, long value) {
        checkOpen();
        byte[] kb = keyBytes(key);
        try (Arena a = Arena.ofConfined()) {
            MemorySegment out = a.allocate(ValueLayout.JAVA_LONG);
            boolean stored = (boolean) h.getOrInsert().invokeExact(
                    handle, keySegment(kb, a), (long) kb.length, value, out);
            return stored ? OptionalLong.empty() : OptionalLong.of(out.get(ValueLayout.JAVA_LONG, 0));
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Replaces the value for {@code key} with {@code fn(current)}: a non-empty result stores
     * that value and an empty result removes the key. {@code current} is empty if the key is
     * absent. A loop over {@link #compareExchange} written in Java: {@code fn} runs again with
     * the observed word whenever a concurrent writer wins, so it may run several times for one
     * call and must be free of side effects.
     *
     * @param key the key
     * @param fn maps the current word to the word to store
     * @return the value that was replaced, or empty if the key was absent
     */
    public OptionalLong update(K key, UnaryOperator<OptionalLong> fn) {
        OptionalLong expected = get(key);
        while (true) {
            OptionalLong next = fn.apply(expected);
            SyncExpanseMap.Exchange x = compareExchange(key, expected, next);
            if (x.stored()) {
                return x.observed();
            }
            expected = x.observed();
        }
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
            return (long) h.len().invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Returns the off-heap bytes the map's nodes occupy. Read with writers excluded, so
     * writers wait for it.
     *
     * @return bytes of native heap memory in use
     */
    public long memUsed() {
        checkOpen();
        try {
            return (long) h.memUsed().invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Returns the off-heap bytes this map holds from the system allocator: its tree's own share
     * plus the blocks its epoch collector keeps for reuse or is waiting to reclaim. Read with
     * writers excluded, so writers wait for it.
     *
     * @return bytes of native heap memory held
     */
    public long memHeld() {
        checkOpen();
        try {
            return (long) h.memHeld().invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /**
     * Returns the freed blocks this map's epoch collector keeps for reuse to the system
     * allocator. Runs beside readers and writers; blocks still in their grace period stay held
     * until a later call.
     *
     * @return bytes released
     */
    public long shrinkToFit() {
        checkOpen();
        try {
            return (long) h.shrinkToFit().invokeExact(handle);
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    /** One entry found by a navigation call: the key's bytes and its value. */
    record NavHit(byte[] key, long value) {}

    /**
     * Runs one ordered-read call, growing the key buffer until the key fits. Each retry after a
     * too-small buffer is a new read, so it may return a different key if a writer ran between.
     *
     * @param mh the navigation entry point, in the (target, key, length, key_out, buf_len,
     *           required_len, value_out) shape
     * @param target the native map or reader
     * @param searchKey the search key's bytes, or null for first/last
     * @param nulTerminated true if keys are NUL-terminated strings (the buffer then ends in a NUL
     *                      that is not part of the returned key)
     */
    static Optional<NavHit> nav(MethodHandle mh, MemorySegment target, byte[] searchKey, boolean nulTerminated) {
        int bufLen = NAV_INITIAL_BUF;
        while (true) {
            try (Arena a = Arena.ofConfined()) {
                MemorySegment key = MemorySegment.NULL;
                long keyLen = 0;
                if (searchKey != null) {
                    keyLen = searchKey.length;
                    key = a.allocate(searchKey.length + 1L); // a trailing NUL serves the string form
                    MemorySegment.copy(searchKey, 0, key, ValueLayout.JAVA_BYTE, 0, searchKey.length);
                }
                MemorySegment buf = a.allocate(bufLen);
                MemorySegment out = a.allocate(16, 8); // required length at 0, value at 8
                int status = (int) mh.invokeExact(
                        target, key, keyLen, buf, (long) bufLen, out, out.asSlice(8, 8));
                switch (status) {
                    case NAV_OK -> {
                        long n = out.get(ValueLayout.JAVA_LONG, 0) - (nulTerminated ? 1 : 0);
                        byte[] k = buf.asSlice(0, n).toArray(ValueLayout.JAVA_BYTE);
                        return Optional.of(new NavHit(k, out.get(ValueLayout.JAVA_LONG, 8)));
                    }
                    case NAV_NOT_FOUND -> {
                        return Optional.empty();
                    }
                    case NAV_BUFFER_TOO_SMALL -> {
                        long required = out.get(ValueLayout.JAVA_LONG, 0);
                        if (required > Integer.MAX_VALUE - 8) {
                            throw new IllegalStateException("Key of " + Long.toUnsignedString(required)
                                    + " bytes exceeds the maximum Java buffer size");
                        }
                        bufLen = (int) Math.max(required, 1);
                    }
                    default -> throw new IllegalStateException("Unknown navigation status: " + status);
                }
            } catch (Throwable t) {
                throw rethrow(t);
            }
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
        // Free every live reader BEFORE the map: the native reader borrows the map.
        for (BaseReader<K> reader : liveReaders) {
            reader.invalidate();
        }
        liveReaders.clear();
        try {
            h.free().invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException("Failed to free " + getClass().getSimpleName(), t);
        } finally {
            handle = MemorySegment.NULL;
            closed = true;
        }
    }
}
