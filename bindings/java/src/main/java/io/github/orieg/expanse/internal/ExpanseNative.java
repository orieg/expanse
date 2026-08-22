package io.github.orieg.expanse.internal;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;

/**
 * Low-level Project Panama FFM binding definitions for the modern libexpanse C API.
 * Holds exact MethodHandles bound to C function downcalls with zero JNI overhead.
 */
public final class ExpanseNative {

    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP = NativeLoader.getSymbolLookup();

    // Version
    public static final MethodHandle MH_expanse_version;

    // Set
    public static final MethodHandle MH_expanse_set_new;
    public static final MethodHandle MH_expanse_set_free;
    public static final MethodHandle MH_expanse_set_insert;
    public static final MethodHandle MH_expanse_set_remove;
    public static final MethodHandle MH_expanse_set_contains;
    public static final MethodHandle MH_expanse_set_len;
    public static final MethodHandle MH_expanse_set_mem_used;
    public static final MethodHandle MH_expanse_set_clear;
    public static final MethodHandle MH_expanse_set_first;
    public static final MethodHandle MH_expanse_set_last;
    public static final MethodHandle MH_expanse_set_next_at_or_after;
    public static final MethodHandle MH_expanse_set_next_after;
    public static final MethodHandle MH_expanse_set_prev_at_or_before;
    public static final MethodHandle MH_expanse_set_prev_before;
    public static final MethodHandle MH_expanse_set_count_below;
    public static final MethodHandle MH_expanse_set_count_range;
    public static final MethodHandle MH_expanse_set_by_count;

    // Map
    public static final MethodHandle MH_expanse_map_new;
    public static final MethodHandle MH_expanse_map_free;
    public static final MethodHandle MH_expanse_map_insert;
    public static final MethodHandle MH_expanse_map_get;
    public static final MethodHandle MH_expanse_map_remove;
    public static final MethodHandle MH_expanse_map_len;
    public static final MethodHandle MH_expanse_map_mem_used;
    public static final MethodHandle MH_expanse_map_clear;
    public static final MethodHandle MH_expanse_map_slot;
    public static final MethodHandle MH_expanse_map_ins_slot;
    public static final MethodHandle MH_expanse_map_first;
    public static final MethodHandle MH_expanse_map_last;
    public static final MethodHandle MH_expanse_map_next_at_or_after;
    public static final MethodHandle MH_expanse_map_next_after;
    public static final MethodHandle MH_expanse_map_prev_at_or_before;
    public static final MethodHandle MH_expanse_map_prev_before;
    public static final MethodHandle MH_expanse_map_count_below;
    public static final MethodHandle MH_expanse_map_count_range;
    public static final MethodHandle MH_expanse_map_by_count;

    // BytesMap
    public static final MethodHandle MH_expanse_bytesmap_new;
    public static final MethodHandle MH_expanse_bytesmap_free;
    public static final MethodHandle MH_expanse_bytesmap_insert;
    public static final MethodHandle MH_expanse_bytesmap_get;
    public static final MethodHandle MH_expanse_bytesmap_remove;
    public static final MethodHandle MH_expanse_bytesmap_slot;
    public static final MethodHandle MH_expanse_bytesmap_ins_slot;
    public static final MethodHandle MH_expanse_bytesmap_len;
    public static final MethodHandle MH_expanse_bytesmap_mem_used;
    public static final MethodHandle MH_expanse_bytesmap_clear;

    // StrMap
    public static final MethodHandle MH_expanse_strmap_new;
    public static final MethodHandle MH_expanse_strmap_free;
    public static final MethodHandle MH_expanse_strmap_insert;
    public static final MethodHandle MH_expanse_strmap_get;
    public static final MethodHandle MH_expanse_strmap_remove;
    public static final MethodHandle MH_expanse_strmap_slot;
    public static final MethodHandle MH_expanse_strmap_ins_slot;
    public static final MethodHandle MH_expanse_strmap_len;
    public static final MethodHandle MH_expanse_strmap_mem_used;
    public static final MethodHandle MH_expanse_strmap_clear;
    public static final MethodHandle MH_expanse_strmap_first;
    public static final MethodHandle MH_expanse_strmap_last;
    public static final MethodHandle MH_expanse_strmap_next_at_or_after;
    public static final MethodHandle MH_expanse_strmap_next_after;
    public static final MethodHandle MH_expanse_strmap_prev_at_or_before;
    public static final MethodHandle MH_expanse_strmap_prev_before;

