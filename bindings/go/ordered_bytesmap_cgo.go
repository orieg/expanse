//go:build cgo && !expanse_purego

package expanse

// #include <stdlib.h>
// #include "expanse.h"
import "C"
import (
	"runtime"
	"unsafe"
)

// OrderedBytesMap is an ordered map from arbitrary byte-slice keys to uint64
// values, backed by ExpanseOrderedBytesMap (expanse_ordered_bytesmap_*).
//
// Keys compare as unsigned bytes in lexicographic order, and a key sorts
// before every longer key it prefixes. Keys may be empty and may contain any
// byte, including 0x00 and 0xFF. Navigation returns a fresh copy of the key,
// so no returned slice aliases native memory.
//
// A pointer returned by Slot or InsSlot is valid only until the next
// structural mutation of the map. Fetch it again after any call that changes
// the map, and keep the OrderedBytesMap referenced while the pointer is in use.
type OrderedBytesMap struct {
	ptr *C.expanse_ordered_bytesmap_t
}

// NewOrderedBytesMap returns an empty map. Call Free when done, or let the
// finalizer release the native handle.
func NewOrderedBytesMap() *OrderedBytesMap {
	m := &OrderedBytesMap{
		ptr: C.expanse_ordered_bytesmap_new(),
	}
	runtime.SetFinalizer(m, (*OrderedBytesMap).Free)
	return m
}

// ordBytesPtr returns the key's address for a C call. An empty key is passed
// as NULL with length zero, which the C ABI accepts.
func ordBytesPtr(key []byte) unsafe.Pointer {
	if len(key) == 0 {
		return nil
	}
	return unsafe.Pointer(&key[0])
}

// Set stores value under key, replacing any previous value. It reports whether
// key was newly inserted.
func (m *OrderedBytesMap) Set(key []byte, value uint64) bool {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return bool(C.expanse_ordered_bytesmap_insert(m.ptr, ordBytesPtr(key), C.size_t(len(key)), C.uint64_t(value), nil))
}

// Get returns the value stored under key and whether key is present.
func (m *OrderedBytesMap) Get(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	var val C.uint64_t
	if bool(C.expanse_ordered_bytesmap_get(m.ptr, ordBytesPtr(key), C.size_t(len(key)), &val)) {
		return uint64(val), true
	}
	return 0, false
}

// Contains reports whether key is present.
func (m *OrderedBytesMap) Contains(key []byte) bool {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return bool(C.expanse_ordered_bytesmap_contains(m.ptr, ordBytesPtr(key), C.size_t(len(key))))
}

// Delete removes key and reports whether it was present.
func (m *OrderedBytesMap) Delete(key []byte) bool {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return bool(C.expanse_ordered_bytesmap_remove(m.ptr, ordBytesPtr(key), C.size_t(len(key)), nil))
}

// Slot returns a writable pointer to the value stored under key, or nil when
// key is absent. The pointer is valid until the next structural mutation.
func (m *OrderedBytesMap) Slot(key []byte) *uint64 {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return (*uint64)(unsafe.Pointer(C.expanse_ordered_bytesmap_slot(m.ptr, ordBytesPtr(key), C.size_t(len(key)))))
}

// InsSlot inserts key with value 0 if it is absent (an existing value is kept)
// and returns a writable pointer to its value. The pointer is valid until the
// next structural mutation.
func (m *OrderedBytesMap) InsSlot(key []byte) *uint64 {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	p := (*uint64)(unsafe.Pointer(C.expanse_ordered_bytesmap_ins_slot(m.ptr, ordBytesPtr(key), C.size_t(len(key)))))
	if p == nil {
		panic("expanse: expanse_ordered_bytesmap_ins_slot returned NULL")
	}
	return p
}

// Size returns the number of entries.
func (m *OrderedBytesMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(C.expanse_ordered_bytesmap_len(m.ptr))
}

// MemoryUsed returns the native heap bytes used by the live structure.
func (m *OrderedBytesMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(C.expanse_ordered_bytesmap_mem_used(m.ptr))
}

// MemoryHeld returns the native heap bytes held: MemoryUsed plus retained free
// capacity.
func (m *OrderedBytesMap) MemoryHeld() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(C.expanse_ordered_bytesmap_mem_held(m.ptr))
}

// ShrinkToFit returns retained free capacity to the allocator and reports the
// bytes released. MemoryUsed is unchanged.
func (m *OrderedBytesMap) ShrinkToFit() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(C.expanse_ordered_bytesmap_shrink_to_fit(m.ptr))
}

// Clear removes every entry.
func (m *OrderedBytesMap) Clear() {
	defer runtime.KeepAlive(m)
	C.expanse_ordered_bytesmap_clear(m.ptr)
}

