//go:build !cgo || expanse_purego

package expanse

import (
	"unsafe"

	"github.com/ebitengine/purego"
)

// Identity
var expanse_version func() string

// Set
var (
	expanse_set_new                 func() uintptr
	expanse_set_free                func(set uintptr)
	expanse_set_insert              func(set uintptr, key uint64) bool
	expanse_set_remove              func(set uintptr, key uint64) bool
	expanse_set_contains            func(set uintptr, key uint64) bool
	expanse_set_len                 func(set uintptr) uint64
	expanse_set_mem_used            func(set uintptr) uintptr
	expanse_set_clear               func(set uintptr)
	expanse_set_first               func(set uintptr, keyOut *uint64) bool
	expanse_set_last                func(set uintptr, keyOut *uint64) bool
	expanse_set_next_at_or_after    func(set uintptr, key uint64, keyOut *uint64) bool
	expanse_set_next_after          func(set uintptr, key uint64, keyOut *uint64) bool
	expanse_set_prev_at_or_before   func(set uintptr, key uint64, keyOut *uint64) bool
	expanse_set_prev_before         func(set uintptr, key uint64, keyOut *uint64) bool
	expanse_set_count_below         func(set uintptr, key uint64) uint64
	expanse_set_count_range         func(set uintptr, lo, hi uint64) uint64
	expanse_set_by_count            func(set uintptr, n uint64, keyOut *uint64) bool
	expanse_set_contains_batch      func(set uintptr, keys *uint64, outPresent *bool, count uintptr) uintptr
)

// Map
var (
	expanse_map_new                 func() uintptr
	expanse_map_free                func(mapPtr uintptr)
	expanse_map_insert              func(mapPtr uintptr, key, value uint64, oldOut *uint64) bool
	expanse_map_get                 func(mapPtr uintptr, key uint64, valueOut *uint64) bool
	expanse_map_get_batch           func(mapPtr uintptr, keys *uint64, outValues *uint64, outFound *bool, count uintptr) uintptr
	expanse_map_remove              func(mapPtr uintptr, key uint64, oldOut *uint64) bool
	expanse_map_len                 func(mapPtr uintptr) uint64
	expanse_map_mem_used            func(mapPtr uintptr) uintptr
	expanse_map_clear               func(mapPtr uintptr)
	expanse_map_slot                func(mapPtr uintptr, key uint64) *uint64
	expanse_map_ins_slot            func(mapPtr uintptr, key uint64) *uint64
	expanse_map_first               func(mapPtr uintptr, keyOut, valueOut *uint64) bool
	expanse_map_last                func(mapPtr uintptr, keyOut, valueOut *uint64) bool
	expanse_map_next_at_or_after    func(mapPtr uintptr, key uint64, keyOut, valueOut *uint64) bool
	expanse_map_next_after          func(mapPtr uintptr, key uint64, keyOut, valueOut *uint64) bool
	expanse_map_prev_at_or_before   func(mapPtr uintptr, key uint64, keyOut, valueOut *uint64) bool
	expanse_map_prev_before         func(mapPtr uintptr, key uint64, keyOut, valueOut *uint64) bool
	expanse_map_count_below         func(mapPtr uintptr, key uint64) uint64
	expanse_map_count_range         func(mapPtr uintptr, lo, hi uint64) uint64
	expanse_map_by_count            func(mapPtr uintptr, n uint64, keyOut, valueOut *uint64) bool
)

// BytesMap
var (
	expanse_bytesmap_new            func() uintptr
	expanse_bytesmap_free           func(mapPtr uintptr)
	expanse_bytesmap_insert         func(mapPtr uintptr, key unsafe.Pointer, len uintptr, value uint64, oldOut *uint64) bool
	expanse_bytesmap_get            func(mapPtr uintptr, key unsafe.Pointer, len uintptr, valueOut *uint64) bool
	expanse_bytesmap_remove         func(mapPtr uintptr, key unsafe.Pointer, len uintptr, oldOut *uint64) bool
	expanse_bytesmap_slot           func(mapPtr uintptr, key unsafe.Pointer, len uintptr) *uint64
	expanse_bytesmap_ins_slot       func(mapPtr uintptr, key unsafe.Pointer, len uintptr) *uint64
	expanse_bytesmap_len            func(mapPtr uintptr) uint64
	expanse_bytesmap_mem_used       func(mapPtr uintptr) uintptr
	expanse_bytesmap_clear          func(mapPtr uintptr)
)

