package expanse

import (
	"bytes"
	"sync"
	"testing"
)

func TestVersion(t *testing.T) {
	v := Version()
	if v == "" {
		t.Fatalf("expected non-empty version string")
	}
}

func TestSet(t *testing.T) {
	s := NewSet()
	s.Add(10)
	s.Add(20)
	s.Add(30)

	if !s.Contains(20) {
		t.Fatalf("set should contain 20")
	}
	if s.Contains(40) {
		t.Fatalf("set should not contain 40")
	}
	if s.Size() != 3 {
		t.Fatalf("set size should be 3")
	}
	if s.MemoryUsed() == 0 {
		t.Fatalf("set memory used should be > 0")
	}

	first, ok := s.First()
	if !ok || first != 10 {
		t.Fatalf("first should be 10, got %d", first)
	}

	next, ok := s.Next(10)
	if !ok || next != 20 {
		t.Fatalf("next after 10 should be 20, got %d", next)
	}

	nextAt, ok := s.NextAtOrAfter(10)
	if !ok || nextAt != 10 {
		t.Fatalf("next at or after 10 should be 10, got %d", nextAt)
	}
	nextAt2, ok := s.NextAtOrAfter(15)
	if !ok || nextAt2 != 20 {
		t.Fatalf("next at or after 15 should be 20, got %d", nextAt2)
	}

	last, ok := s.Last()
	if !ok || last != 30 {
		t.Fatalf("last should be 30, got %d", last)
	}

	prev, ok := s.Prev(30)
	if !ok || prev != 20 {
		t.Fatalf("prev before 30 should be 20, got %d", prev)
	}

	prevAt, ok := s.PrevAtOrBefore(30)
	if !ok || prevAt != 30 {
		t.Fatalf("prev at or before 30 should be 30, got %d", prevAt)
	}
	prevAt2, ok := s.PrevAtOrBefore(25)
	if !ok || prevAt2 != 20 {
		t.Fatalf("prev at or before 25 should be 20, got %d", prevAt2)
	}

	if s.Rank(20) != 1 {
		t.Fatalf("rank of 20 should be 1, got %d", s.Rank(20))
	}

	sel, ok := s.Select(1)
	if !ok || sel != 20 {
		t.Fatalf("select index 1 should be 20, got %d", sel)
	}

	if s.CountRange(10, 20) != 2 {
		t.Fatalf("count range [10, 20] should be 2, got %d", s.CountRange(10, 20))
	}

	// ContainsBatch
	keys := []uint64{10, 15, 20, 25, 30}
	present := make([]bool, len(keys))
	foundCount := s.ContainsBatch(keys, present)
	if foundCount != 3 {
		t.Fatalf("expected 3 found keys in batch, got %d", foundCount)
	}
	if !present[0] || present[1] || !present[2] || present[3] || !present[4] {
		t.Fatalf("unexpected present flags from ContainsBatch: %v", present)
	}

	s.Remove(20)
	if s.Contains(20) {
		t.Fatalf("remove failed")
	}
	if s.Size() != 2 {
		t.Fatalf("size after remove should be 2")
	}
	s.Clear()
	if s.Size() != 0 {
		t.Fatalf("clear failed")
	}
}

func TestMap(t *testing.T) {
	m := NewMap()
	m.Set(1, 100)
	m.Set(2, 200)
	m.Set(3, 300)

	val, ok := m.Get(2)
	if !ok || val != 200 {
		t.Fatalf("get 2 should be 200, got %d", val)
	}

	if !m.Contains(3) {
		t.Fatalf("contains 3 should be true")
	}
	if m.MemoryUsed() == 0 {
		t.Fatalf("map memory used should be > 0")
	}

	firstK, firstV, ok := m.First()
	if !ok || firstK != 1 || firstV != 100 {
		t.Fatalf("first failed")
	}

	nextK, nextV, ok := m.Next(1)
	if !ok || nextK != 2 || nextV != 200 {
		t.Fatalf("next failed")
	}

	nextAtK, nextAtV, ok := m.NextAtOrAfter(1)
	if !ok || nextAtK != 1 || nextAtV != 100 {
		t.Fatalf("next at or after 1 failed")
	}

	lastK, lastV, ok := m.Last()
	if !ok || lastK != 3 || lastV != 300 {
		t.Fatalf("last failed")
	}

	prevK, prevV, ok := m.Prev(3)
	if !ok || prevK != 2 || prevV != 200 {
		t.Fatalf("prev failed")
	}

	prevAtK, prevAtV, ok := m.PrevAtOrBefore(3)
	if !ok || prevAtK != 3 || prevAtV != 300 {
		t.Fatalf("prev at or before 3 failed")
	}

	if m.Rank(2) != 1 {
		t.Fatalf("rank of 2 should be 1, got %d", m.Rank(2))
	}

	selK, selV, ok := m.Select(1)
	if !ok || selK != 2 || selV != 200 {
		t.Fatalf("select 1 failed: %d, %d", selK, selV)
	}

	if m.CountRange(1, 2) != 2 {
		t.Fatalf("count range [1, 2] should be 2, got %d", m.CountRange(1, 2))
	}

	// GetBatch
	bKeys := []uint64{1, 2, 4}
	bVals := make([]uint64, len(bKeys))
	bFound := make([]bool, len(bKeys))
	bCount := m.GetBatch(bKeys, bVals, bFound)
	if bCount != 2 {
		t.Fatalf("expected 2 found in batch, got %d", bCount)
	}
	if !bFound[0] || bVals[0] != 100 || !bFound[1] || bVals[1] != 200 || bFound[2] {
		t.Fatalf("unexpected GetBatch results: found=%v vals=%v", bFound, bVals)
	}

	m.Delete(2)
	if m.Contains(2) {
		t.Fatalf("delete failed")
	}
	if m.Size() != 2 {
		t.Fatalf("size should be 2, got %d", m.Size())
	}
	m.Clear()
	if m.Size() != 0 {
		t.Fatalf("clear failed")
	}
}

