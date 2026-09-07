//go:build cgo && !expanse_purego

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
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	C.expanse_strmap_insert(m.ptr, cKey, C.uint64_t(value), nil)
}

func (m *StrMap) Get(key string) (uint64, bool) {
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	var val C.uint64_t
	if bool(C.expanse_strmap_get(m.ptr, cKey, &val)) {
		return uint64(val), true
	}
	return 0, false
}

func (m *StrMap) Delete(key string) bool {
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	return bool(C.expanse_strmap_remove(m.ptr, cKey, nil))
}

func (m *StrMap) Contains(key string) bool {
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	var val C.uint64_t
	return bool(C.expanse_strmap_get(m.ptr, cKey, &val))
}

func (m *StrMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(C.expanse_strmap_len(m.ptr))
}

func (m *StrMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(C.expanse_strmap_mem_used(m.ptr))
}

func (m *StrMap) Clear() {
	defer runtime.KeepAlive(m)
	C.expanse_strmap_clear(m.ptr)
}

func (m *StrMap) First() (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen C.size_t
	var val C.uint64_t
	status := C.expanse_strmap_first_ex(m.ptr, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	if status == C.EXPANSE_STR_NAV_BUFFER_TOO_SMALL && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = C.expanse_strmap_first_ex(m.ptr, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	}
	if status == C.EXPANSE_STR_NAV_OK {
		return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), uint64(val), true
	}
	return "", 0, false
}

func (m *StrMap) Last() (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen C.size_t
	var val C.uint64_t
	status := C.expanse_strmap_last_ex(m.ptr, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	if status == C.EXPANSE_STR_NAV_BUFFER_TOO_SMALL && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = C.expanse_strmap_last_ex(m.ptr, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	}
	if status == C.EXPANSE_STR_NAV_OK {
		return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), uint64(val), true
	}
	return "", 0, false
}

func (m *StrMap) Next(key string) (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen C.size_t
	var val C.uint64_t
	status := C.expanse_strmap_next_after_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	if status == C.EXPANSE_STR_NAV_BUFFER_TOO_SMALL && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = C.expanse_strmap_next_after_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	}
	if status == C.EXPANSE_STR_NAV_OK {
		return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), uint64(val), true
	}
	return "", 0, false
}

func (m *StrMap) NextAtOrAfter(key string) (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen C.size_t
	var val C.uint64_t
	status := C.expanse_strmap_next_at_or_after_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	if status == C.EXPANSE_STR_NAV_BUFFER_TOO_SMALL && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = C.expanse_strmap_next_at_or_after_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	}
	if status == C.EXPANSE_STR_NAV_OK {
		return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), uint64(val), true
	}
	return "", 0, false
}

func (m *StrMap) Prev(key string) (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen C.size_t
	var val C.uint64_t
	status := C.expanse_strmap_prev_before_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	if status == C.EXPANSE_STR_NAV_BUFFER_TOO_SMALL && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = C.expanse_strmap_prev_before_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	}
	if status == C.EXPANSE_STR_NAV_OK {
		return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), uint64(val), true
	}
	return "", 0, false
}

func (m *StrMap) PrevAtOrBefore(key string) (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	cKey := C.CString(key)
	defer C.free(unsafe.Pointer(cKey))
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen C.size_t
	var val C.uint64_t
	status := C.expanse_strmap_prev_at_or_before_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	if status == C.EXPANSE_STR_NAV_BUFFER_TOO_SMALL && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = C.expanse_strmap_prev_at_or_before_ex(m.ptr, cKey, (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)), &reqLen, &val)
	}
	if status == C.EXPANSE_STR_NAV_OK {
		return C.GoString((*C.char)(unsafe.Pointer(&buf[0]))), uint64(val), true
	}
	return "", 0, false
}

// Free releases the native handle. It is idempotent and optional -- the
// finalizer performs the same release -- but calling it concurrently from
// two goroutines is caller error: the check and the store are unguarded.
func (m *StrMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != nil {
		C.expanse_strmap_free(m.ptr)
		m.ptr = nil
	}
}
