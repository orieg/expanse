using System;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// Thread-safe concurrent unordered map of arbitrary byte keys to <c>ulong</c> values: serialized writers and
/// optimistic parallel readers, the shared form of <see cref="ExpanseBytesMap"/>.
/// </summary>
/// <remarks>
/// Keys are opaque byte strings: embedded <c>0x00</c> bytes are ordinary and the empty key is valid. The map
/// has no key order; use <see cref="ExpanseSyncOrderedBytesMap"/> for ordered reads.
/// </remarks>
public sealed class ExpanseSyncBytesMap : IDisposable
{
    private SafeExpanseSyncBytesMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty <see cref="ExpanseSyncBytesMap"/>.
    /// </summary>
    public ExpanseSyncBytesMap()
    {
        _handle = NativeMethods.expanse_sync_bytesmap_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncBytesMap");
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed || _handle.IsInvalid || _handle.IsClosed, this);
    }

    /// <summary>
    /// Inserts or updates a key-value mapping.
    /// </summary>
    /// <returns><c>true</c> when the key was new; <c>false</c> when an existing value was replaced.</returns>
    public bool Set(ReadOnlySpan<byte> key, ulong value) => Insert(key, value, out _);

    /// <summary>
    /// Inserts or updates a key-value mapping.
    /// </summary>
    public bool Set(byte[] key, ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Insert(key.AsSpan(), value, out _);
    }

    /// <summary>
    /// Inserts or updates key -> value, reporting the replaced value.
    /// </summary>
    /// <param name="key">The byte key.</param>
    /// <param name="value">The value to store.</param>
    /// <param name="oldValue">The replaced value when the key was already present; otherwise <c>0</c>.</param>
    /// <returns><c>true</c> when the key was new.</returns>
    public unsafe bool Insert(ReadOnlySpan<byte> key, ulong value, out ulong oldValue)
    {
        ThrowIfDisposed();
        ulong old = 0;
        bool added;
        fixed (byte* pKey = key)
        {
            added = NativeMethods.expanse_sync_bytesmap_insert(_handle, pKey, (nuint)key.Length, value, &old);
        }
        oldValue = added ? 0 : old;
        return added;
    }

    /// <summary>
    /// Removes a key from the map.
    /// </summary>
    /// <returns><c>true</c> when the key was present.</returns>
    public bool Remove(ReadOnlySpan<byte> key) => Remove(key, out _);

    /// <summary>
    /// Removes a key from the map.
    /// </summary>
    public bool Remove(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Remove(key.AsSpan(), out _);
    }

    /// <summary>
    /// Removes a key from the map, outputting the removed value.
    /// </summary>
    public unsafe bool Remove(ReadOnlySpan<byte> key, out ulong oldValue)
    {
        ThrowIfDisposed();
        ulong old = 0;
        bool removed;
        fixed (byte* pKey = key)
        {
            removed = NativeMethods.expanse_sync_bytesmap_remove(_handle, pKey, (nuint)key.Length, &old);
        }
        oldValue = removed ? old : 0;
        return removed;
    }

    /// <summary>
    /// Looks up a key's value.
    /// </summary>
    public unsafe bool TryGet(ReadOnlySpan<byte> key, out ulong value)
    {
        ThrowIfDisposed();
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_sync_bytesmap_get(_handle, pKey, (nuint)key.Length, out value);
        }
    }

    /// <summary>
    /// Looks up a key's value.
    /// </summary>
    public bool TryGet(byte[] key, out ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TryGet(key.AsSpan(), out value);
    }

    /// <summary>
    /// Checks whether the key exists.
    /// </summary>
    public bool ContainsKey(ReadOnlySpan<byte> key) => TryGet(key, out _);

    /// <summary>
    /// Checks whether the key exists.
    /// </summary>
    public bool ContainsKey(byte[] key) => TryGet(key, out _);

    /// <summary>
    /// Atomic compare-and-swap on one key, as one linearizable step: stores <paramref name="newValue"/>
    /// only if the key's current word is <paramref name="expected"/>. <c>null</c> is "absent" on both
    /// sides, so every <c>ulong</c> stays a legal value: <c>(null, v)</c> inserts if absent and
    /// <c>(e, null)</c> removes if equal.
    /// </summary>
    /// <param name="key">The byte key.</param>
    /// <param name="expected">The word the key must currently hold, or <c>null</c> for "only if absent".</param>
    /// <param name="newValue">The word to store, or <c>null</c> to remove the key.</param>
    /// <param name="observed">The word the compare saw on both outcomes; <c>null</c> if the key was absent.</param>
    /// <returns><c>true</c> when the store happened.</returns>
    public unsafe bool CompareExchange(ReadOnlySpan<byte> key, ulong? expected, ulong? newValue, out ulong? observed)
    {
        ThrowIfDisposed();
        bool stored;
        byte seenPresent;
        ulong seen;
        fixed (byte* pKey = key)
        {
            stored = NativeMethods.expanse_sync_bytesmap_compare_exchange(
                _handle, pKey, (nuint)key.Length,
                expected.HasValue, expected.GetValueOrDefault(),
                newValue.HasValue, newValue.GetValueOrDefault(),
                out seenPresent, out seen);
        }
        observed = seenPresent != 0 ? (ulong?)seen : null;
        return stored;
    }

    /// <summary>
    /// Stores <paramref name="value"/> under <paramref name="key"/> only if the key is absent, as one
    /// linearizable step. Of any number of callers racing on one absent key, exactly one stores.
    /// </summary>
    /// <returns>The value already present (left untouched), or <c>null</c> when <paramref name="value"/> was stored.</returns>
    public unsafe ulong? GetOrInsert(ReadOnlySpan<byte> key, ulong value)
    {
        ThrowIfDisposed();
        bool stored;
        ulong existing;
        fixed (byte* pKey = key)
        {
            stored = NativeMethods.expanse_sync_bytesmap_get_or_insert(_handle, pKey, (nuint)key.Length, value, out existing);
        }
        return stored ? null : (ulong?)existing;
    }

    /// <summary>
    /// Replaces the value for <paramref name="key"/> with <c>fn(current)</c>: a non-null result stores that
    /// value and <c>null</c> removes the key. <c>current</c> is <c>null</c> if the key is absent.
    /// </summary>
    /// <remarks>
    /// A loop over <see cref="CompareExchange"/> written in C#: <paramref name="fn"/> runs again with the
    /// observed word whenever a concurrent writer wins, so it may run several times for one call and must be
    /// free of side effects.
    /// </remarks>
    /// <returns>The value that was replaced, or <c>null</c> if the key was absent.</returns>
    public ulong? Update(byte[] key, Func<ulong?, ulong?> fn)
    {
        ArgumentNullException.ThrowIfNull(key);
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
            return NativeMethods.expanse_sync_bytesmap_len(_handle);
        }
    }

    /// <summary>
    /// Gets the off-heap bytes the map's nodes use. Read with writers excluded, so writers wait for it.
    /// </summary>
    public nuint MemoryUsed
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_bytesmap_mem_used(_handle);
        }
    }

    /// <summary>
    /// Gets the off-heap bytes this map holds from the system allocator, including blocks its epoch
    /// collector keeps for reuse. Read with writers excluded, so writers wait for it.
    /// </summary>
    public nuint MemoryHeld
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_sync_bytesmap_mem_held(_handle);
        }
    }

    /// <summary>
    /// Returns the freed blocks the epoch collector keeps for reuse to the system allocator and returns the
    /// bytes released. Runs beside readers and writers.
    /// </summary>
    public nuint ShrinkToFit()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_bytesmap_shrink_to_fit(_handle);
    }

    /// <summary>
    /// Creates a lightweight reader bound to this map. Each querying thread should use its own reader, and
    /// a reader must be disposed before the map is disposed (disposing the map first defers the native free
    /// until the last reader is disposed).
    /// </summary>
    public ExpanseSyncBytesMapReader CreateReader()
    {
        ThrowIfDisposed();
        SafeExpanseSyncBytesMapReaderHandle readerHandle = NativeMethods.expanse_sync_bytesmap_reader_new(_handle);
        if (readerHandle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncBytesMapReader");
        }
        // The native reader is a borrow of the map's storage: the reader holds a reference count on the
        // map's SafeHandle, so the map cannot be released while the reader is alive.
        return new ExpanseSyncBytesMapReader(readerHandle, _handle);
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
/// Reader for <see cref="ExpanseSyncBytesMap"/>; reads run beside the writer without locking it out. A reader
/// is used by one thread at a time; give each thread its own.
/// </summary>
public sealed class ExpanseSyncBytesMapReader : IDisposable
{
    private SafeExpanseSyncBytesMapReaderHandle _handle;
    // The owning map's handle, ref-counted up for this reader's lifetime so the map
    // cannot be freed while the reader (a native borrow of it) is still alive.
    private readonly SafeExpanseSyncBytesMapHandle _mapHandle;
    private bool _mapHandleRefAdded;
    private bool _disposed;

    internal ExpanseSyncBytesMapReader(SafeExpanseSyncBytesMapReaderHandle handle, SafeExpanseSyncBytesMapHandle mapHandle)
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
    /// Retrieves the value associated with the key without locking out the writer.
    /// </summary>
    public unsafe bool TryGet(ReadOnlySpan<byte> key, out ulong value)
    {
        ThrowIfDisposed();
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_sync_bytesmap_reader_get(_handle, pKey, (nuint)key.Length, out value);
        }
    }

    /// <summary>
    /// Retrieves the value associated with the key without locking out the writer.
    /// </summary>
    public bool TryGet(byte[] key, out ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TryGet(key.AsSpan(), out value);
    }

    /// <summary>
    /// Checks whether the key is present without locking out the writer.
    /// </summary>
    public unsafe bool ContainsKey(ReadOnlySpan<byte> key)
    {
        ThrowIfDisposed();
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_sync_bytesmap_reader_contains(_handle, pKey, (nuint)key.Length);
        }
    }

    /// <summary>
    /// Checks whether the key is present without locking out the writer.
    /// </summary>
    public bool ContainsKey(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return ContainsKey(key.AsSpan());
    }

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
