package expanse

import (
	"bytes"
	"errors"
	"fmt"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
)

// ---- SyncStrMap ----

func TestSyncStrMapPointOps(t *testing.T) {
	m := NewSyncStrMap()
	defer m.Free()

	if _, ok := m.Get("a"); ok {
		t.Fatal("empty map reports a present")
	}
	for i, k := range []string{"", "a", "ab", "abc", "b"} {
		if err := m.Set(k, uint64(i)+10); err != nil {
			t.Fatalf("Set(%q): %v", k, err)
		}
	}
	if m.Size() != 5 {
		t.Fatalf("Size = %d, want 5", m.Size())
	}
	if v, ok := m.Get("ab"); !ok || v != 12 {
		t.Fatalf("Get(ab) = (%d, %v), want (12, true)", v, ok)
	}
	if v, ok := m.Get(""); !ok || v != 10 {
		t.Fatalf("Get(empty) = (%d, %v), want (10, true)", v, ok)
	}
	if err := m.Set("ab", 99); err != nil {
		t.Fatal(err)
	}
	if v, _ := m.Get("ab"); v != 99 || m.Size() != 5 {
		t.Fatalf("overwrite: value %d size %d, want 99 and 5", v, m.Size())
	}
	if !m.Contains("abc") || m.Contains("abd") {
		t.Fatal("Contains wrong")
	}
	if !m.Delete("abc") || m.Delete("abc") {
		t.Fatal("Delete must report presence once")
	}
	if m.Size() != 4 {
		t.Fatalf("Size after delete = %d, want 4", m.Size())
	}

	// A key with an embedded NUL is never stored: writes are refused with
	// ErrNulInKey and the map is unchanged; reads report absent.
	if err := m.Set("a\x00b", 1); !errors.Is(err, ErrNulInKey) {
		t.Fatalf("Set with NUL: err %v, want ErrNulInKey", err)
	}
	if _, ok := m.Get("a\x00b"); ok || m.Contains("a\x00b") || m.Delete("a\x00b") {
		t.Fatal("a NUL-bearing key must read as absent")
	}
	if v, _ := m.Get("a"); v != 11 {
		t.Fatalf("a was disturbed by a NUL-bearing write: %d", v)
	}
	if _, _, err := m.CompareExchange("a\x00b", Word{}, Some(1)); !errors.Is(err, ErrNulInKey) {
		t.Fatalf("CompareExchange with NUL: err %v", err)
	}
	if _, _, err := m.GetOrInsert("a\x00b", 1); !errors.Is(err, ErrNulInKey) {
		t.Fatalf("GetOrInsert with NUL: err %v", err)
	}
	if _, _, err := m.Update("a\x00b", func(uint64, bool) (uint64, bool) { return 1, true }); !errors.Is(err, ErrNulInKey) {
		t.Fatalf("Update with NUL: err %v", err)
	}

	if m.MemoryUsed() == 0 || m.MemoryHeld() < m.MemoryUsed() {
		t.Fatalf("memory accounting: used %d held %d", m.MemoryUsed(), m.MemoryHeld())
	}
	m.ShrinkToFit()

	r := m.Reader()
	defer r.Free()
	if v, ok := r.Get("ab"); !ok || v != 99 {
		t.Fatalf("reader Get(ab) = (%d, %v)", v, ok)
	}
	if !r.Contains("b") || r.Contains("zz") || r.Contains("a\x00b") {
		t.Fatal("reader Contains wrong")
	}
	if _, ok := r.Get("a\x00b"); ok {
		t.Fatal("reader must report a NUL-bearing key absent")
	}
}

func TestSyncStrMapReaderOrderedReads(t *testing.T) {
	m := NewSyncStrMap()
	defer m.Free()
	r := m.Reader()
	defer r.Free()

	if _, _, ok := r.First(); ok {
		t.Fatal("First on empty map")
	}
	if _, _, ok := r.Last(); ok {
		t.Fatal("Last on empty map")
	}

	// A key longer than the first buffer (256) exercises the BUFFER_TOO_SMALL
	// retry; one of exactly 255 bytes fits only with its NUL, 256 does not.
	long := strings.Repeat("x", 1000)
	keys := []string{"", "a", "b", "c", strings.Repeat("m", 255), strings.Repeat("n", 256), long}
	for i, k := range keys {
		if err := m.Set(k, uint64(i)); err != nil {
			t.Fatal(err)
		}
	}
	// Ascending order of the stored keys.
	want := []string{"", "a", "b", "c", strings.Repeat("m", 255), strings.Repeat("n", 256), long}
	// "m...", "n...", "x..." sort after "c", in that order, so want is sorted.

	k, v, ok := r.First()
	if !ok || k != "" || v != 0 {
		t.Fatalf("First = (%q, %d, %v)", k, v, ok)
	}
	k, v, ok = r.Last()
	if !ok || k != long || v != 6 {
		t.Fatalf("Last = (%d bytes, %d, %v)", len(k), v, ok)
	}

	// Walk forward and back.
	cur, _, ok := r.First()
	var got []string
	for ok {
		got = append(got, cur)
		cur, _, ok = r.Next(cur)
	}
	if fmt.Sprint(got) != fmt.Sprint(want) {
		t.Fatalf("forward walk visited %d keys, want %d", len(got), len(want))
	}
	got = got[:0]
	cur, _, ok = r.Last()
	for ok {
		got = append(got, cur)
		cur, _, ok = r.Prev(cur)
	}
	for i := range got {
		if got[i] != want[len(want)-1-i] {
			t.Fatalf("backward walk differs at %d", i)
		}
	}
	if len(got) != len(want) {
		t.Fatalf("backward walk visited %d keys, want %d", len(got), len(want))
	}

	// At-or-after / at-or-before and the strict forms.
	if k, _, ok := r.NextAtOrAfter("b"); !ok || k != "b" {
		t.Fatalf("NextAtOrAfter(b) = %q, %v", k, ok)
	}
	if k, _, ok := r.Next("b"); !ok || k != "c" {
		t.Fatalf("Next(b) = %q, %v", k, ok)
	}
	if k, _, ok := r.NextAtOrAfter("bb"); !ok || k != "c" {
		t.Fatalf("NextAtOrAfter(bb) = %q, %v", k, ok)
	}
	if k, _, ok := r.PrevAtOrBefore("b"); !ok || k != "b" {
		t.Fatalf("PrevAtOrBefore(b) = %q, %v", k, ok)
	}
	if k, _, ok := r.Prev("b"); !ok || k != "a" {
		t.Fatalf("Prev(b) = %q, %v", k, ok)
	}
	if k, _, ok := r.PrevAtOrBefore("bb"); !ok || k != "b" {
		t.Fatalf("PrevAtOrBefore(bb) = %q, %v", k, ok)
	}
	if _, _, ok := r.Next(long); ok {
		t.Fatal("Next(largest) must be absent")
	}
	if _, _, ok := r.Prev(""); ok {
		t.Fatal("Prev(empty) must be absent")
	}

	// A search key containing NUL behaves as its NUL-free prefix p: the next
	// stored key above "b\x00..." is the one above "b", and so on (nulIndex).
	if k, _, ok := r.Next("b\x00zzz"); !ok || k != "c" {
		t.Fatalf("Next(b NUL zzz) = %q, %v", k, ok)
	}
	if k, _, ok := r.NextAtOrAfter("b\x00zzz"); !ok || k != "c" {
		t.Fatalf("NextAtOrAfter(b NUL zzz) = %q, %v", k, ok)
	}
	if k, _, ok := r.Prev("b\x00zzz"); !ok || k != "b" {
		t.Fatalf("Prev(b NUL zzz) = %q, %v", k, ok)
	}
	if k, _, ok := r.PrevAtOrBefore("b\x00zzz"); !ok || k != "b" {
		t.Fatalf("PrevAtOrBefore(b NUL zzz) = %q, %v", k, ok)
	}
}