func TestStrMap(t *testing.T) {
	m := NewStrMap()
	m.Set("alpha", 1)
	m.Set("beta", 2)
	m.Set("gamma", 3)

	val, ok := m.Get("beta")
	if !ok || val != 2 {
		t.Fatalf("get beta should be 2, got %d", val)
	}

	if !m.Contains("gamma") {
		t.Fatalf("contains gamma should be true")
	}
	if m.MemoryUsed() == 0 {
		t.Fatalf("strmap memory used should be > 0")
	}

	// Navigation
	firstK, firstV, ok := m.First()
	if !ok || firstK != "alpha" || firstV != 1 {
		t.Fatalf("first string failed: %s, %d", firstK, firstV)
	}

	lastK, lastV, ok := m.Last()
	if !ok || lastK != "gamma" || lastV != 3 {
		t.Fatalf("last string failed: %s, %d", lastK, lastV)
	}

	nextK, nextV, ok := m.Next("alpha")
	if !ok || nextK != "beta" || nextV != 2 {
		t.Fatalf("next after alpha failed: %s, %d", nextK, nextV)
	}

	nextAtK, nextAtV, ok := m.NextAtOrAfter("alpha")
	if !ok || nextAtK != "alpha" || nextAtV != 1 {
		t.Fatalf("next at or after alpha failed: %s, %d", nextAtK, nextAtV)
	}

	prevK, prevV, ok := m.Prev("gamma")
	if !ok || prevK != "beta" || prevV != 2 {
		t.Fatalf("prev before gamma failed: %s, %d", prevK, prevV)
	}

	prevAtK, prevAtV, ok := m.PrevAtOrBefore("gamma")
	if !ok || prevAtK != "gamma" || prevAtV != 3 {
		t.Fatalf("prev at or before gamma failed: %s, %d", prevAtK, prevAtV)
	}

	m.Delete("beta")
	if m.Contains("beta") {
		t.Fatalf("delete failed")
	}
	if m.Size() != 2 {
		t.Fatalf("size should be 2")
	}
	m.Clear()
}

func TestBytesMap(t *testing.T) {
	m := NewBytesMap()
	m.Set([]byte{0, 1, 2}, 42)
	val, ok := m.Get([]byte{0, 1, 2})
	if !ok || val != 42 {
		t.Fatalf("get failed")
	}
	if !m.Contains([]byte{0, 1, 2}) {
		t.Fatalf("contains failed")
	}
	if m.MemoryUsed() == 0 {
		t.Fatalf("bytesmap memory used should be > 0")
	}
	m.Delete([]byte{0, 1, 2})
	if m.Size() != 0 {
		t.Fatalf("delete failed")
	}
	m.Clear()
}

func TestBlobMap(t *testing.T) {
	b := NewBlobMap(4096)
	b.Set(10, []byte("hello world 1"), 1)
	b.Set(20, []byte("hello world 2"), 2)

	data, meta, ok := b.Get(10)
	if !ok || !bytes.Equal(data, []byte("hello world 1")) || meta != 1 {
		t.Fatalf("get failed: ok=%v data=%q meta=%d", ok, string(data), meta)
	}

	if b.Size() != 2 {
		t.Fatalf("size failed")
	}
	if !b.Contains(20) {
		t.Fatalf("contains failed")
	}
	if b.MemoryUsed() == 0 {
		t.Fatalf("blobmap memory used should be > 0")
	}

	pruned := b.Prune(func(key uint64, hotMeta uint32) bool {
		return hotMeta == 1
	})
	if pruned != 1 {
		t.Fatalf("prune failed, expected 1, got %d", pruned)
	}

	if b.Size() != 1 || b.Contains(10) {
		t.Fatalf("prune didn't remove the right element")
	}

	b.Delete(20)
	if b.Size() != 0 {
		t.Fatalf("delete failed")
	}
	b.Clear()
}

