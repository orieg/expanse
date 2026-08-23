using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// High-performance off-heap map from 64-bit integer keys to arbitrary-length byte payloads
/// backed by inline polymorphic 64-bit value slots and chunked slab arenas.
/// </summary>
public sealed class ExpanseBlobMap : IDisposable
{
    private SafeExpanseBlobMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty <see cref="ExpanseBlobMap"/> with default 2 MiB slab chunks.
    /// </summary>
    public ExpanseBlobMap() : this(0) { }

    /// <summary>
    /// Creates a new empty <see cref="ExpanseBlobMap"/> with custom slab chunk capacity in bytes.
    /// </summary>
    /// <param name="chunkSize">Chunk size in bytes (0 for default 2 MiB).</param>
    public ExpanseBlobMap(nuint chunkSize)
    {
        _handle = NativeMethods.expanse_blob_map_new(chunkSize);
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryError("Failed to allocate native ExpanseBlobMap");
        }
    }

    /// <summary>
    /// Gets the underlying native <see cref="SafeExpanseBlobMapHandle"/>.
    /// </summary>
    public SafeExpanseBlobMapHandle Handle
    {
        get
        {
            ThrowIfDisposed();
            return _handle;
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Inserts or updates a key-blob mapping with optional 32-bit hot metadata.
    /// </summary>
    /// <param name="key">The 64-bit unsigned key.</param>
    /// <param name="payload">The byte payload to store.</param>
    /// <param name="hotMeta">32-bit hot metadata stored directly in index.</param>
    public unsafe void Set(ulong key, ReadOnlySpan<byte> payload, uint hotMeta = 0)
    {
        ThrowIfDisposed();
        if (payload.Length == 0)
        {
            NativeMethods.expanse_blob_map_insert(_handle, key, null, 0, hotMeta);
            return;
        }
        fixed (byte* pData = payload)
        {
            NativeMethods.expanse_blob_map_insert(_handle, key, pData, (nuint)payload.Length, hotMeta);
        }
    }

    /// <summary>
    /// Inserts or updates a key-blob mapping with byte array and optional hot metadata.
    /// </summary>
    public void Set(ulong key, byte[] payload, uint hotMeta = 0)
    {
        ArgumentNullException.ThrowIfNull(payload);
        Set(key, payload.AsSpan(), hotMeta);
    }

    /// <summary>
    /// Inserts or updates a key-blob mapping, returning <c>true</c> on success.
    /// </summary>
    public unsafe bool Insert(ulong key, ReadOnlySpan<byte> payload, uint hotMeta = 0)
    {
        ThrowIfDisposed();
        if (payload.Length == 0)
        {
            return NativeMethods.expanse_blob_map_insert(_handle, key, null, 0, hotMeta);
        }
        fixed (byte* pData = payload)
        {
            return NativeMethods.expanse_blob_map_insert(_handle, key, pData, (nuint)payload.Length, hotMeta);
        }
    }

    /// <summary>
    /// Attempts to retrieve the zero-copy blob payload and hot metadata for the given key.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="payload">Zero-copy span pointing directly to the payload.</param>
    /// <param name="hotMeta">The retrieved 32-bit hot metadata.</param>
    /// <returns><c>true</c> if the key is present; otherwise <c>false</c>.</returns>
    public unsafe bool TryGet(ulong key, out ReadOnlySpan<byte> payload, out uint hotMeta)
    {
        ThrowIfDisposed();
        if (NativeMethods.expanse_blob_map_get(_handle, key, out NativeMethods.NativeBlobView view))
        {
            hotMeta = view.HotMeta;
            payload = view.Len == 0 || view.Ptr == IntPtr.Zero
                ? ReadOnlySpan<byte>.Empty
                : new ReadOnlySpan<byte>((void*)view.Ptr, (int)view.Len);
            return true;
        }
        hotMeta = 0;
        payload = default;
        return false;
    }

    /// <summary>
    /// Attempts to retrieve the zero-copy blob payload for the given key.
    /// </summary>
    public bool TryGet(ulong key, out ReadOnlySpan<byte> payload) => TryGet(key, out payload, out _);

    /// <summary>
    /// Attempts to retrieve the blob payload as a managed byte array.
    /// </summary>
    public bool TryGetBytes(ulong key, out byte[] payload, out uint hotMeta)
    {
        if (TryGet(key, out ReadOnlySpan<byte> span, out hotMeta))
        {
            payload = span.ToArray();
            return true;
        }
        payload = Array.Empty<byte>();
        return false;
    }

    /// <summary>
    /// Attempts to retrieve the blob payload as a managed byte array.
    /// </summary>
    public bool TryGetBytes(ulong key, out byte[] payload) => TryGetBytes(key, out payload, out _);

    /// <summary>
    /// Retrieves the payload byte array for the key, or <c>null</c> if absent.
    /// </summary>
    public byte[]? GetBytes(ulong key)
    {
        return TryGetBytes(key, out byte[] bytes) ? bytes : null;
    }

    /// <summary>
    /// Removes a key from the map.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <returns><c>true</c> if the key was present and removed; otherwise <c>false</c>.</returns>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_blob_map_remove(_handle, key);
    }

    /// <summary>
    /// Checks whether the map contains the specified key.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <returns><c>true</c> if present; otherwise <c>false</c>.</returns>
    public bool Contains(ulong key) => ContainsKey(key);

    /// <summary>
    /// Checks whether the map contains the specified key.
    /// </summary>
    public bool ContainsKey(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_blob_map_contains_key(_handle, key);
    }

    /// <summary>
    /// Gets the number of entries stored in the map (capped at <see cref="int.MaxValue"/>).
    /// </summary>
    public int Count
    {
        get
        {
            ulong len = LongCount;
            return len > int.MaxValue ? int.MaxValue : (int)len;
        }
    }

    /// <summary>
    /// Gets the exact 64-bit count of entries stored in the map.
    /// </summary>
    public ulong LongCount
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_blob_map_len(_handle);
        }
    }

    /// <summary>
    /// Gets whether the map is empty.
    /// </summary>
    public bool IsEmpty => LongCount == 0;

    /// <summary>
    /// Gets the total off-heap heap bytes used by the index and slab arena.
    /// </summary>
    public nuint MemoryUsed
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_blob_map_mem_used(_handle);
        }
    }

    /// <summary>
    /// Removes all entries and resets the slab arena.
    /// </summary>
    public void Clear()
    {
        ThrowIfDisposed();
        NativeMethods.expanse_blob_map_clear(_handle);
    }

    /// <summary>
    /// Runs in-place garbage collection and compaction, consolidating live payloads and freeing dead chunks.
    /// </summary>
    /// <returns><c>true</c> if compaction succeeded.</returns>
    public bool Compact()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_blob_map_compact(_handle);
    }

    /// <summary>
    /// Prunes entries that match the given predicate, removes them, and triggers compaction.
    /// </summary>
    /// <param name="predicate">A delegate taking (key, hotMeta) returning <c>true</c> if the entry should be pruned.</param>
    /// <returns>The number of entries pruned.</returns>
    public ulong Prune(Func<ulong, uint, bool> predicate)
    {
        ArgumentNullException.ThrowIfNull(predicate);
        ThrowIfDisposed();

        var toPrune = new List<ulong>();

        NativeMethods.ExpanseScanCallback scanCb = (ulong key, NativeMethods.NativeBlobView view, IntPtr userCtx) =>
        {
            if (predicate(key, view.HotMeta))
            {
                toPrune.Add(key);
            }
            return true;
        };

        NativeMethods.expanse_blob_map_scan_filtered(
            _handle,
            0,
            ulong.MaxValue,
            null,
            scanCb,
            IntPtr.Zero);

        ulong prunedCount = 0;
        foreach (ulong key in toPrune)
        {
            if (NativeMethods.expanse_blob_map_remove(_handle, key))
            {
                prunedCount++;
            }
        }

        if (prunedCount > 0)
        {
            NativeMethods.expanse_blob_map_compact(_handle);
        }

        return prunedCount;
    }

    /// <summary>
    /// Executes a range scan over keys in [<paramref name="startKey"/>, <paramref name="endKey"/>]
    /// with optional hot metadata predicate filtering.
    /// </summary>
    public unsafe ulong ScanFiltered(
        ulong startKey,
        ulong endKey,
        Func<ulong, uint, bool>? predicate,
        Action<ulong, ReadOnlySpan<byte>, uint> callback)
    {
        ArgumentNullException.ThrowIfNull(callback);
        ThrowIfDisposed();

        NativeMethods.ExpansePredicateCallback? predCb = predicate != null
            ? (ulong key, uint meta, IntPtr ctx) => predicate(key, meta)
            : null;

        NativeMethods.ExpanseScanCallback scanCb = (ulong key, NativeMethods.NativeBlobView view, IntPtr ctx) =>
        {
            ReadOnlySpan<byte> span = view.Len == 0 || view.Ptr == IntPtr.Zero
                ? ReadOnlySpan<byte>.Empty
                : new ReadOnlySpan<byte>((void*)view.Ptr, (int)view.Len);
            callback(key, span, view.HotMeta);
            return true;
        };

        nuint count = NativeMethods.expanse_blob_map_scan_filtered(
            _handle,
            startKey,
            endKey,
            predCb,
            scanCb,
            IntPtr.Zero);

        return (ulong)count;
    }

    /// <summary>
    /// Frees the unmanaged memory allocated by this blob map.
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