// StrMap
var (
	expanse_strmap_new                 func() uintptr
	expanse_strmap_free                func(mapPtr uintptr)
	expanse_strmap_insert              func(mapPtr uintptr, key unsafe.Pointer, value uint64, oldOut *uint64) bool
	expanse_strmap_get                 func(mapPtr uintptr, key unsafe.Pointer, valueOut *uint64) bool
	expanse_strmap_remove              func(mapPtr uintptr, key unsafe.Pointer, oldOut *uint64) bool
	expanse_strmap_slot                func(mapPtr uintptr, key unsafe.Pointer) *uint64
	expanse_strmap_ins_slot            func(mapPtr uintptr, key unsafe.Pointer) *uint64
	expanse_strmap_len                 func(mapPtr uintptr) uint64
	expanse_strmap_mem_used            func(mapPtr uintptr) uintptr
	expanse_strmap_clear               func(mapPtr uintptr)
	expanse_strmap_first               func(mapPtr uintptr, keyOut unsafe.Pointer, bufLen uintptr, valueOut *uint64) bool
	expanse_strmap_last                func(mapPtr uintptr, keyOut unsafe.Pointer, bufLen uintptr, valueOut *uint64) bool
	expanse_strmap_next_at_or_after    func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, valueOut *uint64) bool
	expanse_strmap_next_after          func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, valueOut *uint64) bool
	expanse_strmap_prev_at_or_before   func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, valueOut *uint64) bool
	expanse_strmap_prev_before         func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, valueOut *uint64) bool
	expanse_strmap_first_ex            func(mapPtr uintptr, keyOut unsafe.Pointer, bufLen uintptr, requiredLen *uintptr, valueOut *uint64) int32
	expanse_strmap_last_ex             func(mapPtr uintptr, keyOut unsafe.Pointer, bufLen uintptr, requiredLen *uintptr, valueOut *uint64) int32
	expanse_strmap_next_at_or_after_ex func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, requiredLen *uintptr, valueOut *uint64) int32
	expanse_strmap_next_after_ex       func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, requiredLen *uintptr, valueOut *uint64) int32
	expanse_strmap_prev_at_or_before_ex func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, requiredLen *uintptr, valueOut *uint64) int32
	expanse_strmap_prev_before_ex      func(mapPtr uintptr, key, keyOut unsafe.Pointer, bufLen uintptr, requiredLen *uintptr, valueOut *uint64) int32
)

// SyncSet
var (
	expanse_sync_set_new              func() uintptr
	expanse_sync_set_free             func(set uintptr)
	expanse_sync_set_insert           func(set uintptr, key uint64) bool
	expanse_sync_set_remove           func(set uintptr, key uint64) bool
	expanse_sync_set_contains         func(set uintptr, key uint64) bool
	expanse_sync_set_len              func(set uintptr) uint64
	expanse_sync_set_reader_new       func(set uintptr) uintptr
	expanse_sync_set_reader_free      func(reader uintptr)
	expanse_sync_set_reader_contains  func(reader uintptr, key uint64) bool
)

// SyncMap
var (
	expanse_sync_map_new              func() uintptr
	expanse_sync_map_free             func(mapPtr uintptr)
	expanse_sync_map_insert           func(mapPtr uintptr, key, value uint64, oldOut *uint64) bool
	expanse_sync_map_get              func(mapPtr uintptr, key uint64, valueOut *uint64) bool
	expanse_sync_map_remove           func(mapPtr uintptr, key uint64, oldOut *uint64) bool
	expanse_sync_map_len              func(mapPtr uintptr) uint64
	expanse_sync_map_reader_new       func(mapPtr uintptr) uintptr
	expanse_sync_map_reader_free      func(reader uintptr)
	expanse_sync_map_reader_get       func(reader uintptr, key uint64, valueOut *uint64) bool
)

// BlobView layout matching ExpanseBlobView in expanse.h
type blobView struct {
	ptr      unsafe.Pointer
	len      uintptr
	hotMeta  uint32
	isInline bool
	_        [3]byte
}

