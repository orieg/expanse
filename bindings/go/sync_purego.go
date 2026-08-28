//go:build !cgo || expanse_purego

package expanse

import "runtime"

type SyncSet struct {
	ptr uintptr
}

type SyncSetReader struct {
	ptr uintptr
}

func NewSyncSet() *SyncSet {
	ensureLoaded()
	s := &SyncSet{
		ptr: expanse_sync_set_new(),
	}
	runtime.SetFinalizer(s, (*SyncSet).Free)
	return s
}

func (s *SyncSet) Add(key uint64) bool {
	return expanse_sync_set_insert(s.ptr, key)
}

func (s *SyncSet) Remove(key uint64) bool {
	return expanse_sync_set_remove(s.ptr, key)
}

func (s *SyncSet) Contains(key uint64) bool {
	return expanse_sync_set_contains(s.ptr, key)
}

func (s *SyncSet) Size() uint64 {
	return expanse_sync_set_len(s.ptr)
}

func (s *SyncSet) Reader() *SyncSetReader {
	r := &SyncSetReader{
		ptr: expanse_sync_set_reader_new(s.ptr),
	}
	runtime.SetFinalizer(r, (*SyncSetReader).Free)
	return r
}

func (r *SyncSetReader) Contains(key uint64) bool {
	return expanse_sync_set_reader_contains(r.ptr, key)
}

func (r *SyncSetReader) Free() {
	if r.ptr != 0 {
		expanse_sync_set_reader_free(r.ptr)
		r.ptr = 0
	}
}

func (s *SyncSet) Free() {
	if s.ptr != 0 {
		expanse_sync_set_free(s.ptr)
		s.ptr = 0
	}
}

type SyncMap struct {
	ptr uintptr
}

type SyncMapReader struct {
	ptr uintptr
}

func NewSyncMap() *SyncMap {
	ensureLoaded()
	m := &SyncMap{
		ptr: expanse_sync_map_new(),
	}
	runtime.SetFinalizer(m, (*SyncMap).Free)
	return m
}

func (m *SyncMap) Set(key, value uint64) {
	expanse_sync_map_insert(m.ptr, key, value, nil)
}

func (m *SyncMap) Get(key uint64) (uint64, bool) {
	var val uint64
	if expanse_sync_map_get(m.ptr, key, &val) {
		return val, true
	}
	return 0, false
}

func (m *SyncMap) Delete(key uint64) bool {
	return expanse_sync_map_remove(m.ptr, key, nil)
}

func (m *SyncMap) Size() uint64 {
	return expanse_sync_map_len(m.ptr)
}

func (m *SyncMap) Reader() *SyncMapReader {
	r := &SyncMapReader{
		ptr: expanse_sync_map_reader_new(m.ptr),
	}
	runtime.SetFinalizer(r, (*SyncMapReader).Free)
	return r
}

func (r *SyncMapReader) Get(key uint64) (uint64, bool) {
	var val uint64
	if expanse_sync_map_reader_get(r.ptr, key, &val) {
		return val, true
	}
	return 0, false
}

func (r *SyncMapReader) Free() {
	if r.ptr != 0 {
		expanse_sync_map_reader_free(r.ptr)
		r.ptr = 0
	}
}

func (m *SyncMap) Free() {
	if m.ptr != 0 {
		expanse_sync_map_free(m.ptr)
		m.ptr = 0
	}
}
