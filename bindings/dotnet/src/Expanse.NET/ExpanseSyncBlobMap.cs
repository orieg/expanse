using System;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// A blob map value: its payload bytes and its 24-bit hot metadata word. The record owns a copy of the
/// payload; no value is ever a view into the map.
/// </summary>
/// <param name="Payload">The payload bytes.</param>
/// <param name="HotMeta">The hot metadata word (24 bits).</param>
public sealed record ExpanseBlobValue(byte[] Payload, uint HotMeta);

/// <summary>
/// Thread-safe concurrent <c>ulong</c> to blob map: serialized writers and optimistic parallel readers, the
/// shared form of <see cref="ExpanseBlobMap"/>.
/// </summary>
/// <remarks>
/// <para><b>Values are copied, never borrowed.</b> Unlike <see cref="ExpanseBlobMap.TryGet(ulong, out ReadOnlySpan{byte}, out uint)"/>,
/// no call returns a span into the map: every read copies the payload into a managed <c>byte[]</c> within one
/// call, so nothing a caller holds is invalidated by a later write or compaction.</para>
/// <para>The wrapper owns the native caller-buffer protocol. A read whose payload is longer than its working
/// buffer is repeated with a buffer of the reported size until it fits. The repeat is a new read: if a writer
/// replaced or removed the value in between, the call returns the new state (a different payload, or absent).</para>
/// <para>Hot metadata is the 24-bit field of an arena-stored value. A payload of 7 bytes or fewer, or one the
/// map compresses into its slot word (only when the metadata is 0), is stored without it and reads back metadata
/// 0, whatever metadata the write passed.</para>
/// </remarks>
public sealed class ExpanseSyncBlobMap : IDisposable
{
    private const int StackBytes = 256;

    private SafeExpanseSyncBlobMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty shared blob map with the default 2 MiB arena chunk size.
    /// </summary>
    public ExpanseSyncBlobMap() : this(0)
    {
    }

