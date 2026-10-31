using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Expanse.Native;

/// <summary>
/// Shared base of the libexpanse SafeHandles: an owned native pointer, invalid
/// when zero or minus one, released through the container's <c>*_free</c>
/// function. Only this assembly can derive from it.
/// </summary>
public abstract class ExpanseSafeHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    /// <summary>An owning handle, set by the P/Invoke marshaller.</summary>
    private protected ExpanseSafeHandle() : base(true) { }

    /// <summary>Wraps an existing native pointer.</summary>
    private protected ExpanseSafeHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    /// <summary>Calls the container's <c>*_free</c> function on the pointer.</summary>
    private protected abstract void Free(IntPtr handle);

    /// <inheritdoc />
    protected sealed override bool ReleaseHandle()
    {
        Free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_set_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSetHandle : ExpanseSafeHandle
{
    public SafeExpanseSetHandle() { }

    public SafeExpanseSetHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_set_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_map_t</c> pointer.
/// </summary>
public sealed class SafeExpanseMapHandle : ExpanseSafeHandle
{
    public SafeExpanseMapHandle() { }

    public SafeExpanseMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_map_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_strmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseStrMapHandle : ExpanseSafeHandle
{
    public SafeExpanseStrMapHandle() { }

    public SafeExpanseStrMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_strmap_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseBytesMapHandle : ExpanseSafeHandle
{
    public SafeExpanseBytesMapHandle() { }

    public SafeExpanseBytesMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_bytesmap_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_ordered_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseOrderedBytesMapHandle : ExpanseSafeHandle
{
    public SafeExpanseOrderedBytesMapHandle() { }

    public SafeExpanseOrderedBytesMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_ordered_bytesmap_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>ExpanseBlobMap</c> pointer.
/// </summary>
public sealed class SafeExpanseBlobMapHandle : ExpanseSafeHandle
{
    public SafeExpanseBlobMapHandle() { }

    public SafeExpanseBlobMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_blob_map_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_set_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncSetHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncSetHandle() { }

    public SafeExpanseSyncSetHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_set_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_set_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncSetReaderHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncSetReaderHandle() { }

    public SafeExpanseSyncSetReaderHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_set_reader_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_map_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncMapHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncMapHandle() { }

    public SafeExpanseSyncMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_map_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_map_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncMapReaderHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncMapReaderHandle() { }

    public SafeExpanseSyncMapReaderHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_map_reader_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_strmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncStrMapHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncStrMapHandle() { }

    public SafeExpanseSyncStrMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_strmap_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_strmap_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncStrMapReaderHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncStrMapReaderHandle() { }

    public SafeExpanseSyncStrMapReaderHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_strmap_reader_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBytesMapHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncBytesMapHandle() { }

    public SafeExpanseSyncBytesMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_bytesmap_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_bytesmap_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBytesMapReaderHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncBytesMapReaderHandle() { }

    public SafeExpanseSyncBytesMapReaderHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_bytesmap_reader_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_ordered_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncOrderedBytesMapHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncOrderedBytesMapHandle() { }

    public SafeExpanseSyncOrderedBytesMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_ordered_bytesmap_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_ordered_bytesmap_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncOrderedBytesMapReaderHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncOrderedBytesMapReaderHandle() { }

    public SafeExpanseSyncOrderedBytesMapReaderHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_ordered_bytesmap_reader_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_blob_map_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBlobMapHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncBlobMapHandle() { }

    public SafeExpanseSyncBlobMapHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_blob_map_free(handle);
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_blob_map_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBlobMapReaderHandle : ExpanseSafeHandle
{
    public SafeExpanseSyncBlobMapReaderHandle() { }

    public SafeExpanseSyncBlobMapReaderHandle(IntPtr handle, bool ownsHandle) : base(handle, ownsHandle) { }

    private protected override void Free(IntPtr handle) => NativeMethods.expanse_sync_blob_map_reader_free(handle);
}
