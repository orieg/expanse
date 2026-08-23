using System;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// Thread-safe concurrent map (ulong to ulong) allowing one serialized writer and lock-free readers.
/// </summary>
public sealed class ExpanseSyncMap : IDisposable
{
    private SafeExpanseSyncMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new concurrent <see cref="ExpanseSyncMap"/>.
    /// </summary>
    public ExpanseSyncMap()
    {
        _handle = NativeMethods.expanse_sync_map_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryError("Failed to allocate native expanse_sync_map_t");
        }
    }

    /// <summary>
    /// Gets the native handle.
    /// </summary>
    public SafeExpanseSyncMapHandle Handle
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
    /// Sets key -> value.
    /// </summary>
    public bool Set(ulong key, ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_insert(_handle, key, value, IntPtr.Zero);
    }

    /// <summary>
    /// Inserts or replaces key -> value, reporting previous value.
    /// </summary>
    public bool Insert(ulong key, ulong value, out ulong oldValue)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_insert(_handle, key, value, out oldValue);
    }

    /// <summary>
    /// One-shot lookup.
    /// </summary>
    public bool TryGet(ulong key, out ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_get(_handle, key, out value);
    }

    /// <summary>
    /// Removes a key from the map.
    /// </summary>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_remove(_handle, key, IntPtr.Zero);
    }

    /// <summary>
    /// Number of entries stored in the map.
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
    /// Creates a dedicated per-thread reader handle for zero-overhead lock-free lookups.
    /// </summary>
    public ExpanseSyncMapReader CreateReader()
    {
        ThrowIfDisposed();
        var readerHandle = NativeMethods.expanse_sync_map_reader_new(_handle);
        if (readerHandle.IsInvalid)
        {
            throw new OutOfMemoryError("Failed to create reader handle for ExpanseSyncMap");
        }
        return new ExpanseSyncMapReader(readerHandle);
    }

    /// <summary>
    /// Frees the unmanaged concurrent map.
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
/// A dedicated per-thread lock-free reader for <see cref="ExpanseSyncMap"/>.
/// </summary>
public sealed class ExpanseSyncMapReader : IDisposable
{
    private SafeExpanseSyncMapReaderHandle _handle;
    private bool _disposed;

    internal ExpanseSyncMapReader(SafeExpanseSyncMapReaderHandle handle)
    {
        _handle = handle;
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Fast lock-free lookup for a key.
    /// </summary>
    public bool TryGet(ulong key, out ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_reader_get(_handle, key, out value);
    }

    /// <summary>
    /// Frees the reader handle.
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
