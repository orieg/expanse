//go:build cgo && !expanse_purego

package expanse

// #include <stdlib.h>
// #include "expanse.h"
import "C"
import (
	"runtime"
	"unsafe"
)

// This file is the cgo half of the shared string, bytes, ordered-bytes and
// blob maps: handle types, constructors, Free, and the one-call native shims
// the shared logic in sync_maps.go and sync_shared.go calls. A shim makes one
// C call and converts types; it decides nothing.

// ---- SyncStrMap ----

// SyncStrMap is a concurrent ordered map from NUL-free string keys to uint64
// values, backed by expanse_sync_strmap_t. Writers serialise on one mutex;
// point reads and the reader's ordered reads are optimistic.
type SyncStrMap struct {
	ptr *C.expanse_sync_strmap_t
}

// SyncStrMapReader is a registered reader handle. It borrows from the map it
// was taken from, so it holds the map reachable (see SyncSetReader). One
// reader per goroutine at a time.
type SyncStrMapReader struct {
	ptr    *C.expanse_sync_strmap_reader_t
	parent *SyncStrMap
}

// NewSyncStrMap returns an empty map. Call Free when done, or let the
// finalizer release the native handle.
func NewSyncStrMap() *SyncStrMap {
	m := &SyncStrMap{ptr: C.expanse_sync_strmap_new()}
	runtime.SetFinalizer(m, (*SyncStrMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncStrMap) Reader() *SyncStrMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncStrMapReader{ptr: C.expanse_sync_strmap_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncStrMapReader).Free)
	return r
}

func (m *SyncStrMap) nInsert(key []byte, value uint64) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_strmap_insert(m.ptr, (*C.char)(cptr(key)), C.uint64_t(value), nil))
}

func (m *SyncStrMap) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v C.uint64_t
	ok := bool(C.expanse_sync_strmap_get(m.ptr, (*C.char)(cptr(key)), &v))
	return uint64(v), ok
}

func (m *SyncStrMap) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_strmap_contains(m.ptr, (*C.char)(cptr(key))))
}

func (m *SyncStrMap) nRemove(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_strmap_remove(m.ptr, (*C.char)(cptr(key)), nil))
}

func (m *SyncStrMap) nCompareExchange(key []byte, expected, newValue Word) (Word, bool) {
	defer runtime.KeepAlive(key)
	var seenPresent C.bool
	var seen C.uint64_t
	stored := bool(C.expanse_sync_strmap_compare_exchange(m.ptr, (*C.char)(cptr(key)),
		C.bool(expected.Present), C.uint64_t(expected.Value),
		C.bool(newValue.Present), C.uint64_t(newValue.Value),
		&seenPresent, &seen))
	return Word{Value: uint64(seen), Present: bool(seenPresent)}, stored
}

// nGetOrInsert reports inserted true when value was stored; otherwise existing
// is the value that was there.
func (m *SyncStrMap) nGetOrInsert(key []byte, value uint64) (existing uint64, inserted bool) {
	defer runtime.KeepAlive(key)
	var out C.uint64_t
	inserted = bool(C.expanse_sync_strmap_get_or_insert(m.ptr, (*C.char)(cptr(key)), C.uint64_t(value), &out))
	return uint64(out), inserted
}

func (m *SyncStrMap) nLen() uint64     { return uint64(C.expanse_sync_strmap_len(m.ptr)) }
func (m *SyncStrMap) nMemUsed() uint64 { return uint64(C.expanse_sync_strmap_mem_used(m.ptr)) }
func (m *SyncStrMap) nMemHeld() uint64 { return uint64(C.expanse_sync_strmap_mem_held(m.ptr)) }
func (m *SyncStrMap) nShrink() uint64  { return uint64(C.expanse_sync_strmap_shrink_to_fit(m.ptr)) }

func (r *SyncStrMapReader) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v C.uint64_t
	ok := bool(C.expanse_sync_strmap_reader_get(r.ptr, (*C.char)(cptr(key)), &v))
	return uint64(v), ok
}

