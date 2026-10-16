using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Expanse.Native;

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_set_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSetHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSetHandle() : base(true) { }

    public SafeExpanseSetHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_set_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_map_t</c> pointer.
/// </summary>
public sealed class SafeExpanseMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseMapHandle() : base(true) { }

    public SafeExpanseMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_map_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_strmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseStrMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseStrMapHandle() : base(true) { }

    public SafeExpanseStrMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_strmap_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseBytesMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseBytesMapHandle() : base(true) { }

    public SafeExpanseBytesMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_bytesmap_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_ordered_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseOrderedBytesMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseOrderedBytesMapHandle() : base(true) { }

    public SafeExpanseOrderedBytesMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_ordered_bytesmap_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>ExpanseBlobMap</c> pointer.
/// </summary>
public sealed class SafeExpanseBlobMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseBlobMapHandle() : base(true) { }

    public SafeExpanseBlobMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_blob_map_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_set_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncSetHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncSetHandle() : base(true) { }

    public SafeExpanseSyncSetHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_set_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_set_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncSetReaderHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncSetReaderHandle() : base(true) { }

    public SafeExpanseSyncSetReaderHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_set_reader_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_map_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncMapHandle() : base(true) { }

    public SafeExpanseSyncMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_map_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_map_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncMapReaderHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncMapReaderHandle() : base(true) { }

    public SafeExpanseSyncMapReaderHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_map_reader_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_strmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncStrMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncStrMapHandle() : base(true) { }

    public SafeExpanseSyncStrMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_strmap_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_strmap_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncStrMapReaderHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncStrMapReaderHandle() : base(true) { }

    public SafeExpanseSyncStrMapReaderHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_strmap_reader_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBytesMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncBytesMapHandle() : base(true) { }

    public SafeExpanseSyncBytesMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_bytesmap_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_bytesmap_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBytesMapReaderHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncBytesMapReaderHandle() : base(true) { }

    public SafeExpanseSyncBytesMapReaderHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_bytesmap_reader_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_ordered_bytesmap_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncOrderedBytesMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncOrderedBytesMapHandle() : base(true) { }

    public SafeExpanseSyncOrderedBytesMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_ordered_bytesmap_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_ordered_bytesmap_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncOrderedBytesMapReaderHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncOrderedBytesMapReaderHandle() : base(true) { }

    public SafeExpanseSyncOrderedBytesMapReaderHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_ordered_bytesmap_reader_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_blob_map_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBlobMapHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncBlobMapHandle() : base(true) { }

    public SafeExpanseSyncBlobMapHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_blob_map_free(handle);
        return true;
    }
}

/// <summary>
/// SafeHandle wrapping an unmanaged <c>expanse_sync_blob_map_reader_t</c> pointer.
/// </summary>
public sealed class SafeExpanseSyncBlobMapReaderHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public SafeExpanseSyncBlobMapReaderHandle() : base(true) { }

    public SafeExpanseSyncBlobMapReaderHandle(IntPtr handle, bool ownsHandle) : base(ownsHandle)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.expanse_sync_blob_map_reader_free(handle);
        return true;
    }
}
