using System;
using System.Collections.Generic;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// Thread-safe concurrent ordered string map (<c>string</c> to <c>ulong</c>): serialized writers and
/// optimistic parallel readers, the shared form of <see cref="ExpanseStrMap"/>.
/// </summary>
/// <remarks>
/// Keys are stored as NUL-terminated UTF-8 C strings, as <see cref="ExpanseStrMap"/> stores them, so a key
/// containing an embedded NUL character is rejected with <see cref="ArgumentException"/>; use
/// <see cref="ExpanseSyncBytesMap"/> or <see cref="ExpanseSyncOrderedBytesMap"/> for such keys. Keys order
/// by their UTF-8 bytes. Ordered reads are made through a reader (<see cref="CreateReader"/>).
/// </remarks>
public sealed class ExpanseSyncStrMap : IDisposable
{
    private const int KeyStackBytes = 256;

    private SafeExpanseSyncStrMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty <see cref="ExpanseSyncStrMap"/>.
    /// </summary>
    public ExpanseSyncStrMap()
    {
        _handle = NativeMethods.expanse_sync_strmap_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncStrMap");
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
    public bool Set(string key, ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Insert(key.AsSpan(), value, out _);
    }

    /// <summary>
    /// Inserts or updates a key-value mapping.
    /// </summary>
    /// <returns><c>true</c> when the key was new; <c>false</c> when an existing value was replaced.</returns>
    public bool Set(ReadOnlySpan<char> key, ulong value) => Insert(key, value, out _);

    /// <summary>
    /// Inserts or updates key -> value, reporting the replaced value.
    /// </summary>
    /// <param name="key">The string key.</param>
    /// <param name="value">The value to store.</param>
    /// <param name="oldValue">The replaced value when the key was already present; otherwise <c>0</c>.</param>
    /// <returns><c>true</c> when the key was new.</returns>
    public unsafe bool Insert(ReadOnlySpan<char> key, ulong value, out ulong oldValue)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        ulong old = 0;
        bool added;
        fixed (byte* pKey = k.Bytes)
        {
            added = NativeMethods.expanse_sync_strmap_insert(_handle, pKey, value, &old);
        }
        oldValue = added ? 0 : old;
        return added;
    }