func (r *SyncStrMapReader) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_strmap_reader_contains(r.ptr, (*C.char)(cptr(key))))
}

// nNav makes one ordered read into buf, which is never empty.
func (r *SyncStrMapReader) nNav(op navOp, key, buf []byte) (int32, uintptr, uint64) {
	defer runtime.KeepAlive(key)
	defer runtime.KeepAlive(buf)
	var req C.size_t
	var val C.uint64_t
	out := (*C.char)(unsafe.Pointer(&buf[0]))
	n := C.size_t(len(buf))
	k := (*C.char)(cptr(key))
	var st C.expanse_str_nav_status
	switch op {
	case navFirst:
		st = C.expanse_sync_strmap_reader_first(r.ptr, out, n, &req, &val)
	case navLast:
		st = C.expanse_sync_strmap_reader_last(r.ptr, out, n, &req, &val)
	case navNextAtOrAfter:
		st = C.expanse_sync_strmap_reader_next_at_or_after(r.ptr, k, out, n, &req, &val)
	case navNextAfter:
		st = C.expanse_sync_strmap_reader_next_after(r.ptr, k, out, n, &req, &val)
	case navPrevAtOrBefore:
		st = C.expanse_sync_strmap_reader_prev_at_or_before(r.ptr, k, out, n, &req, &val)
	default:
		st = C.expanse_sync_strmap_reader_prev_before(r.ptr, k, out, n, &req, &val)
	}
	return int32(st), uintptr(req), uint64(val)
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the nil check and the store are unguarded.
func (r *SyncStrMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != nil {
		C.expanse_sync_strmap_reader_free(r.ptr)
		r.ptr = nil
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncStrMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != nil {
		C.expanse_sync_strmap_free(m.ptr)
		m.ptr = nil
	}
}

// ---- SyncBytesMap ----

// SyncBytesMap is a concurrent unordered map from arbitrary byte-slice keys to
// uint64 values, backed by expanse_sync_bytesmap_t. It has no ordered reads;
// SyncOrderedBytesMap does.
type SyncBytesMap struct {
	ptr *C.expanse_sync_bytesmap_t
}

// SyncBytesMapReader is a registered reader handle. See SyncStrMapReader.
type SyncBytesMapReader struct {
	ptr    *C.expanse_sync_bytesmap_reader_t
	parent *SyncBytesMap
}

// NewSyncBytesMap returns an empty map. Call Free when done, or let the
// finalizer release the native handle.
func NewSyncBytesMap() *SyncBytesMap {
	m := &SyncBytesMap{ptr: C.expanse_sync_bytesmap_new()}
	runtime.SetFinalizer(m, (*SyncBytesMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncBytesMap) Reader() *SyncBytesMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncBytesMapReader{ptr: C.expanse_sync_bytesmap_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncBytesMapReader).Free)
	return r
}

func (m *SyncBytesMap) nInsert(key []byte, value uint64) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_bytesmap_insert(m.ptr, cptr(key), C.size_t(len(key)), C.uint64_t(value), nil))
}

func (m *SyncBytesMap) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v C.uint64_t
	ok := bool(C.expanse_sync_bytesmap_get(m.ptr, cptr(key), C.size_t(len(key)), &v))
	return uint64(v), ok
}

func (m *SyncBytesMap) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_bytesmap_contains(m.ptr, cptr(key), C.size_t(len(key))))
}

func (m *SyncBytesMap) nRemove(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_bytesmap_remove(m.ptr, cptr(key), C.size_t(len(key)), nil))
}

func (m *SyncBytesMap) nCompareExchange(key []byte, expected, newValue Word) (Word, bool) {
	defer runtime.KeepAlive(key)
	var seenPresent C.bool
	var seen C.uint64_t
	stored := bool(C.expanse_sync_bytesmap_compare_exchange(m.ptr, cptr(key), C.size_t(len(key)),
		C.bool(expected.Present), C.uint64_t(expected.Value),
		C.bool(newValue.Present), C.uint64_t(newValue.Value),
		&seenPresent, &seen))
	return Word{Value: uint64(seen), Present: bool(seenPresent)}, stored
}

