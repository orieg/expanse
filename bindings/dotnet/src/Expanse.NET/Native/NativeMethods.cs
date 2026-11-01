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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_mem_held")]
    public static extern nuint expanse_set_mem_held(SafeExpanseSetHandle set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_shrink_to_fit")]
    public static extern nuint expanse_set_shrink_to_fit(SafeExpanseSetHandle set);

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
    public static extern unsafe nuint expanse_set_contains_batch(SafeExpanseSetHandle set, ulong* keys, byte* out_present, nuint count);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_set_validate")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_set_validate(SafeExpanseSetHandle set);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_contains(SafeExpanseMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_get_batch")]
    public static extern unsafe nuint expanse_map_get_batch(SafeExpanseMapHandle map, ulong* keys, ulong* out_values, byte* out_found, nuint count);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_mem_held")]
    public static extern nuint expanse_map_mem_held(SafeExpanseMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_shrink_to_fit")]
    public static extern nuint expanse_map_shrink_to_fit(SafeExpanseMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_clear")]
    public static extern void expanse_map_clear(SafeExpanseMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_validate")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_map_validate(SafeExpanseMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_map_validate_explain")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_map_validate_explain(SafeExpanseMapHandle map, byte* buf, nuint bufLen);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_bytesmap_contains(SafeExpanseBytesMapHandle map, byte* key, nuint len);

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

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public unsafe delegate bool ExpanseBytesMapIterCallback(byte* key, nuint keyLen, ulong val, IntPtr userCtx);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_bytesmap_for_each")]
    public static extern nuint expanse_bytesmap_for_each(SafeExpanseBytesMapHandle map, ExpanseBytesMapIterCallback cb, IntPtr userCtx);

    #endregion

    #region ExpanseOrderedBytesMap

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_new")]
    public static extern SafeExpanseOrderedBytesMapHandle expanse_ordered_bytesmap_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_free")]
    public static extern void expanse_ordered_bytesmap_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_ordered_bytesmap_insert(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, ulong value, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_ordered_bytesmap_insert(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, ulong value, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_ordered_bytesmap_get(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_ordered_bytesmap_contains(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_ordered_bytesmap_remove(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, out ulong old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_ordered_bytesmap_remove(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, IntPtr old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_slot")]
    public static extern unsafe ulong* expanse_ordered_bytesmap_slot(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_ins_slot")]
    public static extern unsafe ulong* expanse_ordered_bytesmap_ins_slot(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_len")]
    public static extern ulong expanse_ordered_bytesmap_len(SafeExpanseOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_mem_used")]
    public static extern nuint expanse_ordered_bytesmap_mem_used(SafeExpanseOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_mem_held")]
    public static extern nuint expanse_ordered_bytesmap_mem_held(SafeExpanseOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_shrink_to_fit")]
    public static extern nuint expanse_ordered_bytesmap_shrink_to_fit(SafeExpanseOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_clear")]
    public static extern void expanse_ordered_bytesmap_clear(SafeExpanseOrderedBytesMapHandle map);

    // Navigation returns expanse_ordered_bytes_nav_status (0 OK, 1 NOT_FOUND, 2 BUFFER_TOO_SMALL).
    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_first")]
    public static extern unsafe int expanse_ordered_bytesmap_first(SafeExpanseOrderedBytesMapHandle map, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_last")]
    public static extern unsafe int expanse_ordered_bytesmap_last(SafeExpanseOrderedBytesMapHandle map, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_next_at_or_after")]
    public static extern unsafe int expanse_ordered_bytesmap_next_at_or_after(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_next_after")]
    public static extern unsafe int expanse_ordered_bytesmap_next_after(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_prev_at_or_before")]
    public static extern unsafe int expanse_ordered_bytesmap_prev_at_or_before(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_ordered_bytesmap_prev_before")]
    public static extern unsafe int expanse_ordered_bytesmap_prev_before(SafeExpanseOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_strmap_contains(SafeExpanseStrMapHandle map, byte* key);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_mem_held")]
    public static extern nuint expanse_strmap_mem_held(SafeExpanseStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_strmap_shrink_to_fit")]
    public static extern nuint expanse_strmap_shrink_to_fit(SafeExpanseStrMapHandle map);

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
    public unsafe struct NativeBlobView
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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_new_with_capacity")]
    public static extern SafeExpanseBlobMapHandle expanse_blob_map_new_with_capacity(nuint chunkSize, nuint maxCapacity);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_set_reclaim_at_cap")]
    public static extern void expanse_blob_map_set_reclaim_at_cap(SafeExpanseBlobMapHandle map, [MarshalAs(UnmanagedType.I1)] bool on);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_insert_ex")]
    public static extern unsafe int expanse_blob_map_insert_ex(SafeExpanseBlobMapHandle map, ulong key, byte* data, nuint len, uint hotMeta);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_arena_stats")]
    public static extern int expanse_blob_map_arena_stats(SafeExpanseBlobMapHandle map, out ExpanseBlobArenaStats stats, nuint statsSize);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_blob_map_insert(SafeExpanseBlobMapHandle map, ulong key, byte* data, nuint len, uint hotMeta);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_remove(SafeExpanseBlobMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_get(SafeExpanseBlobMapHandle map, ulong key, out NativeBlobView outView);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_get_into")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_get_into(
        SafeExpanseBlobMapHandle map,
        ulong key,
        byte[] buf,
        nuint bufLen,
        out nuint outLen,
        out uint outMeta);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_blob_map_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_blob_map_contains(SafeExpanseBlobMapHandle map, ulong key);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_mem_held")]
    public static extern nuint expanse_sync_set_mem_held(SafeExpanseSyncSetHandle set);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_set_shrink_to_fit")]
    public static extern nuint expanse_sync_set_shrink_to_fit(SafeExpanseSyncSetHandle set);

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

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_compare_exchange")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_compare_exchange(
        SafeExpanseSyncMapHandle map,
        ulong key,
        [MarshalAs(UnmanagedType.I1)] bool expected_present,
        ulong expected_value,
        [MarshalAs(UnmanagedType.I1)] bool new_present,
        ulong new_value,
        out byte observed_present_out,
        out ulong observed_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_get_or_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_get_or_insert(SafeExpanseSyncMapHandle map, ulong key, ulong value, out ulong existing_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_len")]
    public static extern ulong expanse_sync_map_len(SafeExpanseSyncMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_mem_used")]
    public static extern nuint expanse_sync_map_mem_used(SafeExpanseSyncMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_mem_held")]
    public static extern nuint expanse_sync_map_mem_held(SafeExpanseSyncMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_shrink_to_fit")]
    public static extern nuint expanse_sync_map_shrink_to_fit(SafeExpanseSyncMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_new")]
    public static extern SafeExpanseSyncMapReaderHandle expanse_sync_map_reader_new(SafeExpanseSyncMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_free")]
    public static extern void expanse_sync_map_reader_free(IntPtr reader);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_get(SafeExpanseSyncMapReaderHandle reader, ulong key, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_first")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_first(SafeExpanseSyncMapReaderHandle reader, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_last")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_last(SafeExpanseSyncMapReaderHandle reader, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_next_at_or_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_next_at_or_after(SafeExpanseSyncMapReaderHandle reader, ulong key, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_next_after")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_next_after(SafeExpanseSyncMapReaderHandle reader, ulong key, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_prev_at_or_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_prev_at_or_before(SafeExpanseSyncMapReaderHandle reader, ulong key, out ulong key_out, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_map_reader_prev_before")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_map_reader_prev_before(SafeExpanseSyncMapReaderHandle reader, ulong key, out ulong key_out, out ulong value_out);

    #endregion

    #region Concurrent shared string, bytes, ordered-bytes and blob maps (#1538)

    // Keys are passed as pointers. Callers pin them with `fixed` for the duration of the call only;
    // no pointer is retained by the native side after a call returns.

    // ---- expanse_sync_strmap ----

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_new")]
    public static extern SafeExpanseSyncStrMapHandle expanse_sync_strmap_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_free")]
    public static extern void expanse_sync_strmap_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_insert(SafeExpanseSyncStrMapHandle map, byte* key, ulong value, ulong* old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_get(SafeExpanseSyncStrMapHandle map, byte* key, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_contains(SafeExpanseSyncStrMapHandle map, byte* key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_remove(SafeExpanseSyncStrMapHandle map, byte* key, ulong* old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_len")]
    public static extern ulong expanse_sync_strmap_len(SafeExpanseSyncStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_mem_used")]
    public static extern nuint expanse_sync_strmap_mem_used(SafeExpanseSyncStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_mem_held")]
    public static extern nuint expanse_sync_strmap_mem_held(SafeExpanseSyncStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_shrink_to_fit")]
    public static extern nuint expanse_sync_strmap_shrink_to_fit(SafeExpanseSyncStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_compare_exchange")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_compare_exchange(SafeExpanseSyncStrMapHandle map, byte* key, [MarshalAs(UnmanagedType.I1)] bool expected_present, ulong expected_value, [MarshalAs(UnmanagedType.I1)] bool new_present, ulong new_value, out byte observed_present_out, out ulong observed_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_get_or_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_get_or_insert(SafeExpanseSyncStrMapHandle map, byte* key, ulong value, out ulong existing_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_new")]
    public static extern SafeExpanseSyncStrMapReaderHandle expanse_sync_strmap_reader_new(SafeExpanseSyncStrMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_free")]
    public static extern void expanse_sync_strmap_reader_free(IntPtr reader);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_reader_get(SafeExpanseSyncStrMapReaderHandle reader, byte* key, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_strmap_reader_contains(SafeExpanseSyncStrMapReaderHandle reader, byte* key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_first")]
    public static extern unsafe int expanse_sync_strmap_reader_first(SafeExpanseSyncStrMapReaderHandle reader, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_last")]
    public static extern unsafe int expanse_sync_strmap_reader_last(SafeExpanseSyncStrMapReaderHandle reader, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_next_at_or_after")]
    public static extern unsafe int expanse_sync_strmap_reader_next_at_or_after(SafeExpanseSyncStrMapReaderHandle reader, byte* key, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_next_after")]
    public static extern unsafe int expanse_sync_strmap_reader_next_after(SafeExpanseSyncStrMapReaderHandle reader, byte* key, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_prev_at_or_before")]
    public static extern unsafe int expanse_sync_strmap_reader_prev_at_or_before(SafeExpanseSyncStrMapReaderHandle reader, byte* key, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_strmap_reader_prev_before")]
    public static extern unsafe int expanse_sync_strmap_reader_prev_before(SafeExpanseSyncStrMapReaderHandle reader, byte* key, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    // ---- expanse_sync_bytesmap ----

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_new")]
    public static extern SafeExpanseSyncBytesMapHandle expanse_sync_bytesmap_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_free")]
    public static extern void expanse_sync_bytesmap_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_insert(SafeExpanseSyncBytesMapHandle map, byte* key, nuint len, ulong value, ulong* old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_get(SafeExpanseSyncBytesMapHandle map, byte* key, nuint len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_contains(SafeExpanseSyncBytesMapHandle map, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_remove(SafeExpanseSyncBytesMapHandle map, byte* key, nuint len, ulong* old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_len")]
    public static extern ulong expanse_sync_bytesmap_len(SafeExpanseSyncBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_mem_used")]
    public static extern nuint expanse_sync_bytesmap_mem_used(SafeExpanseSyncBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_mem_held")]
    public static extern nuint expanse_sync_bytesmap_mem_held(SafeExpanseSyncBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_shrink_to_fit")]
    public static extern nuint expanse_sync_bytesmap_shrink_to_fit(SafeExpanseSyncBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_compare_exchange")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_compare_exchange(SafeExpanseSyncBytesMapHandle map, byte* key, nuint len, [MarshalAs(UnmanagedType.I1)] bool expected_present, ulong expected_value, [MarshalAs(UnmanagedType.I1)] bool new_present, ulong new_value, out byte observed_present_out, out ulong observed_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_get_or_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_get_or_insert(SafeExpanseSyncBytesMapHandle map, byte* key, nuint len, ulong value, out ulong existing_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_reader_new")]
    public static extern SafeExpanseSyncBytesMapReaderHandle expanse_sync_bytesmap_reader_new(SafeExpanseSyncBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_reader_free")]
    public static extern void expanse_sync_bytesmap_reader_free(IntPtr reader);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_reader_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_reader_get(SafeExpanseSyncBytesMapReaderHandle reader, byte* key, nuint len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_bytesmap_reader_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_bytesmap_reader_contains(SafeExpanseSyncBytesMapReaderHandle reader, byte* key, nuint len);

    // ---- expanse_sync_ordered_bytesmap ----

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_new")]
    public static extern SafeExpanseSyncOrderedBytesMapHandle expanse_sync_ordered_bytesmap_new();

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_free")]
    public static extern void expanse_sync_ordered_bytesmap_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_insert(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, ulong value, ulong* old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_get(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_contains(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_remove(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, ulong* old_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_len")]
    public static extern ulong expanse_sync_ordered_bytesmap_len(SafeExpanseSyncOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_mem_used")]
    public static extern nuint expanse_sync_ordered_bytesmap_mem_used(SafeExpanseSyncOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_mem_held")]
    public static extern nuint expanse_sync_ordered_bytesmap_mem_held(SafeExpanseSyncOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_shrink_to_fit")]
    public static extern nuint expanse_sync_ordered_bytesmap_shrink_to_fit(SafeExpanseSyncOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_compare_exchange")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_compare_exchange(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, [MarshalAs(UnmanagedType.I1)] bool expected_present, ulong expected_value, [MarshalAs(UnmanagedType.I1)] bool new_present, ulong new_value, out byte observed_present_out, out ulong observed_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_get_or_insert")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_get_or_insert(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, ulong value, out ulong existing_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_reader_new")]
    public static extern SafeExpanseSyncOrderedBytesMapReaderHandle expanse_sync_ordered_bytesmap_reader_new(SafeExpanseSyncOrderedBytesMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_reader_free")]
    public static extern void expanse_sync_ordered_bytesmap_reader_free(IntPtr reader);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_reader_get")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_reader_get(SafeExpanseSyncOrderedBytesMapReaderHandle reader, byte* key, nuint len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_reader_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern unsafe bool expanse_sync_ordered_bytesmap_reader_contains(SafeExpanseSyncOrderedBytesMapReaderHandle reader, byte* key, nuint len);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_first")]
    public static extern unsafe int expanse_sync_ordered_bytesmap_first(SafeExpanseSyncOrderedBytesMapHandle map, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_last")]
    public static extern unsafe int expanse_sync_ordered_bytesmap_last(SafeExpanseSyncOrderedBytesMapHandle map, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_next_at_or_after")]
    public static extern unsafe int expanse_sync_ordered_bytesmap_next_at_or_after(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_next_after")]
    public static extern unsafe int expanse_sync_ordered_bytesmap_next_after(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_prev_at_or_before")]
    public static extern unsafe int expanse_sync_ordered_bytesmap_prev_at_or_before(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_ordered_bytesmap_prev_before")]
    public static extern unsafe int expanse_sync_ordered_bytesmap_prev_before(SafeExpanseSyncOrderedBytesMapHandle map, byte* key, nuint len, byte* key_out, nuint buf_len, out nuint required_len, out ulong value_out);

    // ---- expanse_sync_blob_map ----

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_new")]
    public static extern SafeExpanseSyncBlobMapHandle expanse_sync_blob_map_new(nuint chunk_size);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_new_with_capacity")]
    public static extern SafeExpanseSyncBlobMapHandle expanse_sync_blob_map_new_with_capacity(nuint chunk_size, nuint max_capacity);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_free")]
    public static extern void expanse_sync_blob_map_free(IntPtr map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_insert")]
    public static extern unsafe int expanse_sync_blob_map_insert(SafeExpanseSyncBlobMapHandle map, ulong key, byte* data, nuint len, uint hot_meta);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_remove")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_blob_map_remove(SafeExpanseSyncBlobMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_get_into")]
    public static extern unsafe int expanse_sync_blob_map_get_into(SafeExpanseSyncBlobMapHandle map, ulong key, byte* buf, nuint buf_len, out nuint len_out, out uint meta_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_get_meta")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_blob_map_get_meta(SafeExpanseSyncBlobMapHandle map, ulong key, out uint meta_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_blob_map_contains(SafeExpanseSyncBlobMapHandle map, ulong key);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_len")]
    public static extern ulong expanse_sync_blob_map_len(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_mem_used")]
    public static extern nuint expanse_sync_blob_map_mem_used(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_mem_used_estimate")]
    public static extern nuint expanse_sync_blob_map_mem_used_estimate(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_mem_held")]
    public static extern nuint expanse_sync_blob_map_mem_held(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_mem_held_estimate")]
    public static extern nuint expanse_sync_blob_map_mem_held_estimate(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_shrink_to_fit")]
    public static extern nuint expanse_sync_blob_map_shrink_to_fit(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_set_reclaim_at_cap")]
    public static extern void expanse_sync_blob_map_set_reclaim_at_cap(SafeExpanseSyncBlobMapHandle map, [MarshalAs(UnmanagedType.I1)] bool on);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_compact")]
    public static extern int expanse_sync_blob_map_compact(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_compare_exchange")]
    public static extern unsafe int expanse_sync_blob_map_compare_exchange(SafeExpanseSyncBlobMapHandle map, ulong key, [MarshalAs(UnmanagedType.I1)] bool expected_present, byte* expected_data, nuint expected_len, uint expected_meta, [MarshalAs(UnmanagedType.I1)] bool new_present, byte* new_data, nuint new_len, uint new_meta, out byte observed_present_out, byte* buf, nuint buf_len, out nuint len_out, out uint meta_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_compare_exchange_meta")]
    public static extern int expanse_sync_blob_map_compare_exchange_meta(SafeExpanseSyncBlobMapHandle map, ulong key, [MarshalAs(UnmanagedType.I1)] bool expected_present, uint expected_meta, [MarshalAs(UnmanagedType.I1)] bool new_present, uint new_meta, out byte observed_present_out, out uint observed_meta_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_get_or_insert")]
    public static extern unsafe int expanse_sync_blob_map_get_or_insert(SafeExpanseSyncBlobMapHandle map, ulong key, byte* data, nuint len, uint hot_meta, byte* buf, nuint buf_len, out nuint len_out, out uint meta_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_reader_new")]
    public static extern SafeExpanseSyncBlobMapReaderHandle expanse_sync_blob_map_reader_new(SafeExpanseSyncBlobMapHandle map);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_reader_free")]
    public static extern void expanse_sync_blob_map_reader_free(IntPtr reader);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_reader_get_into")]
    public static extern unsafe int expanse_sync_blob_map_reader_get_into(SafeExpanseSyncBlobMapReaderHandle reader, ulong key, byte* buf, nuint buf_len, out nuint len_out, out uint meta_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_reader_get_meta")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_blob_map_reader_get_meta(SafeExpanseSyncBlobMapReaderHandle reader, ulong key, out uint meta_out);

    [DllImport(LibName, CallingConvention = CallingConvention.Cdecl, EntryPoint = "expanse_sync_blob_map_reader_contains")]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool expanse_sync_blob_map_reader_contains(SafeExpanseSyncBlobMapReaderHandle reader, ulong key);

    #endregion
}
