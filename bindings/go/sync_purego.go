//go:build !cgo || expanse_purego

package expanse

import "runtime"

type SyncSet struct {
	ptr uintptr
}

// SyncSetReader is a registered reader handle. It borrows from the set it was
// taken from: the C handle wraps a reader whose borrow lifetime was erased at
// the ABI boundary, and the contract of expanse_sync_set_reader_new is that
// the set outlives every reader derived from it. The reader's Drop also
// retires its epoch slot from a collector registry that lives inside the set,
// so the set must still be there when the reader is freed.
//
// Go expresses that requirement as reachability, which is the only ordering
// the collector guarantees. Holding the parent here keeps the set from being
// collected while this reader is alive, and therefore also orders the two
// finalizers -- between two independently unreachable objects the runtime
// gives no ordering at all, and the set's finalizer may otherwise run first.
//
// One reader per goroutine at a time. A reader owns a single epoch slot and
// every call pins and unpins it, so two goroutines calling through one reader
// concurrently can clear each other's pin, which is undefined behaviour in the
// library (include/expanse.h, reader-handle ownership). Handing a reader from
// one goroutine to another is fine; sharing one is not. The same holds for
// SyncMapReader.
//
// The handle itself is a uintptr, which holds nothing alive and is invisible
// to the collector, so this field is the only thing that can carry the
// requirement on this build.
type SyncSetReader struct {
	ptr    uintptr
	parent *SyncSet
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
	defer runtime.KeepAlive(s)
	return expanse_sync_set_insert(s.ptr, key)
}

func (s *SyncSet) Remove(key uint64) bool {
	defer runtime.KeepAlive(s)
	return expanse_sync_set_remove(s.ptr, key)
}

func (s *SyncSet) Contains(key uint64) bool {
	defer runtime.KeepAlive(s)
	return expanse_sync_set_contains(s.ptr, key)
}

func (s *SyncSet) Size() uint64 {
	defer runtime.KeepAlive(s)
	return expanse_sync_set_len(s.ptr)
}

// MemoryHeld returns the heap bytes the set holds from the system
// allocator: its tree's own share plus the blocks its epoch collector keeps
// for reuse or is waiting to reclaim. Read with writers excluded.
func (s *SyncSet) MemoryHeld() uint64 {
	defer runtime.KeepAlive(s)
	return uint64(expanse_sync_set_mem_held(s.ptr))
}

// ShrinkToFit returns the freed blocks the set's epoch collector keeps for
// reuse to the system allocator and returns the bytes released, by which
// MemoryHeld then falls. Runs beside readers and writers.
func (s *SyncSet) ShrinkToFit() uint64 {
	defer runtime.KeepAlive(s)
	return uint64(expanse_sync_set_shrink_to_fit(s.ptr))
}

func (s *SyncSet) Reader() *SyncSetReader {
	defer runtime.KeepAlive(s)
	r := &SyncSetReader{
		ptr:    expanse_sync_set_reader_new(s.ptr),
		parent: s,
	}
	runtime.SetFinalizer(r, (*SyncSetReader).Free)
	return r
}

func (r *SyncSetReader) Contains(key uint64) bool {
	defer runtime.KeepAlive(r)
	return expanse_sync_set_reader_contains(r.ptr, key)
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the zero check and the store are unguarded.
func (r *SyncSetReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != 0 {
		expanse_sync_set_reader_free(r.ptr)
		r.ptr = 0
	}
}

// Free releases the set. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (s *SyncSet) Free() {
	runtime.SetFinalizer(s, nil)
	if s.ptr != 0 {
		expanse_sync_set_free(s.ptr)
		s.ptr = 0
	}
}

type SyncMap struct {
	ptr uintptr
}

// SyncMapReader is a registered reader handle. See SyncSetReader for why the
// parent reference is load-bearing rather than decorative.
type SyncMapReader struct {
	ptr    uintptr
	parent *SyncMap
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
	defer runtime.KeepAlive(m)
	expanse_sync_map_insert(m.ptr, key, value, nil)
}

func (m *SyncMap) Get(key uint64) (uint64, bool) {
	defer runtime.KeepAlive(m)
	var val uint64
	if expanse_sync_map_get(m.ptr, key, &val) {
		return val, true
	}
	return 0, false
}

func (m *SyncMap) Delete(key uint64) bool {
	defer runtime.KeepAlive(m)
	return expanse_sync_map_remove(m.ptr, key, nil)
}

// CompareExchange stores newValue under key only if the key's current word is
// expected, as one linearizable step. See Word for the absent forms: an absent
// expected means "only if the key is absent", an absent newValue means "remove
// the key". It reports the word the compare saw whether or not the store
// happened, so a failed call hands back the word to retry against.
func (m *SyncMap) CompareExchange(key uint64, expected, newValue Word) (observed Word, stored bool) {
	defer runtime.KeepAlive(m)
	var seenPresent bool
	var seen uint64
	stored = expanse_sync_map_compare_exchange(m.ptr, key, expected.Present, expected.Value, newValue.Present, newValue.Value, &seenPresent, &seen)
	return Word{Value: seen, Present: seenPresent}, stored
}

// GetOrInsert stores value under key only if the key is absent, as one
// linearizable step. It returns (existing, true) when the key was present,
// leaving it untouched, and (0, false) when value was stored.
func (m *SyncMap) GetOrInsert(key, value uint64) (existing uint64, ok bool) {
	defer runtime.KeepAlive(m)
	if expanse_sync_map_get_or_insert(m.ptr, key, value, &existing) {
		return 0, false
	}
	return existing, true
}

func (m *SyncMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return expanse_sync_map_len(m.ptr)
}

// MemoryHeld returns the heap bytes the map holds from the system
// allocator: its tree's own share plus the blocks its epoch collector keeps
// for reuse or is waiting to reclaim. Read with writers excluded.
func (m *SyncMap) MemoryHeld() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(expanse_sync_map_mem_held(m.ptr))
}

// ShrinkToFit returns the freed blocks the map's epoch collector keeps for
// reuse to the system allocator and returns the bytes released, by which
// MemoryHeld then falls. Runs beside readers and writers.
func (m *SyncMap) ShrinkToFit() uint64 {
	defer runtime.KeepAlive(m)
	return uint64(expanse_sync_map_shrink_to_fit(m.ptr))
}

func (m *SyncMap) Reader() *SyncMapReader {
	defer runtime.KeepAlive(m)
	r := &SyncMapReader{
		ptr:    expanse_sync_map_reader_new(m.ptr),
		parent: m,
	}
	runtime.SetFinalizer(r, (*SyncMapReader).Free)
	return r
}

func (r *SyncMapReader) Get(key uint64) (uint64, bool) {
	defer runtime.KeepAlive(r)
	var val uint64
	if expanse_sync_map_reader_get(r.ptr, key, &val) {
		return val, true
	}
	return 0, false
}

// Free releases the reader handle. Calling it concurrently from two goroutines
// is caller error: the zero check and the store are unguarded.
func (r *SyncMapReader) Free() {
	runtime.SetFinalizer(r, nil)
	if r.ptr != 0 {
		expanse_sync_map_reader_free(r.ptr)
		r.ptr = 0
	}
}

// Free releases the map. Every reader taken from it must be freed first, and
// concurrent calls from two goroutines are caller error.
func (m *SyncMap) Free() {
	runtime.SetFinalizer(m, nil)
	if m.ptr != 0 {
		expanse_sync_map_free(m.ptr)
		m.ptr = 0
	}
}