func TestSyncStrMapReadModifyWrite(t *testing.T) {
	m := NewSyncStrMap()
	defer m.Free()

	// Absent expected inserts; a second try observes the stored word.
	obs, stored, err := m.CompareExchange("k", Word{}, Some(5))
	if err != nil || !stored || obs.Present {
		t.Fatalf("insert via CAS: obs %+v stored %v err %v", obs, stored, err)
	}
	obs, stored, _ = m.CompareExchange("k", Word{}, Some(6))
	if stored || obs != Some(5) {
		t.Fatalf("CAS on a present key expecting absent: obs %+v stored %v", obs, stored)
	}
	// Wrong value reports the word seen; the right value stores (Some(0) is a value).
	obs, stored, _ = m.CompareExchange("k", Some(4), Some(7))
	if stored || obs != Some(5) {
		t.Fatalf("CAS wrong expected: obs %+v stored %v", obs, stored)
	}
	obs, stored, _ = m.CompareExchange("k", Some(5), Some(0))
	if !stored || obs != Some(5) {
		t.Fatalf("CAS right expected: obs %+v stored %v", obs, stored)
	}
	if v, ok := m.Get("k"); !ok || v != 0 {
		t.Fatalf("Get after CAS to zero = (%d, %v)", v, ok)
	}
	// A new absent removes.
	obs, stored, _ = m.CompareExchange("k", Some(0), Word{})
	if !stored || !obs.Present {
		t.Fatalf("CAS remove: obs %+v stored %v", obs, stored)
	}
	if m.Contains("k") {
		t.Fatal("key survived a CAS remove")
	}

	if ex, ok, err := m.GetOrInsert("g", 1); err != nil || ok || ex != 0 {
		t.Fatalf("GetOrInsert new = (%d, %v, %v), want (0, false, nil)", ex, ok, err)
	}
	if ex, ok, _ := m.GetOrInsert("g", 2); !ok || ex != 1 {
		t.Fatalf("GetOrInsert present = (%d, %v), want (1, true)", ex, ok)
	}
	if v, _ := m.Get("g"); v != 1 {
		t.Fatalf("GetOrInsert overwrote: %d", v)
	}

	prev, ok, err := m.Update("u", func(cur uint64, present bool) (uint64, bool) {
		if present {
			t.Error("Update saw u present")
		}
		return 3, true
	})
	if err != nil || ok || prev != 0 {
		t.Fatalf("Update absent = (%d, %v, %v)", prev, ok, err)
	}
	prev, ok, _ = m.Update("u", func(cur uint64, _ bool) (uint64, bool) { return cur + 1, true })
	if !ok || prev != 3 {
		t.Fatalf("Update present = (%d, %v)", prev, ok)
	}
	m.Update("u", func(uint64, bool) (uint64, bool) { return 0, false })
	if m.Contains("u") {
		t.Fatal("Update returning false must remove the key")
	}
}

func TestSyncStrMapConcurrent(t *testing.T) {
	m := NewSyncStrMap()
	defer m.Free()
	const goroutines, perGoroutine = 8, 2000

	var wg sync.WaitGroup
	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := 0; i < perGoroutine; i++ {
				if _, _, err := m.Update("counter", func(cur uint64, _ bool) (uint64, bool) { return cur + 1, true }); err != nil {
					t.Error(err)
					return
				}
			}
		}()
	}
	wg.Wait()
	if v, _ := m.Get("counter"); v != goroutines*perGoroutine {
		t.Fatalf("counter = %d, want %d", v, goroutines*perGoroutine)
	}

	// Of any number of goroutines racing on one absent key, exactly one stores.
	var winners int64
	var racers sync.WaitGroup
	start := make(chan struct{})
	for g := 0; g < goroutines; g++ {
		racers.Add(1)
		go func(g uint64) {
			defer racers.Done()
			<-start
			if _, present, _ := m.GetOrInsert("race", g); !present {
				atomic.AddInt64(&winners, 1)
			}
		}(uint64(g))
	}
	close(start)
	racers.Wait()
	if winners != 1 {
		t.Fatalf("%d goroutines stored into one absent key, want 1", winners)
	}

	// Readers walk and look up while a writer inserts and removes. Keys "k%04d"
	// with value == index are never overwritten with another value, so any value
	// a reader sees must match its key; ordered reads must stay ascending.
	for i := 0; i < 500; i++ {
		m.Set(fmt.Sprintf("k%04d", i), uint64(i))
	}
	done := make(chan struct{})
	var rw sync.WaitGroup
	for g := 0; g < 4; g++ {
		rw.Add(1)
		go func() {
			defer rw.Done()
			r := m.Reader()
			defer r.Free()
			for {
				select {
				case <-done:
					return
				default:
				}
				for i := 0; i < 500; i += 7 {
					if v, ok := r.Get(fmt.Sprintf("k%04d", i)); ok && v != uint64(i) {
						t.Errorf("k%04d = %d", i, v)
						return
					}
				}
				prev := ""
				for k, _, ok := r.NextAtOrAfter("k"); ok && strings.HasPrefix(k, "k"); k, _, ok = r.Next(k) {
					if k <= prev {
						t.Errorf("ordered walk went backwards: %q after %q", k, prev)
						return
					}
					prev = k
				}
			}
		}()
	}
	for round := 0; round < 20; round++ {
		for i := 500; i < 700; i++ {
			m.Set(fmt.Sprintf("k%04d", i), uint64(i))
		}
		for i := 500; i < 700; i++ {
			m.Delete(fmt.Sprintf("k%04d", i))
		}
	}
	close(done)
	rw.Wait()
}

// ---- SyncBytesMap and SyncOrderedBytesMap ----

// byteKeys are keys the string map cannot hold: empty, with embedded NULs and
// 0xFF bytes, and one longer than the first navigation buffer.
func byteKeys() [][]byte {
	return [][]byte{
		{},
		{0},
		{0, 0},
		{0, 1},
		{1},
		[]byte("abc"),
		{0xFF},
		{0xFF, 0xFF},
		bytes.Repeat([]byte{7}, 300),
	}
}