// BlobMap
var (
	expanse_blob_map_new           func(chunkSize uintptr) uintptr
	expanse_blob_map_free          func(mapPtr uintptr)
	expanse_blob_map_insert        func(mapPtr uintptr, key uint64, data unsafe.Pointer, len uintptr, hotMeta uint32) bool
	expanse_blob_map_remove        func(mapPtr uintptr, key uint64) bool
	expanse_blob_map_get           func(mapPtr uintptr, key uint64, outView *blobView) bool
	expanse_blob_map_scan_filtered func(mapPtr uintptr, startKey, endKey uint64, predicate uintptr, callback uintptr, userCtx unsafe.Pointer) uintptr
	expanse_blob_map_compact       func(mapPtr uintptr) bool
	expanse_blob_map_len           func(mapPtr uintptr) uint64
	expanse_blob_map_mem_used      func(mapPtr uintptr) uintptr
	expanse_blob_map_clear         func(mapPtr uintptr)
	expanse_blob_map_contains_key  func(mapPtr uintptr, key uint64) bool
)

var prunePredicateCallbackPtr uintptr

type pruneContext struct {
	predicate func(key uint64, hotMeta uint32) bool
	toRemove  []uint64
}

func goPrunePredicatePurego(key uint64, hotMeta uint32, ctx unsafe.Pointer) uintptr {
	if ctx == nil {
		return 0
	}
	target := (*pruneContext)(ctx)
	if target.predicate != nil && target.predicate(key, hotMeta) {
		target.toRemove = append(target.toRemove, key)
	}
	return 0
}

func initCallbacks() {
	prunePredicateCallbackPtr = purego.NewCallback(goPrunePredicatePurego)
}

