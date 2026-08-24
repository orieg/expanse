package expanse

// #include <stdlib.h>
// #include "expanse.h"
import "C"
import "runtime"

type Map struct {
	ptr *C.expanse_map_t
}

func NewMap() *Map {
	m := &Map{
		ptr: C.expanse_map_new(),
	}
	runtime.SetFinalizer(m, (*Map).Free)
	return m
}

func (m *Map) Set(key, value uint64) {
	C.expanse_map_insert(m.ptr, C.uint64_t(key), C.uint64_t(value), nil)
}

func (m *Map) Get(key uint64) (uint64, bool) {
	var val C.uint64_t
	if bool(C.expanse_map_get(m.ptr, C.uint64_t(key), &val)) {
		return uint64(val), true
	}
	return 0, false
}

func (m *Map) Delete(key uint64) bool {
	return bool(C.expanse_map_remove(m.ptr, C.uint64_t(key), nil))
}

func (m *Map) Contains(key uint64) bool {
	var val C.uint64_t
	return bool(C.expanse_map_get(m.ptr, C.uint64_t(key), &val))
}

func (m *Map) Size() uint64 {
	return uint64(C.expanse_map_len(m.ptr))
}

func (m *Map) Clear() {
	C.expanse_map_clear(m.ptr)
}

func (m *Map) First() (uint64, uint64, bool) {
	var key, val C.uint64_t
	if bool(C.expanse_map_first(m.ptr, &key, &val)) {
		return uint64(key), uint64(val), true
	}
	return 0, 0, false
}

func (m *Map) Next(key uint64) (uint64, uint64, bool) {
	var nextKey, val C.uint64_t
	if bool(C.expanse_map_next_after(m.ptr, C.uint64_t(key), &nextKey, &val)) {
		return uint64(nextKey), uint64(val), true
	}
	return 0, 0, false
}

func (m *Map) Last() (uint64, uint64, bool) {
	var key, val C.uint64_t
	if bool(C.expanse_map_last(m.ptr, &key, &val)) {
		return uint64(key), uint64(val), true
	}
	return 0, 0, false
}

func (m *Map) Prev(key uint64) (uint64, uint64, bool) {
	var prevKey, val C.uint64_t
	if bool(C.expanse_map_prev_before(m.ptr, C.uint64_t(key), &prevKey, &val)) {
		return uint64(prevKey), uint64(val), true
	}
	return 0, 0, false
}

func (m *Map) Free() {
	if m.ptr != nil {
		C.expanse_map_free(m.ptr)
		m.ptr = nil
	}
}