func (m *SyncBytesMap) nGetOrInsert(key []byte, value uint64) (existing uint64, inserted bool) {
	defer runtime.KeepAlive(key)
	var out C.uint64_t
	inserted = bool(C.expanse_sync_bytesmap_get_or_insert(m.ptr, cptr(key), C.size_t(len(key)), C.uint64_t(value), &out))
	return uint64(out), inserted
}

func (m *SyncBytesMap) nLen() uint64     { return uint64(C.expanse_sync_bytesmap_len(m.ptr)) }
func (m *SyncBytesMap) nMemUsed() uint64 { return uint64(C.expanse_sync_bytesmap_mem_used(m.ptr)) }
func (m *SyncBytesMap) nMemHeld() uint64 { return uint64(C.expanse_sync_bytesmap_mem_held(m.ptr)) }
func (m *SyncBytesMap) nShrink() uint64  { return uint64(C.expanse_sync_bytesmap_shrink_to_fit(m.ptr)) }

func (r *SyncBytesMapReader) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v C.uint64_t
	ok := bool(C.expanse_sync_bytesmap_reader_get(r.ptr, cptr(key), C.size_t(len(key)), &v))
	return uint64(v), ok
}

func (r *SyncBytesMapReader) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_bytesmap_reader_contains(r.ptr, cptr(key), C.size_t(len(key))))
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the nil check and the store are unguarded.
func (r *SyncBytesMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != nil {
		C.expanse_sync_bytesmap_reader_free(r.ptr)
		r.ptr = nil
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncBytesMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != nil {
		C.expanse_sync_bytesmap_free(m.ptr)
		m.ptr = nil
	}
}

// ---- SyncOrderedBytesMap ----

// SyncOrderedBytesMap is a concurrent ordered map from arbitrary byte-slice
// keys to uint64 values, backed by expanse_sync_ordered_bytesmap_t. Its ordered
// reads are calls on the map and block writers; see First.
type SyncOrderedBytesMap struct {
	ptr *C.expanse_sync_ordered_bytesmap_t
}

// SyncOrderedBytesMapReader is a registered reader handle. It offers point
// reads only; the ordered reads are on the map. See SyncStrMapReader.
type SyncOrderedBytesMapReader struct {
	ptr    *C.expanse_sync_ordered_bytesmap_reader_t
	parent *SyncOrderedBytesMap
}

// NewSyncOrderedBytesMap returns an empty map. Call Free when done, or let the
// finalizer release the native handle.
func NewSyncOrderedBytesMap() *SyncOrderedBytesMap {
	m := &SyncOrderedBytesMap{ptr: C.expanse_sync_ordered_bytesmap_new()}
	runtime.SetFinalizer(m, (*SyncOrderedBytesMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncOrderedBytesMap) Reader() *SyncOrderedBytesMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncOrderedBytesMapReader{ptr: C.expanse_sync_ordered_bytesmap_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncOrderedBytesMapReader).Free)
	return r
}

func (m *SyncOrderedBytesMap) nInsert(key []byte, value uint64) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_ordered_bytesmap_insert(m.ptr, cptr(key), C.size_t(len(key)), C.uint64_t(value), nil))
}

func (m *SyncOrderedBytesMap) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v C.uint64_t
	ok := bool(C.expanse_sync_ordered_bytesmap_get(m.ptr, cptr(key), C.size_t(len(key)), &v))
	return uint64(v), ok
}

func (m *SyncOrderedBytesMap) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_ordered_bytesmap_contains(m.ptr, cptr(key), C.size_t(len(key))))
}

func (m *SyncOrderedBytesMap) nRemove(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_ordered_bytesmap_remove(m.ptr, cptr(key), C.size_t(len(key)), nil))
}

