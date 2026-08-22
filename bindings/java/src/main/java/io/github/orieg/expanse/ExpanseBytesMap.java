package io.github.orieg.expanse;

import io.github.orieg.expanse.collections.ExpanseJavaBytesMap;
import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.ByteBuffer;
import java.util.Objects;
import java.util.OptionalLong;

/**
 * High-performance off-heap unordered map of arbitrary byte slices to 64-bit values (cf. JudyHS).
 * <p>
 * Supports raw {@code byte[]}, direct {@link ByteBuffer}, and Panama {@link MemorySegment} keys
 * (including embedded null bytes).
 */
public final class ExpanseBytesMap implements AutoCloseable {

    private static final ThreadLocal<MemorySegment> SCRATCH =
            ThreadLocal.withInitial(() -> Arena.ofAuto().allocate(ValueLayout.JAVA_LONG, 2));

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * Creates a new empty off-heap {@link ExpanseBytesMap}.
     */
    public ExpanseBytesMap() {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_bytesmap_new.invokeExact();
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native expanse_bytesmap_t");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating ExpanseBytesMap", t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("ExpanseBytesMap has been closed");
        }
    }

    /**
     * Inserts or updates a byte array key-to-value mapping.
     *
     * @param key byte array key
     * @param value 64-bit value
     * @return true if key was newly inserted, false if replaced
     */
    public boolean put(byte[] key, long value) {
        return insert(key, value);
    }

    /**
     * Inserts or updates a byte array key-to-value mapping.
     *
     * @param key byte array key
     * @param value 64-bit value
     * @return true if key was newly inserted, false if replaced
     */
    public boolean insert(byte[] key, long value) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        if (key.length == 0) {
            return insert(MemorySegment.NULL, 0, value);
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(ValueLayout.JAVA_BYTE, key.length);
            MemorySegment.copy(key, 0, seg, ValueLayout.JAVA_BYTE, 0, key.length);
            return insert(seg, key.length, value);
        }
    }

    /**
     * Inserts or updates a {@link MemorySegment} key-to-value mapping.
     *
     * @param keySegment raw memory segment containing key bytes
     * @param len byte length
     * @param value 64-bit value
     * @return true if key was newly inserted
     */
    public boolean insert(MemorySegment keySegment, long len, long value) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_bytesmap_insert.invokeExact(
                    handle, keySegment, len, value, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Retrieves the value for a byte array key.
     *
     * @param key byte array
     * @return OptionalLong containing value if present
     */
    public OptionalLong get(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        if (key.length == 0) {
            return get(MemorySegment.NULL, 0);
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(ValueLayout.JAVA_BYTE, key.length);
            MemorySegment.copy(key, 0, seg, ValueLayout.JAVA_BYTE, 0, key.length);
            return get(seg, key.length);
        }
    }

    /**
     * Retrieves the value for a {@link MemorySegment} key.
     *
     * @param keySegment raw memory segment
     * @param len byte length
     * @return OptionalLong containing value if present
     */
    public OptionalLong get(MemorySegment keySegment, long len) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_bytesmap_get.invokeExact(
                    handle, keySegment, len, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks if the map contains the given byte array key.
     *
     * @param key byte array
     * @return true if present
     */
    public boolean containsKey(byte[] key) {
        return get(key).isPresent();
    }

    /**
     * Removes the byte array key from the map.
     *
     * @param key byte array
     * @return true if key was present and removed
     */
    public boolean remove(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        if (key.length == 0) {
            return remove(MemorySegment.NULL, 0);
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(ValueLayout.JAVA_BYTE, key.length);
            MemorySegment.copy(key, 0, seg, ValueLayout.JAVA_BYTE, 0, key.length);
            return remove(seg, key.length);
        }
    }

    /**
     * Removes the {@link MemorySegment} key from the map.
     *
     * @param keySegment memory segment
     * @param len byte length
     * @return true if key was present and removed
     */
    public boolean remove(MemorySegment keySegment, long len) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_bytesmap_remove.invokeExact(
                    handle, keySegment, len, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns a direct writable {@link MemorySegment} (8 bytes) to the value slot of {@code key},
     * or {@code null} if absent.
     *
     * @param key byte array
     * @return slot segment or null
     */
    public MemorySegment slot(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        if (key.length == 0) {
            return slot(MemorySegment.NULL, 0);
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(ValueLayout.JAVA_BYTE, key.length);
            MemorySegment.copy(key, 0, seg, ValueLayout.JAVA_BYTE, 0, key.length);
            return slot(seg, key.length);
        }
    }

    /**
     * Returns a direct writable {@link MemorySegment} (8 bytes) to the value slot.
     *
     * @param keySegment memory segment
     * @param len byte length
     * @return slot segment or null
     */
    public MemorySegment slot(MemorySegment keySegment, long len) {
        checkOpen();
        try {
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_bytesmap_slot.invokeExact(
                    handle, keySegment, len);
            return ptr.equals(MemorySegment.NULL) ? null : ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize());
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts the byte array key with value 0 if absent and returns a direct writable value slot.
     *
     * @param key byte array
     * @return direct slot segment
     */
    public MemorySegment insertSlot(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        if (key.length == 0) {
            return insertSlot(MemorySegment.NULL, 0);
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(ValueLayout.JAVA_BYTE, key.length);
            MemorySegment.copy(key, 0, seg, ValueLayout.JAVA_BYTE, 0, key.length);
            return insertSlot(seg, key.length);
        }
    }

    /**
     * Inserts the {@link MemorySegment} key with value 0 if absent and returns a direct writable value slot.
     *
     * @param keySegment memory segment
     * @param len byte length
     * @return direct slot segment
     */
    public MemorySegment insertSlot(MemorySegment keySegment, long len) {
        checkOpen();
        try {
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_bytesmap_ins_slot.invokeExact(
                    handle, keySegment, len);
            if (ptr.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating slot");
            }
            return ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize());
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Number of entries in this map.
     *
     * @return entry count
     */
    public long size() {
        return len();
    }

    /**
     * Number of entries in this map.
     *
     * @return entry count
     */
    public long len() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_bytesmap_len.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks whether the map is empty.
     *
     * @return true if size == 0
     */
    public boolean isEmpty() {
        return size() == 0;
    }

    /**
     * Returns exact native heap memory in bytes used by this map.
     *
     * @return byte count
     */
    public long memoryUsed() {
        return memUsed();
    }

    /**
     * Returns exact native heap memory in bytes used by this map.
     *
     * @return byte count
     */
    public long memUsed() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_bytesmap_mem_used.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes all entries from this map.
     */
    public void clear() {
        checkOpen();
        try {
            ExpanseNative.MH_expanse_bytesmap_clear.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns a standard {@link java.util.Map} view backed by this byte map.
     *
     * @return Map wrapper
     */
    public java.util.Map<byte[], Long> asJavaMap() {
        return new ExpanseJavaBytesMap(this);
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_bytesmap_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException("Failed to free ExpanseBytesMap", t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
            }
        }
    }

    @Override
    public String toString() {
        return "ExpanseBytesMap{size=" + (closed ? "closed" : size()) + ", memUsed=" + (closed ? 0 : memoryUsed()) + "B}";
    }
}
