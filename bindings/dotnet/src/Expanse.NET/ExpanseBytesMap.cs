using System;
using System.Collections.Generic;
using Expanse.Native;

namespace Expanse;

/// <summary>
/// High-performance off-heap binary-safe byte string map (<c>ReadOnlySpan&lt;byte&gt;</c> to <c>ulong</c>) (cf. JudyHS).
/// Handles arbitrary binary keys including embedded NUL (<c>0x00</c>) bytes with zero GC heap pressure.
/// </summary>
public sealed class ExpanseBytesMap : IDisposable
{
    private SafeExpanseBytesMapHandle _handle;
    private bool _disposed;

    /// <summary>
    /// Creates a new empty off-heap <see cref="ExpanseBytesMap"/>.
    /// </summary>
    public ExpanseBytesMap()
    {
        _handle = NativeMethods.expanse_bytesmap_new();
        if (_handle.IsInvalid)
        {
            throw new OutOfMemoryException("Failed to allocate native expanse_bytesmap_t");
        }
    }

    /// <summary>
    /// Gets the underlying native <see cref="SafeExpanseBytesMapHandle"/>.
    /// </summary>
    public SafeExpanseBytesMapHandle Handle
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
    public ulong this[ReadOnlySpan<byte> key]
    {
        get
        {
            if (TryGet(key, out ulong value))
            {
                return value;
            }
            throw new KeyNotFoundException("Key was not found in ExpanseBytesMap.");
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
    /// <param name="key">The binary key payload.</param>
    /// <param name="value">The 64-bit value.</param>
    public unsafe void Set(ReadOnlySpan<byte> key, ulong value)
    {
        ThrowIfDisposed();
        if (key.Length == 0)
        {
            NativeMethods.expanse_bytesmap_insert(_handle, null, 0, value, IntPtr.Zero);
            return;
        }
        fixed (byte* pKey = key)
        {
            NativeMethods.expanse_bytesmap_insert(_handle, pKey, (nuint)key.Length, value, IntPtr.Zero);
        }
    }

    /// <summary>
    /// Stores the key-value pair with byte array.
    /// </summary>
    public void Set(byte[] key, ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        Set(key.AsSpan(), value);
    }

    /// <summary>
    /// Stores key -> value, returning <c>true</c> if newly inserted or <c>false</c> if replaced.
    /// </summary>
    public unsafe bool Insert(ReadOnlySpan<byte> key, ulong value, out ulong oldValue)
    {
        ThrowIfDisposed();
        if (key.Length == 0)
        {
            return NativeMethods.expanse_bytesmap_insert(_handle, null, 0, value, out oldValue);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_bytesmap_insert(_handle, pKey, (nuint)key.Length, value, out oldValue);
        }
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the specified binary key.
    /// </summary>
    /// <param name="key">The binary key payload.</param>
    /// <param name="value">When found, contains the associated value.</param>
    /// <returns><c>true</c> if present; otherwise <c>false</c>.</returns>
    public unsafe bool TryGet(ReadOnlySpan<byte> key, out ulong value)
    {
        ThrowIfDisposed();
        if (key.Length == 0)
        {
            return NativeMethods.expanse_bytesmap_get(_handle, null, 0, out value);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_bytesmap_get(_handle, pKey, (nuint)key.Length, out value);
        }
    }

    /// <summary>
    /// Attempts to retrieve the value associated with the specified byte array.
    /// </summary>
    public bool TryGet(byte[] key, out ulong value)
    {
        ArgumentNullException.ThrowIfNull(key);
        return TryGet(key.AsSpan(), out value);
    }

    /// <summary>
    /// Removes the specified binary key from the map.
    /// </summary>
    /// <param name="key">The binary key payload.</param>
    /// <returns><c>true</c> if found and removed; otherwise <c>false</c>.</returns>
    public unsafe bool Remove(ReadOnlySpan<byte> key)
    {
        ThrowIfDisposed();
        if (key.Length == 0)
        {
            return NativeMethods.expanse_bytesmap_remove(_handle, null, 0, IntPtr.Zero);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_bytesmap_remove(_handle, pKey, (nuint)key.Length, IntPtr.Zero);
        }
    }

    /// <summary>
    /// Removes the specified binary key from the map with byte array.
    /// </summary>
    public bool Remove(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return Remove(key.AsSpan());
    }

    /// <summary>
    /// Removes the specified binary key from the map, outputting the old value.
    /// </summary>
    public unsafe bool Remove(ReadOnlySpan<byte> key, out ulong oldValue)
    {
        ThrowIfDisposed();
        if (key.Length == 0)
        {
            return NativeMethods.expanse_bytesmap_remove(_handle, null, 0, out oldValue);
        }
        fixed (byte* pKey = key)
        {
            return NativeMethods.expanse_bytesmap_remove(_handle, pKey, (nuint)key.Length, out oldValue);
        }
    }

    /// <summary>
    /// Checks whether the map contains the specified binary key.
    /// </summary>
    public bool ContainsKey(ReadOnlySpan<byte> key) => TryGet(key, out _);

    /// <summary>
    /// Checks whether the map contains the specified byte array key.
    /// </summary>
    public bool ContainsKey(byte[] key)
    {
        ArgumentNullException.ThrowIfNull(key);
        return ContainsKey(key.AsSpan());
    }

    /// <summary>
    /// Gets the number of entries in the bytes map (capped at <see cref="int.MaxValue"/>).
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
            return NativeMethods.expanse_bytesmap_len(_handle);
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
            return NativeMethods.expanse_bytesmap_mem_used(_handle);
        }
    }

    /// <summary>
    /// Removes all entries from this map, freeing off-heap nodes.
    /// </summary>
    public void Clear()
    {
        ThrowIfDisposed();
        NativeMethods.expanse_bytesmap_clear(_handle);
    }

    /// <summary>
    /// Frees the unmanaged memory allocated by this bytes map.
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
