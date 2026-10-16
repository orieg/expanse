//go:build !cgo || expanse_purego

package expanse

import (
	"runtime"
	"unsafe"
)

// This file is the purego half of the shared string, bytes, ordered-bytes and
// blob maps: handle types, constructors, Free, and the one-call native shims
// the shared logic in sync_maps.go and sync_shared.go calls. A shim makes one
// native call and converts types; it decides nothing. The handle is a uintptr,
// invisible to the collector, so each reader's parent field is the only thing
// that carries the "map outlives reader" requirement on this build (see
// SyncSetReader).

// ---- SyncStrMap ----

// SyncStrMap is a concurrent ordered map from NUL-free string keys to uint64
// values, backed by expanse_sync_strmap_t. Writers serialise on one mutex;
// point reads and the reader's ordered reads are optimistic.
type SyncStrMap struct {
	ptr uintptr
}

// SyncStrMapReader is a registered reader handle. It borrows from the map it
// was taken from, so it holds the map reachable (see SyncSetReader). One
// reader per goroutine at a time.
type SyncStrMapReader struct {
	ptr    uintptr
	parent *SyncStrMap
}

// NewSyncStrMap returns an empty map. Call Free when done, or let the
// finalizer release the native handle.
func NewSyncStrMap() *SyncStrMap {
	ensureLoaded()
	m := &SyncStrMap{ptr: expanse_sync_strmap_new()}
	runtime.SetFinalizer(m, (*SyncStrMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncStrMap) Reader() *SyncStrMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncStrMapReader{ptr: expanse_sync_strmap_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncStrMapReader).Free)
	return r
}

func (m *SyncStrMap) nInsert(key []byte, value uint64) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_strmap_insert(m.ptr, cptr(key), value, nil)
}

func (m *SyncStrMap) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v uint64
	ok := expanse_sync_strmap_get(m.ptr, cptr(key), &v)
	return v, ok
}

func (m *SyncStrMap) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_strmap_contains(m.ptr, cptr(key))
}

func (m *SyncStrMap) nRemove(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_strmap_remove(m.ptr, cptr(key), nil)
}

func (m *SyncStrMap) nCompareExchange(key []byte, expected, newValue Word) (Word, bool) {
	defer runtime.KeepAlive(key)
	var seenPresent bool
	var seen uint64
	stored := expanse_sync_strmap_compare_exchange(m.ptr, cptr(key),
		expected.Present, expected.Value, newValue.Present, newValue.Value,
		&seenPresent, &seen)
	return Word{Value: seen, Present: seenPresent}, stored
}

// nGetOrInsert reports inserted true when value was stored; otherwise existing
// is the value that was there.
func (m *SyncStrMap) nGetOrInsert(key []byte, value uint64) (existing uint64, inserted bool) {
	defer runtime.KeepAlive(key)
	inserted = expanse_sync_strmap_get_or_insert(m.ptr, cptr(key), value, &existing)
	return existing, inserted
}

func (m *SyncStrMap) nLen() uint64     { return expanse_sync_strmap_len(m.ptr) }
func (m *SyncStrMap) nMemUsed() uint64 { return uint64(expanse_sync_strmap_mem_used(m.ptr)) }
func (m *SyncStrMap) nMemHeld() uint64 { return uint64(expanse_sync_strmap_mem_held(m.ptr)) }
func (m *SyncStrMap) nShrink() uint64  { return uint64(expanse_sync_strmap_shrink_to_fit(m.ptr)) }

func (r *SyncStrMapReader) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v uint64
	ok := expanse_sync_strmap_reader_get(r.ptr, cptr(key), &v)
	return v, ok
}

func (r *SyncStrMapReader) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_strmap_reader_contains(r.ptr, cptr(key))
}

