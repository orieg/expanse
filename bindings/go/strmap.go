package expanse

// #include <stdlib.h>
// #include "expanse.h"
import "C"
import (
	"runtime"
	"unsafe"
)

type StrMap struct {
	ptr *C.expanse_strmap_t
}

func NewStrMap() *StrMap {
	m := &StrMap{
		ptr: C.expanse_strmap_new(),
	}
	runtime.SetFinalizer(m, (*StrMap).Free)
	return m
}

func (m *StrMap) Set(key string, value uint64) {
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	C.expanse_strmap_insert(m.ptr, cKey, C.uint64_t(value), nil)
}

func (m *StrMap) Get(key string) (uint64, bool) {
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	var val C.uint64_t
	if bool(C.expanse_strmap_get(m.ptr, cKey, &val)) {
		return uint64(val), true
	}
	return 0, false
}

func (m *StrMap) Delete(key string) bool {
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	return bool(C.expanse_strmap_remove(m.ptr, cKey, nil))
}

func (m *StrMap) Contains(key string) bool {
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	var val C.uint64_t
	return bool(C.expanse_strmap_get(m.ptr, cKey, &val))
}

func (m *StrMap) Size() uint64 {
	return uint64(C.expanse_strmap_len(m.ptr))
}

func (m *StrMap) Clear() {
	C.expanse_strmap_clear(m.ptr)
}

func (m *StrMap) Free() {
	if m.ptr != nil {
		C.expanse_strmap_free(m.ptr)
		m.ptr = nil
	}
}