func bindSymbols(h *LibraryHandle) error {
	symbols := []struct {
		fptr any
		name string
	}{
		// Identity
		{&expanse_version, "expanse_version"},

		// Set
		{&expanse_set_new, "expanse_set_new"},
		{&expanse_set_free, "expanse_set_free"},
		{&expanse_set_insert, "expanse_set_insert"},
		{&expanse_set_remove, "expanse_set_remove"},
		{&expanse_set_contains, "expanse_set_contains"},
		{&expanse_set_len, "expanse_set_len"},
		{&expanse_set_mem_used, "expanse_set_mem_used"},
		{&expanse_set_clear, "expanse_set_clear"},
		{&expanse_set_first, "expanse_set_first"},
		{&expanse_set_last, "expanse_set_last"},
		{&expanse_set_next_at_or_after, "expanse_set_next_at_or_after"},
		{&expanse_set_next_after, "expanse_set_next_after"},
		{&expanse_set_prev_at_or_before, "expanse_set_prev_at_or_before"},
		{&expanse_set_prev_before, "expanse_set_prev_before"},
		{&expanse_set_count_below, "expanse_set_count_below"},
		{&expanse_set_count_range, "expanse_set_count_range"},
		{&expanse_set_by_count, "expanse_set_by_count"},
		{&expanse_set_contains_batch, "expanse_set_contains_batch"},

		// Map
		{&expanse_map_new, "expanse_map_new"},
		{&expanse_map_free, "expanse_map_free"},
		{&expanse_map_insert, "expanse_map_insert"},
		{&expanse_map_get, "expanse_map_get"},
		{&expanse_map_get_batch, "expanse_map_get_batch"},
		{&expanse_map_remove, "expanse_map_remove"},
		{&expanse_map_len, "expanse_map_len"},
		{&expanse_map_mem_used, "expanse_map_mem_used"},
		{&expanse_map_clear, "expanse_map_clear"},
		{&expanse_map_slot, "expanse_map_slot"},
		{&expanse_map_ins_slot, "expanse_map_ins_slot"},
		{&expanse_map_first, "expanse_map_first"},
		{&expanse_map_last, "expanse_map_last"},
		{&expanse_map_next_at_or_after, "expanse_map_next_at_or_after"},
		{&expanse_map_next_after, "expanse_map_next_after"},
		{&expanse_map_prev_at_or_before, "expanse_map_prev_at_or_before"},
		{&expanse_map_prev_before, "expanse_map_prev_before"},
		{&expanse_map_count_below, "expanse_map_count_below"},
		{&expanse_map_count_range, "expanse_map_count_range"},
		{&expanse_map_by_count, "expanse_map_by_count"},

		// BytesMap
		{&expanse_bytesmap_new, "expanse_bytesmap_new"},
		{&expanse_bytesmap_free, "expanse_bytesmap_free"},
		{&expanse_bytesmap_insert, "expanse_bytesmap_insert"},
		{&expanse_bytesmap_get, "expanse_bytesmap_get"},
		{&expanse_bytesmap_remove, "expanse_bytesmap_remove"},
		{&expanse_bytesmap_slot, "expanse_bytesmap_slot"},
		{&expanse_bytesmap_ins_slot, "expanse_bytesmap_ins_slot"},
		{&expanse_bytesmap_len, "expanse_bytesmap_len"},
		{&expanse_bytesmap_mem_used, "expanse_bytesmap_mem_used"},
		{&expanse_bytesmap_clear, "expanse_bytesmap_clear"},

		// StrMap
		{&expanse_strmap_new, "expanse_strmap_new"},
		{&expanse_strmap_free, "expanse_strmap_free"},
		{&expanse_strmap_insert, "expanse_strmap_insert"},
		{&expanse_strmap_get, "expanse_strmap_get"},
		{&expanse_strmap_remove, "expanse_strmap_remove"},
		{&expanse_strmap_slot, "expanse_strmap_slot"},
		{&expanse_strmap_ins_slot, "expanse_strmap_ins_slot"},
		{&expanse_strmap_len, "expanse_strmap_len"},
		{&expanse_strmap_mem_used, "expanse_strmap_mem_used"},
		{&expanse_strmap_clear, "expanse_strmap_clear"},
		{&expanse_strmap_first, "expanse_strmap_first"},
		{&expanse_strmap_last, "expanse_strmap_last"},
		{&expanse_strmap_next_at_or_after, "expanse_strmap_next_at_or_after"},
		{&expanse_strmap_next_after, "expanse_strmap_next_after"},
		{&expanse_strmap_prev_at_or_before, "expanse_strmap_prev_at_or_before"},
		{&expanse_strmap_prev_before, "expanse_strmap_prev_before"},
		{&expanse_strmap_first_ex, "expanse_strmap_first_ex"},
		{&expanse_strmap_last_ex, "expanse_strmap_last_ex"},
		{&expanse_strmap_next_at_or_after_ex, "expanse_strmap_next_at_or_after_ex"},
		{&expanse_strmap_next_after_ex, "expanse_strmap_next_after_ex"},
		{&expanse_strmap_prev_at_or_before_ex, "expanse_strmap_prev_at_or_before_ex"},
		{&expanse_strmap_prev_before_ex, "expanse_strmap_prev_before_ex"},

		// SyncSet
		{&expanse_sync_set_new, "expanse_sync_set_new"},
		{&expanse_sync_set_free, "expanse_sync_set_free"},
		{&expanse_sync_set_insert, "expanse_sync_set_insert"},
		{&expanse_sync_set_remove, "expanse_sync_set_remove"},
		{&expanse_sync_set_contains, "expanse_sync_set_contains"},
		{&expanse_sync_set_len, "expanse_sync_set_len"},
		{&expanse_sync_set_reader_new, "expanse_sync_set_reader_new"},
		{&expanse_sync_set_reader_free, "expanse_sync_set_reader_free"},
		{&expanse_sync_set_reader_contains, "expanse_sync_set_reader_contains"},

		// SyncMap
		{&expanse_sync_map_new, "expanse_sync_map_new"},
		{&expanse_sync_map_free, "expanse_sync_map_free"},
		{&expanse_sync_map_insert, "expanse_sync_map_insert"},
		{&expanse_sync_map_get, "expanse_sync_map_get"},
		{&expanse_sync_map_remove, "expanse_sync_map_remove"},
		{&expanse_sync_map_len, "expanse_sync_map_len"},
		{&expanse_sync_map_reader_new, "expanse_sync_map_reader_new"},
		{&expanse_sync_map_reader_free, "expanse_sync_map_reader_free"},
		{&expanse_sync_map_reader_get, "expanse_sync_map_reader_get"},

		// BlobMap
		{&expanse_blob_map_new, "expanse_blob_map_new"},
		{&expanse_blob_map_free, "expanse_blob_map_free"},
		{&expanse_blob_map_insert, "expanse_blob_map_insert"},
		{&expanse_blob_map_remove, "expanse_blob_map_remove"},
		{&expanse_blob_map_get, "expanse_blob_map_get"},
		{&expanse_blob_map_scan_filtered, "expanse_blob_map_scan_filtered"},
		{&expanse_blob_map_compact, "expanse_blob_map_compact"},
		{&expanse_blob_map_len, "expanse_blob_map_len"},
		{&expanse_blob_map_mem_used, "expanse_blob_map_mem_used"},
		{&expanse_blob_map_clear, "expanse_blob_map_clear"},
		{&expanse_blob_map_contains_key, "expanse_blob_map_contains_key"},
	}

	for _, s := range symbols {
		if err := h.registerFunc(s.fptr, s.name); err != nil {
			return err
		}
	}
	return nil
}
