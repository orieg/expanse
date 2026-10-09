package expanse

import (
	"bytes"
	"testing"
)

// Shared fixtures. Keys compare as unsigned bytes: EMPTY < NUL < "a\x00b" < LONG < FF.
var (
	obEmpty = []byte{}
	obNul   = []byte{0x00}
	obFF    = []byte{0xFF}
	obANB   = []byte{'a', 0x00, 'b'}
	obLong  = []byte("long-key-0123")
)

// wantNav asserts one navigation result: presence, the exact key bytes and the value.
func wantNav(t *testing.T, what string, key []byte, value uint64, ok bool, wantKey []byte, wantValue uint64) {
	t.Helper()
	if !ok {
		t.Fatalf("%s: expected entry %x, got none", what, wantKey)
	}
	if !bytes.Equal(key, wantKey) {
		t.Fatalf("%s: key = %x, want %x", what, key, wantKey)
	}
	if value != wantValue {
		t.Fatalf("%s: value = %d, want %d", what, value, wantValue)
	}
}

// wantNone asserts a navigation call found no entry.
func wantNone(t *testing.T, what string, ok bool) {
	t.Helper()
	if ok {
		t.Fatalf("%s: expected no entry", what)
	}
}

func TestOrderedBytesMapRoundTrip(t *testing.T) {
	m := NewOrderedBytesMap()
	defer m.Free()

	if m.Size() != 0 {
		t.Fatalf("new map size = %d, want 0", m.Size())
	}
	for i, k := range [][]byte{obEmpty, obNul, obFF, obANB, obLong} {
		if !m.Set(k, uint64(i+1)) {
			t.Fatalf("Set(%x) reported not-new on first insert", k)
		}
	}
	if m.Set(obLong, 50) {
		t.Fatalf("Set on an existing key reported new")
	}
	if m.Size() != 5 {
		t.Fatalf("size = %d, want 5", m.Size())
	}

	if v, ok := m.Get(obEmpty); !ok || v != 1 {
		t.Fatalf("Get(empty) = %d,%v, want 1,true", v, ok)
	}
	if v, ok := m.Get(obNul); !ok || v != 2 {
		t.Fatalf("Get(0x00) = %d,%v, want 2,true", v, ok)
	}
	if v, ok := m.Get(obFF); !ok || v != 3 {
		t.Fatalf("Get(0xFF) = %d,%v, want 3,true", v, ok)
	}
	if v, ok := m.Get(obANB); !ok || v != 4 {
		t.Fatalf("Get(a\\x00b) = %d,%v, want 4,true", v, ok)
	}
	if v, ok := m.Get(obLong); !ok || v != 50 {
		t.Fatalf("Get(long) = %d,%v, want 50,true", v, ok)
	}

	// Absent: a prefix and an extension of present keys are distinct keys.
	if _, ok := m.Get([]byte{'a'}); ok {
		t.Fatalf("Get(\"a\") found a key; \"a\" is a prefix of a\\x00b, not a key")
	}
	if _, ok := m.Get([]byte{'a', 0x00}); ok {
		t.Fatalf("Get(\"a\\x00\") found a key")
	}
	if m.Contains([]byte{0x00, 0x00}) {
		t.Fatalf("Contains(0x00 0x00) = true, want false")
	}
	if !m.Contains(obEmpty) || !m.Contains(obANB) {
		t.Fatalf("Contains lost a present key")
	}

	if !m.Delete(obNul) {
		t.Fatalf("Delete(0x00) = false, want true")
	}
	if m.Delete(obNul) {
		t.Fatalf("second Delete(0x00) = true, want false")
	}
	if m.Contains(obNul) {
		t.Fatalf("Contains(0x00) after Delete = true")
	}
	if m.Size() != 4 {
		t.Fatalf("size after delete = %d, want 4", m.Size())
	}

	m.Clear()
	if m.Size() != 0 {
		t.Fatalf("size after Clear = %d, want 0", m.Size())
	}
	if _, ok := m.Get(obEmpty); ok {
		t.Fatalf("Get(empty) after Clear found a key")
	}
}

func TestOrderedBytesMapOrdering(t *testing.T) {
	m := NewOrderedBytesMap()
	defer m.Free()

	// Inserted out of order; value = rank in sorted order.
	m.Set(obLong, 4)
	m.Set(obFF, 5)
	m.Set(obANB, 3)
	m.Set(obNul, 2)
	m.Set(obEmpty, 1)

	// Sorted: "" < 0x00 < "a\x00b" (0x61) < "long-..." (0x6c) < 0xFF.
	wantKeys := [][]byte{obEmpty, obNul, obANB, obLong, obFF}
	key, v, ok := m.First()
	for i := 0; ; i++ {
		if i >= len(wantKeys) {
			t.Fatalf("iteration produced more than %d entries", len(wantKeys))
		}
		wantNav(t, "walk step", key, v, ok, wantKeys[i], uint64(i+1))
		if i == len(wantKeys)-1 {
			break
		}
		key, v, ok = m.Next(key)
	}
	if _, _, ok := m.Next(obFF); ok {
		t.Fatalf("Next after the largest key found an entry")
	}
}

