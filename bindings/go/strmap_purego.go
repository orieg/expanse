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

func cStringToGo(b []byte) string {
	n := 0
	for n < len(b) && b[n] != 0 {
		n++
	}
	return string(b[:n])
}

func (m *StrMap) Set(key string, value uint64) {
	defer runtime.KeepAlive(m)
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	expanse_strmap_insert(m.ptr, unsafe.Pointer(&cKey[0]), value, nil)
}

func (m *StrMap) Get(key string) (uint64, bool) {
	defer runtime.KeepAlive(m)
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	var val uint64
	if expanse_strmap_get(m.ptr, unsafe.Pointer(&cKey[0]), &val) {
		return val, true
	}
	return 0, false
}

func (m *StrMap) Delete(key string) bool {
	defer runtime.KeepAlive(m)
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	return expanse_strmap_remove(m.ptr, unsafe.Pointer(&cKey[0]), nil)
}

func (m *StrMap) Contains(key string) bool {
	defer runtime.KeepAlive(m)
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	var val uint64
	return expanse_strmap_get(m.ptr, unsafe.Pointer(&cKey[0]), &val)
}

func (m *StrMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return expanse_strmap_len(m.ptr)
}

func (m *StrMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(expanse_strmap_mem_used(m.ptr))
}

func (m *StrMap) Clear() {
	defer runtime.KeepAlive(m)
	expanse_strmap_clear(m.ptr)
}

func (m *StrMap) First() (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
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
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
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
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen uintptr
	var val uint64
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	status := expanse_strmap_next_after_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_next_after_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) NextAtOrAfter(key string) (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen uintptr
	var val uint64
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	status := expanse_strmap_next_at_or_after_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_next_at_or_after_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) Prev(key string) (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen uintptr
	var val uint64
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	status := expanse_strmap_prev_before_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_prev_before_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

func (m *StrMap) PrevAtOrBefore(key string) (string, uint64, bool) {
	defer runtime.KeepAlive(m)
	buf := make([]byte, 1024)
	// Closure, not defer KeepAlive(buf): the retry below reallocates buf.
	defer func() { runtime.KeepAlive(buf) }()
	var reqLen uintptr
	var val uint64
	cKey := cStringBytes(key)
	defer runtime.KeepAlive(cKey)
	status := expanse_strmap_prev_at_or_before_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	if status == 2 && reqLen > 0 {
		buf = make([]byte, reqLen)
		status = expanse_strmap_prev_at_or_before_ex(m.ptr, unsafe.Pointer(&cKey[0]), unsafe.Pointer(&buf[0]), uintptr(len(buf)), &reqLen, &val)
	}
	if status == 0 {
		return cStringToGo(buf), val, true
	}
	return "", 0, false
}

// Free releases the native handle. It is idempotent and optional -- the
// finalizer performs the same release -- but calling it concurrently from
// two goroutines is caller error: the check and the store are unguarded.
func (m *StrMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != 0 {
		expanse_strmap_free(m.ptr)
		m.ptr = 0
	}
}
