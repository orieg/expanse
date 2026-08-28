//go:build !cgo || expanse_purego

package expanse

import (
	"runtime"
	"unsafe"
)

type StrMap struct {
	ptr uintptr
}

func NewStrMap() *StrMap {
	ensureLoaded()
	m := &StrMap{
		ptr: expanse_strmap_new(),
	}
	runtime.SetFinalizer(m, (*StrMap).Free)
	return m
}

func cStringBytes(s string) []byte {
	b := make([]byte, len(s)+1)
	copy(b, s)
	b[len(s)] = 0
	return b
}

func cStringPtr(s string) unsafe.Pointer {
	b := cStringBytes(s)
	return unsafe.Pointer(&b[0])
}

func cStringToGo(b []byte) string {
	n := 0
	for n < len(b) && b[n] != 0 {
		n++
	}
	return string(b[:n])
}

func (m *StrMap) Set(key string, value uint64) {
	expanse_strmap_insert(m.ptr, cStringPtr(key), value, nil)
}

func (m *StrMap) Get(key string) (uint64, bool) {
	var val uint64
	if expanse_strmap_get(m.ptr, cStringPtr(key), &val) {
		return val, true
	}
	return 0, false
}

func (m *StrMap) Delete(key string) bool {
	return expanse_strmap_remove(m.ptr, cStringPtr(key), nil)
}

func (m *StrMap) Contains(key string) bool {
	var val uint64
	return expanse_strmap_get(m.ptr, cStringPtr(key), &val)
}

func (m *StrMap) Size() uint64 {
	return expanse_strmap_len(m.ptr)
}

func (m *StrMap) MemoryUsed() uint64 {
	return uint64(expanse_strmap_mem_used(m.ptr))
}

func (m *StrMap) Clear() {
	expanse_strmap_clear(m.ptr)
}

func (m *StrMap) First() (string, uint64, bool) {
	buf := make([]byte, 1024)
	var reqLen uintptr
	var val uint64
	status := expanse_strmap_first_ex(m.ptr, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_first_ex(m.ptr, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) Last() (string, uint64, bool) {
	buf := make([]byte, 1024)
	var reqLen uintptr
	var val uint64
	status := expanse_strmap_last_ex(m.ptr, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_last_ex(m.ptr, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) Next(key string) (string, uint64, bool) {
	buf := make([]byte, 1024)
	var reqLen uintptr
	var val uint64
	cKey := cStringPtr(key)
	status := expanse_strmap_next_after_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_next_after_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) NextAtOrAfter(key string) (string, uint64, bool) {
	buf := make([]byte, 1024)
	var reqLen uintptr
	var val uint64
	cKey := cStringPtr(key)
	status := expanse_strmap_next_at_or_after_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_next_at_or_after_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) Prev(key string) (string, uint64, bool) {
	buf := make([]byte, 1024)
	var reqLen uintptr
	var val uint64
	cKey := cStringPtr(key)
	status := expanse_strmap_prev_before_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_prev_before_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) PrevAtOrBefore(key string) (string, uint64, bool) {
	buf := make([]byte, 1024)
	var reqLen uintptr
	var val uint64
	cKey := cStringPtr(key)
	status := expanse_strmap_prev_at_or_before_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_prev_at_or_before_ex(m.ptr, cKey, unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) Free() {
	if m.ptr != 0 {
		expanse_strmap_free(m.ptr)
		m.ptr = 0
	}
}