func TestSyncBytesMapPointOps(t *testing.T) {
	m := NewSyncBytesMap()
	defer m.Free()
	keys := byteKeys()
	for i, k := range keys {
		if !m.Set(k, uint64(i)+100) {
			t.Fatalf("Set(%v) reported not new", k)
		}
	}
	if m.Set(keys[2], 555) {
		t.Fatal("overwrite reported new")
	}
	if m.Size() != uint64(len(keys)) {
		t.Fatalf("Size = %d, want %d", m.Size(), len(keys))
	}
	for i, k := range keys {
		want := uint64(i) + 100
		if i == 2 {
			want = 555
		}
		if v, ok := m.Get(k); !ok || v != want {
			t.Fatalf("Get(%d) = (%d, %v), want %d", i, v, ok, want)
		}
		if !m.Contains(k) {
			t.Fatalf("Contains(%d) false", i)
		}
	}
	if m.Contains([]byte{9, 9}) {
		t.Fatal("absent key reported present")
	}
	if !m.Delete(keys[3]) || m.Delete(keys[3]) {
		t.Fatal("Delete must report presence once")
	}
	// A nil key is the empty key.
	if v, ok := m.Get(nil); !ok || v != 100 {
		t.Fatalf("Get(nil) = (%d, %v)", v, ok)
	}
	if m.MemoryUsed() == 0 || m.MemoryHeld() < m.MemoryUsed() {
		t.Fatalf("memory accounting: used %d held %d", m.MemoryUsed(), m.MemoryHeld())
	}
	m.ShrinkToFit()

	r := m.Reader()
	defer r.Free()
	if v, ok := r.Get(keys[8]); !ok || v != 108 {
		t.Fatalf("reader Get(long) = (%d, %v)", v, ok)
	}
	if !r.Contains(keys[0]) || r.Contains(keys[3]) {
		t.Fatal("reader Contains wrong")
	}
}

func TestSyncBytesMapReadModifyWrite(t *testing.T) {
	m := NewSyncBytesMap()
	defer m.Free()
	key := []byte{0, 1, 0}

	obs, stored := m.CompareExchange(key, Word{}, Some(5))
	if !stored || obs.Present {
		t.Fatalf("insert via CAS: obs %+v stored %v", obs, stored)
	}
	obs, stored = m.CompareExchange(key, Some(4), Some(7))
	if stored || obs != Some(5) {
		t.Fatalf("CAS wrong expected: obs %+v stored %v", obs, stored)
	}
	obs, stored = m.CompareExchange(key, Some(5), Some(0))
	if !stored || obs != Some(5) {
		t.Fatalf("CAS right expected: obs %+v stored %v", obs, stored)
	}
	obs, stored = m.CompareExchange(key, Some(0), Word{})
	if !stored || m.Contains(key) {
		t.Fatalf("CAS remove: obs %+v stored %v", obs, stored)
	}
	if ex, ok := m.GetOrInsert(nil, 1); ok || ex != 0 {
		t.Fatalf("GetOrInsert new = (%d, %v)", ex, ok)
	}
	if ex, ok := m.GetOrInsert([]byte{}, 2); !ok || ex != 1 {
		t.Fatalf("GetOrInsert present = (%d, %v)", ex, ok)
	}
	prev, ok := m.Update(key, func(cur uint64, present bool) (uint64, bool) { return cur + 10, true })
	if ok || prev != 0 {
		t.Fatalf("Update absent = (%d, %v)", prev, ok)
	}
	prev, ok = m.Update(key, func(cur uint64, _ bool) (uint64, bool) { return cur + 10, true })
	if !ok || prev != 10 {
		t.Fatalf("Update present = (%d, %v)", prev, ok)
	}
}

func TestSyncBytesMapConcurrent(t *testing.T) {
	m := NewSyncBytesMap()
	defer m.Free()
	key := []byte{0, 0xFF, 0}
	const goroutines, perGoroutine = 8, 2000
	var wg sync.WaitGroup
	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := 0; i < perGoroutine; i++ {
				m.Update(key, func(cur uint64, _ bool) (uint64, bool) { return cur + 1, true })
			}
		}()
	}
	wg.Wait()
	if v, _ := m.Get(key); v != goroutines*perGoroutine {
		t.Fatalf("counter = %d, want %d", v, goroutines*perGoroutine)
	}

	var winners int64
	var racers sync.WaitGroup
	start := make(chan struct{})
	for g := 0; g < goroutines; g++ {
		racers.Add(1)
		go func(g uint64) {
			defer racers.Done()
			<-start
			if _, present := m.GetOrInsert([]byte("race"), g); !present {
				atomic.AddInt64(&winners, 1)
			}
		}(uint64(g))
	}
	close(start)
	racers.Wait()
	if winners != 1 {
		t.Fatalf("%d goroutines stored into one absent key, want 1", winners)
	}

	done := make(chan struct{})
	var rw sync.WaitGroup
	for g := 0; g < 4; g++ {
		rw.Add(1)
		go func() {
			defer rw.Done()
			r := m.Reader()
			defer r.Free()
			for {
				select {
				case <-done:
					return
				default:
				}
				for i := 0; i < 300; i += 5 {
					if v, ok := r.Get([]byte{1, byte(i), byte(i >> 8)}); ok && v != uint64(i) {
						t.Errorf("key %d = %d", i, v)
						return
					}
				}
			}
		}()
	}
	for round := 0; round < 20; round++ {
		for i := 0; i < 300; i++ {
			m.Set([]byte{1, byte(i), byte(i >> 8)}, uint64(i))
		}
		for i := 0; i < 300; i++ {
			m.Delete([]byte{1, byte(i), byte(i >> 8)})
		}
	}
	close(done)
	rw.Wait()
}

func TestSyncOrderedBytesMapPointOps(t *testing.T) {
	m := NewSyncOrderedBytesMap()
	defer m.Free()
	keys := byteKeys()
	for i, k := range keys {
		if !m.Set(k, uint64(i)+100) {
			t.Fatalf("Set(%v) reported not new", k)
		}
	}
	if m.Set(keys[2], 555) {
		t.Fatal("overwrite reported new")
	}
	if m.Size() != uint64(len(keys)) {
		t.Fatalf("Size = %d, want %d", m.Size(), len(keys))
	}
	if v, ok := m.Get(keys[8]); !ok || v != 108 {
		t.Fatalf("Get(long) = (%d, %v)", v, ok)
	}
	if !m.Contains(nil) || m.Contains([]byte{9, 9}) {
		t.Fatal("Contains wrong")
	}
	if !m.Delete(keys[3]) || m.Delete(keys[3]) {
		t.Fatal("Delete must report presence once")
	}
	if m.MemoryUsed() == 0 || m.MemoryHeld() < m.MemoryUsed() {
		t.Fatalf("memory accounting: used %d held %d", m.MemoryUsed(), m.MemoryHeld())
	}
	m.ShrinkToFit()
	r := m.Reader()
	defer r.Free()
	if v, ok := r.Get(keys[5]); !ok || v != 105 {
		t.Fatalf("reader Get = (%d, %v)", v, ok)
	}
	if !r.Contains(keys[0]) || r.Contains(keys[3]) {
		t.Fatal("reader Contains wrong")
	}
}

