using System;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// Thread-safe concurrent bit set supporting serialized single-writer updates
/// and scalable, wait-free, lock-free parallel readers.
/// </summary>
public sealed class ExpanseSyncSet : IDisposable
{
    private SafeExpanseSyncSetHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty <see cref="ExpanseSyncSet"/>.
    /// </summary>
    public ExpanseSyncSet()
    {
        _handle = NativeMethods.expanse_sync_set_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncSet");
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Inserts a key into the set (writer thread).
    /// </summary>
    public bool Add(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_insert(_handle, key);
    }

    /// <summary>
    /// Removes a key from the set (writer thread).
    /// </summary>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_remove(_handle, key);
    }

    /// <summary>
    /// Checks whether the key exists (writer thread).
    /// </summary>
    public bool Contains(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_contains(_handle, key);
    }

    /// <summary>
    /// Gets the number of keys stored in the set.
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
    /// Creates a lightweight lock-free reader handle bound to this concurrent set.
    /// Each querying thread should maintain its own reader handle.
    /// </summary>
    public ExpanseSyncSetReader CreateReader()
    {
        ThrowIfDisposed();
        SafeExpanseSyncSetReaderHandle readerHandle = NativeMethods.expanse_sync_set_reader_new(_handle);
        if (readerHandle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncSetReader");
        }
        return new ExpanseSyncSetReader(readerHandle);
    }

    /// <summary>
    /// Frees the unmanaged memory allocated by this concurrent set.
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
/// Lightweight, lock-free reader for <see cref="ExpanseSyncSet"/>.
/// Safe for parallel multi-threaded queries without locking out the writer.
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
    /// Checks whether the key is present in the concurrent set without acquiring locks.
    /// </summary>
    public bool Contains(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_set_reader_contains(_handle, key);
    }


    /// <summary>
    /// Disposes this reader instance.
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
