using System;
using System.Buffers;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// High-performance off-heap ordered string trie (<c>string</c> / <c>ReadOnlySpan&lt;char&gt;</c> to <c>ulong</c>) (cf. JudySL).
/// Backed directly by native <c>expanse_strmap_t</c> with bidirectional lexicographic navigation.
/// </summary>
public sealed class ExpanseStrMap : IDisposable, IEnumerable<KeyValuePair<string, ulong>>, IReadOnlyCollection<KeyValuePair<string, ulong>>
{
    private SafeExpanseStrMapHandle _handle;
    private bool _disposed;
    private const int InitialNavBufferLen = 4096;

    /// <summary>
    /// Creates a new empty off-heap <see cref="ExpanseStrMap"/>.
    /// </summary>
    public ExpanseStrMap()
    {
        _handle = NativeMethods.expanse_strmap_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryError("Failed to allocate native expanse_strmap_t");
        }
    }

    /// <summary>
    /// Gets the underlying native <see cref="SafeExpanseStrMapHandle"/>.
    /// </summary>
    public SafeExpanseStrMapHandle Handle
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
    /// Gets or sets the value associated with the specified string key.
    /// </summary>
    /// <param name="key">The string key.</param>
    /// <returns>The value associated with the key.</returns>
    /// <exception cref="KeyNotFoundException">The key was not found when getting.</exception>
    public ulong this[string key]
    {
        get
        {
            ArgumentNullException.ThrowIfNull(key);
            if (TryGet(key, out ulong value))
            {
                return value;
            }
            throw new KeyNotFoundException($"Key '{key}' was not found in ExpanseStrMap.");
        }
        set
        {
            ArgumentNullException.ThrowIfNull(key);
            Set(key, value);
        }
    }

    /// <summary>
    /// Gets or sets the value associated with the specified character span key.
    /// </summary>
    /// <param name="key">The character span key.</param>
    /// <returns>The value associated with the key.</returns>
    public ulong this[ReadOnlySpan<char> key]
    {
        get
        {
            if (TryGet(key, out ulong value))
            {
                return value;
            }
            throw new KeyNotFoundException($"Key '{key.ToString()}' was not found in ExpanseStrMap.");
        }
        set => Set(key, value);
    }

    /// <summary>
    /// Stores the string key-value pair.
    /// </summary>
    /// <param name="key">The string key.</param>
    /// <param name="value">The 64-bit value.</param>
    public void Set(string key, ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        Set(key.AsSpan(), value);
    }

    /// <summary>
    /// Stores the character span key-value pair.
    /// </summary>
    /// <param name="key">The character span key.</param>
    /// <param name="value">The 64-bit value.</param>
    public unsafe void Set(ReadOnlySpan<char> key, ulong value)
    {
        ThrowIfDisposed();
        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        byte[]? rented = null;
        Span<byte> utf8Buf = maxBytes <= 512 ? stackalloc byte[512] : (rented = ArrayPool<byte>.Shared.Rent(maxBytes));

        try
        {
            int written = Encoding.UTF8.GetBytes(key, utf8Buf);
            utf8Buf[written] = 0; // null-terminated

            fixed (byte* pKey = utf8Buf)
            {
                NativeMethods.expanse_strmap_insert(_handle, pKey, value, IntPtr.Zero);
            }
        }
        finally
        {
            if (rented != null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the string key.
    /// </summary>
    public bool TryGet(string key, out ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TryGet(key.AsSpan(), out value);
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the character span key.
    /// </summary>
    public unsafe bool TryGet(ReadOnlySpan<char> key, out ulong value)
    {
        ThrowIfDisposed();
        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        byte[]? rented = null;
        Span<byte> utf8Buf = maxBytes <= 512 ? stackalloc byte[512] : (rented = ArrayPool<byte>.Shared.Rent(maxBytes));

        try
        {
            int written = Encoding.UTF8.GetBytes(key, utf8Buf);
            utf8Buf[written] = 0;

            fixed (byte* pKey = utf8Buf)
            {
                return NativeMethods.expanse_strmap_get(_handle, pKey, out value);
            }
        }
        finally
        {
            if (rented != null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }

    /// <summary>
    /// Removes the specified string key from the map.
    /// </summary>
    public bool Remove(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Remove(key.AsSpan());
    }

    /// <summary>
    /// Removes the specified character span key from the map.
    /// </summary>
    public unsafe bool Remove(ReadOnlySpan<char> key)
    {
        ThrowIfDisposed();
        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        byte[]? rented = null;
        Span<byte> utf8Buf = maxBytes <= 512 ? stackalloc byte[512] : (rented = ArrayPool<byte>.Shared.Rent(maxBytes));

        try
        {
            int written = Encoding.UTF8.GetBytes(key, utf8Buf);
            utf8Buf[written] = 0;

            fixed (byte* pKey = utf8Buf)
            {
                return NativeMethods.expanse_strmap_remove(_handle, pKey, IntPtr.Zero);
            }
        }
        finally
        {
            if (rented != null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }

    /// <summary>
    /// Checks whether the map contains the specified string key.
    /// </summary>
    public bool ContainsKey(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return ContainsKey(key.AsSpan());
    }

    /// <summary>
    /// Checks whether the map contains the specified character span key.
    /// </summary>
    public bool ContainsKey(ReadOnlySpan<char> key) => TryGet(key, out _);

    /// <summary>
    /// Gets the number of entries in the string map (capped at <see cref="int.MaxValue"/>).
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
            return NativeMethods.expanse_strmap_len(_handle);
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
            return NativeMethods.expanse_strmap_mem_used(_handle);
        }
    }

    /// <summary>
    /// Removes all entries from this map, freeing off-heap nodes.
    /// </summary>
    public void Clear()
    {
        ThrowIfDisposed();
        NativeMethods.expanse_strmap_clear(_handle);
    }

    /// <summary>
    /// Returns the lexicographically smallest entry in the map, or <c>null</c> if empty.
    /// </summary>
    public unsafe KeyValuePair<string, ulong>? First()
    {
        ThrowIfDisposed();
        byte[] buf = new byte[InitialNavBufferLen];
        fixed (byte* pBuf = buf)
        {
            if (NativeMethods.expanse_strmap_first(_handle, pBuf, (nuint)buf.Length, out ulong value))
            {
                string key = Marshal.PtrToStringUTF8((IntPtr)pBuf) ?? string.Empty;
                return new KeyValuePair<string, ulong>(key, value);
            }
        }
        return null;
    }

    /// <summary>
    /// Returns the lexicographically largest entry in the map, or <c>null</c> if empty.
    /// </summary>
    public unsafe KeyValuePair<string, ulong>? Last()
    {
        ThrowIfDisposed();
        byte[] buf = new byte[InitialNavBufferLen];
        fixed (byte* pBuf = buf)
        {
            if (NativeMethods.expanse_strmap_last(_handle, pBuf, (nuint)buf.Length, out ulong value))
            {
                string key = Marshal.PtrToStringUTF8((IntPtr)pBuf) ?? string.Empty;
                return new KeyValuePair<string, ulong>(key, value);
            }
        }
        return null;
    }

    /// <summary>
    /// Returns the entry with the lexicographically smallest key strictly greater than <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? Next(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Next(key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the lexicographically smallest key strictly greater than <paramref name="key"/>.
    /// </summary>
    public unsafe KeyValuePair<string, ulong>? Next(ReadOnlySpan<char> key)
    {
        ThrowIfDisposed();
        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        byte[]? rented = null;
        Span<byte> utf8Key = maxBytes <= 512 ? stackalloc byte[512] : (rented = ArrayPool<byte>.Shared.Rent(maxBytes));

        try
        {
            int written = Encoding.UTF8.GetBytes(key, utf8Key);
            utf8Key[written] = 0;

            byte[] outBuf = new byte[InitialNavBufferLen];
            fixed (byte* pKey = utf8Key)
            fixed (byte* pOut = outBuf)
            {
                if (NativeMethods.expanse_strmap_next_after(_handle, pKey, pOut, (nuint)outBuf.Length, out ulong value))
                {
                    string outKeyStr = Marshal.PtrToStringUTF8((IntPtr)pOut) ?? string.Empty;
                    return new KeyValuePair<string, ulong>(outKeyStr, value);
                }
            }
            return null;
        }
        finally
        {
            if (rented != null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }

    /// <summary>
    /// Returns the entry with the lexicographically smallest key greater than or equal to <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? NextAtOrAfter(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return NextAtOrAfter(key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the lexicographically smallest key greater than or equal to <paramref name="key"/>.
    /// </summary>
    public unsafe KeyValuePair<string, ulong>? NextAtOrAfter(ReadOnlySpan<char> key)
    {
        ThrowIfDisposed();
        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        byte[]? rented = null;
        Span<byte> utf8Key = maxBytes <= 512 ? stackalloc byte[512] : (rented = ArrayPool<byte>.Shared.Rent(maxBytes));

        try
        {
            int written = Encoding.UTF8.GetBytes(key, utf8Key);
            utf8Key[written] = 0;

            byte[] outBuf = new byte[InitialNavBufferLen];
            fixed (byte* pKey = utf8Key)
            fixed (byte* pOut = outBuf)
            {
                if (NativeMethods.expanse_strmap_next_at_or_after(_handle, pKey, pOut, (nuint)outBuf.Length, out ulong value))
                {
                    string outKeyStr = Marshal.PtrToStringUTF8((IntPtr)pOut) ?? string.Empty;
                    return new KeyValuePair<string, ulong>(outKeyStr, value);
                }
            }
            return null;
        }
        finally
        {
            if (rented != null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }

    /// <summary>
    /// Returns the entry with the lexicographically largest key strictly less than <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? Prev(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Prev(key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the lexicographically largest key strictly less than <paramref name="key"/>.
    /// </summary>
    public unsafe KeyValuePair<string, ulong>? Prev(ReadOnlySpan<char> key)
    {
        ThrowIfDisposed();
        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        byte[]? rented = null;
        Span<byte> utf8Key = maxBytes <= 512 ? stackalloc byte[512] : (rented = ArrayPool<byte>.Shared.Rent(maxBytes));

        try
        {
            int written = Encoding.UTF8.GetBytes(key, utf8Key);
            utf8Key[written] = 0;

            byte[] outBuf = new byte[InitialNavBufferLen];
            fixed (byte* pKey = utf8Key)
            fixed (byte* pOut = outBuf)
            {
                if (NativeMethods.expanse_strmap_prev_before(_handle, pKey, pOut, (nuint)outBuf.Length, out ulong value))
                {
                    string outKeyStr = Marshal.PtrToStringUTF8((IntPtr)pOut) ?? string.Empty;
                    return new KeyValuePair<string, ulong>(outKeyStr, value);
                }
            }
            return null;
        }
        finally
        {
            if (rented != null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }

    /// <summary>
    /// Returns the entry with the lexicographically largest key less than or equal to <paramref name="key"/>.
    /// </summary>
    public KeyValuePair<string, ulong>? PrevAtOrBefore(string key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return PrevAtOrBefore(key.AsSpan());
    }

    /// <summary>
    /// Returns the entry with the lexicographically largest key less than or equal to <paramref name="key"/>.
    /// </summary>
    public unsafe KeyValuePair<string, ulong>? PrevAtOrBefore(ReadOnlySpan<char> key)
    {
        ThrowIfDisposed();
        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        byte[]? rented = null;
        Span<byte> utf8Key = maxBytes <= 512 ? stackalloc byte[512] : (rented = ArrayPool<byte>.Shared.Rent(maxBytes));

        try
        {
            int written = Encoding.UTF8.GetBytes(key, utf8Key);
            utf8Key[written] = 0;

            byte[] outBuf = new byte[InitialNavBufferLen];
            fixed (byte* pKey = utf8Key)
            fixed (byte* pOut = outBuf)
            {
                if (NativeMethods.expanse_strmap_prev_at_or_before(_handle, pKey, pOut, (nuint)outBuf.Length, out ulong value))
                {
                    string outKeyStr = Marshal.PtrToStringUTF8((IntPtr)pOut) ?? string.Empty;
                    return new KeyValuePair<string, ulong>(outKeyStr, value);
                }
            }
            return null;
        }
        finally
        {
            if (rented != null)
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }
    }

    /// <summary>
    /// Returns an enumerator that iterates through the string map in lexicographical order.
    /// </summary>
    public IEnumerator<KeyValuePair<string, ulong>> GetEnumerator()
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
    /// Frees the unmanaged memory allocated by this string map.
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
