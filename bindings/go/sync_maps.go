package expanse

import "runtime"

// The shared (concurrent) string, bytes, ordered-bytes and blob maps. Each is
// the Go face of one C handle family (expanse_sync_strmap_*,
// expanse_sync_bytesmap_*, expanse_sync_ordered_bytesmap_*,
// expanse_sync_blob_map_*; include/expanse.h, expanse_sync(3), docs/COMPAT.md).
// The handle types and the one-call native shims (the n* methods) are in
// sync_maps_cgo.go and sync_maps_purego.go; this file is what both builds share.
//
// Thread rules, as for SyncMap. A map is safe from any number of goroutines at
// once except Free, which needs no call in progress and every reader freed
// first. A reader is for one goroutine at a time: it owns a single epoch slot
// and every call pins and unpins it, so two goroutines calling through one
// reader concurrently is undefined behaviour in the library. Handing a reader
// from one goroutine to another is fine; sharing one is not. Free a reader
// before its map. The reader holds its map reachable, which orders the two
// finalizers (see SyncSetReader), but an explicit Free of the map while a
// reader is still in use is caller error that the binding cannot prevent.
//
// Nothing a call returns aliases native memory: keys and payloads are copied,
// and no C pointer is kept past the call that produced it.

// ---- SyncStrMap ----

// Set stores value under key, replacing any previous value. It returns
// ErrNulInKey, and leaves the map unchanged, when key contains a NUL byte: the
// C ABI takes a key as a NUL-terminated string (see ErrNulInKey).
func (m *SyncStrMap) Set(key string, value uint64) error {
	if nulIndex(key) >= 0 {
		return ErrNulInKey
	}
	defer runtime.KeepAlive(m)
	m.nInsert(syncStrKey(key), value)
	return nil
}

// Get returns the value stored under key and whether key is present. A key
// containing NUL is never stored and reports absent.
func (m *SyncStrMap) Get(key string) (uint64, bool) {
	if nulIndex(key) >= 0 {
		return 0, false
	}
	defer runtime.KeepAlive(m)
	return m.nGet(syncStrKey(key))
}

// Contains reports whether key is present.
func (m *SyncStrMap) Contains(key string) bool {
	if nulIndex(key) >= 0 {
		return false
	}
	defer runtime.KeepAlive(m)
	return m.nContains(syncStrKey(key))
}

// Delete removes key and reports whether it was present.
func (m *SyncStrMap) Delete(key string) bool {
	if nulIndex(key) >= 0 {
		return false
	}
	defer runtime.KeepAlive(m)
	return m.nRemove(syncStrKey(key))
}

// CompareExchange stores newValue under key only if the key's current word is
// expected, as one linearizable step, with the convention of
// (*SyncMap).CompareExchange: see Word for the absent forms, and note that
// observed is the word the compare saw whether or not the store happened. It
// returns ErrNulInKey, and does nothing, when key contains a NUL byte.
func (m *SyncStrMap) CompareExchange(key string, expected, newValue Word) (observed Word, stored bool, err error) {
	if nulIndex(key) >= 0 {
		return Word{}, false, ErrNulInKey
	}
	defer runtime.KeepAlive(m)
	observed, stored = m.nCompareExchange(syncStrKey(key), expected, newValue)
	return observed, stored, nil
}

// GetOrInsert stores value under key only if the key is absent, as one
// linearizable step. It returns (existing, true) when the key was present,
// leaving it untouched, and (0, false) when value was stored. It returns
// ErrNulInKey, and does nothing, when key contains a NUL byte.
func (m *SyncStrMap) GetOrInsert(key string, value uint64) (existing uint64, ok bool, err error) {
	if nulIndex(key) >= 0 {
		return 0, false, ErrNulInKey
	}
	defer runtime.KeepAlive(m)
	existing, inserted := m.nGetOrInsert(syncStrKey(key), value)
	if inserted {
		return 0, false, nil
	}
	return existing, true, nil
}

// Update replaces the value for key with fn(current), as (*SyncMap).Update
// does: a loop over CompareExchange, so fn may run more than once per call and
// must be free of side effects. It returns the value it replaced as
// (previous, true), or (0, false) when the key was absent, and ErrNulInKey when
// key contains a NUL byte.
func (m *SyncStrMap) Update(key string, fn func(current uint64, ok bool) (uint64, bool)) (previous uint64, ok bool, err error) {
	if nulIndex(key) >= 0 {
		return 0, false, ErrNulInKey
	}
	v, present := m.Get(key)
	previous, ok = updateWord(Word{Value: v, Present: present}, func(expected, newValue Word) (Word, bool) {
		observed, stored, _ := m.CompareExchange(key, expected, newValue)
		return observed, stored
	}, fn)
	return previous, ok, nil
}

