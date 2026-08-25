using System;
using System.Runtime.InteropServices;

namespace Expanse.Native;

/// <summary>
/// Native P/Invoke declarations for libexpanse modern C API.
/// </summary>
public static class NativeMethods
{
    private const string LibName = "expanse";

    static NativeMethods()
    {
        NativeLoader.Initialize();
    }

    #region Library Identity

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_version")]
    public static extern IntPtr expanse_version();

    #endregion

    #region ExpanseSet

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_new")]
    public static extern SafeExpanseSetHandle expanse_set_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_free")]
    public static extern void expanse_set_free(IntPtr set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_insert(SafeExpanseSetHandle set, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_remove(SafeExpanseSetHandle set, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_contains(SafeExpanseSetHandle set, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_len")]
    public static extern ulong expanse_set_len(SafeExpanseSetHandle set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_mem_used")]
    public static extern nuint expanse_set_mem_used(SafeExpanseSetHandle set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_clear")]
    public static extern void expanse_set_clear(SafeExpanseSetHandle set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_first")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_first(SafeExpanseSetHandle set, out ulong key_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_last")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_last(SafeExpanseSetHandle set, out ulong key_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_next_at_or_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_next_at_or_after(SafeExpanseSetHandle set, ulong key, out ulong key_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_next_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_next_after(SafeExpanseSetHandle set, ulong key, out ulong key_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_prev_at_or_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_prev_at_or_before(SafeExpanseSetHandle set, ulong key, out ulong key_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_prev_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_prev_before(SafeExpanseSetHandle set, ulong key, out ulong key_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_count_below")]
    public static extern ulong expanse_set_count_below(SafeExpanseSetHandle set, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_count_range")]
    public static extern ulong expanse_set_count_range(SafeExpanseSetHandle set, ulong lo, ulong hi);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_by_count")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_by_count(SafeExpanseSetHandle set, ulong n, out ulong key_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_contains_batch")]
    public static extern nuint expanse_set_contains_batch(SafeExpanseSetHandle set, [In] ulong[] keys, [Out] bool[] out_present, nuint count);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_contains_batch")]
    public static extern unsafe nuint expanse_set_contains_batch(SafeExpanseSetHandle set, ulong* keys, bool* out_present, nuint count);

    #endregion

    #region ExpanseMap

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_new")]
    public static extern SafeExpanseMapHandle expanse_map_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_free")]
    public static extern void expanse_map_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_insert(SafeExpanseMapHandle map, ulong key, ulong value, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_insert(SafeExpanseMapHandle map, ulong key, ulong value, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_get(SafeExpanseMapHandle map, ulong key, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_get_batch")]
    public static extern nuint expanse_map_get_batch(SafeExpanseMapHandle map, [In] ulong[] keys, [Out] ulong[] out_values, [Out] bool[]? out_found, nuint count);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_get_batch")]
    public static extern unsafe nuint expanse_map_get_batch(SafeExpanseMapHandle map, ulong* keys, ulong* out_values, bool* out_found, nuint count);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_remove(SafeExpanseMapHandle map, ulong key, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_remove(SafeExpanseMapHandle map, ulong key, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_len")]
    public static extern ulong expanse_map_len(SafeExpanseMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_mem_used")]
    public static extern nuint expanse_map_mem_used(SafeExpanseMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_clear")]
    public static extern void expanse_map_clear(SafeExpanseMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_slot")]
    public static extern unsafe ulong* expanse_map_slot(SafeExpanseMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_ins_slot")]
    public static extern unsafe ulong* expanse_map_ins_slot(SafeExpanseMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_first")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_first(SafeExpanseMapHandle map, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_last")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_last(SafeExpanseMapHandle map, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_next_at_or_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_next_at_or_after(SafeExpanseMapHandle map, ulong key, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_next_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_next_after(SafeExpanseMapHandle map, ulong key, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_prev_at_or_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_prev_at_or_before(SafeExpanseMapHandle map, ulong key, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_prev_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_prev_before(SafeExpanseMapHandle map, ulong key, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_count_below")]
    public static extern ulong expanse_map_count_below(SafeExpanseMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_count_range")]
    public static extern ulong expanse_map_count_range(SafeExpanseMapHandle map, ulong lo, ulong hi);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_by_count")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_by_count(SafeExpanseMapHandle map, ulong n, out ulong key_out, out ulong value_out);

    #endregion

    #region ExpanseBytesMap

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_new")]
    public static extern SafeExpanseBytesMapHandle expanse_bytesmap_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_free")]
    public static extern void expanse_bytesmap_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_bytesmap_insert(SafeExpanseBytesMapHandle map, byte* key, nuint len, ulong value, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_bytesmap_insert(SafeExpanseBytesMapHandle map, byte* key, nuint len, ulong value, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_bytesmap_get(SafeExpanseBytesMapHandle map, byte* key, nuint len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_bytesmap_remove(SafeExpanseBytesMapHandle map, byte* key, nuint len, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_bytesmap_remove(SafeExpanseBytesMapHandle map, byte* key, nuint len, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_slot")]
    public static extern unsafe ulong* expanse_bytesmap_slot(SafeExpanseBytesMapHandle map, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_ins_slot")]
    public static extern unsafe ulong* expanse_bytesmap_ins_slot(SafeExpanseBytesMapHandle map, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_len")]
    public static extern ulong expanse_bytesmap_len(SafeExpanseBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_mem_used")]
    public static extern nuint expanse_bytesmap_mem_used(SafeExpanseBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_clear")]
    public static extern void expanse_bytesmap_clear(SafeExpanseBytesMapHandle map);

    #endregion

    #region ExpanseStrMap

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_new")]
    public static extern SafeExpanseStrMapHandle expanse_strmap_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_free")]
    public static extern void expanse_strmap_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_insert(SafeExpanseStrMapHandle map, byte* key, ulong value, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_insert(SafeExpanseStrMapHandle map, byte* key, ulong value, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_get(SafeExpanseStrMapHandle map, byte* key, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_remove(SafeExpanseStrMapHandle map, byte* key, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_remove(SafeExpanseStrMapHandle map, byte* key, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_slot")]
    public static extern unsafe ulong* expanse_strmap_slot(SafeExpanseStrMapHandle map, byte* key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_ins_slot")]
    public static extern unsafe ulong* expanse_strmap_ins_slot(SafeExpanseStrMapHandle map, byte* key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_len")]
    public static extern ulong expanse_strmap_len(SafeExpanseStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_mem_used")]
    public static extern nuint expanse_strmap_mem_used(SafeExpanseStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_clear")]
    public static extern void expanse_strmap_clear(SafeExpanseStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_first")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_first(SafeExpanseStrMapHandle map, byte* key_out, nuint buf_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_last")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_last(SafeExpanseStrMapHandle map, byte* key_out, nuint buf_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_next_at_or_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_next_at_or_after(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_next_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_next_after(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_prev_at_or_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_prev_at_or_before(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_prev_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_prev_before(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, out ulong value_out);

    // Truncation-aware navigation (_ex): return an int status
    // (0 = OK, 1 = NOT_FOUND, 2 = BUFFER_TOO_SMALL) and report the needed
    // buffer size through required_len.
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_first_ex")]
    public static extern unsafe int expanse_strmap_first_ex(SafeExpanseStrMapHandle map, byte* key_out, nuint buf_len, nuint* required_len, ulong* value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_last_ex")]
    public static extern unsafe int expanse_strmap_last_ex(SafeExpanseStrMapHandle map, byte* key_out, nuint buf_len, nuint* required_len, ulong* value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_next_at_or_after_ex")]
    public static extern unsafe int expanse_strmap_next_at_or_after_ex(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, nuint* required_len, ulong* value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_next_after_ex")]
    public static extern unsafe int expanse_strmap_next_after_ex(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, nuint* required_len, ulong* value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_prev_at_or_before_ex")]
    public static extern unsafe int expanse_strmap_prev_at_or_before_ex(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, nuint* required_len, ulong* value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_prev_before_ex")]
    public static extern unsafe int expanse_strmap_prev_before_ex(SafeExpanseStrMapHandle map, byte* key, byte* key_out, nuint buf_len, nuint* required_len, ulong* value_out);

    #endregion

    #region ExpanseBlobMap

    [StructLayout(LayoutKind.Sequential)]
    public struct NativeBlobView
    {
        public IntPtr Ptr;
        public nuint Len;
        public uint HotMeta;
        [MarshalAs(UnmanagedType.I1)]
        public bool IsInline;
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public delegate bool ExpansePredicateCallback(ulong key, uint hotMeta, IntPtr userCtx);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public delegate bool ExpanseScanCallback(ulong key, NativeBlobView view, IntPtr userCtx);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_new")]
    public static extern SafeExpanseBlobMapHandle expanse_blob_map_new(nuint chunkSize);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_free")]
    public static extern void expanse_blob_map_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_blob_map_insert(SafeExpanseBlobMapHandle map, ulong key, byte* data, nuint len, uint hotMeta);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_remove(SafeExpanseBlobMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_get(SafeExpanseBlobMapHandle map, ulong key, out NativeBlobView outView);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_scan_filtered")]
    public static extern nuint expanse_blob_map_scan_filtered(
        SafeExpanseBlobMapHandle map,
        ulong startKey,
        ulong endKey,
        ExpansePredicateCallback? predicate,
        ExpanseScanCallback? callback,
        IntPtr userCtx);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_compact")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_compact(SafeExpanseBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_len")]
    public static extern ulong expanse_blob_map_len(SafeExpanseBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_mem_used")]
    public static extern nuint expanse_blob_map_mem_used(SafeExpanseBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_clear")]
    public static extern void expanse_blob_map_clear(SafeExpanseBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_contains_key")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_contains_key(SafeExpanseBlobMapHandle map, ulong key);

    #endregion

    #region Concurrent (Sync) Types

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_new")]
    public static extern SafeExpanseSyncSetHandle expanse_sync_set_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_free")]
    public static extern void expanse_sync_set_free(IntPtr set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_set_insert(SafeExpanseSyncSetHandle set, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_set_remove(SafeExpanseSyncSetHandle set, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_set_contains(SafeExpanseSyncSetHandle set, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_len")]
    public static extern ulong expanse_sync_set_len(SafeExpanseSyncSetHandle set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_reader_new")]
    public static extern SafeExpanseSyncSetReaderHandle expanse_sync_set_reader_new(SafeExpanseSyncSetHandle set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_reader_free")]
    public static extern void expanse_sync_set_reader_free(IntPtr reader);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_reader_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_set_reader_contains(SafeExpanseSyncSetReaderHandle reader, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_new")]
    public static extern SafeExpanseSyncMapHandle expanse_sync_map_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_free")]
    public static extern void expanse_sync_map_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_insert(SafeExpanseSyncMapHandle map, ulong key, ulong value, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_insert(SafeExpanseSyncMapHandle map, ulong key, ulong value, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_get(SafeExpanseSyncMapHandle map, ulong key, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_remove(SafeExpanseSyncMapHandle map, ulong key, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_remove(SafeExpanseSyncMapHandle map, ulong key, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_len")]
    public static extern ulong expanse_sync_map_len(SafeExpanseSyncMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_new")]
    public static extern SafeExpanseSyncMapReaderHandle expanse_sync_map_reader_new(SafeExpanseSyncMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_free")]
    public static extern void expanse_sync_map_reader_free(IntPtr reader);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_get(SafeExpanseSyncMapReaderHandle reader, ulong key, out ulong value_out);

    #endregion
}
