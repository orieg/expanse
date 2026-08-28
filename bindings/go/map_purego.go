//go:build !cgo || expanse_purego

package expanse

import "runtime"

type Map struct {
	ptr uintptr
}

func NewMap() *Map {
	ensureLoaded()
	m := &Map{
		ptr: expanse_map_new(),
	}
	runtime.SetFinalizer(m, (*Map).Free)
	return m
}

func (m *Map) Set(key, value uint64) {
	expanse_map_insert(m.ptr, key, value, nil)
}

func (m *Map) Get(key uint64) (uint64, bool) {
	var val uint64
	if expanse_map_get(m.ptr, key, &val) {
		return val, true
	}
	return 0, false
}

func (m *Map) Delete(key uint64) bool {
	return expanse_map_remove(m.ptr, key, nil)
}

func (m *Map) Contains(key uint64) bool {
	var val uint64
	return expanse_map_get(m.ptr, key, &val)
}

func (m *Map) Size() uint64 {
	return expanse_map_len(m.ptr)
}

func (m *Map) MemoryUsed() uint64 {
	return uint64(expanse_map_mem_used(m.ptr))
}

func (m *Map) Clear() {
	expanse_map_clear(m.ptr)
}

func (m *Map) First() (uint64, uint64, bool) {
	var key, val uint64
	if expanse_map_first(m.ptr, &key, &val) {
		return key, val, true
	}
	return 0, 0, false
}

func (m *Map) Last() (uint64, uint64, bool) {
	var key, val uint64
	if expanse_map_last(m.ptr, &key, &val) {
		return key, val, true
	}
	return 0, 0, false
}

func (m *Map) Next(key uint64) (uint64, uint64, bool) {
	var nextKey, val uint64
	if expanse_map_next_after(m.ptr, key, &nextKey, &val) {
		return nextKey, val, true
	}
	return 0, 0, false
}

func (m *Map) NextAtOrAfter(key uint64) (uint64, uint64, bool) {
	var nextKey, val uint64
	if expanse_map_next_at_or_after(m.ptr, key, &nextKey, &val) {
		return nextKey, val, true
	}
	return 0, 0, false
}

func (m *Map) Prev(key uint64) (uint64, uint64, bool) {
	var prevKey, val uint64
	if expanse_map_prev_before(m.ptr, key, &prevKey, &val) {
		return prevKey, val, true
	}
	return 0, 0, false
}

func (m *Map) PrevAtOrBefore(key uint64) (uint64, uint64, bool) {
	var prevKey, val uint64
	if expanse_map_prev_at_or_before(m.ptr, key, &prevKey, &val) {
		return prevKey, val, true
	}
	return 0, 0, false
}

func (m *Map) Rank(key uint64) uint64 {
	return expanse_map_count_below(m.ptr, key)
}

func (m *Map) Select(k uint64) (uint64, uint64, bool) {
	var key, val uint64
	if expanse_map_by_count(m.ptr, k, &key, &val) {
		return key, val, true
	}
	return 0, 0, false
}

func (m *Map) CountRange(start, end uint64) uint64 {
	return expanse_map_count_range(m.ptr, start, end)
}

func (m *Map) GetBatch(keys []uint64, outValues []uint64, outFound []bool) uint64 {
	count := len(keys)
	if count == 0 {
		return 0
	}
	if len(outValues) < count {
		count = len(outValues)
	}
	if outFound != nil && len(outFound) < count {
		count = len(outFound)
	}
	if count == 0 {
		return 0
	}
	keysPtr := &keys[0]
	valuesPtr := &outValues[0]
	var foundPtr *bool
	if len(outFound) > 0 {
		foundPtr = &outFound[0]
	}
	return uint64(expanse_map_get_batch(m.ptr, keysPtr, valuesPtr, foundPtr, uintptr(count)))
}

func (m *Map) Free() {
	if m.ptr != 0 {
		expanse_map_free(m.ptr)
		m.ptr = 0
	}
}