// Size returns the number of entries.
func (m *SyncStrMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return m.nLen()
}

// MemoryUsed returns the native heap bytes used by the live structure. Read
// with writers excluded.
func (m *SyncStrMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemUsed()
}

// MemoryHeld returns the heap bytes the map holds from the system allocator:
// MemoryUsed plus the blocks its epoch collector keeps for reuse or is waiting
// to reclaim. Read with writers excluded.
func (m *SyncStrMap) MemoryHeld() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemHeld()
}

// ShrinkToFit returns the freed blocks the map's epoch collector keeps for
// reuse to the system allocator and returns the bytes released. Runs beside
// readers and writers.
func (m *SyncStrMap) ShrinkToFit() uint64 {
	defer runtime.KeepAlive(m)
	return m.nShrink()
}

// Get returns the value stored under key and whether key is present. A key
// containing NUL is never stored and reports absent.
func (r *SyncStrMapReader) Get(key string) (uint64, bool) {
	if nulIndex(key) >= 0 {
		return 0, false
	}
	defer runtime.KeepAlive(r)
	return r.nGet(syncStrKey(key))
}

// Contains reports whether key is present.
func (r *SyncStrMapReader) Contains(key string) bool {
	if nulIndex(key) >= 0 {
		return false
	}
	defer runtime.KeepAlive(r)
	return r.nContains(syncStrKey(key))
}

// nav runs one ordered read. key is NUL-free; ops that take no key ignore it.
func (r *SyncStrMapReader) nav(op navOp, key string) (string, uint64, bool) {
	defer runtime.KeepAlive(r)
	var kb []byte
	if op != navFirst && op != navLast {
		kb = syncStrKey(key)
	}
	k, v, ok := syncNav(func(buf []byte) (int32, uintptr, uint64) {
		return r.nNav(op, kb, buf)
	}, true)
	if !ok {
		return "", 0, false
	}
	return string(k), v, true
}

// First returns the entry with the smallest key. Like the other ordered reads
// it is one validated read: the entry returned was present, and every key the
// search passed over was absent, at one instant. A walk built from repeated
// calls is not a snapshot; it sees every key present throughout it, in order,
// and may or may not see keys inserted or removed meanwhile.
func (r *SyncStrMapReader) First() (string, uint64, bool) { return r.nav(navFirst, "") }

// Last returns the entry with the largest key.
func (r *SyncStrMapReader) Last() (string, uint64, bool) { return r.nav(navLast, "") }

// Next returns the entry with the smallest key strictly greater than key.
func (r *SyncStrMapReader) Next(key string) (string, uint64, bool) {
	if i := nulIndex(key); i >= 0 {
		return r.Next(key[:i]) // see nulIndex
	}
	return r.nav(navNextAfter, key)
}

// NextAtOrAfter returns the entry with the smallest key greater than or equal
// to key.
func (r *SyncStrMapReader) NextAtOrAfter(key string) (string, uint64, bool) {
	if i := nulIndex(key); i >= 0 {
		return r.Next(key[:i]) // see nulIndex
	}
	return r.nav(navNextAtOrAfter, key)
}

// Prev returns the entry with the largest key strictly less than key.
func (r *SyncStrMapReader) Prev(key string) (string, uint64, bool) {
	if i := nulIndex(key); i >= 0 {
		return r.PrevAtOrBefore(key[:i]) // see nulIndex
	}
	return r.nav(navPrevBefore, key)
}

// PrevAtOrBefore returns the entry with the largest key less than or equal to
// key.
func (r *SyncStrMapReader) PrevAtOrBefore(key string) (string, uint64, bool) {
	if i := nulIndex(key); i >= 0 {
		return r.PrevAtOrBefore(key[:i]) // see nulIndex
	}
	return r.nav(navPrevAtOrBefore, key)
}

// ---- SyncBytesMap ----

// Set stores value under key, replacing any previous value, and reports
// whether key was newly inserted. Keys are opaque bytes: empty keys and keys
// with embedded NUL bytes are ordinary.
func (m *SyncBytesMap) Set(key []byte, value uint64) bool {
	defer runtime.KeepAlive(m)
	return m.nInsert(key, value)
}