func TestSyncOrderedBytesMapOrderedReads(t *testing.T) {
	m := NewSyncOrderedBytesMap()
	defer m.Free()
	if _, _, ok := m.First(); ok {
		t.Fatal("First on empty map")
	}
	if _, _, ok := m.Last(); ok {
		t.Fatal("Last on empty map")
	}
	// Byte order: {} < {0} < {0,0} < {0,1} < {1} < "abc" < {0xFF} < {0xFF,0xFF}
	// and the 300-byte run of 7s sits between {1} and "abc".
	sorted := [][]byte{
		{}, {0}, {0, 0}, {0, 1}, {1},
		bytes.Repeat([]byte{7}, 300),
		[]byte("abc"), {0xFF}, {0xFF, 0xFF},
	}
	for i, k := range sorted {
		m.Set(k, uint64(i))
	}

	k, v, ok := m.First()
	if !ok || len(k) != 0 || v != 0 {
		t.Fatalf("First = (%v, %d, %v), want the empty key", k, v, ok)
	}
	k, v, ok = m.Last()
	if !ok || !bytes.Equal(k, []byte{0xFF, 0xFF}) || v != 8 {
		t.Fatalf("Last = (%v, %d, %v)", k, v, ok)
	}

	// Forward and backward walks cover every key in order, including the
	// 300-byte key that needs the retry at the reported length.
	cur, _, ok := m.First()
	for i := 0; i < len(sorted); i++ {
		if !ok || !bytes.Equal(cur, sorted[i]) {
			t.Fatalf("forward walk step %d: ok %v, got %d-byte key %v", i, ok, len(cur), cur)
		}
		cur, _, ok = m.Next(cur)
	}
	if ok {
		t.Fatal("Next(largest) must be absent")
	}
	cur, _, ok = m.Last()
	for i := len(sorted) - 1; i >= 0; i-- {
		if !ok || !bytes.Equal(cur, sorted[i]) {
			t.Fatalf("backward walk step %d: ok %v, got %d-byte key", i, ok, len(cur))
		}
		cur, _, ok = m.Prev(cur)
	}
	if ok {
		t.Fatal("Prev(empty key) must be absent")
	}

	// Inclusive forms, and keys that are not stored.
	if k, _, ok := m.NextAtOrAfter([]byte{0, 1}); !ok || !bytes.Equal(k, []byte{0, 1}) {
		t.Fatalf("NextAtOrAfter(0,1) = %v, %v", k, ok)
	}
	if k, _, ok := m.NextAtOrAfter([]byte{0, 2}); !ok || !bytes.Equal(k, []byte{1}) {
		t.Fatalf("NextAtOrAfter(0,2) = %v, %v", k, ok)
	}
	if k, _, ok := m.PrevAtOrBefore([]byte{0, 2}); !ok || !bytes.Equal(k, []byte{0, 1}) {
		t.Fatalf("PrevAtOrBefore(0,2) = %v, %v", k, ok)
	}
	if k, _, ok := m.PrevAtOrBefore([]byte{0, 1}); !ok || !bytes.Equal(k, []byte{0, 1}) {
		t.Fatalf("PrevAtOrBefore(0,1) = %v, %v", k, ok)
	}
	if k, _, ok := m.Prev([]byte{0, 1}); !ok || !bytes.Equal(k, []byte{0, 0}) {
		t.Fatalf("Prev(0,1) = %v, %v", k, ok)
	}
	if k, _, ok := m.Next(nil); !ok || !bytes.Equal(k, []byte{0}) {
		t.Fatalf("Next(empty) = %v, %v", k, ok)
	}
	if k, _, ok := m.NextAtOrAfter(nil); !ok || len(k) != 0 {
		t.Fatalf("NextAtOrAfter(empty) = %v, %v, want the empty key", k, ok)
	}
	// Navigation returns copies: writing to one must not touch the map.
	k, _, _ = m.First()
	k = append(k, 1)
	if _, ok := m.Get(nil); !ok {
		t.Fatal("empty key lost")
	}
}

func TestSyncOrderedBytesMapReadModifyWrite(t *testing.T) {
	m := NewSyncOrderedBytesMap()
	defer m.Free()
	key := []byte{0, 1, 0}

	obs, stored := m.CompareExchange(key, Word{}, Some(5))
	if !stored || obs.Present {
		t.Fatalf("insert via CAS: obs %+v stored %v", obs, stored)
	}
	obs, stored = m.CompareExchange(key, Word{}, Some(6))
	if stored || obs != Some(5) {
		t.Fatalf("CAS expecting absent on a present key: obs %+v stored %v", obs, stored)
	}
	obs, stored = m.CompareExchange(key, Some(5), Some(0))
	if !stored || obs != Some(5) {
		t.Fatalf("CAS right expected: obs %+v stored %v", obs, stored)
	}
	obs, stored = m.CompareExchange(key, Some(0), Word{})
	if !stored || m.Contains(key) {
		t.Fatalf("CAS remove: obs %+v stored %v", obs, stored)
	}
	if ex, ok := m.GetOrInsert(nil, 1); ok || ex != 0 {
		t.Fatalf("GetOrInsert new = (%d, %v)", ex, ok)
	}
	if ex, ok := m.GetOrInsert(nil, 2); !ok || ex != 1 {
		t.Fatalf("GetOrInsert present = (%d, %v)", ex, ok)
	}
	prev, ok := m.Update(key, func(cur uint64, _ bool) (uint64, bool) { return cur + 10, true })
	if ok || prev != 0 {
		t.Fatalf("Update absent = (%d, %v)", prev, ok)
	}
	prev, ok = m.Update(key, func(cur uint64, _ bool) (uint64, bool) { return cur + 10, true })
	if !ok || prev != 10 {
		t.Fatalf("Update present = (%d, %v)", prev, ok)
	}
}

