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
    /// Atomic compare-and-swap on one key, as one linearizable step: stores <paramref name="newValue"/>
    /// only if the key's current word is <paramref name="expected"/>. <c>null</c> is "absent" on both
    /// sides, so every <c>ulong</c> stays a legal value: <c>(null, v)</c> inserts if absent and
    /// <c>(e, null)</c> removes if equal.
    /// </summary>
    /// <param name="key">The key.</param>
    /// <param name="expected">The word the key must currently hold, or <c>null</c> for "only if absent".</param>
    /// <param name="newValue">The word to store, or <c>null</c> to remove the key.</param>
    /// <param name="observed">The word the compare saw on both outcomes (equal to <paramref name="expected"/>
    /// when the store happened); <c>null</c> if the key was absent. A failed call hands back the word to retry against.</param>
    /// <returns><c>true</c> when the store happened.</returns>
    public bool CompareExchange(ulong key, ulong? expected, ulong? newValue, out ulong? observed)
    {
        ThrowIfDisposed();
        bool stored = NativeMethods.expanse_sync_map_compare_exchange(
            _handle, key,
            expected.HasValue, expected.GetValueOrDefault(),
            newValue.HasValue, newValue.GetValueOrDefault(),
            out byte seenPresent, out ulong seen);
        observed = seenPresent != 0 ? (ulong?)seen : null;
        return stored;
    }

    /// <summary>
    /// Stores <paramref name="value"/> under <paramref name="key"/> only if the key is absent, as one
    /// linearizable step. Of any number of callers racing on one absent key, exactly one stores.
    /// </summary>
    /// <returns>The value already present (left untouched), or <c>null</c> when <paramref name="value"/> was stored.</returns>
    public ulong? GetOrInsert(ulong key, ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_get_or_insert(_handle, key, value, out ulong existing)
            ? null
            : (ulong?)existing;
    }

    /// <summary>
    /// Replaces the value for <paramref name="key"/> with <c>fn(current)</c>: a non-null result stores that
    /// value and <c>null</c> removes the key. <c>current</c> is <c>null</c> if the key is absent.
    /// </summary>
    /// <remarks>
    /// This is a loop over <see cref="CompareExchange"/> written in C#, as <c>SyncExpanseMap::update</c> is in
    /// Rust (the C ABI exports no callback form): <paramref name="fn"/> runs again with the observed word
    /// whenever a concurrent writer wins, so it may run several times for one call and must be free of side
    /// effects. An exception thrown by <paramref name="fn"/> propagates and leaves the key unchanged.
    /// </remarks>
    /// <returns>The value that was replaced, or <c>null</c> if the key was absent.</returns>
    public ulong? Update(ulong key, Func<ulong?, ulong?> fn)
    {
        ArgumentNullException.ThrowIfNull(fn);
        ThrowIfDisposed();
        ulong? expected = TryGet(key, out ulong current) ? (ulong?)current : null;
        while (true)
        {
            ulong? next = fn(expected);
            if (CompareExchange(key, expected, next, out ulong? observed))
            {
                return observed;
            }
            expected = observed;
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
            return NativeMethods.expanse_sync_map_len(_handle);
        }
    }

    /// <summary>
    /// Gets the off-heap bytes this map holds from the system allocator: its
    /// tree's own share plus the blocks its epoch collector keeps for reuse or is
    /// waiting to reclaim. Read with writers excluded, so writers wait for it.
    /// </summary>
    public nuint MemoryHeld
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_map_mem_held(_handle);
        }
    }

    /// <summary>
    /// Returns the freed blocks this map's epoch collector keeps for reuse to
    /// the system allocator and returns the bytes released, by which
    /// <see cref="MemoryHeld"/> then falls. Runs beside readers and writers;
    /// blocks still in their grace period stay held until a later call.
    /// </summary>
    public nuint ShrinkToFit()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_map_shrink_to_fit(_handle);
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
