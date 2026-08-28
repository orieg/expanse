//go:build cgo && !expanse_purego

package expanse

// #include <stdlib.h>
// #include "expanse.h"
import "C"
import "runtime"

type Set struct {
	ptr *C.expanse_set_t
}

func NewSet() *Set {
	s := &Set{
		ptr: C.expanse_set_new(),
	}
	runtime.SetFinalizer(s, (*Set).Free)
	return s
}

func (s *Set) Add(key uint64) bool {
	return bool(C.expanse_set_insert(s.ptr, C.uint64_t(key)))
}

func (s *Set) Remove(key uint64) bool {
	return bool(C.expanse_set_remove(s.ptr, C.uint64_t(key)))
}

func (s *Set) Contains(key uint64) bool {
	return bool(C.expanse_set_contains(s.ptr, C.uint64_t(key)))
}

func (s *Set) Size() uint64 {
	return uint64(C.expanse_set_len(s.ptr))
}

func (s *Set) MemoryUsed() uint64 {
	return uint64(C.expanse_set_mem_used(s.ptr))
}

func (s *Set) Clear() {
	C.expanse_set_clear(s.ptr)
}

func (s *Set) First() (uint64, bool) {
	var key C.uint64_t
	if bool(C.expanse_set_first(s.ptr, &key)) {
		return uint64(key), true
	}
	return 0, false
}

func (s *Set) Next(key uint64) (uint64, bool) {
	var nextKey C.uint64_t
	if bool(C.expanse_set_next_after(s.ptr, C.uint64_t(key), &nextKey)) {
		return uint64(nextKey), true
	}
	return 0, false
}

func (s *Set) NextAtOrAfter(key uint64) (uint64, bool) {
	var nextKey C.uint64_t
	if bool(C.expanse_set_next_at_or_after(s.ptr, C.uint64_t(key), &nextKey)) {
		return uint64(nextKey), true
	}
	return 0, false
}

func (s *Set) Last() (uint64, bool) {
	var key C.uint64_t
	if bool(C.expanse_set_last(s.ptr, &key)) {
		return uint64(key), true
	}
	return 0, false
}

func (s *Set) Prev(key uint64) (uint64, bool) {
	var prevKey C.uint64_t
	if bool(C.expanse_set_prev_before(s.ptr, C.uint64_t(key), &prevKey)) {
		return uint64(prevKey), true
	}
	return 0, false
}

func (s *Set) PrevAtOrBefore(key uint64) (uint64, bool) {
	var prevKey C.uint64_t
	if bool(C.expanse_set_prev_at_or_before(s.ptr, C.uint64_t(key), &prevKey)) {
		return uint64(prevKey), true
	}
	return 0, false
}

func (s *Set) Rank(key uint64) uint64 {
	return uint64(C.expanse_set_count_below(s.ptr, C.uint64_t(key)))
}

func (s *Set) Select(k uint64) (uint64, bool) {
	var key C.uint64_t
	if bool(C.expanse_set_by_count(s.ptr, C.uint64_t(k), &key)) {
		return uint64(key), true
	}
	return 0, false
}

func (s *Set) CountRange(start, end uint64) uint64 {
	return uint64(C.expanse_set_count_range(s.ptr, C.uint64_t(start), C.uint64_t(end)))
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
	keysPtr := (*C.uint64_t)(&keys[0])
	presentPtr := (*C.bool)(&outPresent[0])
	return uint64(C.expanse_set_contains_batch(s.ptr, keysPtr, presentPtr, C.size_t(count)))
}

func (s *Set) Free() {
	if s.ptr != nil {
		C.expanse_set_free(s.ptr)
		s.ptr = nil
	}
}