func (m *SyncOrderedBytesMap) nCompareExchange(key []byte, expected, newValue Word) (Word, bool) {
	defer runtime.KeepAlive(key)
	var seenPresent C.bool
	var seen C.uint64_t
	stored := bool(C.expanse_sync_ordered_bytesmap_compare_exchange(m.ptr, cptr(key), C.size_t(len(key)),
		C.bool(expected.Present), C.uint64_t(expected.Value),
		C.bool(newValue.Present), C.uint64_t(newValue.Value),
		&seenPresent, &seen))
	return Word{Value: uint64(seen), Present: bool(seenPresent)}, stored
}

func (m *SyncOrderedBytesMap) nGetOrInsert(key []byte, value uint64) (existing uint64, inserted bool) {
	defer runtime.KeepAlive(key)
	var out C.uint64_t
	inserted = bool(C.expanse_sync_ordered_bytesmap_get_or_insert(m.ptr, cptr(key), C.size_t(len(key)), C.uint64_t(value), &out))
	return uint64(out), inserted
}

func (m *SyncOrderedBytesMap) nLen() uint64 {
	return uint64(C.expanse_sync_ordered_bytesmap_len(m.ptr))
}
func (m *SyncOrderedBytesMap) nMemUsed() uint64 {
	return uint64(C.expanse_sync_ordered_bytesmap_mem_used(m.ptr))
}
func (m *SyncOrderedBytesMap) nMemHeld() uint64 {
	return uint64(C.expanse_sync_ordered_bytesmap_mem_held(m.ptr))
}
func (m *SyncOrderedBytesMap) nShrink() uint64 {
	return uint64(C.expanse_sync_ordered_bytesmap_shrink_to_fit(m.ptr))
}

// nNav makes one ordered read into buf, which is never empty.
func (m *SyncOrderedBytesMap) nNav(op navOp, key, buf []byte) (int32, uintptr, uint64) {
	defer runtime.KeepAlive(key)
	defer runtime.KeepAlive(buf)
	var req C.size_t
	var val C.uint64_t
	out := unsafe.Pointer(&buf[0])
	n := C.size_t(len(buf))
	k := cptr(key)
	kl := C.size_t(len(key))
	var st C.expanse_ordered_bytes_nav_status
	switch op {
	case navFirst:
		st = C.expanse_sync_ordered_bytesmap_first(m.ptr, out, n, &req, &val)
	case navLast:
		st = C.expanse_sync_ordered_bytesmap_last(m.ptr, out, n, &req, &val)
	case navNextAtOrAfter:
		st = C.expanse_sync_ordered_bytesmap_next_at_or_after(m.ptr, k, kl, out, n, &req, &val)
	case navNextAfter:
		st = C.expanse_sync_ordered_bytesmap_next_after(m.ptr, k, kl, out, n, &req, &val)
	case navPrevAtOrBefore:
		st = C.expanse_sync_ordered_bytesmap_prev_at_or_before(m.ptr, k, kl, out, n, &req, &val)
	default:
		st = C.expanse_sync_ordered_bytesmap_prev_before(m.ptr, k, kl, out, n, &req, &val)
	}
	return int32(st), uintptr(req), uint64(val)
}

func (r *SyncOrderedBytesMapReader) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v C.uint64_t
	ok := bool(C.expanse_sync_ordered_bytesmap_reader_get(r.ptr, cptr(key), C.size_t(len(key)), &v))
	return uint64(v), ok
}