func TestSyncSet(t *testing.T) {
	s := NewSyncSet()
	s.Add(100)
	s.Add(200)

	if !s.Contains(100) {
		t.Fatalf("sync set should contain 100")
	}
	if s.Contains(300) {
		t.Fatalf("sync set should not contain 300")
	}
	if s.Size() != 2 {
		t.Fatalf("sync set size should be 2, got %d", s.Size())
	}

	reader := s.Reader()
	if !reader.Contains(100) {
		t.Fatalf("reader should contain 100")
	}
	if reader.Contains(300) {
		t.Fatalf("reader should not contain 300")
	}
	reader.Free()

	s.Remove(100)
	if s.Contains(100) {
		t.Fatalf("remove failed")
	}
	if s.Size() != 1 {
		t.Fatalf("size after remove should be 1")
	}
}

func TestSyncMap(t *testing.T) {
	m := NewSyncMap()
	m.Set(10, 1000)
	m.Set(20, 2000)

	val, ok := m.Get(10)
	if !ok || val != 1000 {
		t.Fatalf("get 10 failed: %v, %d", ok, val)
	}
	if m.Size() != 2 {
		t.Fatalf("sync map size should be 2, got %d", m.Size())
	}

	reader := m.Reader()
	rVal, rOk := reader.Get(20)
	if !rOk || rVal != 2000 {
		t.Fatalf("reader get 20 failed: %v, %d", rOk, rVal)
	}
	reader.Free()

	m.Delete(10)
	_, ok = m.Get(10)
	if ok {
		t.Fatalf("delete 10 failed")
	}
	if m.Size() != 1 {
		t.Fatalf("size should be 1")
	}
}

func TestBoundaryKeys(t *testing.T) {
	minKey := uint64(0)
	maxKey := ^uint64(0) // 0xFFFFFFFFFFFFFFFF

	// Set
	s := NewSet()
	defer s.Free()

	s.Add(minKey)
	s.Add(maxKey)

	if !s.Contains(minKey) || !s.Contains(maxKey) {
		t.Fatalf("set should contain minKey and maxKey")
	}
	if s.Size() != 2 {
		t.Fatalf("expected size 2, got %d", s.Size())
	}
	if s.Rank(maxKey) != 1 {
		t.Fatalf("rank of maxKey should be 1, got %d", s.Rank(maxKey))
	}
	firstK, ok := s.First()
	if !ok || firstK != minKey {
		t.Fatalf("firstKey should be minKey, got %d", firstK)
	}
	lastK, ok := s.Last()
	if !ok || lastK != maxKey {
		t.Fatalf("lastKey should be maxKey, got %d", lastK)
	}

	// Map
	m := NewMap()
	defer m.Free()

	m.Set(minKey, 111)
	m.Set(maxKey, 999)

	v0, ok0 := m.Get(minKey)
	vMax, okMax := m.Get(maxKey)
	if !ok0 || v0 != 111 || !okMax || vMax != 999 {
		t.Fatalf("map boundary key retrieval failed: (0: %d, %v), (max: %d, %v)", v0, ok0, vMax, okMax)
	}
	if m.CountRange(0, maxKey) != 2 {
		t.Fatalf("count range [0, maxKey] should be 2, got %d", m.CountRange(0, maxKey))
	}
}

