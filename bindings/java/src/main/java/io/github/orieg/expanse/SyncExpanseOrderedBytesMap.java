package io.github.orieg.expanse;

import static io.github.orieg.expanse.internal.ExpanseNative.*;

import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandle;
import java.util.Objects;
import java.util.Optional;

/**
 * Multithreaded concurrent ordered map from byte-string keys to 64-bit values, with optimistic
 * point readers (OCC) beside a serialized writer: the shared form of
 * {@link ExpanseOrderedBytesMap}. Keys are opaque byte arrays ordered lexicographically as
 * unsigned bytes; embedded zero bytes are ordinary and the empty array is a valid key.
 * <p>
 * <b>Ordered reads block writers.</b> {@link #firstEntry()}, {@link #lastEntry()},
 * {@link #ceilingEntry}, {@link #higherEntry}, {@link #floorEntry} and {@link #lowerEntry} are
 * calls on the map, not on a {@link Reader}: each runs with every writer excluded (it quiesces
 * writers and takes the writer mutex), so the entry returned is the answer at one instant and
 * every write waits for the call. They are not calls for a hot loop beside writers. A walk built
 * from repeated {@code higherEntry} calls is not a snapshot: it sees every key present
 * throughout it, in order, and may or may not see keys inserted or removed between calls.
 * Point reads through a {@link Reader} stay optimistic.
 */
public final class SyncExpanseOrderedBytesMap extends SyncKeyedMap<byte[]> {

    /**
     * One entry of the map.
     *
     * @param key the key bytes
     * @param value its value
     */
    public record Entry(byte[] key, long value) {}

    private static final MethodHandle FIRST = noKey(MH_expanse_sync_ordered_bytesmap_first);
    private static final MethodHandle LAST = noKey(MH_expanse_sync_ordered_bytesmap_last);

    /**
     * A registered reader handle: optimistic point reads that never take the writer's lock. Used
     * by one thread at a time and closed before its map (closing the map closes it too).
     */
    public final class Reader extends SyncKeyedMap.BaseReader<byte[]> {
        private Reader(MemorySegment readerHandle) {
            super(SyncExpanseOrderedBytesMap.this, readerHandle);
        }
    }

    /** Creates a new empty concurrent ordered-bytes map. */
    public SyncExpanseOrderedBytesMap() {
        super(new Handles(
                MH_expanse_sync_ordered_bytesmap_free,
                MH_expanse_sync_ordered_bytesmap_insert,
                MH_expanse_sync_ordered_bytesmap_get,
                MH_expanse_sync_ordered_bytesmap_contains,
                MH_expanse_sync_ordered_bytesmap_remove,
                MH_expanse_sync_ordered_bytesmap_len,
                MH_expanse_sync_ordered_bytesmap_mem_used,
                MH_expanse_sync_ordered_bytesmap_mem_held,
                MH_expanse_sync_ordered_bytesmap_shrink_to_fit,
                MH_expanse_sync_ordered_bytesmap_compare_exchange,
                MH_expanse_sync_ordered_bytesmap_get_or_insert,
                MH_expanse_sync_ordered_bytesmap_reader_new,
                MH_expanse_sync_ordered_bytesmap_reader_free,
                MH_expanse_sync_ordered_bytesmap_reader_get,
                MH_expanse_sync_ordered_bytesmap_reader_contains),
                MH_expanse_sync_ordered_bytesmap_new, "expanse_sync_ordered_bytesmap_t", false);
    }

    @Override
    byte[] keyBytes(byte[] key) {
        return Objects.requireNonNull(key, "key must not be null");
    }

    /**
     * Registers and returns a new {@link Reader}. Close it before the map, or let the map's
     * {@link #close()} close it.
     *
     * @return new Reader instance
     */
    public Reader reader() {
        return register(Reader::new);
    }

    private Optional<Entry> entry(MethodHandle mh, byte[] key) {
        MemorySegment m = nativeHandle();
        return nav(mh, m, key, false).map(h -> new Entry(h.key(), h.value()));
    }

    /**
     * The smallest entry. Blocks writers for the duration of the call.
     *
     * @return the entry, or empty if the map is empty
     */
    public Optional<Entry> firstEntry() {
        return entry(FIRST, null);
    }

    /**
     * The largest entry. Blocks writers for the duration of the call.
     *
     * @return the entry, or empty if the map is empty
     */
    public Optional<Entry> lastEntry() {
        return entry(LAST, null);
    }

    /**
     * The entry with the smallest key at or after {@code key}. Blocks writers for the duration
     * of the call.
     *
     * @param key search key
     * @return the entry, or empty if there is none
     */
    public Optional<Entry> ceilingEntry(byte[] key) {
        return entry(MH_expanse_sync_ordered_bytesmap_next_at_or_after,
                Objects.requireNonNull(key, "key must not be null"));
    }

    /**
     * The entry with the smallest key strictly after {@code key}. Blocks writers for the
     * duration of the call.
     *
     * @param key search key
     * @return the entry, or empty if there is none
     */
    public Optional<Entry> higherEntry(byte[] key) {
        return entry(MH_expanse_sync_ordered_bytesmap_next_after,
                Objects.requireNonNull(key, "key must not be null"));
    }

    /**
     * The entry with the largest key at or before {@code key}. Blocks writers for the duration
     * of the call.
     *
     * @param key search key
     * @return the entry, or empty if there is none
     */
    public Optional<Entry> floorEntry(byte[] key) {
        return entry(MH_expanse_sync_ordered_bytesmap_prev_at_or_before,
                Objects.requireNonNull(key, "key must not be null"));
    }

    /**
     * The entry with the largest key strictly before {@code key}; empty for the empty key.
     * Blocks writers for the duration of the call.
     *
     * @param key search key
     * @return the entry, or empty if there is none
     */
    public Optional<Entry> lowerEntry(byte[] key) {
        return entry(MH_expanse_sync_ordered_bytesmap_prev_before,
                Objects.requireNonNull(key, "key must not be null"));
    }
}
