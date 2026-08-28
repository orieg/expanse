//go:build cgo && !expanse_purego

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

func (m *Map) MemoryUsed() uint64 {
	return uint64(C.expanse_map_mem_used(m.ptr))
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

func (m *Map) Last() (uint64, uint64, bool) {
	var key, val C.uint64_t
	if bool(C.expanse_map_last(m.ptr, &key, &val)) {
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

func (m *Map) NextAtOrAfter(key uint64) (uint64, uint64, bool) {
	var nextKey, val C.uint64_t
	if bool(C.expanse_map_next_at_or_after(m.ptr, C.uint64_t(key), &nextKey, &val)) {
		return uint64(nextKey), uint64(val), true
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

func (m *Map) PrevAtOrBefore(key uint64) (uint64, uint64, bool) {
	var prevKey, val C.uint64_t
	if bool(C.expanse_map_prev_at_or_before(m.ptr, C.uint64_t(key), &prevKey, &val)) {
		return uint64(prevKey), uint64(val), true
	}
	return 0, 0, false
}

func (m *Map) Rank(key uint64) uint64 {
	return uint64(C.expanse_map_count_below(m.ptr, C.uint64_t(key)))
}

func (m *Map) Select(k uint64) (uint64, uint64, bool) {
	var key, val C.uint64_t
	if bool(C.expanse_map_by_count(m.ptr, C.uint64_t(k), &key, &val)) {
		return uint64(key), uint64(val), true
	}
	return 0, 0, false
}

func (m *Map) CountRange(start, end uint64) uint64 {
	return uint64(C.expanse_map_count_range(m.ptr, C.uint64_t(start), C.uint64_t(end)))
}

func (m *Map) GetBatch(keys []uint64, outValues []uint64, outFound []bool) uint64 {
	count := len(keys)
	if count == 0 {
		return 0
	}
	var keysPtr *C.uint64_t
	var valuesPtr *C.uint64_t
	var foundPtr *C.bool
	if len(keys) > 0 {
		keysPtr = (*C.uint64_t)(&keys[0])
	}
	if len(outValues) > 0 {
		valuesPtr = (*C.uint64_t)(&outValues[0])
	}
	if len(outFound) > 0 {
		foundPtr = (*C.bool)(&outFound[0])
	}
	return uint64(C.expanse_map_get_batch(m.ptr, keysPtr, valuesPtr, foundPtr, C.size_t(count)))
}

func (m *Map) Free() {
	if m.ptr != nil {
		C.expanse_map_free(m.ptr)
		m.ptr = nil
	}
}