// ordBytesNav runs one navigation call. It retries once at the exact length
// the native call reports when the first buffer is too small, and returns a
// copy of the key. ok is false when no key matched.
func ordBytesNav(call func(buf unsafe.Pointer, bufLen C.size_t, reqLen *C.size_t, val *C.uint64_t) C.expanse_ordered_bytes_nav_status) ([]byte, uint64, bool) {
	buf := make([]byte, ordBytesNavBufLen)
	var reqLen C.size_t
	var val C.uint64_t
	status := call(unsafe.Pointer(&buf[0]), C.size_t(len(buf)), &reqLen, &val)
	if status == C.EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL {
		buf = make([]byte, reqLen)
		status = call(unsafe.Pointer(&buf[0]), C.size_t(len(buf)), &reqLen, &val)
		if status == C.EXPANSE_ORDERED_BYTES_NAV_BUFFER_TOO_SMALL {
			panic(ordBytesNavRetryFailed)
		}
	}
	if status != C.EXPANSE_ORDERED_BYTES_NAV_OK {
		return nil, 0, false
	}
	key := make([]byte, int(reqLen))
	copy(key, buf)
	runtime.KeepAlive(buf)
	return key, uint64(val), true
}

// First returns the entry with the smallest key.
func (m *OrderedBytesMap) First() ([]byte, uint64, bool) {
	defer runtime.KeepAlive(m)
	return ordBytesNav(func(buf unsafe.Pointer, bufLen C.size_t, reqLen *C.size_t, val *C.uint64_t) C.expanse_ordered_bytes_nav_status {
		return C.expanse_ordered_bytesmap_first(m.ptr, buf, bufLen, reqLen, val)
	})
}

// Last returns the entry with the largest key.
func (m *OrderedBytesMap) Last() ([]byte, uint64, bool) {
	defer runtime.KeepAlive(m)
	return ordBytesNav(func(buf unsafe.Pointer, bufLen C.size_t, reqLen *C.size_t, val *C.uint64_t) C.expanse_ordered_bytes_nav_status {
		return C.expanse_ordered_bytesmap_last(m.ptr, buf, bufLen, reqLen, val)
	})
}

// Next returns the entry with the smallest key strictly greater than key.
func (m *OrderedBytesMap) Next(key []byte) ([]byte, uint64, bool) {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return ordBytesNav(func(buf unsafe.Pointer, bufLen C.size_t, reqLen *C.size_t, val *C.uint64_t) C.expanse_ordered_bytes_nav_status {
		return C.expanse_ordered_bytesmap_next_after(m.ptr, ordBytesPtr(key), C.size_t(len(key)), buf, bufLen, reqLen, val)
	})
}

// NextAtOrAfter returns the entry with the smallest key greater than or equal
// to key.
func (m *OrderedBytesMap) NextAtOrAfter(key []byte) ([]byte, uint64, bool) {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return ordBytesNav(func(buf unsafe.Pointer, bufLen C.size_t, reqLen *C.size_t, val *C.uint64_t) C.expanse_ordered_bytes_nav_status {
		return C.expanse_ordered_bytesmap_next_at_or_after(m.ptr, ordBytesPtr(key), C.size_t(len(key)), buf, bufLen, reqLen, val)
	})
}

// Prev returns the entry with the largest key strictly less than key.
func (m *OrderedBytesMap) Prev(key []byte) ([]byte, uint64, bool) {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return ordBytesNav(func(buf unsafe.Pointer, bufLen C.size_t, reqLen *C.size_t, val *C.uint64_t) C.expanse_ordered_bytes_nav_status {
		return C.expanse_ordered_bytesmap_prev_before(m.ptr, ordBytesPtr(key), C.size_t(len(key)), buf, bufLen, reqLen, val)
	})
}

// PrevAtOrBefore returns the entry with the largest key less than or equal to
// key.
func (m *OrderedBytesMap) PrevAtOrBefore(key []byte) ([]byte, uint64, bool) {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	return ordBytesNav(func(buf unsafe.Pointer, bufLen C.size_t, reqLen *C.size_t, val *C.uint64_t) C.expanse_ordered_bytes_nav_status {
		return C.expanse_ordered_bytesmap_prev_at_or_before(m.ptr, ordBytesPtr(key), C.size_t(len(key)), buf, bufLen, reqLen, val)
	})
}

// Free releases the native handle. It is idempotent and optional -- the
// finalizer performs the same release -- but calling it concurrently from
// two goroutines is caller error: the check and the store are unguarded.
func (m *OrderedBytesMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != nil {
		C.expanse_ordered_bytesmap_free(m.ptr)
		m.ptr = nil
	}
}