// nNav makes one ordered read into buf, which is never empty.
func (r *SyncStrMapReader) nNav(op navOp, key, buf []byte) (int32, uintptr, uint64) {
	defer runtime.KeepAlive(key)
	defer runtime.KeepAlive(buf)
	var req uintptr
	var val uint64
	out := unsafe.Pointer(&buf[0])
	n := uintptr(len(buf))
	k := cptr(key)
	var st int32
	switch op {
	case navFirst:
		st = expanse_sync_strmap_reader_first(r.ptr, out, n, &req, &val)
	case navLast:
		st = expanse_sync_strmap_reader_last(r.ptr, out, n, &req, &val)
	case navNextAtOrAfter:
		st = expanse_sync_strmap_reader_next_at_or_after(r.ptr, k, out, n, &req, &val)
	case navNextAfter:
		st = expanse_sync_strmap_reader_next_after(r.ptr, k, out, n, &req, &val)
	case navPrevAtOrBefore:
		st = expanse_sync_strmap_reader_prev_at_or_before(r.ptr, k, out, n, &req, &val)
	default:
		st = expanse_sync_strmap_reader_prev_before(r.ptr, k, out, n, &req, &val)
	}
	return st, req, val
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the zero check and the store are unguarded.
func (r *SyncStrMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != 0 {
		expanse_sync_strmap_reader_free(r.ptr)
		r.ptr = 0
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncStrMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != 0 {
		expanse_sync_strmap_free(m.ptr)
		m.ptr = 0
	}
}

// ---- SyncBytesMap ----

// SyncBytesMap is a concurrent unordered map from arbitrary byte-slice keys to
// uint64 values, backed by expanse_sync_bytesmap_t. It has no ordered reads;
// SyncOrderedBytesMap does.
type SyncBytesMap struct {
	ptr uintptr
}

// SyncBytesMapReader is a registered reader handle. See SyncStrMapReader.
type SyncBytesMapReader struct {
	ptr    uintptr
	parent *SyncBytesMap
}

// NewSyncBytesMap returns an empty map. Call Free when done, or let the
// finalizer release the native handle.
func NewSyncBytesMap() *SyncBytesMap {
	ensureLoaded()
	m := &SyncBytesMap{ptr: expanse_sync_bytesmap_new()}
	runtime.SetFinalizer(m, (*SyncBytesMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncBytesMap) Reader() *SyncBytesMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncBytesMapReader{ptr: expanse_sync_bytesmap_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncBytesMapReader).Free)
	return r
}

func (m *SyncBytesMap) nInsert(key []byte, value uint64) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_bytesmap_insert(m.ptr, cptr(key), uintptr(len(key)), value, nil)
}

func (m *SyncBytesMap) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v uint64
	ok := expanse_sync_bytesmap_get(m.ptr, cptr(key), uintptr(len(key)), &v)
	return v, ok
}

func (m *SyncBytesMap) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_bytesmap_contains(m.ptr, cptr(key), uintptr(len(key)))
}

func (m *SyncBytesMap) nRemove(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_bytesmap_remove(m.ptr, cptr(key), uintptr(len(key)), nil)
}

func (m *SyncBytesMap) nCompareExchange(key []byte, expected, newValue Word) (Word, bool) {
	defer runtime.KeepAlive(key)
	var seenPresent bool
	var seen uint64
	stored := expanse_sync_bytesmap_compare_exchange(m.ptr, cptr(key), uintptr(len(key)),
		expected.Present, expected.Value, newValue.Present, newValue.Value,
		&seenPresent, &seen)
	return Word{Value: seen, Present: seenPresent}, stored
}

func (m *SyncBytesMap) nGetOrInsert(key []byte, value uint64) (existing uint64, inserted bool) {
	defer runtime.KeepAlive(key)
	inserted = expanse_sync_bytesmap_get_or_insert(m.ptr, cptr(key), uintptr(len(key)), value, &existing)
	return existing, inserted
}

func (m *SyncBytesMap) nLen() uint64     { return expanse_sync_bytesmap_len(m.ptr) }
func (m *SyncBytesMap) nMemUsed() uint64 { return uint64(expanse_sync_bytesmap_mem_used(m.ptr)) }
func (m *SyncBytesMap) nMemHeld() uint64 { return uint64(expanse_sync_bytesmap_mem_held(m.ptr)) }
func (m *SyncBytesMap) nShrink() uint64  { return uint64(expanse_sync_bytesmap_shrink_to_fit(m.ptr)) }

func (r *SyncBytesMapReader) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v uint64
	ok := expanse_sync_bytesmap_reader_get(r.ptr, cptr(key), uintptr(len(key)), &v)
	return v, ok
}

