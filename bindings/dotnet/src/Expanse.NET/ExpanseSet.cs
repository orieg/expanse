using System;
using System.Collections;
using System.Collections.Generic;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// High-performance off-heap ordered set of 64-bit integer keys (cf. Judy1).
/// Backed directly by native <c>expanse_set_t</c> with zero GC pressure,
/// O(depth) rank/select, and bidirectional navigation.
/// </summary>
public sealed class ExpanseSet : IDisposable, IEnumerable<ulong>, IReadOnlyCollection<ulong>
{
    private SafeExpanseSetHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty off-heap <see cref="ExpanseSet"/>.
    /// </summary>
    public ExpanseSet()
    {
        _handle = NativeMethods.expanse_set_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native expanse_set_t");
        }
    }

    /// <summary>
    /// Gets the underlying native <see cref="SafeExpanseSetHandle"/>.
    /// </summary>
    public SafeExpanseSetHandle Handle
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
    /// Inserts a 64-bit key into the set.
    /// </summary>
    /// <param name="key">The 64-bit key to insert.</param>
    /// <returns><c>true</c> if the key was newly inserted; <c>false</c> if already present.</returns>
    public bool Add(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_insert(_handle, key);
    }

    /// <summary>
    /// Inserts a 64-bit key into the set (alias for <see cref="Add"/>).
    /// </summary>
    public bool Insert(ulong key) => Add(key);

    /// <summary>
    /// Removes a 64-bit key from the set.
    /// </summary>
    /// <param name="key">The 64-bit key to remove.</param>
    /// <returns><c>true</c> if the key was present and removed; <c>false</c> otherwise.</returns>
    public bool Remove(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_remove(_handle, key);
    }

    /// <summary>
    /// Tests whether the set contains the given 64-bit key.
    /// </summary>
    /// <param name="key">The key to check.</param>
    /// <returns><c>true</c> if present; <c>false</c> otherwise.</returns>
    public bool Contains(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_contains(_handle, key);
    }

    /// <summary>
    /// Checks membership for a batch of keys simultaneously with memory-level parallelism prefetching.
    /// </summary>
    /// <param name="keys">The keys to check.</param>
    /// <param name="outPresent">Boolean array to store presence flags (length must be >= keys.Length).</param>
    /// <returns>The number of keys found.</returns>
    public unsafe nuint ContainsBatch(ulong[] keys, bool[] outPresent)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(keys);
        ArgumentNullException.ThrowIfNull(outPresent);
        if (outPresent.Length < keys.Length)
        {
            throw new ArgumentException("outPresent array length must be >= keys length", nameof(outPresent));
        }
        if (keys.Length == 0)
        {
            return 0;
        }
        fixed (ulong* kPtr = keys)
        fixed (bool* outPtr = outPresent)
        {
            return NativeMethods.expanse_set_contains_batch(_handle, kPtr, (byte*)outPtr, (nuint)keys.Length);
        }
    }

    /// <summary>
    /// Gets the total number of keys in the set (capped at <see cref="int.MaxValue"/>).
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
    /// Gets the exact 64-bit count of keys stored in the set.
    /// </summary>
    public ulong LongCount
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_set_len(_handle);
        }
    }

    /// <summary>
    /// Gets whether the set is empty.
    /// </summary>
    public bool IsEmpty => LongCount == 0;

    /// <summary>
    /// Gets the exact off-heap memory in bytes used by this set.
    /// </summary>
    public nuint MemoryUsed
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_set_mem_used(_handle);
        }
    }

    /// <summary>
    /// Removes all keys from this set, freeing off-heap nodes.
    /// </summary>
    public void Clear()
    {
        ThrowIfDisposed();
        NativeMethods.expanse_set_clear(_handle);
    }

    /// <summary>
    /// Returns the smallest key in the set, or <c>null</c> if the set is empty.
    /// </summary>
    public ulong? First()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_first(_handle, out ulong key) ? key : null;
    }

    /// <summary>
    /// Returns the largest key in the set, or <c>null</c> if the set is empty.
    /// </summary>
    public ulong? Last()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_last(_handle, out ulong key) ? key : null;
    }

    /// <summary>
    /// Returns the smallest key strictly greater than <paramref name="key"/> (higher).
    /// </summary>
    public ulong? Next(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_next_after(_handle, key, out ulong keyOut) ? keyOut : null;
    }

    /// <summary>
    /// Returns the smallest key greater than or equal to <paramref name="key"/> (ceiling).
    /// </summary>
    public ulong? NextAtOrAfter(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_next_at_or_after(_handle, key, out ulong keyOut) ? keyOut : null;
    }

    /// <summary>
    /// Returns the largest key strictly less than <paramref name="key"/> (lower).
    /// </summary>
    public ulong? Prev(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_prev_before(_handle, key, out ulong keyOut) ? keyOut : null;
    }

    /// <summary>
    /// Returns the largest key less than or equal to <paramref name="key"/> (floor).
    /// </summary>
    public ulong? PrevAtOrBefore(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_prev_at_or_before(_handle, key, out ulong keyOut) ? keyOut : null;
    }

    /// <summary>
    /// Returns the number of keys strictly below <paramref name="key"/> (O(depth) rank).
    /// </summary>
    public ulong Rank(ulong key)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_count_below(_handle, key);
    }

    /// <summary>
    /// Returns the number of keys strictly below <paramref name="key"/> (alias for <see cref="Rank"/>).
    /// </summary>
    public ulong CountBelow(ulong key) => Rank(key);

    /// <summary>
    /// Returns the number of keys in the inclusive range [<paramref name="lo"/>, <paramref name="hi"/>] (O(depth)).
    /// </summary>
    public ulong CountRange(ulong lo, ulong hi)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_count_range(_handle, lo, hi);
    }

    /// <summary>
    /// Returns the key with exactly <paramref name="n"/> keys below it (0-based select, O(depth)).
    /// </summary>
    public ulong? Select(ulong n)
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_set_by_count(_handle, n, out ulong keyOut) ? keyOut : null;
    }

    /// <summary>
    /// Returns an enumerator that iterates through the set in ascending order.
    /// </summary>
    public IEnumerator<ulong> GetEnumerator()
    {
        ThrowIfDisposed();
        if (First() is { } current)
        {
            yield return current;
            while (Next(current) is { } next)
            {
                current = next;
                yield return current;
            }
        }
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    /// <summary>
    /// Frees the unmanaged memory allocated by this set.
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
