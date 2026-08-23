using System;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// Thread-safe concurrent bit set allowing one serialized writer and lock-free readers.
/// </summary>
public sealed class ExpanseSyncSet : IDisposable
{
    private SafeExpanseSyncSetHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new concurrent <see cref="ExpanseSyncSet"/>.
    /// </summary>
    public ExpanseSyncSet()
    {
        _handle = NativeMethods.expanse_sync_set_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryError("Failed to allocate native expanse_sync_set_t");
        }
    }

    /// <summary>
    /// Gets the native handle.
    /// </summary>
    public SafeExpanseSyncSetHandle Handle
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
    /// Inserts a key into the concurrent set.
    /// </summary>
    public bool Add(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_insert(_handle, key);
    }

    /// <summary>
    /// Removes a key from the concurrent set.
    /// </summary>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_remove(_handle, key);
    }

    /// <summary>
    /// One-shot membership test.
    /// </summary>
    public bool Contains(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_contains(_handle, key);
    }

    /// <summary>
    /// Number of elements in the set.
    /// </summary>
    public ulong Count
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_set_len(_handle);
        }
    }

    /// <summary>
    /// Creates a dedicated per-thread reader handle for zero-overhead lock-free lookups.
    /// </summary>
    public ExpanseSyncSetReader CreateReader()
    {
        ThrowIfDisposed();
        var readerHandle = NativeMethods.expanse_sync_set_reader_new(_handle);
        if (readerHandle.IsInvalid)
        {
            throw new OutOfMemoryError("Failed to create reader handle for ExpanseSyncSet");
        }
        return new ExpanseSyncSetReader(readerHandle);
    }

    /// <summary>
    /// Frees the unmanaged concurrent set.
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
/// A dedicated per-thread lock-free reader for <see cref="ExpanseSyncSet"/>.
/// </summary>
public sealed class ExpanseSyncSetReader : IDisposable
{
    private SafeExpanseSyncSetReaderHandle _handle;
    private bool _disposed;

    internal ExpanseSyncSetReader(SafeExpanseSyncSetReaderHandle handle)
    {
        _handle = handle;
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Fast lock-free membership test.
    /// </summary>
    public bool Contains(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_reader_contains(_handle, key);
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