// Get returns the value stored under key and whether key is present.
func (m *SyncBytesMap) Get(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(m)
	return m.nGet(key)
}

// Contains reports whether key is present.
func (m *SyncBytesMap) Contains(key []byte) bool {
	defer runtime.KeepAlive(m)
	return m.nContains(key)
}

// Delete removes key and reports whether it was present.
func (m *SyncBytesMap) Delete(key []byte) bool {
	defer runtime.KeepAlive(m)
	return m.nRemove(key)
}

// CompareExchange stores newValue under key only if the key's current word is
// expected, as one linearizable step, with the convention of
// (*SyncMap).CompareExchange: see Word for the absent forms, and note that
// observed is the word the compare saw whether or not the store happened.
func (m *SyncBytesMap) CompareExchange(key []byte, expected, newValue Word) (observed Word, stored bool) {
	defer runtime.KeepAlive(m)
	return m.nCompareExchange(key, expected, newValue)
}

// GetOrInsert stores value under key only if the key is absent, as one
// linearizable step. It returns (existing, true) when the key was present,
// leaving it untouched, and (0, false) when value was stored.
func (m *SyncBytesMap) GetOrInsert(key []byte, value uint64) (existing uint64, ok bool) {
	defer runtime.KeepAlive(m)
	existing, inserted := m.nGetOrInsert(key, value)
	if inserted {
		return 0, false
	}
	return existing, true
}

// Update replaces the value for key with fn(current), as (*SyncMap).Update
// does: a loop over CompareExchange, so fn may run more than once per call and
// must be free of side effects. It returns the value it replaced as
// (previous, true), or (0, false) when the key was absent.
func (m *SyncBytesMap) Update(key []byte, fn func(current uint64, ok bool) (uint64, bool)) (previous uint64, ok bool) {
	v, present := m.Get(key)
	return updateWord(Word{Value: v, Present: present}, func(expected, newValue Word) (Word, bool) {
		return m.CompareExchange(key, expected, newValue)
	}, fn)
}

// Size returns the number of entries.
func (m *SyncBytesMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return m.nLen()
}

// MemoryUsed returns the native heap bytes used by the live structure. Read
// with writers excluded.
func (m *SyncBytesMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemUsed()
}

// MemoryHeld returns the heap bytes the map holds from the system allocator:
// MemoryUsed plus the blocks its epoch collector keeps for reuse or is waiting
// to reclaim. Read with writers excluded.
func (m *SyncBytesMap) MemoryHeld() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemHeld()
}

// ShrinkToFit returns the freed blocks the map's epoch collector keeps for
// reuse to the system allocator and returns the bytes released. Runs beside
// readers and writers.
func (m *SyncBytesMap) ShrinkToFit() uint64 {
	defer runtime.KeepAlive(m)
	return m.nShrink()
}

// Get returns the value stored under key and whether key is present.
func (r *SyncBytesMapReader) Get(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(r)
	return r.nGet(key)
}

// Contains reports whether key is present.
func (r *SyncBytesMapReader) Contains(key []byte) bool {
	defer runtime.KeepAlive(r)
	return r.nContains(key)
}

// ---- SyncOrderedBytesMap ----

// Set stores value under key, replacing any previous value, and reports
// whether key was newly inserted. Keys are opaque bytes ordered as unsigned
// bytes, lexicographically; empty keys and keys with embedded NUL bytes are
// ordinary.
func (m *SyncOrderedBytesMap) Set(key []byte, value uint64) bool {
	defer runtime.KeepAlive(m)
	return m.nInsert(key, value)
}

// Get returns the value stored under key and whether key is present.
func (m *SyncOrderedBytesMap) Get(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(m)
	return m.nGet(key)
}

// Contains reports whether key is present.
func (m *SyncOrderedBytesMap) Contains(key []byte) bool {
	defer runtime.KeepAlive(m)
	return m.nContains(key)
}

// Delete removes key and reports whether it was present.
func (m *SyncOrderedBytesMap) Delete(key []byte) bool {
	defer runtime.KeepAlive(m)
	return m.nRemove(key)
}

// CompareExchange stores newValue under key only if the key's current word is
// expected, as one linearizable step, with the convention of
// (*SyncMap).CompareExchange: see Word for the absent forms, and note that
// observed is the word the compare saw whether or not the store happened.
func (m *SyncOrderedBytesMap) CompareExchange(key []byte, expected, newValue Word) (observed Word, stored bool) {
	defer runtime.KeepAlive(m)
	return m.nCompareExchange(key, expected, newValue)
}

