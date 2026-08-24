using System;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// Thread-safe concurrent 64-bit integer map (<c>ulong</c> to <c>ulong</c>) supporting serialized
/// single-writer updates and scalable, wait-free, lock-free parallel readers.
/// </summary>
public sealed class ExpanseSyncMap : IDisposable
{
    private SafeExpanseSyncMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty <see cref="ExpanseSyncMap"/>.
    /// </summary>
    public ExpanseSyncMap()
    {
        _handle = NativeMethods.expanse_sync_map_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncMap");
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Inserts or updates a key-value mapping (writer thread).
    /// </summary>
    public bool Set(ulong key, ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_insert(_handle, key, value, IntPtr.Zero);
    }

    /// <summary>
    /// Inserts or updates key -> value (writer thread).
    /// </summary>
    public bool Insert(ulong key, ulong value, out ulong oldValue)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_insert(_handle, key, value, out oldValue);
    }

    /// <summary>
    /// Removes a key from the map (writer thread).
    /// </summary>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_remove(_handle, key, IntPtr.Zero);
    }

    /// <summary>
    /// Removes a key from the map, outputting the old value (writer thread).
    /// </summary>
    public bool Remove(ulong key, out ulong oldValue)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_remove(_handle, key, out oldValue);
    }

    /// <summary>
    /// Looks up a key's value (writer thread).
    /// </summary>
    public bool TryGet(ulong key, out ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_get(_handle, key, out value);
    }

    /// <summary>
    /// Checks whether the key exists (writer thread).
    /// </summary>
    public bool ContainsKey(ulong key) => TryGet(key, out _);

    /// <summary>
    /// Gets the number of entries stored in the map.
    /// </summary>
    public ulong Count
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_map_len(_handle);
        }
    }

    /// <summary>
    /// Creates a lightweight lock-free reader handle bound to this concurrent map.
    /// Each querying thread should maintain its own reader handle.
    /// </summary>
    public ExpanseSyncMapReader CreateReader()
    {
        ThrowIfDisposed();
        SafeExpanseSyncMapReaderHandle readerHandle = NativeMethods.expanse_sync_map_reader_new(_handle);
        if (readerHandle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncMapReader");
        }
        // The native reader is a borrow of the map's storage. Pin the map's SafeHandle
        // for the reader's lifetime so the GC cannot finalize (free) the map while a
        // reader still references it — that would be a use-after-free from safe C#.
        return new ExpanseSyncMapReader(readerHandle, _handle);
    }

    /// <summary>
    /// Frees the unmanaged memory allocated by this concurrent map.
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
/// Lightweight, lock-free reader for <see cref="ExpanseSyncMap"/>.
/// Safe for parallel multi-threaded queries without locking out the writer.
/// </summary>
public sealed class ExpanseSyncMapReader : IDisposable
{
    private SafeExpanseSyncMapReaderHandle _handle;
    // The owning map's handle, ref-counted up for this reader's lifetime so the map
    // cannot be freed while the reader (a native borrow of it) is still alive.
    private readonly SafeExpanseSyncMapHandle _mapHandle;
    private bool _mapHandleRefAdded;
    private bool _disposed;

    internal ExpanseSyncMapReader(SafeExpanseSyncMapReaderHandle handle, SafeExpanseSyncMapHandle mapHandle)
    {
        _handle = handle;
        _mapHandle = mapHandle;
        // Keep the map alive for as long as this reader exists.
        _mapHandle.DangerousAddRef(ref _mapHandleRefAdded);
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Retrieves the value associated with the key without acquiring locks.
    /// </summary>
    public bool TryGet(ulong key, out ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_reader_get(_handle, key, out value);
    }

    /// <summary>
    /// Checks whether the key is present in the concurrent map without acquiring locks.
    /// </summary>
    public bool ContainsKey(ulong key) => TryGet(key, out _);


    /// <summary>
    /// Disposes this reader instance.
    /// </summary>
    public void Dispose()
    {
        if (!_disposed)
        {
            // Free the reader first (it borrows the map), then release our ref-count
            // on the map so it can be collected once nothing else references it.
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
