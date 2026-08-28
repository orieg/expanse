//go:build cgo && !expanse_purego

package expanse

// #include <stdlib.h>
// #include "expanse.h"
import "C"
import (
	"runtime"
	"unsafe"
)

type BytesMap struct {
	ptr *C.expanse_bytesmap_t
}

func NewBytesMap() *BytesMap {
	m := &BytesMap{
		ptr: C.expanse_bytesmap_new(),
	}
	runtime.SetFinalizer(m, (*BytesMap).Free)
	return m
}

func (m *BytesMap) Set(key []byte, value uint64) {
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	C.expanse_bytesmap_insert(m.ptr, cKey, C.size_t(len(key)), C.uint64_t(value), nil)
}

func (m *BytesMap) Get(key []byte) (uint64, bool) {
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	var val C.uint64_t
	if bool(C.expanse_bytesmap_get(m.ptr, cKey, C.size_t(len(key)), &val)) {
		return uint64(val), true
	}
	return 0, false
}

func (m *BytesMap) Delete(key []byte) bool {
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	return bool(C.expanse_bytesmap_remove(m.ptr, cKey, C.size_t(len(key)), nil))
}

func (m *BytesMap) Contains(key []byte) bool {
	var cKey unsafe.Pointer
	if len(key) > 0 {
		cKey = unsafe.Pointer(&key[0])
	}
	var val C.uint64_t
	return bool(C.expanse_bytesmap_get(m.ptr, cKey, C.size_t(len(key)), &val))
}

func (m *BytesMap) Size() uint64 {
	return uint64(C.expanse_bytesmap_len(m.ptr))
}

func (m *BytesMap) MemoryUsed() uint64 {
	return uint64(C.expanse_bytesmap_mem_used(m.ptr))
}

func (m *BytesMap) Clear() {
	C.expanse_bytesmap_clear(m.ptr)
}

func (m *BytesMap) Free() {
	if m.ptr != nil {
		C.expanse_bytesmap_free(m.ptr)
		m.ptr = nil
	}
}
