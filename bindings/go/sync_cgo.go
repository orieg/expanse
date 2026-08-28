//go:build cgo && !expanse_purego

package expanse

// #include <stdlib.h>
// #include "expanse.h"
import "C"
import "runtime"

type SyncSet struct {
	ptr *C.expanse_sync_set_t
}

type SyncSetReader struct {
	ptr *C.expanse_sync_set_reader_t
}

func NewSyncSet() *SyncSet {
	s := &SyncSet{
		ptr: C.expanse_sync_set_new(),
	}
	runtime.SetFinalizer(s, (*SyncSet).Free)
	return s
}

func (s *SyncSet) Add(key uint64) bool {
	return bool(C.expanse_sync_set_insert(s.ptr, C.uint64_t(key)))
}

func (s *SyncSet) Remove(key uint64) bool {
	return bool(C.expanse_sync_set_remove(s.ptr, C.uint64_t(key)))
}

func (s *SyncSet) Contains(key uint64) bool {
	return bool(C.expanse_sync_set_contains(s.ptr, C.uint64_t(key)))
}

func (s *SyncSet) Size() uint64 {
	return uint64(C.expanse_sync_set_len(s.ptr))
}

func (s *SyncSet) Reader() *SyncSetReader {
	r := &SyncSetReader{
		ptr: C.expanse_sync_set_reader_new(s.ptr),
	}
	runtime.SetFinalizer(r, (*SyncSetReader).Free)
	return r
}

func (r *SyncSetReader) Contains(key uint64) bool {
	return bool(C.expanse_sync_set_reader_contains(r.ptr, C.uint64_t(key)))
}

func (r *SyncSetReader) Free() {
	if r.ptr != nil {
		C.expanse_sync_set_reader_free(r.ptr)
		r.ptr = nil
	}
}

func (s *SyncSet) Free() {
	if s.ptr != nil {
		C.expanse_sync_set_free(s.ptr)
		s.ptr = nil
	}
}

type SyncMap struct {
	ptr *C.expanse_sync_map_t
}

type SyncMapReader struct {
	ptr *C.expanse_sync_map_reader_t
}

func NewSyncMap() *SyncMap {
	m := &SyncMap{
		ptr: C.expanse_sync_map_new(),
	}
	runtime.SetFinalizer(m, (*SyncMap).Free)
	return m
}

func (m *SyncMap) Set(key, value uint64) {
	C.expanse_sync_map_insert(m.ptr, C.uint64_t(key), C.uint64_t(value), nil)
}

func (m *SyncMap) Get(key uint64) (uint64, bool) {
	var val C.uint64_t
	if bool(C.expanse_sync_map_get(m.ptr, C.uint64_t(key), &val)) {
		return uint64(val), true
	}
	return 0, false
}

func (m *SyncMap) Delete(key uint64) bool {
	return bool(C.expanse_sync_map_remove(m.ptr, C.uint64_t(key), nil))
}

func (m *SyncMap) Size() uint64 {
	return uint64(C.expanse_sync_map_len(m.ptr))
}

func (m *SyncMap) Reader() *SyncMapReader {
	r := &SyncMapReader{
		ptr: C.expanse_sync_map_reader_new(m.ptr),
	}
	runtime.SetFinalizer(r, (*SyncMapReader).Free)
	return r
}

func (r *SyncMapReader) Get(key uint64) (uint64, bool) {
	var val C.uint64_t
	if bool(C.expanse_sync_map_reader_get(r.ptr, C.uint64_t(key), &val)) {
		return uint64(val), true
	}
	return 0, false
}

func (r *SyncMapReader) Free() {
	if r.ptr != nil {
		C.expanse_sync_map_reader_free(r.ptr)
		r.ptr = nil
	}
}

func (m *SyncMap) Free() {
	if m.ptr != nil {
		C.expanse_sync_map_free(m.ptr)
		m.ptr = nil
	}
}
