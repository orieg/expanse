using System;
using System.Buffers;
using System.Text;

namespace Expanse.Native;

/// <summary>
/// A NUL-terminated UTF-8 copy of a string key, in caller-stack or pooled storage. The native shared
/// string map takes keys as C strings, so a key with an embedded NUL cannot be represented: it is
/// rejected here instead of being silently truncated at the NUL.
/// </summary>
internal ref struct Utf8CKey
{
    private byte[]? _rented;

    /// <summary>The key's UTF-8 bytes followed by one NUL byte.</summary>
    public readonly Span<byte> Bytes;

    public Utf8CKey(ReadOnlySpan<char> key, Span<byte> stackStorage)
    {
        if (key.IndexOf('\0') >= 0)
        {
            throw new ArgumentException(
                "A string key must not contain an embedded NUL character; use the bytes map for such keys.", "key");
        }

        int maxBytes = Encoding.UTF8.GetMaxByteCount(key.Length) + 1;
        _rented = null;
        Span<byte> storage = stackStorage;
        if (maxBytes > stackStorage.Length)
        {
            _rented = ArrayPool<byte>.Shared.Rent(maxBytes);
            storage = _rented;
        }

        int written = Encoding.UTF8.GetBytes(key, storage);
        storage[written] = 0;
        Bytes = storage.Slice(0, written + 1);
    }

    public void Dispose()
    {
        if (_rented != null)
        {
            ArrayPool<byte>.Shared.Return(_rented);
            _rented = null;
        }
    }
}

/// <summary>
/// An output buffer that starts in caller-stack storage and moves to pooled storage when the native side
/// reports that it needs more room.
/// </summary>
internal ref struct ScratchBuffer
{
    private byte[]? _rented;

    /// <summary>The current buffer. Pin it with <c>fixed</c> for one native call only.</summary>
    public Span<byte> Span;

    public ScratchBuffer(Span<byte> stackStorage)
    {
        _rented = null;
        Span = stackStorage;
    }

    /// <summary>Replaces the buffer with one of at least <paramref name="required"/> bytes.</summary>
    public void Grow(nuint required)
    {
        if ((ulong)required > (ulong)Array.MaxLength)
        {
            throw new InvalidOperationException(
                $"A value of {(ulong)required} bytes exceeds the maximum managed buffer size ({Array.MaxLength}).");
        }

        if (_rented != null)
        {
            ArrayPool<byte>.Shared.Return(_rented);
            _rented = null;
        }

        // The required length is exact for the read that reported it; a writer may change the value
        // before the retry, which then reports its own size and grows again.
        _rented = ArrayPool<byte>.Shared.Rent((int)Math.Max((nuint)1, required));
        Span = _rented;
    }

    public void Dispose()
    {
        if (_rented != null)
        {
            ArrayPool<byte>.Shared.Return(_rented);
            _rented = null;
        }
    }
}