func TestEmptyCollections(t *testing.T) {
	s := NewSet()
	defer s.Free()

	if s.Size() != 0 {
		t.Fatalf("new set size should be 0")
	}
	if _, ok := s.First(); ok {
		t.Fatalf("First on empty set should return false")
	}
	if _, ok := s.Last(); ok {
		t.Fatalf("Last on empty set should return false")
	}
	if _, ok := s.Next(0); ok {
		t.Fatalf("Next on empty set should return false")
	}
	if _, ok := s.Prev(0); ok {
		t.Fatalf("Prev on empty set should return false")
	}
	if s.ContainsBatch(nil, nil) != 0 {
		t.Fatalf("ContainsBatch on nil should return 0")
	}

	m := NewMap()
	defer m.Free()

	if m.Size() != 0 {
		t.Fatalf("new map size should be 0")
	}
	if _, _, ok := m.First(); ok {
		t.Fatalf("First on empty map should return false")
	}
	if _, _, ok := m.Last(); ok {
		t.Fatalf("Last on empty map should return false")
	}
	if _, _, ok := m.Next(0); ok {
		t.Fatalf("Next on empty map should return false")
	}
	if _, _, ok := m.Prev(0); ok {
		t.Fatalf("Prev on empty map should return false")
	}
	if m.GetBatch(nil, nil, nil) != 0 {
		t.Fatalf("GetBatch on nil should return 0")
	}

	sm := NewStrMap()
	defer sm.Free()

	if sm.Size() != 0 {
		t.Fatalf("new strmap size should be 0")
	}
	if _, _, ok := sm.First(); ok {
		t.Fatalf("First on empty strmap should return false")
	}
	if _, _, ok := sm.Last(); ok {
		t.Fatalf("Last on empty strmap should return false")
	}
}

func TestBatchBoundsSafety(t *testing.T) {
	m := NewMap()
	defer m.Free()

	for i := uint64(0); i < 50; i++ {
		m.Set(i, i*10)
	}

	keys := []uint64{5, 10, 15, 20}

	// Case 1: outValues shorter than keys -> should safely process min(len(keys), len(outValues))
	shortValues := make([]uint64, 2)
	shortFound := make([]bool, 2)
	count := m.GetBatch(keys, shortValues, shortFound)
	if count != 2 {
		t.Fatalf("expected count 2 for truncated slices, got %d", count)
	}
	if shortValues[0] != 50 || shortValues[1] != 100 || !shortFound[0] || !shortFound[1] {
		t.Fatalf("unexpected short batch results: %v, %v", shortValues, shortFound)
	}

	// Set batch
	s := NewSet()
	defer s.Free()
	for i := uint64(0); i < 50; i++ {
		s.Add(i)
	}
	shortPresent := make([]bool, 2)
	sCount := s.ContainsBatch(keys, shortPresent)
	if sCount != 2 {
		t.Fatalf("expected set count 2 for truncated slices, got %d", sCount)
	}
}

func TestStrMapEdgeCases(t *testing.T) {
	m := NewStrMap()
	defer m.Free()

	// Empty string key
	m.Set("", 42)
	val, ok := m.Get("")
	if !ok || val != 42 {
		t.Fatalf("empty string key lookup failed: %v, %d", ok, val)
	}

	// Long key (1024 chars)
	longKey := string(bytes.Repeat([]byte("a"), 1024))
	m.Set(longKey, 1024)
	lVal, lOk := m.Get(longKey)
	if !lOk || lVal != 1024 {
		t.Fatalf("long string key lookup failed: %v, %d", lOk, lVal)
	}

	// Navigation with long key
	firstK, _, ok := m.First()
	if !ok || firstK != "" {
		t.Fatalf("expected first key to be empty string, got %q", firstK)
	}
	lastK, _, ok := m.Last()
	if !ok || lastK != longKey {
		t.Fatalf("expected last key to be longKey")
	}
}

func TestConcurrentSyncMapStress(t *testing.T) {
	m := NewSyncMap()
	defer m.Free()

	// Pre-populate
	for i := uint64(0); i < 1000; i++ {
		m.Set(i, i*100)
	}

	done := make(chan struct{})
	readers := 4
	var wg sync.WaitGroup

	// Spawn readers
	for r := 0; r < readers; r++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for {
				select {
				case <-done:
					return
				default:
					reader := m.Reader()
					for k := uint64(0); k < 100; k++ {
						if val, ok := reader.Get(k); ok {
							_ = val
						}
					}
					reader.Free()
				}
			}
		}()
	}

	// Perform writes
	for i := uint64(1000); i < 2000; i++ {
		m.Set(i, i*100)
		if i%2 == 0 {
			m.Delete(i - 1000)
		}
	}

	close(done)
	wg.Wait()
}

func TestBlobMapCompaction(t *testing.T) {
	b := NewBlobMap(0)
	defer b.Clear()

	// Large payload (64 KB)
	largeData := bytes.Repeat([]byte("X"), 65536)
	b.Set(100, largeData, 42)

	out, meta, ok := b.Get(100)
	if !ok || len(out) != 65536 || meta != 42 {
		t.Fatalf("large blob retrieval failed: ok=%v len=%d meta=%d", ok, len(out), meta)
	}

	// MemoryUsed and Compact
	memBefore := b.MemoryUsed()
	if memBefore == 0 {
		t.Fatalf("memory before compact should be > 0")
	}
	b.Compact()
	memAfter := b.MemoryUsed()
	if memAfter == 0 {
		t.Fatalf("memory after compact should be > 0")
	}
}