func (r *SyncOrderedBytesMapReader) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return bool(C.expanse_sync_ordered_bytesmap_reader_contains(r.ptr, cptr(key), C.size_t(len(key))))
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the nil check and the store are unguarded.
func (r *SyncOrderedBytesMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != nil {
		C.expanse_sync_ordered_bytesmap_reader_free(r.ptr)
		r.ptr = nil
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncOrderedBytesMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != nil {
		C.expanse_sync_ordered_bytesmap_free(m.ptr)
		m.ptr = nil
	}
}

// ---- SyncBlobMap ----

// SyncBlobMap is a concurrent uint64 -> blob map, backed by
// expanse_sync_blob_map_t. Every read copies: no call returns memory that a
// later insert, remove or compaction could invalidate.
type SyncBlobMap struct {
	ptr *C.expanse_sync_blob_map_t
}

// SyncBlobMapReader is a registered reader handle. See SyncStrMapReader.
type SyncBlobMapReader struct {
	ptr    *C.expanse_sync_blob_map_reader_t
	parent *SyncBlobMap
}

// NewSyncBlobMap returns an empty map with the given arena chunk size (0
// selects the default 2 MiB chunk). Call Free when done, or let the finalizer
// release the native handle.
func NewSyncBlobMap(chunkSize uint64) *SyncBlobMap {
	m := &SyncBlobMap{ptr: C.expanse_sync_blob_map_new(C.size_t(chunkSize))}
	runtime.SetFinalizer(m, (*SyncBlobMap).Free)
	return m
}

// NewSyncBlobMapWithCapacity is NewSyncBlobMap with an arena capacity cap of
// maxCapacity bytes of allocated chunks (0: the default 1 GiB), clamped to
// [chunkSize, 64 GiB], as NewBlobMapWithCapacity.
func NewSyncBlobMapWithCapacity(chunkSize, maxCapacity uint64) *SyncBlobMap {
	m := &SyncBlobMap{ptr: C.expanse_sync_blob_map_new_with_capacity(C.size_t(chunkSize), C.size_t(maxCapacity))}
	runtime.SetFinalizer(m, (*SyncBlobMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncBlobMap) Reader() *SyncBlobMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncBlobMapReader{ptr: C.expanse_sync_blob_map_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncBlobMapReader).Free)
	return r
}

func (m *SyncBlobMap) nInsert(key uint64, data []byte, meta uint32) int32 {
	defer runtime.KeepAlive(data)
	return int32(C.expanse_sync_blob_map_insert(m.ptr, C.uint64_t(key), (*C.uint8_t)(cptr(data)), C.size_t(len(data)), C.uint32_t(meta)))
}

func (m *SyncBlobMap) nRemove(key uint64) bool {
	return bool(C.expanse_sync_blob_map_remove(m.ptr, C.uint64_t(key)))
}

func (m *SyncBlobMap) nGetInto(key uint64, buf []byte) (int32, uintptr, uint32) {
	defer runtime.KeepAlive(buf)
	var n C.size_t
	var meta C.uint32_t
	st := C.expanse_sync_blob_map_get_into(m.ptr, C.uint64_t(key), (*C.uint8_t)(cptr(buf)), C.size_t(len(buf)), &n, &meta)
	return int32(st), uintptr(n), uint32(meta)
}

func (m *SyncBlobMap) nGetMeta(key uint64) (uint32, bool) {
	var meta C.uint32_t
	ok := bool(C.expanse_sync_blob_map_get_meta(m.ptr, C.uint64_t(key), &meta))
	return uint32(meta), ok
}

func (m *SyncBlobMap) nContains(key uint64) bool {
	return bool(C.expanse_sync_blob_map_contains(m.ptr, C.uint64_t(key)))
}

func (m *SyncBlobMap) nLen() uint64 { return uint64(C.expanse_sync_blob_map_len(m.ptr)) }
func (m *SyncBlobMap) nMemUsed() uint64 {
	return uint64(C.expanse_sync_blob_map_mem_used(m.ptr))
}
func (m *SyncBlobMap) nMemUsedEstimate() uint64 {
	return uint64(C.expanse_sync_blob_map_mem_used_estimate(m.ptr))
}
func (m *SyncBlobMap) nMemHeld() uint64 { return uint64(C.expanse_sync_blob_map_mem_held(m.ptr)) }
func (m *SyncBlobMap) nShrink() uint64 {
	return uint64(C.expanse_sync_blob_map_shrink_to_fit(m.ptr))
}
func (m *SyncBlobMap) nSetReclaimAtCap(on bool) {
	C.expanse_sync_blob_map_set_reclaim_at_cap(m.ptr, C.bool(on))
}
func (m *SyncBlobMap) nCompact() int32 { return int32(C.expanse_sync_blob_map_compact(m.ptr)) }

// nCompareExchange makes one compare_exchange call whose observed value, if
// any, lands in buf.
func (m *SyncBlobMap) nCompareExchange(key uint64, expected, newValue BlobValue, buf []byte) (int32, bool, uintptr, uint32) {
	ed, nd := blobData(expected), blobData(newValue)
	defer runtime.KeepAlive(ed)
	defer runtime.KeepAlive(nd)
	defer runtime.KeepAlive(buf)
	var seenPresent C.bool
	var n C.size_t
	var meta C.uint32_t
	st := C.expanse_sync_blob_map_compare_exchange(m.ptr, C.uint64_t(key),
		C.bool(expected.Present), (*C.uint8_t)(cptr(ed)), C.size_t(len(ed)), C.uint32_t(expected.Meta),
		C.bool(newValue.Present), (*C.uint8_t)(cptr(nd)), C.size_t(len(nd)), C.uint32_t(newValue.Meta),
		&seenPresent, (*C.uint8_t)(cptr(buf)), C.size_t(len(buf)), &n, &meta)
	return int32(st), bool(seenPresent), uintptr(n), uint32(meta)
}

func (m *SyncBlobMap) nCompareExchangeMeta(key uint64, expected, newValue BlobMeta) (int32, BlobMeta) {
	var seenPresent C.bool
	var seen C.uint32_t
	st := C.expanse_sync_blob_map_compare_exchange_meta(m.ptr, C.uint64_t(key),
		C.bool(expected.Present), C.uint32_t(expected.Meta),
		C.bool(newValue.Present), C.uint32_t(newValue.Meta),
		&seenPresent, &seen)
	return int32(st), BlobMeta{Meta: uint32(seen), Present: bool(seenPresent)}
}

func (m *SyncBlobMap) nGetOrInsert(key uint64, data []byte, meta uint32, buf []byte) (int32, uintptr, uint32) {
	defer runtime.KeepAlive(data)
	defer runtime.KeepAlive(buf)
	var n C.size_t
	var seen C.uint32_t
	st := C.expanse_sync_blob_map_get_or_insert(m.ptr, C.uint64_t(key),
		(*C.uint8_t)(cptr(data)), C.size_t(len(data)), C.uint32_t(meta),
		(*C.uint8_t)(cptr(buf)), C.size_t(len(buf)), &n, &seen)
	return int32(st), uintptr(n), uint32(seen)
}

func (r *SyncBlobMapReader) nGetInto(key uint64, buf []byte) (int32, uintptr, uint32) {
	defer runtime.KeepAlive(buf)
	var n C.size_t
	var meta C.uint32_t
	st := C.expanse_sync_blob_map_reader_get_into(r.ptr, C.uint64_t(key), (*C.uint8_t)(cptr(buf)), C.size_t(len(buf)), &n, &meta)
	return int32(st), uintptr(n), uint32(meta)
}

func (r *SyncBlobMapReader) nGetMeta(key uint64) (uint32, bool) {
	var meta C.uint32_t
	ok := bool(C.expanse_sync_blob_map_reader_get_meta(r.ptr, C.uint64_t(key), &meta))
	return uint32(meta), ok
}

func (r *SyncBlobMapReader) nContains(key uint64) bool {
	return bool(C.expanse_sync_blob_map_reader_contains(r.ptr, C.uint64_t(key)))
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the nil check and the store are unguarded.
func (r *SyncBlobMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != nil {
		C.expanse_sync_blob_map_reader_free(r.ptr)
		r.ptr = nil
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncBlobMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != nil {
		C.expanse_sync_blob_map_free(m.ptr)
		m.ptr = nil
	}
}
