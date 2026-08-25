using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics.CodeAnalysis;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// High-performance off-heap ordered 64-bit integer map (<c>ulong</c> to <c>ulong</c>) (cf. JudyL).
/// Backed directly by native <c>expanse_map_t</c> with zero GC allocations for entries,
/// O(depth) rank/select, and bidirectional navigation.
/// </summary>
public sealed class ExpanseMap : IDisposable, IEnumerable<KeyValuePair<ulong, ulong>>, IReadOnlyCollection<KeyValuePair<ulong, ulong>>
{
    private SafeExpanseMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty off-heap <see cref="ExpanseMap"/>.
    /// </summary>
    public ExpanseMap()
    {
        _handle = NativeMethods.expanse_map_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native expanse_map_t");
        }
    }

    /// <summary>
    /// Gets the underlying native <see cref="SafeExpanseMapHandle"/>.
    /// </summary>
    public SafeExpanseMapHandle Handle
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
    /// Gets or sets the value associated with the specified 64-bit key.
    /// </summary>
    /// <param name="key">The key to locate or set.</param>
    /// <returns>The value associated with the key.</returns>
    /// <exception cref="KeyNotFoundException">The key was not found when getting.</exception>
    public ulong this[ulong key]
    {
        get
        {
            if (TryGet(key, out ulong value))
            {
                return value;
            }
            throw new KeyNotFoundException($"Key {key} was not found in ExpanseMap.");
        }
        set => Set(key, value);
    }

    /// <summary>
    /// Stores the key-value pair.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="value">The 64-bit value.</param>
    public void Set(ulong key, ulong value)
    {
        ThrowIfDisposed();
        NativeMethods.expanse_map_insert(_handle, key, value, IntPtr.Zero);
    }

    /// <summary>
    /// Stores key -> value. Returns <c>true</c> if newly inserted; <c>false</c> if an existing entry was replaced.
    /// </summary>
    /// <param name="key">The 64-bit key.</param>
    /// <param name="value">The 64-bit value.</param>
    /// <param name="oldValue">Receives the previous value if replaced, or 0 if newly inserted.</param>
    public bool Insert(ulong key, ulong value, out ulong oldValue)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_insert(_handle, key, value, out oldValue);
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the specified key.
    /// </summary>
    /// <param name="key">The key to locate.</param>
    /// <param name="value">When found, contains the associated value.</param>
    /// <returns><c>true</c> if the key is present; otherwise <c>false</c>.</returns>
    public bool TryGet(ulong key, out ulong value)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_get(_handle, key, out value);
    }

    /// <summary>
    /// Look up a batch of keys simultaneously with memory-level parallelism prefetching.
    /// </summary>
    /// <param name="keys">The keys to look up.</param>
    /// <param name="outValues">Array to store found values (length must be >= keys.Length).</param>
    /// <param name="outFound">Optional boolean array to store presence flags (null or length >= keys.Length).</param>
    /// <returns>The number of keys found.</returns>
    public unsafe nuint GetBatch(ulong[] keys, ulong[] outValues, bool[]? outFound = null)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(keys);
        ArgumentNullException.ThrowIfNull(outValues);
        if (outValues.Length < keys.Length)
        {
            throw new ArgumentException("outValues array length must be >= keys length", nameof(outValues));
        }
        if (outFound != null && outFound.Length < keys.Length)
        {
            throw new ArgumentException("outFound array length must be >= keys length", nameof(outFound));
        }
        if (keys.Length == 0)
        {
            return 0;
        }
        fixed (ulong* kPtr = keys)
        fixed (ulong* vPtr = outValues)
        {
            if (outFound != null)
            {
                fixed (bool* fPtr = outFound)
                {
                    return NativeMethods.expanse_map_get_batch(_handle, kPtr, vPtr, (byte*)fPtr, (nuint)keys.Length);
                }
            }
            else
            {
                return NativeMethods.expanse_map_get_batch(_handle, kPtr, vPtr, null, (nuint)keys.Length);
            }
        }
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the specified key (alias for <see cref="TryGet"/>).
    /// </summary>
    public bool TryGetValue(ulong key, out ulong value) => TryGet(key, out value);

    /// <summary>
    /// Removes the specified key from the map.
    /// </summary>
    /// <param name="key">The key to remove.</param>
    /// <returns><c>true</c> if the key was present and removed; otherwise <c>false</c>.</returns>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_remove(_handle, key, IntPtr.Zero);
    }

    /// <summary>
    /// Removes the specified key from the map, outputting its old value.
    /// </summary>
    /// <param name="key">The key to remove.</param>
    /// <param name="oldValue">Receives the removed value if found.</param>
    /// <returns><c>true</c> if the key was present and removed; otherwise <c>false</c>.</returns>
    public bool Remove(ulong key, out ulong oldValue)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_remove(_handle, key, out oldValue);
    }

    /// <summary>
    /// Checks whether the map contains the specified key.
    /// </summary>
    /// <param name="key">The key to check.</param>
    /// <returns><c>true</c> if the key exists; otherwise <c>false</c>.</returns>
    public bool ContainsKey(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_get(_handle, key, out _);
    }

    /// <summary>
    /// Gets the number of key-value pairs in the map (capped at <see cref="int.MaxValue"/>).
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
            return NativeMethods.expanse_map_len(_handle);
        }
    }

    /// <summary>
    /// Gets whether the map is empty.
    /// </summary>
    public bool IsEmpty => LongCount == 0;

    /// <summary>
    /// Gets the exact off-heap memory in bytes used by this map.
    /// </summary>
    public nuint MemoryUsed
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_map_mem_used(_handle);
        }
    }

    /// <summary>
    /// Removes all entries from this map, freeing off-heap nodes.
    /// </summary>
    public void Clear()
    {
        ThrowIfDisposed();
        NativeMethods.expanse_map_clear(_handle);
    }

    /// <summary>
    /// Returns the smallest entry in the map, or <c>null</c> if the map is empty.
    /// </summary>
    public KeyValuePair<ulong, ulong>? First()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_first(_handle, out ulong key, out ulong value)
            ? new KeyValuePair<ulong, ulong>(key, value)
            : null;
    }

    /// <summary>
    /// Returns the largest entry in the map, or <c>null</c> if the map is empty.
    /// </summary>
    public KeyValuePair<ulong, ulong>? Last()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_last(_handle, out ulong key, out ulong value)
            ? new KeyValuePair<ulong, ulong>(key, value)
            : null;
    }

    /// <summary>
    /// Returns the entry with the smallest key strictly greater than <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<ulong, ulong>? Next(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_next_after(_handle, key, out ulong keyOut, out ulong valOut)
            ? new KeyValuePair<ulong, ulong>(keyOut, valOut)
            : null;
    }

    /// <summary>
    /// Returns the entry with the smallest key greater than or equal to <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<ulong, ulong>? NextAtOrAfter(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_next_at_or_after(_handle, key, out ulong keyOut, out ulong valOut)
            ? new KeyValuePair<ulong, ulong>(keyOut, valOut)
            : null;
    }

    /// <summary>
    /// Returns the entry with the largest key strictly less than <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<ulong, ulong>? Prev(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_prev_before(_handle, key, out ulong keyOut, out ulong valOut)
            ? new KeyValuePair<ulong, ulong>(keyOut, valOut)
            : null;
    }

    /// <summary>
    /// Returns the entry with the largest key less than or equal to <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<ulong, ulong>? PrevAtOrBefore(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_prev_at_or_before(_handle, key, out ulong keyOut, out ulong valOut)
            ? new KeyValuePair<ulong, ulong>(keyOut, valOut)
            : null;
    }

    /// <summary>
    /// Number of entries with keys strictly below <paramref name="key"/> (O(depth) rank).
    /// </summary>
    public ulong Rank(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_count_below(_handle, key);
    }

    /// <summary>
    /// Number of entries with keys in the inclusive range [<paramref name="lo"/>, <paramref name="hi"/>].
    /// </summary>
    public ulong CountRange(ulong lo, ulong hi)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_count_range(_handle, lo, hi);
    }

    /// <summary>
    /// Returns the entry with exactly <paramref name="n"/> entries below it (0-based select, O(depth)).
    /// </summary>
    public KeyValuePair<ulong, ulong>? Select(ulong n)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_map_by_count(_handle, n, out ulong keyOut, out ulong valOut)
            ? new KeyValuePair<ulong, ulong>(keyOut, valOut)
            : null;
    }

    /// <summary>
    /// Returns an enumerator that iterates through the map entries in ascending key order.
    /// </summary>
    public IEnumerator<KeyValuePair<ulong, ulong>> GetEnumerator()
    {
        ThrowIfDisposed();
        if (First() is { } current)
        {
            yield return current;
            while (Next(current.Key) is { } next)
            {
                current = next;
                yield return current;
            }
        }
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    /// <summary>
    /// Frees the unmanaged memory allocated by this map.
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
