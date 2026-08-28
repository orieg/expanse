//go:build !cgo || expanse_purego

package expanse

import "runtime"

type Set struct {
	ptr uintptr
}

func NewSet() *Set {
	ensureLoaded()
	s := &Set{
		ptr: expanse_set_new(),
	}
	runtime.SetFinalizer(s, (*Set).Free)
	return s
}

func (s *Set) Add(key uint64) bool {
	return expanse_set_insert(s.ptr, key)
}

func (s *Set) Remove(key uint64) bool {
	return expanse_set_remove(s.ptr, key)
}

func (s *Set) Contains(key uint64) bool {
	return expanse_set_contains(s.ptr, key)
}

func (s *Set) Size() uint64 {
	return expanse_set_len(s.ptr)
}

func (s *Set) MemoryUsed() uint64 {
	return uint64(expanse_set_mem_used(s.ptr))
}

func (s *Set) Clear() {
	expanse_set_clear(s.ptr)
}

func (s *Set) First() (uint64, bool) {
	var key uint64
	if expanse_set_first(s.ptr, &key) {
		return key, true
	}
	return 0, false
}

func (s *Set) Next(key uint64) (uint64, bool) {
	var nextKey uint64
	if expanse_set_next_after(s.ptr, key, &nextKey) {
		return nextKey, true
	}
	return 0, false
}

func (s *Set) NextAtOrAfter(key uint64) (uint64, bool) {
	var nextKey uint64
	if expanse_set_next_at_or_after(s.ptr, key, &nextKey) {
		return nextKey, true
	}
	return 0, false
}

func (s *Set) Last() (uint64, bool) {
	var key uint64
	if expanse_set_last(s.ptr, &key) {
		return key, true
	}
	return 0, false
}

func (s *Set) Prev(key uint64) (uint64, bool) {
	var prevKey uint64
	if expanse_set_prev_before(s.ptr, key, &prevKey) {
		return prevKey, true
	}
	return 0, false
}

func (s *Set) PrevAtOrBefore(key uint64) (uint64, bool) {
	var prevKey uint64
	if expanse_set_prev_at_or_before(s.ptr, key, &prevKey) {
		return prevKey, true
	}
	return 0, false
}

func (s *Set) Rank(key uint64) uint64 {
	return expanse_set_count_below(s.ptr, key)
}

func (s *Set) Select(k uint64) (uint64, bool) {
	var key uint64
	if expanse_set_by_count(s.ptr, k, &key) {
		return key, true
	}
	return 0, false
}

func (s *Set) CountRange(start, end uint64) uint64 {
	return expanse_set_count_range(s.ptr, start, end)
}

func (s *Set) ContainsBatch(keys []uint64, outPresent []bool) uint64 {
	count := len(keys)
	if count == 0 {
		return 0
	}
	if len(outPresent) < count {
		count = len(outPresent)
	}
	if count == 0 {
		return 0
	}
	keysPtr := &keys[0]
	presentPtr := &outPresent[0]
	return uint64(expanse_set_contains_batch(s.ptr, keysPtr, presentPtr, uintptr(count)))
}

func (s *Set) Free() {
	if s.ptr != 0 {
		expanse_set_free(s.ptr)
		s.ptr = 0
	}
}
