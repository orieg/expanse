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
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	expanse_bytesmap_insert(m.ptr, cKey, uintptr(len(key)), value, nil)
}

func (m *BytesMap) Get(key []byte) (uint64, bool) {
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
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	return expanse_bytesmap_remove(m.ptr, cKey, uintptr(len(key)), nil)
}

func (m *BytesMap) Contains(key []byte) bool {
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	var val uint64
	return expanse_bytesmap_get(m.ptr, cKey, uintptr(len(key)), &val)
}

func (m *BytesMap) Size() uint64 {
	return expanse_bytesmap_len(m.ptr)
}

func (m *BytesMap) MemoryUsed() uint64 {
	return uint64(expanse_bytesmap_mem_used(m.ptr))
}

func (m *BytesMap) Clear() {
	expanse_bytesmap_clear(m.ptr)
}

func (m *BytesMap) Free() {
	if m.ptr != 0 {
		expanse_bytesmap_free(m.ptr)
		m.ptr = 0
	}
}