func TestSyncOrderedBytesMapConcurrent(t *testing.T) {
	m := NewSyncOrderedBytesMap()
	defer m.Free()
	key := []byte{0, 0xFF, 0}
	const goroutines, perGoroutine = 8, 2000
	var wg sync.WaitGroup
	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for i := 0; i < perGoroutine; i++ {
				m.Update(key, func(cur uint64, _ bool) (uint64, bool) { return cur + 1, true })
			}
		}()
	}
	wg.Wait()
	if v, _ := m.Get(key); v != goroutines*perGoroutine {
		t.Fatalf("counter = %d, want %d", v, goroutines*perGoroutine)
	}

	var winners int64
	var racers sync.WaitGroup
	start := make(chan struct{})
	for g := 0; g < goroutines; g++ {
		racers.Add(1)
		go func(g uint64) {
			defer racers.Done()
			<-start
			if _, present := m.GetOrInsert([]byte("race"), g); !present {
				atomic.AddInt64(&winners, 1)
			}
		}(uint64(g))
	}
	close(start)
	racers.Wait()
	if winners != 1 {
		t.Fatalf("%d goroutines stored into one absent key, want 1", winners)
	}

	// Point readers and ordered walkers beside a writer. Values equal the key's
	// index, so a torn or misattributed read shows as a mismatch; a walk must
	// stay strictly ascending.
	key3 := func(i int) []byte { return []byte{1, byte(i >> 8), byte(i)} }
	for i := 0; i < 300; i++ {
		m.Set(key3(i), uint64(i))
	}
	done := make(chan struct{})
	var rw sync.WaitGroup
	for g := 0; g < 3; g++ {
		rw.Add(1)
		go func() {
			defer rw.Done()
			r := m.Reader()
			defer r.Free()
			for {
				select {
				case <-done:
					return
				default:
				}
				for i := 0; i < 300; i += 7 {
					if v, ok := r.Get(key3(i)); ok && v != uint64(i) {
						t.Errorf("key %d = %d", i, v)
						return
					}
				}
			}
		}()
	}
	rw.Add(1)
	go func() {
		defer rw.Done()
		for {
			select {
			case <-done:
				return
			default:
			}
			var prev []byte
			for k, v, ok := m.NextAtOrAfter([]byte{1}); ok && len(k) == 3 && k[0] == 1; k, v, ok = m.Next(k) {
				if prev != nil && bytes.Compare(k, prev) <= 0 {
					t.Errorf("ordered walk went backwards: %v after %v", k, prev)
					return
				}
				if want := uint64(k[1])<<8 | uint64(k[2]); v != want {
					t.Errorf("key %v = %d, want %d", k, v, want)
					return
				}
				prev = k
			}
		}
	}()
	for round := 0; round < 20; round++ {
		for i := 300; i < 400; i++ {
			m.Set(key3(i), uint64(i))
		}
		for i := 300; i < 400; i++ {
			m.Delete(key3(i))
		}
	}
	close(done)
	rw.Wait()
}

// ---- SyncBlobMap ----

// pattern is n bytes that depend on seed and position, so a payload read back
// from the wrong key, the wrong length or a torn copy is unlikely to compare equal.
func pattern(seed byte, n int) []byte {
	b := make([]byte, n)
	for i := range b {
		b[i] = seed + byte(i*31)
	}
	return b
}

func TestSyncBlobMapPointOps(t *testing.T) {
	m := NewSyncBlobMap(1 << 16)
	defer m.Free()

	if _, _, ok := m.Get(1); ok {
		t.Fatal("empty map reports key present")
	}
	// Lengths around the 256-byte first buffer: the retry loop must return the
	// whole payload at every length, including a payload far above it.
	lengths := []int{0, 1, 7, 8, 255, 256, 257, 4000, 30000}
	for i, n := range lengths {
		data := pattern(byte(i), n)
		if err := m.Set(uint64(i), data, uint32(i)+1); err != nil {
			t.Fatalf("Set(len %d): %v", n, err)
		}
	}
	if m.Size() != uint64(len(lengths)) {
		t.Fatalf("Size = %d", m.Size())
	}
	for i, n := range lengths {
		got, meta, ok := m.Get(uint64(i))
		if !ok || !bytes.Equal(got, pattern(byte(i), n)) {
			t.Fatalf("Get(len %d): ok %v, %d bytes returned, payload equal %v", n, ok, len(got), bytes.Equal(got, pattern(byte(i), n)))
		}
		// Payloads of 7 bytes or fewer keep no metadata (#1548).
		wantMeta := uint32(i) + 1
		if n <= 7 {
			wantMeta = 0
		}
		if meta != wantMeta {
			t.Fatalf("Get(len %d) meta = %d, want %d", n, meta, wantMeta)
		}
		if gm, ok := m.GetMeta(uint64(i)); !ok || gm != wantMeta {
			t.Fatalf("GetMeta(len %d) = (%d, %v), want %d", n, gm, ok, wantMeta)
		}
		if !m.Contains(uint64(i)) {
			t.Fatalf("Contains(len %d) false", n)
		}
	}
	if got, _, ok := m.Get(0); !ok || len(got) != 0 {
		t.Fatalf("empty payload: ok %v, %d bytes", ok, len(got))
	}
	if _, ok := m.GetMeta(99); ok || m.Contains(99) {
		t.Fatal("absent key reported present")
	}

	// Overwrite with a payload of a different size.
	if err := m.Set(5, pattern(9, 100), 5); err != nil {
		t.Fatal(err)
	}
	if got, meta, _ := m.Get(5); !bytes.Equal(got, pattern(9, 100)) || meta != 5 {
		t.Fatalf("overwrite not visible: %d bytes meta %d", len(got), meta)
	}
	if err := m.Set(5, pattern(5, lengths[5]), 6); err != nil {
		t.Fatal(err)
	}
	if err := m.Set(50, pattern(9, 100), 5); err != nil {
		t.Fatal(err)
	}
	if !m.Delete(50) || m.Delete(50) {
		t.Fatal("Delete must report presence once")
	}

	// Refusals.
	if err := m.Set(100, pattern(1, 64), 1<<24); !errors.Is(err, ErrBlobMetaOverflow) || !errors.Is(err, ErrBlobInsertRefused) {
		t.Fatalf("meta overflow: %v", err)
	}
	if err := m.Set(100, pattern(1, 1<<17), 1); !errors.Is(err, ErrBlobAllocationFailed) {
		t.Fatalf("payload larger than a chunk: %v", err)
	}
	if m.Contains(100) {
		t.Fatal("a refused insert stored the key")
	}

	if m.MemoryUsed() == 0 || m.MemoryUsedEstimate() == 0 || m.MemoryHeld() < m.MemoryUsed() {
		t.Fatalf("memory accounting: used %d est %d held %d", m.MemoryUsed(), m.MemoryUsedEstimate(), m.MemoryHeld())
	}
	m.ShrinkToFit()
	if err := m.Compact(); err != nil {
		t.Fatalf("Compact: %v", err)
	}
	m.SetReclaimAtCap(false)
	m.SetReclaimAtCap(true)
	for i, n := range lengths {
		if got, _, ok := m.Get(uint64(i)); !ok || !bytes.Equal(got, pattern(byte(i), n)) {
			t.Fatalf("Get(len %d) after Compact differs", n)
		}
	}

	r := m.Reader()
	defer r.Free()
	for i, n := range lengths {
		got, _, ok := r.Get(uint64(i))
		if !ok || !bytes.Equal(got, pattern(byte(i), n)) {
			t.Fatalf("reader Get(len %d) differs", n)
		}
	}
	if _, _, ok := r.Get(99); ok {
		t.Fatal("reader reports an absent key")
	}
	if meta, ok := r.GetMeta(6); !ok || meta != 7 {
		t.Fatalf("reader GetMeta = (%d, %v)", meta, ok)
	}
	if !r.Contains(5) || r.Contains(99) {
		t.Fatal("reader Contains wrong")
	}
}