func TestOrderedBytesMapFirstLast(t *testing.T) {
	m := NewOrderedBytesMap()
	defer m.Free()

	_, _, ok := m.First()
	wantNone(t, "First on empty map", ok)
	_, _, ok = m.Last()
	wantNone(t, "Last on empty map", ok)

	m.Set(obEmpty, 10)
	k, v, ok := m.First()
	wantNav(t, "First with only the empty key", k, v, ok, obEmpty, 10)
	k, v, ok = m.Last()
	wantNav(t, "Last with only the empty key", k, v, ok, obEmpty, 10)

	m.Set(obFF, 20)
	m.Set(obLong, 30)
	k, v, ok = m.First()
	wantNav(t, "First", k, v, ok, obEmpty, 10)
	k, v, ok = m.Last()
	wantNav(t, "Last", k, v, ok, obFF, 20)

	m.Delete(obEmpty)
	k, v, ok = m.First()
	wantNav(t, "First after deleting the empty key", k, v, ok, obLong, 30)
}

func TestOrderedBytesMapNeighbours(t *testing.T) {
	m := NewOrderedBytesMap()
	defer m.Free()
	m.Set(obNul, 1)
	m.Set(obANB, 2)
	m.Set(obFF, 3)

	// At a present key.
	k, v, ok := m.NextAtOrAfter(obANB)
	wantNav(t, "NextAtOrAfter(present)", k, v, ok, obANB, 2)
	k, v, ok = m.Next(obANB)
	wantNav(t, "Next(present)", k, v, ok, obFF, 3)
	k, v, ok = m.PrevAtOrBefore(obANB)
	wantNav(t, "PrevAtOrBefore(present)", k, v, ok, obANB, 2)
	k, v, ok = m.Prev(obANB)
	wantNav(t, "Prev(present)", k, v, ok, obNul, 1)

	// Between present keys (absent search key).
	mid := []byte{'a', 0x01}
	k, v, ok = m.NextAtOrAfter(mid)
	wantNav(t, "NextAtOrAfter(between)", k, v, ok, obFF, 3)
	k, v, ok = m.Next(mid)
	wantNav(t, "Next(between)", k, v, ok, obFF, 3)
	k, v, ok = m.PrevAtOrBefore(mid)
	wantNav(t, "PrevAtOrBefore(between)", k, v, ok, obANB, 2)
	k, v, ok = m.Prev(mid)
	wantNav(t, "Prev(between)", k, v, ok, obANB, 2)

	// The empty search key sorts before everything.
	k, v, ok = m.NextAtOrAfter(obEmpty)
	wantNav(t, "NextAtOrAfter(empty)", k, v, ok, obNul, 1)
	k, v, ok = m.Next(obEmpty)
	wantNav(t, "Next(empty)", k, v, ok, obNul, 1)
	_, _, ok = m.PrevAtOrBefore(obEmpty)
	wantNone(t, "PrevAtOrBefore(empty)", ok)
	_, _, ok = m.Prev(obEmpty)
	wantNone(t, "Prev(empty)", ok)

	// Past both ends.
	_, _, ok = m.Next(obFF)
	wantNone(t, "Next(largest)", ok)
	_, _, ok = m.NextAtOrAfter([]byte{0xFF, 0x00})
	wantNone(t, "NextAtOrAfter(past end)", ok)
	k, v, ok = m.PrevAtOrBefore([]byte{0xFF, 0x00})
	wantNav(t, "PrevAtOrBefore(past end)", k, v, ok, obFF, 3)
	_, _, ok = m.Prev(obNul)
	wantNone(t, "Prev(0x00)", ok)

	// A prefix sorts before its extension: the entry below "a" is not "a\x00b".
	k, v, ok = m.NextAtOrAfter([]byte{'a'})
	wantNav(t, "NextAtOrAfter(prefix)", k, v, ok, obANB, 2)
	k, v, ok = m.Prev([]byte{'a'})
	wantNav(t, "Prev(prefix)", k, v, ok, obNul, 1)
}