    // Concurrent Types
    public static final MethodHandle MH_expanse_sync_set_new;
    public static final MethodHandle MH_expanse_sync_set_free;
    public static final MethodHandle MH_expanse_sync_set_insert;
    public static final MethodHandle MH_expanse_sync_set_remove;
    public static final MethodHandle MH_expanse_sync_set_contains;
    public static final MethodHandle MH_expanse_sync_set_len;
    public static final MethodHandle MH_expanse_sync_set_reader_new;
    public static final MethodHandle MH_expanse_sync_set_reader_free;
    public static final MethodHandle MH_expanse_sync_set_reader_contains;

    public static final MethodHandle MH_expanse_sync_map_new;
    public static final MethodHandle MH_expanse_sync_map_free;
    public static final MethodHandle MH_expanse_sync_map_insert;
    public static final MethodHandle MH_expanse_sync_map_get;
    public static final MethodHandle MH_expanse_sync_map_remove;
    public static final MethodHandle MH_expanse_sync_map_len;
    public static final MethodHandle MH_expanse_sync_map_reader_new;
    public static final MethodHandle MH_expanse_sync_map_reader_free;
    public static final MethodHandle MH_expanse_sync_map_reader_get;

    static {
        // Version
        MH_expanse_version = downcall("expanse_version", FunctionDescriptor.of(ValueLayout.ADDRESS));

        // Set
        MH_expanse_set_new = downcall("expanse_set_new", FunctionDescriptor.of(ValueLayout.ADDRESS));
        MH_expanse_set_free = downcall("expanse_set_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_set_insert = downcall("expanse_set_insert", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_set_remove = downcall("expanse_set_remove", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_set_contains = downcall("expanse_set_contains", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_set_len = downcall("expanse_set_len", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_set_mem_used = downcall("expanse_set_mem_used", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_set_clear = downcall("expanse_set_clear", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_set_first = downcall("expanse_set_first", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_set_last = downcall("expanse_set_last", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_set_next_at_or_after = downcall("expanse_set_next_at_or_after", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_set_next_after = downcall("expanse_set_next_after", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_set_prev_at_or_before = downcall("expanse_set_prev_at_or_before", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_set_prev_before = downcall("expanse_set_prev_before", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_set_count_below = downcall("expanse_set_count_below", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_set_count_range = downcall("expanse_set_count_range", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG));
        MH_expanse_set_by_count = downcall("expanse_set_by_count", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));

        // Map
        MH_expanse_map_new = downcall("expanse_map_new", FunctionDescriptor.of(ValueLayout.ADDRESS));
        MH_expanse_map_free = downcall("expanse_map_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_map_insert = downcall("expanse_map_insert", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_map_get = downcall("expanse_map_get", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_map_remove = downcall("expanse_map_remove", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_map_len = downcall("expanse_map_len", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_map_mem_used = downcall("expanse_map_mem_used", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_map_clear = downcall("expanse_map_clear", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_map_slot = downcall("expanse_map_slot", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_map_ins_slot = downcall("expanse_map_ins_slot", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_map_first = downcall("expanse_map_first", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_map_last = downcall("expanse_map_last", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_map_next_at_or_after = downcall("expanse_map_next_at_or_after", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_map_next_after = downcall("expanse_map_next_after", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_map_prev_at_or_before = downcall("expanse_map_prev_at_or_before", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_map_prev_before = downcall("expanse_map_prev_before", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_map_count_below = downcall("expanse_map_count_below", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_map_count_range = downcall("expanse_map_count_range", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG));
        MH_expanse_map_by_count = downcall("expanse_map_by_count", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.ADDRESS));

        // BytesMap
        MH_expanse_bytesmap_new = downcall("expanse_bytesmap_new", FunctionDescriptor.of(ValueLayout.ADDRESS));
        MH_expanse_bytesmap_free = downcall("expanse_bytesmap_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_bytesmap_insert = downcall("expanse_bytesmap_insert", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_bytesmap_get = downcall("expanse_bytesmap_get", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_bytesmap_remove = downcall("expanse_bytesmap_remove", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_bytesmap_slot = downcall("expanse_bytesmap_slot", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_bytesmap_ins_slot = downcall("expanse_bytesmap_ins_slot", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_bytesmap_len = downcall("expanse_bytesmap_len", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_bytesmap_mem_used = downcall("expanse_bytesmap_mem_used", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_bytesmap_clear = downcall("expanse_bytesmap_clear", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));

        // StrMap
        MH_expanse_strmap_new = downcall("expanse_strmap_new", FunctionDescriptor.of(ValueLayout.ADDRESS));
        MH_expanse_strmap_free = downcall("expanse_strmap_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_strmap_insert = downcall("expanse_strmap_insert", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_get = downcall("expanse_strmap_get", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_strmap_remove = downcall("expanse_strmap_remove", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_strmap_slot = downcall("expanse_strmap_slot", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_strmap_ins_slot = downcall("expanse_strmap_ins_slot", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_strmap_len = downcall("expanse_strmap_len", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_mem_used = downcall("expanse_strmap_mem_used", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_clear = downcall("expanse_strmap_clear", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_strmap_first = downcall("expanse_strmap_first", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_last = downcall("expanse_strmap_last", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_next_at_or_after = downcall("expanse_strmap_next_at_or_after", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_next_after = downcall("expanse_strmap_next_after", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_prev_at_or_before = downcall("expanse_strmap_prev_at_or_before", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_strmap_prev_before = downcall("expanse_strmap_prev_before", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));

        // Concurrent
        MH_expanse_sync_set_new = downcall("expanse_sync_set_new", FunctionDescriptor.of(ValueLayout.ADDRESS));
        MH_expanse_sync_set_free = downcall("expanse_sync_set_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_sync_set_insert = downcall("expanse_sync_set_insert", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_sync_set_remove = downcall("expanse_sync_set_remove", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_sync_set_contains = downcall("expanse_sync_set_contains", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));
        MH_expanse_sync_set_len = downcall("expanse_sync_set_len", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_sync_set_reader_new = downcall("expanse_sync_set_reader_new", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_sync_set_reader_free = downcall("expanse_sync_set_reader_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_sync_set_reader_contains = downcall("expanse_sync_set_reader_contains", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));

        MH_expanse_sync_map_new = downcall("expanse_sync_map_new", FunctionDescriptor.of(ValueLayout.ADDRESS));
        MH_expanse_sync_map_free = downcall("expanse_sync_map_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_sync_map_insert = downcall("expanse_sync_map_insert", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_sync_map_get = downcall("expanse_sync_map_get", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_sync_map_remove = downcall("expanse_sync_map_remove", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_sync_map_len = downcall("expanse_sync_map_len", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
        MH_expanse_sync_map_reader_new = downcall("expanse_sync_map_reader_new", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS));
        MH_expanse_sync_map_reader_free = downcall("expanse_sync_map_reader_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
        MH_expanse_sync_map_reader_get = downcall("expanse_sync_map_reader_get", FunctionDescriptor.of(ValueLayout.JAVA_BOOLEAN, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS));
    }

    private static MethodHandle downcall(String name, FunctionDescriptor desc) {
        MemorySegment symbol = LOOKUP.find(name)
                .orElseThrow(() -> new UnsatisfiedLinkError("Native symbol not found in libexpanse: " + name));
        return LINKER.downcallHandle(symbol, desc);
    }

    private ExpanseNative() {}
}