func TestSyncBlobMapCapacityRefusals(t *testing.T) {
	// One 4 KiB chunk fits a 6000-byte cap, as in TestBlobMapCapAndRefusalKinds.
	m := NewSyncBlobMapWithCapacity(4096, 6000)
	defer m.Free()
	for k := uint64(0); k < 4; k++ {
		if err := m.Set(k, bytes.Repeat([]byte{byte(k)}, 1000), 1); err != nil {
			t.Fatalf("set %d: %v", k, err)
		}
	}
	// With the reclaim off, the cap refuses and compacts nothing.
	m.SetReclaimAtCap(false)
	if err := m.Set(9, bytes.Repeat([]byte{9}, 3100), 1); !errors.Is(err, ErrBlobCapRefused) {
		t.Fatalf("cap refusal: %v", err)
	}
	// The exchanges never run the reclaim: the same pressure refuses a
	// compare_exchange whose comparison held, and the map is unchanged.
	m.SetReclaimAtCap(true)
	_, stored, err := m.CompareExchange(9, BlobValue{}, SomeBlob(bytes.Repeat([]byte{9}, 3100), 1))
	if stored || !errors.Is(err, ErrBlobInsertRefused) {
		t.Fatalf("CAS under cap pressure: stored %v err %v", stored, err)
	}
	if m.Contains(9) {
		t.Fatal("a refused exchange stored the key")
	}
	_, found, err := m.GetOrInsert(9, bytes.Repeat([]byte{9}, 3100), 1)
	if found || !errors.Is(err, ErrBlobInsertRefused) {
		t.Fatalf("GetOrInsert under cap pressure: found %v err %v", found, err)
	}
}

func TestSyncBlobMapCompareExchange(t *testing.T) {
	m := NewSyncBlobMap(1 << 16)
	defer m.Free()
	a := pattern(1, 40)
	b := pattern(2, 5000) // above the first buffer
	c := pattern(3, 24)

	// Absent expected inserts.
	obs, stored, err := m.CompareExchange(1, BlobValue{}, SomeBlob(a, 11))
	if err != nil || !stored || obs.Present {
		t.Fatalf("insert via CAS: obs %+v stored %v err %v", obs, stored, err)
	}
	// Expecting absent on a present key mismatches and reports the value.
	obs, stored, err = m.CompareExchange(1, BlobValue{}, SomeBlob(b, 12))
	if err != nil || stored || !obs.Present || !bytes.Equal(obs.Data, a) || obs.Meta != 11 {
		t.Fatalf("expect-absent mismatch: obs %d bytes meta %d present %v stored %v err %v", len(obs.Data), obs.Meta, obs.Present, stored, err)
	}
	// The comparison covers the payload and the metadata.
	obs, stored, _ = m.CompareExchange(1, SomeBlob(a, 10), SomeBlob(b, 12))
	if stored || !bytes.Equal(obs.Data, a) || obs.Meta != 11 {
		t.Fatalf("wrong meta must mismatch: stored %v meta %d", stored, obs.Meta)
	}
	obs, stored, _ = m.CompareExchange(1, SomeBlob(pattern(1, 41), 11), SomeBlob(b, 12))
	if stored || !bytes.Equal(obs.Data, a) {
		t.Fatalf("wrong payload must mismatch: stored %v", stored)
	}
	obs, stored, err = m.CompareExchange(1, SomeBlob(a, 11), SomeBlob(b, 12))
	if err != nil || !stored || !bytes.Equal(obs.Data, a) {
		t.Fatalf("matching exchange: stored %v err %v", stored, err)
	}
	if got, meta, _ := m.Get(1); !bytes.Equal(got, b) || meta != 12 {
		t.Fatalf("value after exchange: %d bytes meta %d", len(got), meta)
	}

	// A mismatch whose current payload is longer than the first buffer: the
	// wrapper grows its buffer and compares again, and returns the whole value.
	obs, stored, err = m.CompareExchange(1, SomeBlob(c, 3), SomeBlob(a, 1))
	if err != nil || stored || !bytes.Equal(obs.Data, b) || obs.Meta != 12 {
		t.Fatalf("large mismatch: stored %v err %v, %d bytes meta %d", stored, err, len(obs.Data), obs.Meta)
	}
	// The observed value is the one to retry against.
	obs, stored, err = m.CompareExchange(1, obs, SomeBlob(c, 13))
	if err != nil || !stored {
		t.Fatalf("retry on the observed value: stored %v err %v", stored, err)
	}

	// Mismatch on an absent key reports absent.
	obs, stored, err = m.CompareExchange(77, SomeBlob(a, 1), SomeBlob(b, 2))
	if err != nil || stored || obs.Present {
		t.Fatalf("mismatch on absent: obs %+v stored %v err %v", obs, stored, err)
	}
	// New absent removes; expected absent and new absent on an absent key is a no-op store.
	obs, stored, err = m.CompareExchange(1, SomeBlob(c, 13), BlobValue{})
	if err != nil || !stored || m.Contains(1) {
		t.Fatalf("CAS remove: stored %v err %v", stored, err)
	}
	// An empty payload is a value, and is not the same as absent.
	if _, stored, err = m.CompareExchange(2, BlobValue{}, SomeBlob(nil, 0)); err != nil || !stored {
		t.Fatalf("insert empty payload: stored %v err %v", stored, err)
	}
	if got, _, ok := m.Get(2); !ok || len(got) != 0 {
		t.Fatalf("empty payload not stored: ok %v len %d", ok, len(got))
	}
	obs, stored, _ = m.CompareExchange(2, BlobValue{}, SomeBlob(a, 1))
	if stored || !obs.Present || len(obs.Data) != 0 {
		t.Fatalf("empty payload observed as %+v", obs)
	}

	// A refusal is reported only when the comparison held.
	_, stored, err = m.CompareExchange(3, BlobValue{}, SomeBlob(pattern(5, 64), 1<<24))
	if stored || !errors.Is(err, ErrBlobInsertRefused) {
		t.Fatalf("overflowing metadata on a holding comparison: stored %v err %v", stored, err)
	}
	_, stored, err = m.CompareExchange(2, SomeBlob(a, 1), SomeBlob(pattern(5, 64), 1<<24))
	if stored || err != nil {
		t.Fatalf("a failing comparison must be a mismatch, not a refusal: stored %v err %v", stored, err)
	}

	// The retry-until-stored loop ends on a refusal.
	attempts := 0
	expected := BlobValue{}
	for {
		attempts++
		o, ok, e := m.CompareExchange(3, expected, SomeBlob(pattern(5, 64), 1<<24))
		if e != nil {
			break
		}
		if ok {
			t.Fatal("must not store")
		}
		expected = o
		if attempts > 3 {
			t.Fatal("retry loop did not stop on a refusal")
		}
	}

	// #1548: a short payload reads back metadata 0, so expecting the metadata it
	// was inserted with reports a mismatch carrying metadata 0.
	if _, stored, _ = m.CompareExchange(4, BlobValue{}, SomeBlob([]byte("tiny"), 9)); !stored {
		t.Fatal("insert tiny")
	}
	obs, stored, _ = m.CompareExchange(4, SomeBlob([]byte("tiny"), 9), SomeBlob([]byte("tiny2"), 9))
	if stored || obs.Meta != 0 || string(obs.Data) != "tiny" {
		t.Fatalf("#1548: stored %v obs %+v", stored, obs)
	}
	if _, stored, _ = m.CompareExchange(4, obs, SomeBlob([]byte("tiny2"), 9)); !stored {
		t.Fatal("exchange against the observed (meta 0) value")
	}
}