    /// <summary>
    /// Creates a new empty shared blob map with a custom arena chunk size.
    /// </summary>
    /// <param name="chunkSize">Chunk size in bytes, or 0 for the default 2 MiB.</param>
    public ExpanseSyncBlobMap(nuint chunkSize)
    {
        _handle = NativeMethods.expanse_sync_blob_map_new(chunkSize);
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncBlobMap");
        }
    }

    /// <summary>
    /// Creates a new empty shared blob map whose arena capacity cap is <paramref name="maxCapacity"/> bytes of
    /// allocated chunks, clamped to [chunkSize, 64 GiB].
    /// </summary>
    /// <param name="chunkSize">Chunk size in bytes, or 0 for the default 2 MiB.</param>
    /// <param name="maxCapacity">Capacity cap in bytes, or 0 for the default 1 GiB.</param>
    public ExpanseSyncBlobMap(nuint chunkSize, nuint maxCapacity)
    {
        _handle = NativeMethods.expanse_sync_blob_map_new_with_capacity(chunkSize, maxCapacity);
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncBlobMap");
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    internal static ExpanseBlobStatus ToStatus(int rc) =>
        Enum.IsDefined(typeof(ExpanseBlobStatus), rc) ? (ExpanseBlobStatus)rc : ExpanseBlobStatus.Error;

    /// <summary>
    /// Inserts or replaces a key-blob pair, returning why a refused insert was refused instead of throwing:
    /// <see cref="ExpanseBlobStatus.MetaOverflow"/>, <see cref="ExpanseBlobStatus.AllocationFailed"/>,
    /// <see cref="ExpanseBlobStatus.CapRefused"/> or <see cref="ExpanseBlobStatus.ArenaFull"/>. A refused insert
    /// leaves the map unchanged.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="data">The payload to store.</param>
    /// <param name="hotMeta">24-bit hot metadata word.</param>
    public unsafe ExpanseBlobStatus TrySet(ulong key, ReadOnlySpan<byte> data, uint hotMeta = 0)
    {
        ThrowIfDisposed();
        fixed (byte* ptr = data)
        {
            return ToStatus(NativeMethods.expanse_sync_blob_map_insert(_handle, key, ptr, (nuint)data.Length, hotMeta));
        }
    }

    /// <summary>
    /// Inserts or replaces a key-blob pair.
    /// </summary>
    /// <exception cref="InvalidOperationException">The insert was refused; see <see cref="TrySet"/> for the status.</exception>
    public void Set(ulong key, ReadOnlySpan<byte> data, uint hotMeta = 0)
    {
        ExpanseBlobStatus status = TrySet(key, data, hotMeta);
        if (status != ExpanseBlobStatus.Ok)
        {
            throw new InvalidOperationException($"Failed to insert blob for key {key}: {status}.");
        }
    }

    /// <summary>
    /// Inserts or replaces a key-blob pair.
    /// </summary>
    /// <exception cref="InvalidOperationException">The insert was refused; see <see cref="TrySet"/> for the status.</exception>
    public void Set(ulong key, byte[] data, uint hotMeta = 0)
    {
        ArgumentNullException.ThrowIfNull(data);
        Set(key, data.AsSpan(), hotMeta);
    }

    /// <summary>
    /// Looks up a key, copying the payload into a new managed array.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="payload">A copy of the payload; empty when the key is absent.</param>
    /// <param name="hotMeta">The hot metadata word; 0 when the key is absent.</param>
    /// <returns><c>true</c> if the key was found.</returns>
    public unsafe bool TryGetBytes(ulong key, out byte[] payload, out uint hotMeta)
    {
        ThrowIfDisposed();
        var scratch = new ScratchBuffer(stackalloc byte[StackBytes]);
        try
        {
            while (true)
            {
                int rc;
                nuint len;
                uint meta;
                fixed (byte* pBuf = scratch.Span)
                {
                    rc = NativeMethods.expanse_sync_blob_map_get_into(
                        _handle, key, pBuf, (nuint)scratch.Span.Length, out len, out meta);
                }

                switch ((ExpanseBlobStatus)rc)
                {
                    case ExpanseBlobStatus.Ok:
                        payload = scratch.Span.Slice(0, (int)len).ToArray();
                        hotMeta = meta;
                        return true;
                    case ExpanseBlobStatus.NotFound:
                        payload = [];
                        hotMeta = 0;
                        return false;
                    case ExpanseBlobStatus.BufferTooSmall:
                        // `len` is the payload's length from the read that found the key. The next
                        // read is a new one and reports whatever the key holds by then.
                        scratch.Grow(len);
                        break;
                    default:
                        throw new InvalidOperationException($"expanse_sync_blob_map_get_into returned status {rc}.");
                }
            }
        }
        finally
        {
            scratch.Dispose();
        }
    }

    /// <summary>
    /// Looks up a key, copying the payload into a new managed array.
    /// </summary>
    public bool TryGetBytes(ulong key, out byte[] payload) => TryGetBytes(key, out payload, out _);

    /// <summary>
    /// Retrieves a copy of the payload for the key, or <c>null</c> if absent.
    /// </summary>
    public byte[]? GetBytes(ulong key) => TryGetBytes(key, out byte[] bytes, out _) ? bytes : null;

    /// <summary>
    /// Reads a key's hot metadata without copying its payload.
    /// </summary>
    /// <returns><c>true</c> if the key is present.</returns>
    public bool TryGetMeta(ulong key, out uint hotMeta)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_blob_map_get_meta(_handle, key, out hotMeta);
    }

    /// <summary>
    /// Removes a key from the map.
    /// </summary>
    /// <returns><c>true</c> if the key was present.</returns>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_blob_map_remove(_handle, key);
    }

    /// <summary>
    /// Checks whether the map contains the key.
    /// </summary>
    public bool Contains(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_blob_map_contains(_handle, key);
    }

    /// <summary>
    /// Checks whether the map contains the key.
    /// </summary>
    public bool ContainsKey(ulong key) => Contains(key);

    /// <summary>
    /// Atomic compare-and-swap on one key, as one linearizable step. A value is its payload bytes and its
    /// metadata, and the comparison covers both. <c>null</c> is "absent" on both sides: <c>(null, v)</c> stores
    /// only into an absent key and <c>(e, null)</c> removes the key if it holds <c>e</c>.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="expected">The value the key must hold, or <c>null</c> for "only if absent".</param>
    /// <param name="newValue">The value to store, or <c>null</c> to remove the key.</param>
    /// <param name="observed">On <see cref="ExpanseBlobStatus.Mismatch"/>, the current value (<c>null</c> if the
    /// key is absent), to retry against; otherwise <c>null</c>.</param>
    /// <returns>
    /// <see cref="ExpanseBlobStatus.Ok"/> when stored (or removed); <see cref="ExpanseBlobStatus.Mismatch"/> when
    /// nothing was stored; or a refusal (<see cref="ExpanseBlobStatus.MetaOverflow"/>,
    /// <see cref="ExpanseBlobStatus.AllocationFailed"/>, <see cref="ExpanseBlobStatus.CapRefused"/>,
    /// <see cref="ExpanseBlobStatus.ArenaFull"/>) when the comparison held but the new value cannot be stored.
    /// A refusal leaves the map unchanged. Because a value stored in its slot word reads back metadata 0,
    /// expecting the metadata it was inserted with reports <see cref="ExpanseBlobStatus.Mismatch"/> with metadata 0.
    /// </returns>
    public unsafe ExpanseBlobStatus CompareExchange(
        ulong key, ExpanseBlobValue? expected, ExpanseBlobValue? newValue, out ExpanseBlobValue? observed)
    {
        ThrowIfDisposed();
        byte[]? expectedData = expected?.Payload;
        byte[]? newData = newValue?.Payload;
        nuint expectedLen = (nuint)(expectedData?.Length ?? 0);
        nuint newLen = (nuint)(newData?.Length ?? 0);
        uint expectedMeta = expected?.HotMeta ?? 0;
        uint newMeta = newValue?.HotMeta ?? 0;

        var scratch = new ScratchBuffer(stackalloc byte[StackBytes]);
        try
        {
            while (true)
            {
                int rc;
                byte seenPresent;
                nuint len;
                uint meta;
                fixed (byte* pExpected = expectedData)
                fixed (byte* pNew = newData)
                fixed (byte* pBuf = scratch.Span)
                {
                    rc = NativeMethods.expanse_sync_blob_map_compare_exchange(
                        _handle, key,
                        expected is not null, pExpected, expectedLen, expectedMeta,
                        newValue is not null, pNew, newLen, newMeta,
                        out seenPresent, pBuf, (nuint)scratch.Span.Length, out len, out meta);
                }

                if (rc == (int)ExpanseBlobStatus.BufferTooSmall)
                {
                    // A mismatch whose current payload is longer than the buffer: nothing was stored.
                    // Repeat the exchange with room for it; the repeat is a new attempt.
                    scratch.Grow(len);
                    continue;
                }

                ExpanseBlobStatus status = ToStatus(rc);
                observed = status == ExpanseBlobStatus.Mismatch && seenPresent != 0
                    ? new ExpanseBlobValue(scratch.Span.Slice(0, (int)len).ToArray(), meta)
                    : null;
                return status;
            }
        }
        finally
        {
            scratch.Dispose();
        }
    }

    /// <summary>
    /// Atomic compare-and-swap on one key's 24-bit metadata alone, with no payload copy and no allocation:
    /// the form for a retry loop whose state lives in the metadata (a sequence number, a status tag).
    /// <c>null</c> is "absent" on both sides. The current metadata of a present key is its field, or 0 for a
    /// payload stored in its slot word.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="expected">The metadata the key must hold, or <c>null</c> for "only if absent".</param>
    /// <param name="newValue">The metadata to store, or <c>null</c> to remove the key.</param>
    /// <param name="observed">On <see cref="ExpanseBlobStatus.Mismatch"/>, the current metadata (<c>null</c> if the
    /// key is absent); otherwise <c>null</c>.</param>
    /// <returns>
    /// <see cref="ExpanseBlobStatus.Ok"/> when swapped (or removed); <see cref="ExpanseBlobStatus.Mismatch"/>;
    /// <see cref="ExpanseBlobStatus.MetaOverflow"/> when <paramref name="newValue"/> is wider than 24 bits; or
    /// <see cref="ExpanseBlobStatus.NoMetaField"/> when the comparison held but the value has no metadata field
    /// (the key is absent, or the new metadata is non-zero for a payload stored in its slot word).
    /// </returns>
    public ExpanseBlobStatus CompareExchangeMeta(ulong key, uint? expected, uint? newValue, out uint? observed)
    {
        ThrowIfDisposed();
        int rc = NativeMethods.expanse_sync_blob_map_compare_exchange_meta(
            _handle, key,
            expected.HasValue, expected.GetValueOrDefault(),
            newValue.HasValue, newValue.GetValueOrDefault(),
            out byte seenPresent, out uint seenMeta);
        ExpanseBlobStatus status = ToStatus(rc);
        observed = status == ExpanseBlobStatus.Mismatch && seenPresent != 0 ? (uint?)seenMeta : null;
        return status;
    }

    /// <summary>
    /// Stores the payload under <paramref name="key"/> only if the key is absent, as one linearizable step. Of
    /// any number of callers racing on one absent key, exactly one stores.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="data">The payload to store.</param>
    /// <param name="hotMeta">24-bit hot metadata word.</param>
    /// <param name="existing">On <see cref="ExpanseBlobStatus.Mismatch"/>, a copy of the value already present
    /// (left untouched); otherwise <c>null</c>.</param>
    /// <returns>
    /// <see cref="ExpanseBlobStatus.Ok"/> when stored; <see cref="ExpanseBlobStatus.Mismatch"/> when the key was
    /// present; or a refusal (as <see cref="TrySet"/>) when the key was absent but the value cannot be stored.
    /// </returns>
    public unsafe ExpanseBlobStatus GetOrInsert(ulong key, ReadOnlySpan<byte> data, uint hotMeta, out ExpanseBlobValue? existing)
    {
        ThrowIfDisposed();
        var scratch = new ScratchBuffer(stackalloc byte[StackBytes]);
        try
        {
            while (true)
            {
                int rc;
                nuint len;
                uint meta;
                fixed (byte* pData = data)
                fixed (byte* pBuf = scratch.Span)
                {
                    rc = NativeMethods.expanse_sync_blob_map_get_or_insert(
                        _handle, key, pData, (nuint)data.Length, hotMeta,
                        pBuf, (nuint)scratch.Span.Length, out len, out meta);
                }

                if (rc == (int)ExpanseBlobStatus.BufferTooSmall)
                {
                    // The key is present with a payload longer than the buffer: nothing was stored.
                    scratch.Grow(len);
                    continue;
                }

                ExpanseBlobStatus status = ToStatus(rc);
                existing = status == ExpanseBlobStatus.Mismatch
                    ? new ExpanseBlobValue(scratch.Span.Slice(0, (int)len).ToArray(), meta)
                    : null;
                return status;
            }
        }
        finally
        {
            scratch.Dispose();
        }
    }

    /// <summary>
    /// Gets the number of entries stored in the map.
    /// </summary>
    public ulong Count
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_blob_map_len(_handle);
        }
    }

    /// <summary>
    /// Gets the off-heap bytes used by the index and arena. Read with writers excluded; it waits for a running compaction.
    /// </summary>
    public nuint MemoryUsed
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_blob_map_mem_used(_handle);
        }
    }

    /// <summary>
    /// Gets an estimate of <see cref="MemoryUsed"/> that takes no lock and waits for nothing.
    /// </summary>
    public nuint MemoryUsedEstimate
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_blob_map_mem_used_estimate(_handle);
        }
    }

    /// <summary>
    /// Gets the off-heap bytes this map holds from the system allocator, including blocks its epoch collector
    /// keeps for reuse. Read with writers excluded; it waits for a running compaction.
    /// </summary>
    public nuint MemoryHeld
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_blob_map_mem_held(_handle);
        }
    }

    /// <summary>
    /// Returns the collector's freed index blocks to the system allocator and returns the bytes released.
    /// </summary>
    public nuint ShrinkToFit()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_blob_map_shrink_to_fit(_handle);
    }

    /// <summary>
    /// Turns the reclaim at the capacity cap on (the default) or off. Off, an insert the cap refuses compacts nothing.
    /// </summary>
    public void SetReclaimAtCap(bool on)
    {
        ThrowIfDisposed();
        NativeMethods.expanse_sync_blob_map_set_reclaim_at_cap(_handle, on);
    }

    /// <summary>
    /// Compacts the arena, freeing dead chunks. Readers keep reading while it copies.
    /// </summary>
    /// <returns><see cref="ExpanseBlobStatus.Ok"/>, or the refusal.</returns>
    public ExpanseBlobStatus Compact()
    {
        ThrowIfDisposed();
        return ToStatus(NativeMethods.expanse_sync_blob_map_compact(_handle));
    }

    /// <summary>
    /// Creates a lightweight reader bound to this map. Each querying thread should use its own reader, and
    /// a reader must be disposed before the map is disposed (disposing the map first defers the native free
    /// until the last reader is disposed).
    /// </summary>
    public ExpanseSyncBlobMapReader CreateReader()
    {
        ThrowIfDisposed();
        SafeExpanseSyncBlobMapReaderHandle readerHandle = NativeMethods.expanse_sync_blob_map_reader_new(_handle);
        if (readerHandle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncBlobMapReader");
        }
        // The native reader is a borrow of the map's storage: the reader holds a reference count on the
        // map's SafeHandle, so the map cannot be released while the reader is alive.
        return new ExpanseSyncBlobMapReader(readerHandle, _handle);
    }

    /// <summary>
    /// Frees the unmanaged memory allocated by this map once no reader still references it.
    /// </summary>
    public void Dispose()
    {
        if (!_disposed)
        {
            _handle.Dispose();
            _disposed = true;
        }
    }
}

