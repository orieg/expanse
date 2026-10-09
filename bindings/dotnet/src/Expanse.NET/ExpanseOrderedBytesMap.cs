using System;
using System.Collections.Generic;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// High-performance off-heap <em>ordered</em> map of arbitrary byte keys to <c>ulong</c> values (cf. JudyHS, ordered).
/// Keys compare as unsigned bytes in lexicographic order; a shorter key sorts before any longer key it prefixes.
/// The empty key is valid and sorts first; keys may contain <c>0x00</c> and <c>0xFF</c>.
/// </summary>
/// <remarks>
/// Navigation writes the matching key into a managed buffer that the wrapper allocates. When the native side
/// reports <c>EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL</c> it writes nothing and returns the exact key length;
/// the wrapper retries with a buffer of that size, so a key is never truncated.
/// </remarks>
public sealed class ExpanseOrderedBytesMap : IDisposable
{
    private const int DefaultBufLen = 256;

    // expanse_ordered_bytes_nav_status values (include/expanse.h).
    private const int NavOk = 0;
    private const int NavNotFound = 1;
    private const int NavBufferTooSmall = 2;

    private enum NavKind
    {
        First,
        Last,
        CeilingAt,
        HigherThan,
        FloorAt,
        LowerThan,
    }

    private SafeExpanseOrderedBytesMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// An entry returned by ordered navigation. <see cref="Key"/> is a fresh array owned by the entry;
    /// compare keys by content (for example with <c>Key.AsSpan().SequenceEqual(other)</c>), not by array reference.
    /// </summary>
    /// <param name="Key">The byte key.</param>
    /// <param name="Value">The 64-bit value.</param>
    public sealed record Entry(byte[] Key, ulong Value);