func TestSyncBlobMapCompareExchangeMeta(t *testing.T) {
	m := NewSyncBlobMap(1 << 16)
	defer m.Free()
	if err := m.Set(1, pattern(1, 64), 5); err != nil {
		t.Fatal(err)
	}
	// Swap on a match; the payload is untouched.
	obs, stored, err := m.CompareExchangeMeta(1, SomeBlobMeta(5), SomeBlobMeta(6))
	if err != nil || !stored || obs != SomeBlobMeta(5) {
		t.Fatalf("swap: obs %+v stored %v err %v", obs, stored, err)
	}
	if got, meta, _ := m.Get(1); !bytes.Equal(got, pattern(1, 64)) || meta != 6 {
		t.Fatalf("after swap: %d bytes meta %d", len(got), meta)
	}
	// A mismatch reports the current metadata.
	obs, stored, err = m.CompareExchangeMeta(1, SomeBlobMeta(5), SomeBlobMeta(9))
	if err != nil || stored || obs != SomeBlobMeta(6) {
		t.Fatalf("mismatch: obs %+v stored %v err %v", obs, stored, err)
	}
	// Expecting absent on a present key mismatches.
	obs, stored, _ = m.CompareExchangeMeta(1, BlobMeta{}, SomeBlobMeta(9))
	if stored || obs != SomeBlobMeta(6) {
		t.Fatalf("expect absent: obs %+v stored %v", obs, stored)
	}
	// An absent key mismatches with absent observed.
	obs, stored, err = m.CompareExchangeMeta(99, SomeBlobMeta(1), SomeBlobMeta(2))
	if err != nil || stored || obs.Present {
		t.Fatalf("absent key: obs %+v stored %v err %v", obs, stored, err)
	}
	// Refusals: metadata alone cannot create a key; overflow; a payload kept in
	// its slot word has no field to hold a non-zero metadata.
	_, stored, err = m.CompareExchangeMeta(99, BlobMeta{}, SomeBlobMeta(1))
	if stored || !errors.Is(err, ErrBlobNoMetaField) || !errors.Is(err, ErrBlobInsertRefused) {
		t.Fatalf("creating a key from metadata: stored %v err %v", stored, err)
	}
	_, stored, err = m.CompareExchangeMeta(1, SomeBlobMeta(6), SomeBlobMeta(1<<24))
	if stored || !errors.Is(err, ErrBlobMetaOverflow) {
		t.Fatalf("overflow: stored %v err %v", stored, err)
	}
	if err := m.Set(2, []byte("tiny"), 0); err != nil {
		t.Fatal(err)
	}
	_, stored, err = m.CompareExchangeMeta(2, SomeBlobMeta(0), SomeBlobMeta(3))
	if stored || !errors.Is(err, ErrBlobNoMetaField) {
		t.Fatalf("metadata on a slot-word payload: stored %v err %v", stored, err)
	}
	// New absent removes.
	obs, stored, err = m.CompareExchangeMeta(1, SomeBlobMeta(6), BlobMeta{})
	if err != nil || !stored || m.Contains(1) {
		t.Fatalf("remove: stored %v err %v", stored, err)
	}
}

func TestSyncBlobMapGetOrInsert(t *testing.T) {
	m := NewSyncBlobMap(1 << 16)
	defer m.Free()
	a := pattern(1, 50)
	big := pattern(2, 6000)

	ex, found, err := m.GetOrInsert(1, a, 7)
	if err != nil || found || ex.Present {
		t.Fatalf("insert: ex %+v found %v err %v", ex, found, err)
	}
	// A present key is left untouched and its value returned, whole, even when
	// it is far above the first buffer.
	ex, found, err = m.GetOrInsert(1, big, 8)
	if err != nil || !found || !ex.Present || !bytes.Equal(ex.Data, a) || ex.Meta != 7 {
		t.Fatalf("present: ex %d bytes meta %d found %v err %v", len(ex.Data), ex.Meta, found, err)
	}
	if got, meta, _ := m.Get(1); !bytes.Equal(got, a) || meta != 7 {
		t.Fatal("GetOrInsert overwrote a present key")
	}
	if _, found, err = m.GetOrInsert(2, big, 9); err != nil || found {
		t.Fatalf("insert big: found %v err %v", found, err)
	}
	ex, found, err = m.GetOrInsert(2, a, 1)
	if err != nil || !found || !bytes.Equal(ex.Data, big) || ex.Meta != 9 {
		t.Fatalf("present big: %d bytes meta %d found %v err %v", len(ex.Data), ex.Meta, found, err)
	}
	// Empty payload.
	if _, found, err = m.GetOrInsert(3, nil, 0); err != nil || found {
		t.Fatalf("insert empty: found %v err %v", found, err)
	}
	ex, found, _ = m.GetOrInsert(3, a, 0)
	if !found || !ex.Present || len(ex.Data) != 0 {
		t.Fatalf("present empty: %+v found %v", ex, found)
	}
	// A refusal.
	if _, found, err = m.GetOrInsert(4, pattern(1, 64), 1<<24); found || !errors.Is(err, ErrBlobInsertRefused) {
		t.Fatalf("refusal: found %v err %v", found, err)
	}
	if m.Contains(4) {
		t.Fatal("a refused GetOrInsert stored the key")
	}
}