func TestOrderedBytesMapLongKeys(t *testing.T) {
	// 300 bytes is just past the first 256-byte buffer; 5000 and 70000 force a
	// larger retry. Each key is returned whole, with no truncation.
	long300 := bytes.Repeat([]byte{0xAB}, 300)
	long300[0] = 0x01
	big := bytes.Repeat([]byte{0xAB}, 5000)
	big[0] = 0x01
	big[4999] = 0x00
	bigger := bytes.Repeat([]byte{0xFE}, 70000)
	bigger[0] = 0x01

	m := NewOrderedBytesMap()
	defer m.Free()
	m.Set(big, 7)
	m.Set(bigger, 8)
	m.Set(long300, 9)

	// Sorted: long300 and big share the leading 0x01 0xAB..., so compare explicitly.
	k, v, ok := m.First()
	wantNav(t, "First long", k, v, ok, long300, 9)
	k, v, ok = m.Next(long300)
	wantNav(t, "Next after long300", k, v, ok, big, 7)
	k, v, ok = m.Last()
	wantNav(t, "Last long", k, v, ok, bigger, 8)
	k, v, ok = m.Prev(bigger)
	wantNav(t, "Prev(bigger)", k, v, ok, big, 7)
	k, v, ok = m.PrevAtOrBefore(big)
	wantNav(t, "PrevAtOrBefore(big)", k, v, ok, big, 7)
	k, v, ok = m.NextAtOrAfter(obEmpty)
	wantNav(t, "NextAtOrAfter(empty) long", k, v, ok, long300, 9)
	k, v, ok = m.PrevAtOrBefore([]byte{0xFF})
	wantNav(t, "PrevAtOrBefore(0xFF) long", k, v, ok, bigger, 8)
}

func TestOrderedBytesMapIterationAfterMutation(t *testing.T) {
	m := NewOrderedBytesMap()
	defer m.Free()
	for i := 0; i < 10; i++ {
		m.Set([]byte{byte(i), 0x00}, uint64(i))
	}
	if !m.Delete([]byte{3, 0x00}) {
		t.Fatalf("Delete({3,0}) = false")
	}
	m.Set([]byte{3, 0x01}, 33)
	m.Set([]byte{0xFF, 0xFF}, 99)

	want := []uint64{0, 1, 2, 33, 4, 5, 6, 7, 8, 9, 99}
	var got []uint64
	k, v, ok := m.First()
	for ok {
		if len(got) > len(want) {
			t.Fatalf("iteration did not terminate: %v", got)
		}
		got = append(got, v)
		k, v, ok = m.Next(k)
	}
	if !equalU64(got, want) {
		t.Fatalf("values after mutation = %v, want %v", got, want)
	}

	// Mutate while stepping: removing the cursor's key still lets Next advance.
	k, v, ok = m.First()
	wantNav(t, "cursor start", k, v, ok, []byte{0, 0}, 0)
	m.Delete(k)
	k, v, ok = m.Next(k)
	wantNav(t, "Next after deleting cursor key", k, v, ok, []byte{1, 0}, 1)
	k, v, ok = m.First()
	wantNav(t, "First after deleting cursor key", k, v, ok, []byte{1, 0}, 1)
	if m.Size() != 10 {
		t.Fatalf("size = %d, want 10", m.Size())
	}
}

func TestOrderedBytesMapSlots(t *testing.T) {
	m := NewOrderedBytesMap()
	defer m.Free()

	if p := m.Slot(obANB); p != nil {
		t.Fatalf("Slot on absent key returned a pointer")
	}
	p := m.InsSlot(obANB)
	if *p != 0 {
		t.Fatalf("InsSlot of new key: value = %d, want 0", *p)
	}
	*p = 777
	if v, ok := m.Get(obANB); !ok || v != 777 {
		t.Fatalf("Get after writing through InsSlot = %d,%v, want 777,true", v, ok)
	}

	// Re-inserting an existing key keeps its value and returns its slot.
	again := m.InsSlot(obANB)
	if *again != 777 {
		t.Fatalf("InsSlot of present key: value = %d, want 777 (existing value kept)", *again)
	}
	if s := m.Slot(obANB); s == nil || *s != 777 {
		t.Fatalf("Slot of present key did not return the stored value")
	}

	e := m.InsSlot(obEmpty)
	*e = 5
	if v, ok := m.Get(obEmpty); !ok || v != 5 {
		t.Fatalf("Get(empty) after writing through InsSlot = %d,%v, want 5,true", v, ok)
	}
	if m.Size() != 2 {
		t.Fatalf("size = %d, want 2", m.Size())
	}
}

func TestOrderedBytesMapMemory(t *testing.T) {
	m := NewOrderedBytesMap()
	defer m.Free()
	for i := 0; i < 2000; i++ {
		m.Set([]byte{byte(i >> 8), byte(i), 0x00, 0xFF}, uint64(i))
	}
	if m.MemoryUsed() == 0 {
		t.Fatalf("MemoryUsed = 0 for a populated map")
	}
	if m.MemoryHeld() < m.MemoryUsed() {
		t.Fatalf("MemoryHeld %d < MemoryUsed %d", m.MemoryHeld(), m.MemoryUsed())
	}
	m.Clear()
	before := m.MemoryHeld()
	m.ShrinkToFit()
	if m.MemoryHeld() > before {
		t.Fatalf("ShrinkToFit grew MemoryHeld: %d -> %d", before, m.MemoryHeld())
	}
	if m.Size() != 0 {
		t.Fatalf("size after Clear = %d, want 0", m.Size())
	}
}

func equalU64(a, b []uint64) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