    /// <summary>
    /// Inserts or updates key -> value, reporting the replaced value.
    /// </summary>
    public bool Insert(string key, ulong value, out ulong oldValue)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Insert(key.AsSpan(), value, out oldValue);
    }

    /// <summary>
    /// Removes a key from the map.
    /// </summary>
    /// <returns><c>true</c> when the key was present.</returns>
    public bool Remove(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Remove(key.AsSpan(), out _);
    }

    /// <summary>
    /// Removes a key from the map.
    /// </summary>
    public bool Remove(ReadOnlySpan<char> key) => Remove(key, out _);

    /// <summary>
    /// Removes a key from the map, outputting the removed value.
    /// </summary>
    public unsafe bool Remove(ReadOnlySpan<char> key, out ulong oldValue)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        ulong old = 0;
        bool removed;
        fixed (byte* pKey = k.Bytes)
        {
            removed = NativeMethods.expanse_sync_strmap_remove(_handle, pKey, &old);
        }
        oldValue = removed ? old : 0;
        return removed;
    }

    /// <summary>
    /// Removes a key from the map, outputting the removed value.
    /// </summary>
    public bool Remove(string key, out ulong oldValue)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Remove(key.AsSpan(), out oldValue);
    }

    /// <summary>
    /// Looks up a key's value.
    /// </summary>
    public bool TryGet(string key, out ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TryGet(key.AsSpan(), out value);
    }

    /// <summary>
    /// Looks up a key's value.
    /// </summary>
    public unsafe bool TryGet(ReadOnlySpan<char> key, out ulong value)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        fixed (byte* pKey = k.Bytes)
        {
            return NativeMethods.expanse_sync_strmap_get(_handle, pKey, out value);
        }
    }

    /// <summary>
    /// Checks whether the key exists.
    /// </summary>
    public bool ContainsKey(string key) => TryGet(key, out _);

    /// <summary>
    /// Checks whether the key exists.
    /// </summary>
    public bool ContainsKey(ReadOnlySpan<char> key) => TryGet(key, out _);

    /// <summary>
    /// Atomic compare-and-swap on one key, as one linearizable step: stores <paramref name="newValue"/>
    /// only if the key's current word is <paramref name="expected"/>. <c>null</c> is "absent" on both
    /// sides, so every <c>ulong</c> stays a legal value: <c>(null, v)</c> inserts if absent and
    /// <c>(e, null)</c> removes if equal.
    /// </summary>
    /// <param name="key">The string key.</param>
    /// <param name="expected">The word the key must currently hold, or <c>null</c> for "only if absent".</param>
    /// <param name="newValue">The word to store, or <c>null</c> to remove the key.</param>
    /// <param name="observed">The word the compare saw on both outcomes; <c>null</c> if the key was absent.</param>
    /// <returns><c>true</c> when the store happened.</returns>
    public bool CompareExchange(string key, ulong? expected, ulong? newValue, out ulong? observed)
    {
        ArgumentNullException.ThrowIfNull(key);
        return CompareExchange(key.AsSpan(), expected, newValue, out observed);
    }

    /// <summary>
    /// Atomic compare-and-swap on one key; see <see cref="CompareExchange(string, ulong?, ulong?, out ulong?)"/>.
    /// </summary>
    public unsafe bool CompareExchange(ReadOnlySpan<char> key, ulong? expected, ulong? newValue, out ulong? observed)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        bool stored;
        byte seenPresent;
        ulong seen;
        fixed (byte* pKey = k.Bytes)
        {
            stored = NativeMethods.expanse_sync_strmap_compare_exchange(
                _handle, pKey,
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
    public ulong? GetOrInsert(string key, ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return GetOrInsert(key.AsSpan(), value);
    }

    /// <summary>
    /// Stores <paramref name="value"/> under <paramref name="key"/> only if the key is absent.
    /// </summary>
    /// <returns>The value already present (left untouched), or <c>null</c> when <paramref name="value"/> was stored.</returns>
    public unsafe ulong? GetOrInsert(ReadOnlySpan<char> key, ulong value)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        bool stored;
        ulong existing;
        fixed (byte* pKey = k.Bytes)
        {
            stored = NativeMethods.expanse_sync_strmap_get_or_insert(_handle, pKey, value, out existing);
        }
        return stored ? null : (ulong?)existing;
    }

    /// <summary>
    /// Replaces the value for <paramref name="key"/> with <c>fn(current)</c>: a non-null result stores that
    /// value and <c>null</c> removes the key. <c>current</c> is <c>null</c> if the key is absent.
    /// </summary>
    /// <remarks>
    /// A loop over <see cref="CompareExchange(string, ulong?, ulong?, out ulong?)"/> written in C#, as
    /// <see cref="ExpanseSyncMap.Update"/> is: <paramref name="fn"/> runs again with the observed word whenever
    /// a concurrent writer wins, so it may run several times for one call and must be free of side effects.
    /// </remarks>
    /// <returns>The value that was replaced, or <c>null</c> if the key was absent.</returns>
    public ulong? Update(string key, Func<ulong?, ulong?> fn)
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
            return NativeMethods.expanse_sync_strmap_len(_handle);
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
            return NativeMethods.expanse_sync_strmap_mem_used(_handle);
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
            return NativeMethods.expanse_sync_strmap_mem_held(_handle);
        }
    }

    /// <summary>
    /// Returns the freed blocks the epoch collector keeps for reuse to the system allocator and returns the
    /// bytes released. Runs beside readers and writers.
    /// </summary>
    public nuint ShrinkToFit()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_sync_strmap_shrink_to_fit(_handle);
    }

    /// <summary>
    /// Creates a lightweight reader bound to this map. Each querying thread should use its own reader, and
    /// a reader must be disposed before the map is disposed (disposing the map first defers the native free
    /// until the last reader is disposed).
    /// </summary>
    public ExpanseSyncStrMapReader CreateReader()
    {
        ThrowIfDisposed();
        SafeExpanseSyncStrMapReaderHandle readerHandle = NativeMethods.expanse_sync_strmap_reader_new(_handle);
        if (readerHandle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native ExpanseSyncStrMapReader");
        }
        // The native reader is a borrow of the map's storage: the reader holds a reference count on the
        // map's SafeHandle, so the map cannot be released while the reader is alive.
        return new ExpanseSyncStrMapReader(readerHandle, _handle);
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
/// Reader for <see cref="ExpanseSyncStrMap"/>. Point reads and ordered reads run beside the writer without
/// locking it out. A reader is used by one thread at a time; give each thread its own.
/// </summary>
/// <remarks>
/// An ordered walk built from repeated <see cref="Next(string)"/> calls is not a snapshot: it sees every key
/// present throughout the walk, in order, and may or may not see keys inserted or removed meanwhile.
/// </remarks>
public sealed class ExpanseSyncStrMapReader : IDisposable
{
    private const int KeyStackBytes = 256;
    private const int NavStackBytes = 256;

    // expanse_str_nav_status values (include/expanse.h).
    private const int NavOk = 0;
    private const int NavNotFound = 1;
    private const int NavBufferTooSmall = 2;

    private enum NavKind
    {
        First,
        Last,
        NextAtOrAfter,
        NextAfter,
        PrevAtOrBefore,
        PrevBefore,
    }

    private SafeExpanseSyncStrMapReaderHandle _handle;
    // The owning map's handle, ref-counted up for this reader's lifetime so the map
    // cannot be freed while the reader (a native borrow of it) is still alive.
    private readonly SafeExpanseSyncStrMapHandle _mapHandle;
    private bool _mapHandleRefAdded;
    private bool _disposed;

    internal ExpanseSyncStrMapReader(SafeExpanseSyncStrMapReaderHandle handle, SafeExpanseSyncStrMapHandle mapHandle)
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
    public bool TryGet(string key, out ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TryGet(key.AsSpan(), out value);
    }

    /// <summary>
    /// Retrieves the value associated with the key without locking out the writer.
    /// </summary>
    public unsafe bool TryGet(ReadOnlySpan<char> key, out ulong value)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        fixed (byte* pKey = k.Bytes)
        {
            return NativeMethods.expanse_sync_strmap_reader_get(_handle, pKey, out value);
        }
    }

    /// <summary>
    /// Checks whether the key is present without locking out the writer.
    /// </summary>
    public bool ContainsKey(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return ContainsKey(key.AsSpan());
    }

    /// <summary>
    /// Checks whether the key is present without locking out the writer.
    /// </summary>
    public unsafe bool ContainsKey(ReadOnlySpan<char> key)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        fixed (byte* pKey = k.Bytes)
        {
            return NativeMethods.expanse_sync_strmap_reader_contains(_handle, pKey);
        }
    }

    /// <summary>
    /// Returns the lexicographically smallest entry, or <c>null</c> if the map is empty.
    /// </summary>
    public KeyValuePair<string, ulong>? First() => Navigate(NavKind.First, ReadOnlySpan<char>.Empty);

    /// <summary>
    /// Returns the lexicographically largest entry, or <c>null</c> if the map is empty.
    /// </summary>
    public KeyValuePair<string, ulong>? Last() => Navigate(NavKind.Last, ReadOnlySpan<char>.Empty);

    /// <summary>
    /// Returns the entry with the smallest key strictly greater than <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? Next(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Navigate(NavKind.NextAfter, key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the smallest key greater than or equal to <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? NextAtOrAfter(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Navigate(NavKind.NextAtOrAfter, key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the largest key strictly less than <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? Prev(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Navigate(NavKind.PrevBefore, key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the largest key less than or equal to <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? PrevAtOrBefore(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Navigate(NavKind.PrevAtOrBefore, key.AsSpan());
    }

    private unsafe KeyValuePair<string, ulong>? Navigate(NavKind kind, ReadOnlySpan<char> key)
    {
        ThrowIfDisposed();
        using var k = new Utf8CKey(key, stackalloc byte[KeyStackBytes]);
        var scratch = new ScratchBuffer(stackalloc byte[NavStackBytes]);
        try
        {
            // BUFFER_TOO_SMALL reports the exact size (key length plus NUL); the retry is a new read, so
            // it may find a different key if a writer ran in between, and it may need to grow again.
            while (true)
            {
                int status;
                nuint required;
                ulong value;
                fixed (byte* pKey = k.Bytes)
                fixed (byte* pOut = scratch.Span)
                {
                    nuint bufLen = (nuint)scratch.Span.Length;
                    switch (kind)
                    {
                        case NavKind.First:
                            status = NativeMethods.expanse_sync_strmap_reader_first(_handle, pOut, bufLen, out required, out value);
                            break;
                        case NavKind.Last:
                            status = NativeMethods.expanse_sync_strmap_reader_last(_handle, pOut, bufLen, out required, out value);
                            break;
                        case NavKind.NextAtOrAfter:
                            status = NativeMethods.expanse_sync_strmap_reader_next_at_or_after(_handle, pKey, pOut, bufLen, out required, out value);
                            break;
                        case NavKind.NextAfter:
                            status = NativeMethods.expanse_sync_strmap_reader_next_after(_handle, pKey, pOut, bufLen, out required, out value);
                            break;
                        case NavKind.PrevAtOrBefore:
                            status = NativeMethods.expanse_sync_strmap_reader_prev_at_or_before(_handle, pKey, pOut, bufLen, out required, out value);
                            break;
                        case NavKind.PrevBefore:
                            status = NativeMethods.expanse_sync_strmap_reader_prev_before(_handle, pKey, pOut, bufLen, out required, out value);
                            break;
                        default:
                            throw new InvalidOperationException("Unknown navigation kind: " + kind);
                    }
                }

                switch (status)
                {
                    case NavOk:
                        // `required` is the key length plus the terminating NUL.
                        string found = System.Text.Encoding.UTF8.GetString(scratch.Span.Slice(0, (int)required - 1));
                        return new KeyValuePair<string, ulong>(found, value);
                    case NavNotFound:
                        return null;
                    case NavBufferTooSmall:
                        scratch.Grow(required);
                        break;
                    default:
                        throw new InvalidOperationException("Unknown expanse_str_nav_status: " + status);
                }
            }
        }
        finally
        {
            scratch.Dispose();
        }
    }

    /// <summary>
    /// Disposes this reader. The reader's native handle is released before its hold on the map.
    /// </summary>
    public void Dispose()
    {
        if (!_disposed)
        {
            // Free the reader first (it borrows the map), then release our ref-count
            // on the map so it can be freed once nothing else references it.
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