// GetOrInsert stores value under key only if the key is absent, as one
// linearizable step. It returns (existing, true) when the key was present,
// leaving it untouched, and (0, false) when value was stored.
func (m *SyncOrderedBytesMap) GetOrInsert(key []byte, value uint64) (existing uint64, ok bool) {
	defer runtime.KeepAlive(m)
	existing, inserted := m.nGetOrInsert(key, value)
	if inserted {
		return 0, false
	}
	return existing, true
}

// Update replaces the value for key with fn(current), as (*SyncMap).Update
// does: a loop over CompareExchange, so fn may run more than once per call and
// must be free of side effects. It returns the value it replaced as
// (previous, true), or (0, false) when the key was absent.
func (m *SyncOrderedBytesMap) Update(key []byte, fn func(current uint64, ok bool) (uint64, bool)) (previous uint64, ok bool) {
	v, present := m.Get(key)
	return updateWord(Word{Value: v, Present: present}, func(expected, newValue Word) (Word, bool) {
		return m.CompareExchange(key, expected, newValue)
	}, fn)
}

// Size returns the number of entries.
func (m *SyncOrderedBytesMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return m.nLen()
}

// MemoryUsed returns the native heap bytes used by the live structure. Read
// with writers excluded.
func (m *SyncOrderedBytesMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemUsed()
}

// MemoryHeld returns the heap bytes the map holds from the system allocator:
// MemoryUsed plus the blocks its epoch collector keeps for reuse or is waiting
// to reclaim. Read with writers excluded.
func (m *SyncOrderedBytesMap) MemoryHeld() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemHeld()
}

// ShrinkToFit returns the freed blocks the map's epoch collector keeps for
// reuse to the system allocator and returns the bytes released. Runs beside
// readers and writers.
func (m *SyncOrderedBytesMap) ShrinkToFit() uint64 {
	defer runtime.KeepAlive(m)
	return m.nShrink()
}

// nav runs one ordered read on the map handle.
func (m *SyncOrderedBytesMap) nav(op navOp, key []byte) ([]byte, uint64, bool) {
	defer runtime.KeepAlive(m)
	return syncNav(func(buf []byte) (int32, uintptr, uint64) {
		return m.nNav(op, key, buf)
	}, false)
}

// The ordered reads are calls on the MAP, not on a reader, and they BLOCK
// WRITERS for their duration: each runs with every writer excluded (it
// quiesces writers and takes the writer mutex), so the entry returned is the
// answer at one instant, and every write waits for the call. They are not for
// a hot loop beside writers; the string map's reader offers non-blocking
// ordered reads. A walk built from repeated Next calls is not a snapshot: it
// sees every key present throughout it, in order, and may or may not see keys
// inserted or removed between calls. Each returns a copy of the key; a key
// longer than the first buffer is fetched by reading again at the length the
// native call reports, and that retry is a new read.

// First returns the entry with the smallest key. It blocks writers while it runs.
func (m *SyncOrderedBytesMap) First() ([]byte, uint64, bool) { return m.nav(navFirst, nil) }

// Last returns the entry with the largest key. It blocks writers while it runs.
func (m *SyncOrderedBytesMap) Last() ([]byte, uint64, bool) { return m.nav(navLast, nil) }

// Next returns the entry with the smallest key strictly greater than key. It
// blocks writers while it runs.
func (m *SyncOrderedBytesMap) Next(key []byte) ([]byte, uint64, bool) {
	return m.nav(navNextAfter, key)
}

// NextAtOrAfter returns the entry with the smallest key greater than or equal
// to key. It blocks writers while it runs.
func (m *SyncOrderedBytesMap) NextAtOrAfter(key []byte) ([]byte, uint64, bool) {
	return m.nav(navNextAtOrAfter, key)
}

// Prev returns the entry with the largest key strictly less than key. It
// blocks writers while it runs.
func (m *SyncOrderedBytesMap) Prev(key []byte) ([]byte, uint64, bool) {
	return m.nav(navPrevBefore, key)
}

// PrevAtOrBefore returns the entry with the largest key less than or equal to
// key. It blocks writers while it runs.
func (m *SyncOrderedBytesMap) PrevAtOrBefore(key []byte) ([]byte, uint64, bool) {
	return m.nav(navPrevAtOrBefore, key)
}

