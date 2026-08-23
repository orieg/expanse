using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// Delegate invoked during zero-copy blob scans.
/// </summary>
/// <param name="key">The 64-bit key.</param>
/// <param name="payload">Zero-copy read-only span pointing directly to the blob payload bytes.</param>
/// <param name="hotMeta">The 32-bit hot metadata word.</param>
public delegate void ExpanseBlobScanAction(ulong key, ReadOnlySpan<byte> payload, uint hotMeta);

/// <summary>
/// High-performance polymorphic large-value map with inline payload packing (0..=7 bytes),
/// off-heap arena slab allocation, hot metadata filtering, and in-place garbage collection.
/// </summary>
public sealed class ExpanseBlobMap : IDisposable
{
    private readonly SafeExpanseBlobMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty blob map with default 2 MiB arena chunk capacity.
    /// </summary>
    public ExpanseBlobMap() : this(0)
    {
    }

    /// <summary>
    /// Creates a new empty blob map with custom chunk capacity.
    /// </summary>
    /// <param name="chunkSize">Chunk size in bytes, or 0 for default 2 MiB.</param>
    public ExpanseBlobMap(nuint chunkSize)
    {
        _handle = NativeMethods.expanse_blob_map_new(chunkSize);
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate expanse_blob_map_t.");
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsClosed || _handle.IsInvalid, this);
    }

    /// <summary>
    /// Inserts or replaces a key-blob pair with 32-bit hot metadata.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="data">The byte span payload to store.</param>
    /// <param name="hotMeta">32-bit hot metadata word evaluated during zero-copy filter predicates.</param>
    public unsafe void Set(ulong key, ReadOnlySpan<byte> data, uint hotMeta = 0)
    {
        ThrowIfDisposed();
        fixed (byte* ptr = data)
        {
            if (!NativeMethods.expanse_blob_map_insert(_handle, key, ptr, (nuint)data.Length, hotMeta))
            {
                throw new InvalidOperationException($"Failed to insert blob for key {key}.");
            }
        }
    }

    /// <summary>
    /// Inserts or replaces a key-blob pair with 32-bit hot metadata.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="data">The byte array payload to store.</param>
    /// <param name="hotMeta">32-bit hot metadata word.</param>
    public void Set(ulong key, byte[] data, uint hotMeta = 0)
    {
        ArgumentNullException.ThrowIfNull(data);
        Set(key, data.AsSpan(), hotMeta);
    }

    /// <summary>
    /// Looks up a key, providing zero-copy span access to the payload and hot metadata.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="payload">Output read-only span pointing directly to the blob payload.</param>
    /// <param name="hotMeta">Output 32-bit hot metadata word.</param>
    /// <returns><c>true</c> if key was found; otherwise <c>false</c>.</returns>
    public unsafe bool TryGet(ulong key, out ReadOnlySpan<byte> payload, out uint hotMeta)
    {
        ThrowIfDisposed();
        if (NativeMethods.expanse_blob_map_get(_handle, key, out NativeMethods.NativeBlobView view))
        {
            hotMeta = view.HotMeta;
            if (view.Len == 0 || view.Ptr == IntPtr.Zero)
            {
                payload = ReadOnlySpan<byte>.Empty;
            }
            else
            {
                payload = new ReadOnlySpan<byte>((void*)view.Ptr, (int)view.Len);
            }
            return true;
        }

        payload = default;
        hotMeta = 0;
        return false;
    }

    /// <summary>
    /// Looks up a key, providing zero-copy span access to the payload.
    /// </summary>
    public bool TryGet(ulong key, out ReadOnlySpan<byte> payload) => TryGet(key, out payload, out _);

    /// <summary>
    /// Looks up a key, copying the payload into a newly allocated managed byte array.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="payload">Output managed byte array containing a copy of the payload.</param>
    /// <param name="hotMeta">Output 32-bit hot metadata word.</param>
    /// <returns><c>true</c> if key was found; otherwise <c>false</c>.</returns>
    public bool TryGetBytes(ulong key, out byte[] payload, out uint hotMeta)
    {
        if (TryGet(key, out ReadOnlySpan<byte> span, out hotMeta))
        {
            payload = span.ToArray();
            return true;
        }

        payload = [];
        return false;
    }

    /// <summary>
    /// Looks up a key, copying the payload into a newly allocated managed byte array.
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

        NativeMethods.ExpansePredicateCallback predCb = (ulong key, uint meta, IntPtr ctx) =>
        {
            if (predicate(key, meta))
            {
                toPrune.Add(key);
            }
            return false;
        };

        NativeMethods.expanse_blob_map_scan_filtered(
            _handle,
            0,
            ulong.MaxValue,
            predCb,
            null,
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
        ExpanseBlobScanAction callback)
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
