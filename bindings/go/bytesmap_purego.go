//go:build !cgo || expanse_purego

package expanse

import (
	"runtime"
	"unsafe"
)

type BytesMap struct {
	ptr uintptr
}

func NewBytesMap() *BytesMap {
	ensureLoaded()
	m := &BytesMap{
		ptr: expanse_bytesmap_new(),
	}
	runtime.SetFinalizer(m, (*BytesMap).Free)
	return m
}

func (m *BytesMap) Set(key []byte, value uint64) {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	expanse_bytesmap_insert(m.ptr, cKey, uintptr(len(key)), value, nil)
}

func (m *BytesMap) Get(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	var val uint64
	if expanse_bytesmap_get(m.ptr, cKey, uintptr(len(key)), &val) {
		return val, true
	}
	return 0, false
}

func (m *BytesMap) Delete(key []byte) bool {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	return expanse_bytesmap_remove(m.ptr, cKey, uintptr(len(key)), nil)
}

func (m *BytesMap) Contains(key []byte) bool {
	defer runtime.KeepAlive(m)
	defer runtime.KeepAlive(key)
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	var val uint64
	return expanse_bytesmap_get(m.ptr, cKey, uintptr(len(key)), &val)
}

func (m *BytesMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return expanse_bytesmap_len(m.ptr)
}

func (m *BytesMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(expanse_bytesmap_mem_used(m.ptr))
}

func (m *BytesMap) Clear() {
	defer runtime.KeepAlive(m)
	expanse_bytesmap_clear(m.ptr)
}

// Free releases the native handle. It is idempotent and optional -- the
// finalizer performs the same release -- but calling it concurrently from
// two goroutines is caller error: the check and the store are unguarded.
func (m *BytesMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != 0 {
		expanse_bytesmap_free(m.ptr)
		m.ptr = 0
	}
}