func (r *SyncBytesMapReader) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_bytesmap_reader_contains(r.ptr, cptr(key), uintptr(len(key)))
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the zero check and the store are unguarded.
func (r *SyncBytesMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != 0 {
		expanse_sync_bytesmap_reader_free(r.ptr)
		r.ptr = 0
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncBytesMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != 0 {
		expanse_sync_bytesmap_free(m.ptr)
		m.ptr = 0
	}
}

// ---- SyncOrderedBytesMap ----

// SyncOrderedBytesMap is a concurrent ordered map from arbitrary byte-slice
// keys to uint64 values, backed by expanse_sync_ordered_bytesmap_t. Its ordered
// reads are calls on the map and block writers; see First.
type SyncOrderedBytesMap struct {
	ptr uintptr
}

// SyncOrderedBytesMapReader is a registered reader handle. It offers point
// reads only; the ordered reads are on the map. See SyncStrMapReader.
type SyncOrderedBytesMapReader struct {
	ptr    uintptr
	parent *SyncOrderedBytesMap
}

// NewSyncOrderedBytesMap returns an empty map. Call Free when done, or let the
// finalizer release the native handle.
func NewSyncOrderedBytesMap() *SyncOrderedBytesMap {
	ensureLoaded()
	m := &SyncOrderedBytesMap{ptr: expanse_sync_ordered_bytesmap_new()}
	runtime.SetFinalizer(m, (*SyncOrderedBytesMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncOrderedBytesMap) Reader() *SyncOrderedBytesMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncOrderedBytesMapReader{ptr: expanse_sync_ordered_bytesmap_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncOrderedBytesMapReader).Free)
	return r
}

func (m *SyncOrderedBytesMap) nInsert(key []byte, value uint64) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_ordered_bytesmap_insert(m.ptr, cptr(key), uintptr(len(key)), value, nil)
}

func (m *SyncOrderedBytesMap) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v uint64
	ok := expanse_sync_ordered_bytesmap_get(m.ptr, cptr(key), uintptr(len(key)), &v)
	return v, ok
}

func (m *SyncOrderedBytesMap) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_ordered_bytesmap_contains(m.ptr, cptr(key), uintptr(len(key)))
}

func (m *SyncOrderedBytesMap) nRemove(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_ordered_bytesmap_remove(m.ptr, cptr(key), uintptr(len(key)), nil)
}

func (m *SyncOrderedBytesMap) nCompareExchange(key []byte, expected, newValue Word) (Word, bool) {
	defer runtime.KeepAlive(key)
	var seenPresent bool
	var seen uint64
	stored := expanse_sync_ordered_bytesmap_compare_exchange(m.ptr, cptr(key), uintptr(len(key)),
		expected.Present, expected.Value, newValue.Present, newValue.Value,
		&seenPresent, &seen)
	return Word{Value: seen, Present: seenPresent}, stored
}

func (m *SyncOrderedBytesMap) nGetOrInsert(key []byte, value uint64) (existing uint64, inserted bool) {
	defer runtime.KeepAlive(key)
	inserted = expanse_sync_ordered_bytesmap_get_or_insert(m.ptr, cptr(key), uintptr(len(key)), value, &existing)
	return existing, inserted
}

func (m *SyncOrderedBytesMap) nLen() uint64 { return expanse_sync_ordered_bytesmap_len(m.ptr) }
func (m *SyncOrderedBytesMap) nMemUsed() uint64 {
	return uint64(expanse_sync_ordered_bytesmap_mem_used(m.ptr))
}
func (m *SyncOrderedBytesMap) nMemHeld() uint64 {
	return uint64(expanse_sync_ordered_bytesmap_mem_held(m.ptr))
}
func (m *SyncOrderedBytesMap) nShrink() uint64 {
	return uint64(expanse_sync_ordered_bytesmap_shrink_to_fit(m.ptr))
}