// Get returns the value stored under key and whether key is present.
func (r *SyncOrderedBytesMapReader) Get(key []byte) (uint64, bool) {
	defer runtime.KeepAlive(r)
	return r.nGet(key)
}

// Contains reports whether key is present.
func (r *SyncOrderedBytesMapReader) Contains(key []byte) bool {
	defer runtime.KeepAlive(r)
	return r.nContains(key)
}

// ---- SyncBlobMap ----

// Set stores data under key with the given hot metadata, replacing any
// previous value. A refused insert returns an error wrapping
// ErrBlobInsertRefused that names the refusal (ErrBlobCapRefused,
// ErrBlobArenaFull, ErrBlobMetaOverflow, ...); nil means the value is stored.
// Metadata is the 24-bit field of an arena-stored value: a payload of 7 bytes
// or fewer, or one the map compresses into its slot word, is stored without it
// and reads back metadata 0.
func (m *SyncBlobMap) Set(key uint64, data []byte, hotMeta uint32) error {
	defer runtime.KeepAlive(m)
	return blobStatusErr(m.nInsert(key, data, hotMeta))
}

// Get returns a copy of the payload stored under key, its metadata, and
// whether key is present. A payload longer than the first buffer is read again
// at the length the native call reports; that retry is a new read, so it
// reports the new state if a writer replaced the value in between. An empty
// payload is returned as nil.
func (m *SyncBlobMap) Get(key uint64) ([]byte, uint32, bool) {
	defer runtime.KeepAlive(m)
	data, meta, found, err := syncBlobGet(func(buf []byte) (int32, uintptr, uint32) {
		return m.nGetInto(key, buf)
	})
	if err != nil {
		return nil, 0, false // INVALID_ARGUMENT: a freed map
	}
	return data, meta, found
}

// GetMeta returns the metadata stored under key and whether key is present,
// without copying the payload.
func (m *SyncBlobMap) GetMeta(key uint64) (uint32, bool) {
	defer runtime.KeepAlive(m)
	return m.nGetMeta(key)
}

// Contains reports whether key is present.
func (m *SyncBlobMap) Contains(key uint64) bool {
	defer runtime.KeepAlive(m)
	return m.nContains(key)
}

// Delete removes key and reports whether it was present.
func (m *SyncBlobMap) Delete(key uint64) bool {
	defer runtime.KeepAlive(m)
	return m.nRemove(key)
}

// Size returns the number of entries.
func (m *SyncBlobMap) Size() uint64 {
	defer runtime.KeepAlive(m)
	return m.nLen()
}

// MemoryUsed returns the native heap bytes used. Read with writers excluded;
// it waits for a running compaction. MemoryUsedEstimate takes no lock.
func (m *SyncBlobMap) MemoryUsed() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemUsed()
}

// MemoryUsedEstimate is MemoryUsed without the lock: it takes no lock and
// waits for nothing, so under concurrent writers it is an estimate.
func (m *SyncBlobMap) MemoryUsedEstimate() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemUsedEstimate()
}

// MemoryHeld returns the heap bytes the map holds from the system allocator.
// Read with writers excluded; it waits for a running compaction.
// MemoryHeldEstimate waits for nothing.
func (m *SyncBlobMap) MemoryHeld() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemHeld()
}

// MemoryHeldEstimate is MemoryHeld without the wait: it takes no map lock and
// waits for no collector lock, so beside a running compaction or busy writers
// it is an estimate.
func (m *SyncBlobMap) MemoryHeldEstimate() uint64 {
	defer runtime.KeepAlive(m)
	return m.nMemHeldEstimate()
}

// ShrinkToFit returns the collector's freed index blocks to the system
// allocator and returns the bytes released.
func (m *SyncBlobMap) ShrinkToFit() uint64 {
	defer runtime.KeepAlive(m)
	return m.nShrink()
}

// SetReclaimAtCap turns the reclaim at the capacity cap on (the default) or
// off. Off, an insert the cap refuses compacts nothing.
func (m *SyncBlobMap) SetReclaimAtCap(on bool) {
	defer runtime.KeepAlive(m)
	m.nSetReclaimAtCap(on)
}

// Compact rewrites the arena to free dead bytes. Readers keep reading while it
// copies. A nil error means it ran; a refusal is an error wrapping
// ErrBlobInsertRefused.
func (m *SyncBlobMap) Compact() error {
	defer runtime.KeepAlive(m)
	return blobStatusErr(m.nCompact())
}