func TestSyncBlobMapConcurrent(t *testing.T) {
	m := NewSyncBlobMap(1 << 16)
	defer m.Free()
	const goroutines, perGoroutine = 8, 1000

	// A counter in the metadata, bumped with CompareExchangeMeta loops, loses
	// no update.
	if err := m.Set(0, pattern(0, 32), 0); err != nil {
		t.Fatal(err)
	}
	var wg sync.WaitGroup
	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			meta, _ := m.GetMeta(0)
			cur := SomeBlobMeta(meta)
			for i := 0; i < perGoroutine; i++ {
				for {
					obs, stored, err := m.CompareExchangeMeta(0, cur, SomeBlobMeta(cur.Meta+1))
					if err != nil {
						t.Error(err)
						return
					}
					if stored {
						cur = SomeBlobMeta(cur.Meta + 1)
						break
					}
					cur = obs
				}
			}
		}()
	}
	wg.Wait()
	if meta, _ := m.GetMeta(0); meta != goroutines*perGoroutine {
		t.Fatalf("meta counter = %d, want %d", meta, goroutines*perGoroutine)
	}

	// The same through payload exchanges: the payload is the counter.
	enc := func(v uint64) []byte {
		b := make([]byte, 16)
		for i := 0; i < 8; i++ {
			b[i] = byte(v >> (8 * i))
		}
		return b
	}
	dec := func(b []byte) uint64 {
		var v uint64
		for i := 0; i < 8; i++ {
			v |= uint64(b[i]) << (8 * i)
		}
		return v
	}
	if err := m.Set(1, enc(0), 1); err != nil {
		t.Fatal(err)
	}
	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			cur, meta, _ := m.Get(1)
			expected := SomeBlob(cur, meta)
			for i := 0; i < perGoroutine/4; i++ {
				for {
					obs, stored, err := m.CompareExchange(1, expected, SomeBlob(enc(dec(expected.Data)+1), 1))
					if err != nil {
						t.Error(err)
						return
					}
					if stored {
						expected = SomeBlob(enc(dec(expected.Data)+1), 1)
						break
					}
					expected = obs
				}
			}
		}()
	}
	wg.Wait()
	if got, _, _ := m.Get(1); dec(got) != goroutines*(perGoroutine/4) {
		t.Fatalf("payload counter = %d, want %d", dec(got), goroutines*(perGoroutine/4))
	}

	// Of any number of goroutines racing GetOrInsert on one absent key, exactly
	// one stores, and every loser sees the winner's value whole.
	var winners int64
	var racers sync.WaitGroup
	start := make(chan struct{})
	for g := 0; g < goroutines; g++ {
		racers.Add(1)
		go func(g int) {
			defer racers.Done()
			<-start
			data := pattern(byte(g), 600+g)
			ex, found, err := m.GetOrInsert(2, data, uint32(g))
			if err != nil {
				t.Error(err)
				return
			}
			if !found {
				atomic.AddInt64(&winners, 1)
				return
			}
			// The winner's payload is pattern(w, 600+w) for the w its metadata names.
			w := int(ex.Meta)
			if !bytes.Equal(ex.Data, pattern(byte(w), 600+w)) {
				t.Errorf("loser saw a payload that is not the winner's (meta %d, %d bytes)", w, len(ex.Data))
			}
		}(g)
	}
	close(start)
	racers.Wait()
	if winners != 1 {
		t.Fatalf("%d goroutines stored into one absent key, want 1", winners)
	}

	// Readers and a map-handle reader beside writers that replace, remove and
	// compact: every payload read must be one a writer stored whole (its length
	// and every byte match its key and version), never a torn mix of two.
	version := func(key uint64, v int) []byte {
		return bytes.Repeat([]byte{byte(key + uint64(v))}, 300+int(key)*40+v*50)
	}
	check := func(key uint64, got []byte) bool {
		if len(got) == 0 {
			return false
		}
		for v := 0; v < 6; v++ {
			if bytes.Equal(got, version(key, v)) {
				return true
			}
		}
		return false
	}
	const keys = 12
	for k := uint64(10); k < 10+keys; k++ {
		if err := m.Set(k, version(k, 0), 1); err != nil {
			t.Fatal(err)
		}
	}
	done := make(chan struct{})
	var rw sync.WaitGroup
	for g := 0; g < 4; g++ {
		rw.Add(1)
		go func(g int) {
			defer rw.Done()
			r := m.Reader()
			defer r.Free()
			for i := 0; ; i++ {
				select {
				case <-done:
					return
				default:
				}
				k := uint64(10 + (i+g)%keys)
				var got []byte
				var ok bool
				if g == 0 {
					got, _, ok = m.Get(k) // the one-shot map form
				} else {
					got, _, ok = r.Get(k)
				}
				if ok && !check(k, got) {
					t.Errorf("key %d: a payload no writer stored (%d bytes)", k, len(got))
					return
				}
			}
		}(g)
	}
	for round := 0; round < 30; round++ {
		for k := uint64(10); k < 10+keys; k++ {
			m.Set(k, version(k, round%6), 1)
		}
		for k := uint64(10); k < 10+keys; k += 3 {
			m.Delete(k)
		}
		if round%10 == 0 {
			m.Compact()
		}
		for k := uint64(10); k < 10+keys; k += 3 {
			m.GetOrInsert(k, version(k, 1), 1)
		}
	}
	close(done)
	rw.Wait()
}

// ---- lifetime ----

// A reader borrows from its map; the wrapper holds the map reachable, so a
// reader whose map went out of scope still reads correctly after collection.
func TestSharedMapReadersOutliveUnreferencedMaps(t *testing.T) {
	str := func() *SyncStrMapReader {
		m := NewSyncStrMap()
		for i := 0; i < 64; i++ {
			m.Set(fmt.Sprintf("k%02d", i), uint64(i))
		}
		return m.Reader()
	}()
	byt := func() *SyncBytesMapReader {
		m := NewSyncBytesMap()
		for i := 0; i < 64; i++ {
			m.Set([]byte{byte(i), 0}, uint64(i))
		}
		return m.Reader()
	}()
	ord := func() *SyncOrderedBytesMapReader {
		m := NewSyncOrderedBytesMap()
		for i := 0; i < 64; i++ {
			m.Set([]byte{byte(i), 0}, uint64(i))
		}
		return m.Reader()
	}()
	blob := func() *SyncBlobMapReader {
		m := NewSyncBlobMap(4096)
		for i := 0; i < 64; i++ {
			m.Set(uint64(i), pattern(byte(i), 100), 1)
		}
		return m.Reader()
	}()

	runtime.GC()
	runtime.GC()
	churn()
	runtime.GC()

	for i := 0; i < 64; i++ {
		if v, ok := str.Get(fmt.Sprintf("k%02d", i)); !ok || v != uint64(i) {
			t.Fatalf("str reader key %d: (%d, %v)", i, v, ok)
		}
		if v, ok := byt.Get([]byte{byte(i), 0}); !ok || v != uint64(i) {
			t.Fatalf("bytes reader key %d: (%d, %v)", i, v, ok)
		}
		if v, ok := ord.Get([]byte{byte(i), 0}); !ok || v != uint64(i) {
			t.Fatalf("ordered reader key %d: (%d, %v)", i, v, ok)
		}
		if got, _, ok := blob.Get(uint64(i)); !ok || !bytes.Equal(got, pattern(byte(i), 100)) {
			t.Fatalf("blob reader key %d differs", i)
		}
	}
	runtime.KeepAlive(str)
	runtime.KeepAlive(byt)
	runtime.KeepAlive(ord)
	runtime.KeepAlive(blob)
}

// Freeing the reader first and then the map is the documented order and must
// stay sound; Free is idempotent.
func TestSharedMapExplicitFreeOrder(t *testing.T) {
	sm := NewSyncStrMap()
	sm.Set("a", 1)
	sr := sm.Reader()
	if v, ok := sr.Get("a"); !ok || v != 1 {
		t.Fatalf("str reader: (%d, %v)", v, ok)
	}
	sr.Free()
	sr.Free()
	sm.Free()
	sm.Free()

	bm := NewSyncBytesMap()
	bm.Set([]byte("a"), 1)
	br := bm.Reader()
	br.Free()
	br.Free()
	bm.Free()
	bm.Free()

	om := NewSyncOrderedBytesMap()
	om.Set([]byte("a"), 1)
	or := om.Reader()
	or.Free()
	or.Free()
	om.Free()
	om.Free()

	xm := NewSyncBlobMap(4096)
	xm.Set(1, pattern(1, 40), 1)
	xr := xm.Reader()
	if got, _, ok := xr.Get(1); !ok || !bytes.Equal(got, pattern(1, 40)) {
		t.Fatal("blob reader read differs")
	}
	xr.Free()
	xr.Free()
	xm.Free()
	xm.Free()
}