    /// <summary>
    /// Creates a new empty off-heap <see cref="ExpanseOrderedBytesMap"/>.
    /// </summary>
    public ExpanseOrderedBytesMap()
    {
        _handle = NativeMethods.expanse_ordered_bytesmap_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native expanse_ordered_bytesmap_t");
        }
    }

    /// <summary>
    /// Gets the underlying native <see cref="SafeExpanseOrderedBytesMapHandle"/>.
    /// </summary>
    public SafeExpanseOrderedBytesMapHandle Handle
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
    /// Gets or sets the value associated with the specified byte sequence.
    /// </summary>
    /// <exception cref="KeyNotFoundException">Thrown by the getter when the key is absent.</exception>
    public ulong this[ReadOnlySpan<byte> key]
    {
        get
        {
            if (TryGet(key, out ulong value))
            {
                return value;
            }
            throw new KeyNotFoundException("Key was not found in ExpanseOrderedBytesMap.");
        }
        set => Set(key, value);
    }

    /// <summary>
    /// Gets or sets the value associated with the specified byte array.
    /// </summary>
    public ulong this[byte[] key]
    {
        get
        {
            ArgumentNullException.ThrowIfNull(key);
            return this[key.AsSpan()];
        }
        set
        {
            ArgumentNullException.ThrowIfNull(key);
            this[key.AsSpan()] = value;
        }
    }

    /// <summary>
    /// Stores the key-value pair.
    /// </summary>
    /// <param name="key">The byte key.</param>
    /// <param name="value">The 64-bit value.</param>
    public unsafe void Set(ReadOnlySpan<byte> key, ulong value)
    {
        ThrowIfDisposed();
        if (key.IsEmpty)
        {
            NativeMethods.expanse_ordered_bytesmap_insert(_handle, null, 0, value, IntPtr.Zero);
            return;
        }
        fixed (byte* pKey = key)
        {
            NativeMethods.expanse_ordered_bytesmap_insert(_handle, pKey, (nuint)key.Length, value, IntPtr.Zero);
        }
    }

    /// <summary>
    /// Stores the key-value pair with a byte array key.
    /// </summary>
    public void Set(byte[] key, ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        Set(key.AsSpan(), value);
    }

    /// <summary>
    /// Stores key -&gt; value, returning <c>true</c> if newly inserted or <c>false</c> if replaced.
    /// </summary>
    /// <param name="key">The byte key.</param>
    /// <param name="value">The 64-bit value.</param>
    /// <param name="oldValue">Set to 0 before the call; the native side writes the replaced value only on replacement.</param>
    public unsafe bool Insert(ReadOnlySpan<byte> key, ulong value, out ulong oldValue)
    {
        ThrowIfDisposed();
        oldValue = 0;
        if (key.IsEmpty)
        {
            return NativeMethods.expanse_ordered_bytesmap_insert(_handle, null, 0, value, out oldValue);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_ordered_bytesmap_insert(_handle, pKey, (nuint)key.Length, value, out oldValue);
        }
    }

    /// <summary>
    /// Stores key -&gt; value with a byte array key, returning <c>true</c> if newly inserted or <c>false</c> if replaced.
    /// </summary>
    public bool Insert(byte[] key, ulong value, out ulong oldValue)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Insert(key.AsSpan(), value, out oldValue);
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the specified byte key.
    /// </summary>
    /// <param name="key">The byte key.</param>
    /// <param name="value">When found, contains the associated value.</param>
    /// <returns><c>true</c> if present; otherwise <c>false</c>.</returns>
    public unsafe bool TryGet(ReadOnlySpan<byte> key, out ulong value)
    {
        ThrowIfDisposed();
        if (key.IsEmpty)
        {
            return NativeMethods.expanse_ordered_bytesmap_get(_handle, null, 0, out value);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_ordered_bytesmap_get(_handle, pKey, (nuint)key.Length, out value);
        }
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the specified byte array key.
    /// </summary>
    public bool TryGet(byte[] key, out ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TryGet(key.AsSpan(), out value);
    }

    /// <summary>
    /// Checks whether the map contains the specified byte key.
    /// </summary>
    public unsafe bool ContainsKey(ReadOnlySpan<byte> key)
    {
        ThrowIfDisposed();
        if (key.IsEmpty)
        {
            return NativeMethods.expanse_ordered_bytesmap_contains(_handle, null, 0);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_ordered_bytesmap_contains(_handle, pKey, (nuint)key.Length);
        }
    }

    /// <summary>
    /// Checks whether the map contains the specified byte array key.
    /// </summary>
    public bool ContainsKey(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return ContainsKey(key.AsSpan());
    }

    /// <summary>
    /// Removes the specified byte key from the map.
    /// </summary>
    /// <returns><c>true</c> if found and removed; otherwise <c>false</c>.</returns>
    public unsafe bool Remove(ReadOnlySpan<byte> key)
    {
        ThrowIfDisposed();
        if (key.IsEmpty)
        {
            return NativeMethods.expanse_ordered_bytesmap_remove(_handle, null, 0, IntPtr.Zero);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_ordered_bytesmap_remove(_handle, pKey, (nuint)key.Length, IntPtr.Zero);
        }
    }

    /// <summary>
    /// Removes the specified byte array key from the map.
    /// </summary>
    public bool Remove(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Remove(key.AsSpan());
    }

    /// <summary>
    /// Removes the specified byte key from the map, outputting the removed value.
    /// </summary>
    /// <param name="key">The byte key.</param>
    /// <param name="oldValue">Set to 0 before the call; receives the removed value when the key was present.</param>
    public unsafe bool Remove(ReadOnlySpan<byte> key, out ulong oldValue)
    {
        ThrowIfDisposed();
        oldValue = 0;
        if (key.IsEmpty)
        {
            return NativeMethods.expanse_ordered_bytesmap_remove(_handle, null, 0, out oldValue);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_ordered_bytesmap_remove(_handle, pKey, (nuint)key.Length, out oldValue);
        }
    }

    /// <summary>
    /// Removes the specified byte array key from the map, outputting the removed value.
    /// </summary>
    public bool Remove(byte[] key, out ulong oldValue)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Remove(key.AsSpan(), out oldValue);
    }

    /// <summary>
    /// Returns a writable span over the 64-bit value slot of <paramref name="key"/>, or <c>false</c> if the key is absent.
    /// </summary>
    /// <remarks>
    /// The slot is valid only until the next structural mutation of this map and until <see cref="Dispose"/>.
    /// The span does not extend that lifetime: the caller must not use it after either event.
    /// </remarks>
    public unsafe bool TrySlot(ReadOnlySpan<byte> key, out Span<ulong> slot)
    {
        ThrowIfDisposed();
        ulong* ptr;
        if (key.IsEmpty)
        {
            ptr = NativeMethods.expanse_ordered_bytesmap_slot(_handle, null, 0);
        }
        else
        {
            fixed (byte* pKey = key)
            {
                ptr = NativeMethods.expanse_ordered_bytesmap_slot(_handle, pKey, (nuint)key.Length);
            }
        }
        if (ptr == null)
        {
            slot = default;
            return false;
        }
        slot = new Span<ulong>(ptr, 1);
        return true;
    }

    /// <summary>
    /// Returns a writable span over the value slot of a byte array key, or <c>false</c> if the key is absent.
    /// </summary>
    public bool TrySlot(byte[] key, out Span<ulong> slot)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TrySlot(key.AsSpan(), out slot);
    }

    /// <summary>
    /// Inserts the key with value 0 if absent (an existing value is kept) and returns a writable span over its value slot.
    /// </summary>
    /// <remarks>
    /// The slot is valid only until the next structural mutation of this map and until <see cref="Dispose"/>.
    /// </remarks>
    public unsafe Span<ulong> InsertSlot(ReadOnlySpan<byte> key)
    {
        ThrowIfDisposed();
        ulong* ptr;
        if (key.IsEmpty)
        {
            ptr = NativeMethods.expanse_ordered_bytesmap_ins_slot(_handle, null, 0);
        }
        else
        {
            fixed (byte* pKey = key)
            {
                ptr = NativeMethods.expanse_ordered_bytesmap_ins_slot(_handle, pKey, (nuint)key.Length);
            }
        }
        if (ptr == null)
        {
            throw new OutOfMemoryException("Failed allocating slot");
        }
        return new Span<ulong>(ptr, 1);
    }

    /// <summary>
    /// Inserts a byte array key with value 0 if absent and returns a writable span over its value slot.
    /// </summary>
    public Span<ulong> InsertSlot(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return InsertSlot(key.AsSpan());
    }

    /// <summary>
    /// Gets the number of entries in the map (capped at <see cref="int.MaxValue"/>).
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
            return NativeMethods.expanse_ordered_bytesmap_len(_handle);
        }
    }

    /// <summary>
    /// Gets whether the map is empty.
    /// </summary>
    public bool IsEmpty => LongCount == 0;

    /// <summary>
    /// Gets the off-heap bytes used by the live structure of this map.
    /// </summary>
    public nuint MemoryUsed
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_ordered_bytesmap_mem_used(_handle);
        }
    }

    /// <summary>
    /// Gets the off-heap bytes held by this map (live structure plus retained free capacity).
    /// </summary>
    public nuint MemoryHeld
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.expanse_ordered_bytesmap_mem_held(_handle);
        }
    }

    /// <summary>
    /// Releases retained free capacity back to the allocator.
    /// </summary>
    /// <returns>The number of bytes released.</returns>
    public nuint ShrinkToFit()
    {
        ThrowIfDisposed();
        return NativeMethods.expanse_ordered_bytesmap_shrink_to_fit(_handle);
    }

    /// <summary>
    /// Removes all entries from this map, freeing off-heap nodes.
    /// </summary>
    public void Clear()
    {
        ThrowIfDisposed();
        NativeMethods.expanse_ordered_bytesmap_clear(_handle);
    }

    /// <summary>
    /// Returns the entry with the lexicographically smallest key, or <c>null</c> if the map is empty.
    /// </summary>
    public Entry? FirstEntry() => Navigate(NavKind.First, ReadOnlySpan<byte>.Empty);

    /// <summary>
    /// Returns the entry with the lexicographically largest key, or <c>null</c> if the map is empty.
    /// </summary>
    public Entry? LastEntry() => Navigate(NavKind.Last, ReadOnlySpan<byte>.Empty);

    /// <summary>
    /// Returns the entry with the smallest key greater than or equal to <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? CeilingEntry(ReadOnlySpan<byte> key) => Navigate(NavKind.CeilingAt, key);

    /// <summary>
    /// Returns the entry with the smallest key greater than or equal to <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? CeilingEntry(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return CeilingEntry(key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the smallest key strictly greater than <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? HigherEntry(ReadOnlySpan<byte> key) => Navigate(NavKind.HigherThan, key);

    /// <summary>
    /// Returns the entry with the smallest key strictly greater than <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? HigherEntry(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return HigherEntry(key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the largest key less than or equal to <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? FloorEntry(ReadOnlySpan<byte> key) => Navigate(NavKind.FloorAt, key);

    /// <summary>
    /// Returns the entry with the largest key less than or equal to <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? FloorEntry(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return FloorEntry(key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the largest key strictly less than <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? LowerEntry(ReadOnlySpan<byte> key) => Navigate(NavKind.LowerThan, key);

    /// <summary>
    /// Returns the entry with the largest key strictly less than <paramref name="key"/>, or <c>null</c>.
    /// </summary>
    public Entry? LowerEntry(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return LowerEntry(key.AsSpan());
    }

    /// <summary>
    /// Visits all entries in ascending key order.
    /// </summary>
    /// <remarks>
    /// Each step re-navigates from the previous key, so a mutation made between steps is observed by the following steps.
    /// </remarks>
    /// <param name="action">Receives each key and value.</param>
    public void ForEach(Action<byte[], ulong> action)
    {
        ArgumentNullException.ThrowIfNull(action);
        Entry? entry = FirstEntry();
        while (entry is not null)
        {
            action(entry.Key, entry.Value);
            entry = HigherEntry(entry.Key);
        }
    }

    private Entry? Navigate(NavKind kind, ReadOnlySpan<byte> key)
    {
        ThrowIfDisposed();
        int bufLen = DefaultBufLen;
        while (true)
        {
            byte[] buf = new byte[bufLen];
            int status = CallNav(kind, key, buf, out nuint required, out ulong value);
            switch (status)
            {
                case NavOk:
                {
                    byte[] found = buf.AsSpan(0, (int)required).ToArray();
                    return new Entry(found, value);
                }
                case NavNotFound:
                    return null;
                case NavBufferTooSmall:
                    bufLen = GrowBuffer(bufLen, required);
                    break;
                default:
                    throw new InvalidOperationException("Unknown expanse_ordered_bytes_nav_status: " + status);
            }
        }
    }

    private unsafe int CallNav(NavKind kind, ReadOnlySpan<byte> key, byte[] buf, out nuint required, out ulong value)
    {
        fixed (byte* pKey = key)
        fixed (byte* pBuf = buf)
        {
            byte* k = key.IsEmpty ? null : pKey;
            nuint keyLen = (nuint)key.Length;
            nuint bufLen = (nuint)buf.Length;
            switch (kind)
            {
                case NavKind.First:
                    return NativeMethods.expanse_ordered_bytesmap_first(_handle, pBuf, bufLen, out required, out value);
                case NavKind.Last:
                    return NativeMethods.expanse_ordered_bytesmap_last(_handle, pBuf, bufLen, out required, out value);
                case NavKind.CeilingAt:
                    return NativeMethods.expanse_ordered_bytesmap_next_at_or_after(_handle, k, keyLen, pBuf, bufLen, out required, out value);
                case NavKind.HigherThan:
                    return NativeMethods.expanse_ordered_bytesmap_next_after(_handle, k, keyLen, pBuf, bufLen, out required, out value);
                case NavKind.FloorAt:
                    return NativeMethods.expanse_ordered_bytesmap_prev_at_or_before(_handle, k, keyLen, pBuf, bufLen, out required, out value);
                case NavKind.LowerThan:
                    return NativeMethods.expanse_ordered_bytesmap_prev_before(_handle, k, keyLen, pBuf, bufLen, out required, out value);
                default:
                    throw new InvalidOperationException("Unknown navigation kind: " + kind);
            }
        }
    }

    private static int GrowBuffer(int currentLen, nuint required)
    {
        if ((ulong)required > int.MaxValue)
        {
            throw new InvalidOperationException(
                $"Byte key of {(ulong)required} bytes exceeds the maximum managed buffer size ({int.MaxValue} bytes).");
        }
        // The required length is exact; still grow at least geometrically so a map mutated between
        // retries cannot cause one-byte-at-a-time growth.
        int doubled = currentLen <= int.MaxValue / 2 ? currentLen * 2 : int.MaxValue;
        return Math.Max((int)required, doubled);
    }

    /// <summary>
    /// Frees the unmanaged memory allocated by this ordered bytes map.
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
