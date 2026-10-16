package io.github.orieg.expanse;

import static io.github.orieg.expanse.internal.ExpanseNative.*;

import java.lang.foreign.MemorySegment;
import java.util.Objects;

/**
 * Multithreaded concurrent unordered map from byte-string keys to 64-bit values, with optimistic
 * readers (OCC) beside a serialized writer: the shared form of {@link ExpanseBytesMap}.
 * <p>
 * Keys are opaque byte arrays: embedded zero bytes are ordinary and the empty array is a valid
 * key. The map is not ordered; use {@link SyncExpanseOrderedBytesMap} for ordered reads. Key
 * arrays are copied on the way in, so a caller may reuse them.
 */
public final class SyncExpanseBytesMap extends SyncKeyedMap<byte[]> {

    /**
     * A registered reader handle: optimistic reads that never take the writer's lock. Used by
     * one thread at a time and closed before its map (closing the map closes it too).
     */
    public final class Reader extends SyncKeyedMap.BaseReader<byte[]> {
        private Reader(MemorySegment readerHandle) {
            super(SyncExpanseBytesMap.this, readerHandle);
        }
    }

    /** Creates a new empty concurrent bytes map. */
    public SyncExpanseBytesMap() {
        super(new Handles(
                MH_expanse_sync_bytesmap_free,
                MH_expanse_sync_bytesmap_insert,
                MH_expanse_sync_bytesmap_get,
                MH_expanse_sync_bytesmap_contains,
                MH_expanse_sync_bytesmap_remove,
                MH_expanse_sync_bytesmap_len,
                MH_expanse_sync_bytesmap_mem_used,
                MH_expanse_sync_bytesmap_mem_held,
                MH_expanse_sync_bytesmap_shrink_to_fit,
                MH_expanse_sync_bytesmap_compare_exchange,
                MH_expanse_sync_bytesmap_get_or_insert,
                MH_expanse_sync_bytesmap_reader_new,
                MH_expanse_sync_bytesmap_reader_free,
                MH_expanse_sync_bytesmap_reader_get,
                MH_expanse_sync_bytesmap_reader_contains),
                MH_expanse_sync_bytesmap_new, "expanse_sync_bytesmap_t", false);
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
}