// nNav makes one ordered read into buf, which is never empty.
func (m *SyncOrderedBytesMap) nNav(op navOp, key, buf []byte) (int32, uintptr, uint64) {
	defer runtime.KeepAlive(key)
	defer runtime.KeepAlive(buf)
	var req uintptr
	var val uint64
	out := unsafe.Pointer(&buf[0])
	n := uintptr(len(buf))
	k := cptr(key)
	kl := uintptr(len(key))
	var st int32
	switch op {
	case navFirst:
		st = expanse_sync_ordered_bytesmap_first(m.ptr, out, n, &req, &val)
	case navLast:
		st = expanse_sync_ordered_bytesmap_last(m.ptr, out, n, &req, &val)
	case navNextAtOrAfter:
		st = expanse_sync_ordered_bytesmap_next_at_or_after(m.ptr, k, kl, out, n, &req, &val)
	case navNextAfter:
		st = expanse_sync_ordered_bytesmap_next_after(m.ptr, k, kl, out, n, &req, &val)
	case navPrevAtOrBefore:
		st = expanse_sync_ordered_bytesmap_prev_at_or_before(m.ptr, k, kl, out, n, &req, &val)
	default:
		st = expanse_sync_ordered_bytesmap_prev_before(m.ptr, k, kl, out, n, &req, &val)
	}
	return st, req, val
}

func (r *SyncOrderedBytesMapReader) nGet(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(key)
	var v uint64
	ok := expanse_sync_ordered_bytesmap_reader_get(r.ptr, cptr(key), uintptr(len(key)), &v)
	return v, ok
}