// CompareExchange stores newValue under key only if the key's current value is
// expected, as one linearizable step. A value is its payload AND its metadata,
// and the comparison covers both; see BlobValue for the absent forms (an absent
// expected means "only if the key is absent", an absent newValue means "remove
// the key"). It reports whether the store happened. On a mismatch observed is
// the current value (absent if the key is absent), copied out within the step
// that compared, so it is the value to retry against; on a store observed is
// expected.
//
// A nil error with stored false is a mismatch. A non-nil error means the
// comparison held but newValue cannot be stored, or the call was a usage error;
// the map is unchanged. It wraps ErrBlobInsertRefused and is one of
// ErrBlobMetaOverflow, ErrBlobAllocationFailed, ErrBlobCapRefused (this call
// never runs the reclaim at the cap that Set runs: Compact and a retry is the
// caller's choice), ErrBlobArenaFull or ErrBlobInvalidArgument. A refusal is
// reported only when the comparison held, so a loop that retries on mismatch
// and stops on an error ends.
func (m *SyncBlobMap) CompareExchange(key uint64, expected, newValue BlobValue) (observed BlobValue, stored bool, err error) {
	defer runtime.KeepAlive(m)
	return syncBlobRMW(expected, func(buf []byte) (int32, bool, uintptr, uint32) {
		return m.nCompareExchange(key, expected, newValue, buf)
	})
}

// CompareExchangeMeta swaps the 24-bit metadata in the key's slot with no
// payload copy and no allocation, as one linearizable step: the form for a
// retry loop whose state lives in the metadata (a sequence number, a status
// tag). The current metadata of a present key is its field, or 0 for a payload
// stored in its slot word. It reports whether the swap happened; on a mismatch
// observed is the current metadata (absent if the key is absent), and on a
// swap observed is expected. A new absent BlobMeta removes the key.
//
// Errors wrap ErrBlobInsertRefused: ErrBlobMetaOverflow (newValue is wider than
// 24 bits), ErrBlobNoMetaField (the comparison held but the value has no field:
// the key is absent, since metadata alone cannot create it, or newValue is
// non-zero for a payload stored in its slot word), ErrBlobInvalidArgument. The
// map is unchanged on an error.
func (m *SyncBlobMap) CompareExchangeMeta(key uint64, expected, newValue BlobMeta) (observed BlobMeta, stored bool, err error) {
	defer runtime.KeepAlive(m)
	status, obs := m.nCompareExchangeMeta(key, expected, newValue)
	switch status {
	case syncBlobOK:
		return expected, true, nil
	case syncBlobMismatch:
		return obs, false, nil
	default:
		return BlobMeta{}, false, blobStatusErr(status)
	}
}

// GetOrInsert stores data with hotMeta under key only if the key is absent, as
// one linearizable step. It returns (existing, true, nil) when the key was
// present, leaving it untouched, and (BlobValue{}, false, nil) when the value
// was stored. The errors are those of CompareExchange.
func (m *SyncBlobMap) GetOrInsert(key uint64, data []byte, hotMeta uint32) (existing BlobValue, found bool, err error) {
	defer runtime.KeepAlive(m)
	existing, stored, err := syncBlobRMW(BlobValue{}, func(buf []byte) (int32, bool, uintptr, uint32) {
		status, n, meta := m.nGetOrInsert(key, data, hotMeta, buf)
		// A MISMATCH here means the key was present: it is the only way a
		// get_or_insert reports one.
		return status, true, n, meta
	})
	if err != nil {
		return BlobValue{}, false, err
	}
	return existing, !stored, nil
}

// Get returns a copy of the payload stored under key, its metadata, and
// whether key is present. See (*SyncBlobMap).Get.
func (r *SyncBlobMapReader) Get(key uint64) ([]byte, uint32, bool) {
	defer runtime.KeepAlive(r)
	data, meta, found, err := syncBlobGet(func(buf []byte) (int32, uintptr, uint32) {
		return r.nGetInto(key, buf)
	})
	if err != nil {
		return nil, 0, false // INVALID_ARGUMENT: a freed reader
	}
	return data, meta, found
}

// GetMeta returns the metadata stored under key and whether key is present,
// without copying the payload.
func (r *SyncBlobMapReader) GetMeta(key uint64) (uint32, bool) {
	defer runtime.KeepAlive(r)
	return r.nGetMeta(key)
}

// Contains reports whether key is present.
func (r *SyncBlobMapReader) Contains(key uint64) bool {
	defer runtime.KeepAlive(r)
	return r.nContains(key)
}