/// <summary>
/// Reader for <see cref="ExpanseSyncBlobMap"/>; reads run beside the writer without locking it out. A reader
/// is used by one thread at a time; give each thread its own. Reads copy the payload, as the map's do.
/// </summary>
public sealed class ExpanseSyncBlobMapReader : IDisposable
{
    private const int StackBytes = 256;

    private SafeExpanseSyncBlobMapReaderHandle _handle;
    // The owning map's handle, ref-counted up for this reader's lifetime so the map
    // cannot be freed while the reader (a native borrow of it) is still alive.
    private readonly SafeExpanseSyncBlobMapHandle _mapHandle;
    private bool _mapHandleRefAdded;
    private bool _disposed;

    internal ExpanseSyncBlobMapReader(SafeExpanseSyncBlobMapReaderHandle handle, SafeExpanseSyncBlobMapHandle mapHandle)
    {
        _handle = handle;
        _mapHandle = mapHandle;
        _mapHandle.DangerousAddRef(ref _mapHandleRefAdded);
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Looks up a key without locking out the writer, copying the payload into a new managed array. A payload
    /// longer than the working buffer is read again with a buffer of the reported size; the repeat is a new read.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="payload">A copy of the payload; empty when the key is absent.</param>
    /// <param name="hotMeta">The hot metadata word; 0 when the key is absent.</param>
    /// <returns><c>true</c> if the key was found.</returns>
    public unsafe bool TryGetBytes(ulong key, out byte[] payload, out uint hotMeta)
    {
        ThrowIfDisposed();
        var scratch = new ScratchBuffer(stackalloc byte[StackBytes]);
        try
        {
            while (true)
            {
                int rc;
                nuint len;
                uint meta;
                fixed (byte* pBuf = scratch.Span)
                {
                    rc = NativeMethods.expanse_sync_blob_map_reader_get_into(
                        _handle, key, pBuf, (nuint)scratch.Span.Length, out len, out meta);
                }

                switch ((ExpanseBlobStatus)rc)
                {
                    case ExpanseBlobStatus.Ok:
                        payload = scratch.Span.Slice(0, (int)len).ToArray();
                        hotMeta = meta;
                        return true;
                    case ExpanseBlobStatus.NotFound:
                        payload = [];
                        hotMeta = 0;
                        return false;
                    case ExpanseBlobStatus.BufferTooSmall:
                        scratch.Grow(len);
                        break;
                    default:
                        throw new InvalidOperationException($"expanse_sync_blob_map_reader_get_into returned status {rc}.");
                }
            }
        }
        finally
        {
            scratch.Dispose();
        }
    }

    /// <summary>
    /// Looks up a key without locking out the writer, copying the payload into a new managed array.
    /// </summary>
    public bool TryGetBytes(ulong key, out byte[] payload) => TryGetBytes(key, out payload, out _);

    /// <summary>
    /// Retrieves a copy of the payload for the key, or <c>null</c> if absent.
    /// </summary>
    public byte[]? GetBytes(ulong key) => TryGetBytes(key, out byte[] bytes, out _) ? bytes : null;

    /// <summary>
    /// Reads a key's hot metadata without copying its payload.
    /// </summary>
    /// <returns><c>true</c> if the key is present.</returns>
    public bool TryGetMeta(ulong key, out uint hotMeta)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_blob_map_reader_get_meta(_handle, key, out hotMeta);
    }

    /// <summary>
    /// Checks whether the key is present without locking out the writer.
    /// </summary>
    public bool Contains(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_blob_map_reader_contains(_handle, key);
    }

    /// <summary>
    /// Checks whether the key is present without locking out the writer.
    /// </summary>
    public bool ContainsKey(ulong key) => Contains(key);

    /// <summary>
    /// Disposes this reader. The reader's native handle is released before its hold on the map.
    /// </summary>
    public void Dispose()
    {
        if (!_disposed)
        {
            _handle.Dispose();
            if (_mapHandleRefAdded)
            {
                _mapHandle.DangerousRelease();
                _mapHandleRefAdded = false;
            }
            _disposed = true;
        }
    }
}