func (r *SyncOrderedBytesMapReader) nContains(key []byte) bool {
	defer runtime.KeepAlive(key)
	return expanse_sync_ordered_bytesmap_reader_contains(r.ptr, cptr(key), uintptr(len(key)))
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the zero check and the store are unguarded.
func (r *SyncOrderedBytesMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != 0 {
		expanse_sync_ordered_bytesmap_reader_free(r.ptr)
		r.ptr = 0
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncOrderedBytesMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != 0 {
		expanse_sync_ordered_bytesmap_free(m.ptr)
		m.ptr = 0
	}
}

// ---- SyncBlobMap ----

// SyncBlobMap is a concurrent uint64 -> blob map, backed by
// expanse_sync_blob_map_t. Every read copies: no call returns memory that a
// later insert, remove or compaction could invalidate.
type SyncBlobMap struct {
	ptr uintptr
}

// SyncBlobMapReader is a registered reader handle. See SyncStrMapReader.
type SyncBlobMapReader struct {
	ptr    uintptr
	parent *SyncBlobMap
}

// NewSyncBlobMap returns an empty map with the given arena chunk size (0
// selects the default 2 MiB chunk). Call Free when done, or let the finalizer
// release the native handle.
func NewSyncBlobMap(chunkSize uint64) *SyncBlobMap {
	ensureLoaded()
	m := &SyncBlobMap{ptr: expanse_sync_blob_map_new(uintptr(chunkSize))}
	runtime.SetFinalizer(m, (*SyncBlobMap).Free)
	return m
}

// NewSyncBlobMapWithCapacity is NewSyncBlobMap with an arena capacity cap of
// maxCapacity bytes of allocated chunks (0: the default 1 GiB), clamped to
// [chunkSize, 64 GiB], as NewBlobMapWithCapacity.
func NewSyncBlobMapWithCapacity(chunkSize, maxCapacity uint64) *SyncBlobMap {
	ensureLoaded()
	m := &SyncBlobMap{ptr: expanse_sync_blob_map_new_with_capacity(uintptr(chunkSize), uintptr(maxCapacity))}
	runtime.SetFinalizer(m, (*SyncBlobMap).Free)
	return m
}

// Reader returns a registered reader handle for the map. Free it before the map.
func (m *SyncBlobMap) Reader() *SyncBlobMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncBlobMapReader{ptr: expanse_sync_blob_map_reader_new(m.ptr), parent: m}
	runtime.SetFinalizer(r, (*SyncBlobMapReader).Free)
	return r
}

func (m *SyncBlobMap) nInsert(key uint64, data []byte, meta uint32) int32 {
	defer runtime.KeepAlive(data)
	return expanse_sync_blob_map_insert(m.ptr, key, cptr(data), uintptr(len(data)), meta)
}

func (m *SyncBlobMap) nRemove(key uint64) bool {
	return expanse_sync_blob_map_remove(m.ptr, key)
}

func (m *SyncBlobMap) nGetInto(key uint64, buf []byte) (int32, uintptr, uint32) {
	defer runtime.KeepAlive(buf)
	var n uintptr
	var meta uint32
	st := expanse_sync_blob_map_get_into(m.ptr, key, cptr(buf), uintptr(len(buf)), &n, &meta)
	return st, n, meta
}

func (m *SyncBlobMap) nGetMeta(key uint64) (uint32, bool) {
	var meta uint32
	ok := expanse_sync_blob_map_get_meta(m.ptr, key, &meta)
	return meta, ok
}

func (m *SyncBlobMap) nContains(key uint64) bool {
	return expanse_sync_blob_map_contains(m.ptr, key)
}

func (m *SyncBlobMap) nLen() uint64     { return expanse_sync_blob_map_len(m.ptr) }
func (m *SyncBlobMap) nMemUsed() uint64 { return uint64(expanse_sync_blob_map_mem_used(m.ptr)) }
func (m *SyncBlobMap) nMemUsedEstimate() uint64 {
	return uint64(expanse_sync_blob_map_mem_used_estimate(m.ptr))
}
func (m *SyncBlobMap) nMemHeld() uint64 { return uint64(expanse_sync_blob_map_mem_held(m.ptr)) }
func (m *SyncBlobMap) nShrink() uint64  { return uint64(expanse_sync_blob_map_shrink_to_fit(m.ptr)) }
func (m *SyncBlobMap) nSetReclaimAtCap(on bool) {
	expanse_sync_blob_map_set_reclaim_at_cap(m.ptr, on)
}
func (m *SyncBlobMap) nCompact() int32 { return expanse_sync_blob_map_compact(m.ptr) }

// nCompareExchange makes one compare_exchange call whose observed value, if
// any, lands in buf.
func (m *SyncBlobMap) nCompareExchange(key uint64, expected, newValue BlobValue, buf []byte) (int32, bool, uintptr, uint32) {
	ed, nd := blobData(expected), blobData(newValue)
	defer runtime.KeepAlive(ed)
	defer runtime.KeepAlive(nd)
	defer runtime.KeepAlive(buf)
	var seenPresent bool
	var n uintptr
	var meta uint32
	st := expanse_sync_blob_map_compare_exchange(m.ptr, key,
		expected.Present, cptr(ed), uintptr(len(ed)), expected.Meta,
		newValue.Present, cptr(nd), uintptr(len(nd)), newValue.Meta,
		&seenPresent, cptr(buf), uintptr(len(buf)), &n, &meta)
	return st, seenPresent, n, meta
}

func (m *SyncBlobMap) nCompareExchangeMeta(key uint64, expected, newValue BlobMeta) (int32, BlobMeta) {
	var seenPresent bool
	var seen uint32
	st := expanse_sync_blob_map_compare_exchange_meta(m.ptr, key,
		expected.Present, expected.Meta, newValue.Present, newValue.Meta,
		&seenPresent, &seen)
	return st, BlobMeta{Meta: seen, Present: seenPresent}
}

func (m *SyncBlobMap) nGetOrInsert(key uint64, data []byte, meta uint32, buf []byte) (int32, uintptr, uint32) {
	defer runtime.KeepAlive(data)
	defer runtime.KeepAlive(buf)
	var n uintptr
	var seen uint32
	st := expanse_sync_blob_map_get_or_insert(m.ptr, key,
		cptr(data), uintptr(len(data)), meta,
		cptr(buf), uintptr(len(buf)), &n, &seen)
	return st, n, seen
}

func (r *SyncBlobMapReader) nGetInto(key uint64, buf []byte) (int32, uintptr, uint32) {
	defer runtime.KeepAlive(buf)
	var n uintptr
	var meta uint32
	st := expanse_sync_blob_map_reader_get_into(r.ptr, key, cptr(buf), uintptr(len(buf)), &n, &meta)
	return st, n, meta
}

func (r *SyncBlobMapReader) nGetMeta(key uint64) (uint32, bool) {
	var meta uint32
	ok := expanse_sync_blob_map_reader_get_meta(r.ptr, key, &meta)
	return meta, ok
}

func (r *SyncBlobMapReader) nContains(key uint64) bool {
	return expanse_sync_blob_map_reader_contains(r.ptr, key)
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the zero check and the store are unguarded.
func (r *SyncBlobMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != 0 {
		expanse_sync_blob_map_reader_free(r.ptr)
		r.ptr = 0
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncBlobMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != 0 {
		expanse_sync_blob_map_free(m.ptr)
		m.ptr = 0
	}
}
