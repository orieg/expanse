package expanse

import (
	"bytes"
	"testing"
)

func TestSet(t *testing.T) {
	s := NewSet()
	if s.Size() != 0 {
		t.Fatalf("expected size 0, got %d", s.Size())
	}
	s.Add(10)
	s.Add(20)
	if !s.Contains(10) || !s.Contains(20) || s.Contains(30) {
		t.Fatalf("contains failed")
	}
	if s.Size() != 2 {
		t.Fatalf("expected size 2, got %d", s.Size())
	}

	if k, ok := s.First(); !ok || k != 10 {
		t.Fatalf("first failed")
	}
	if k, ok := s.Last(); !ok || k != 20 {
		t.Fatalf("last failed")
	}

	if k, ok := s.Next(10); !ok || k != 20 {
		t.Fatalf("next failed")
	}
	if k, ok := s.Prev(20); !ok || k != 10 {
		t.Fatalf("prev failed")
	}

	if r := s.Rank(15); r != 1 {
		t.Fatalf("rank failed, got %d", r)
	}

	if k, ok := s.Select(1); !ok || k != 20 {
		t.Fatalf("select failed")
	}

	if r := s.CountRange(5, 15); r != 1 {
		t.Fatalf("count range failed")
	}

	s.Remove(10)
	if s.Size() != 1 {
		t.Fatalf("remove failed")
	}
	s.Clear()
	if s.Size() != 0 {
		t.Fatalf("clear failed")
	}
}

func TestMap(t *testing.T) {
	m := NewMap()
	m.Set(10, 100)
	m.Set(20, 200)

	if v, ok := m.Get(10); !ok || v != 100 {
		t.Fatalf("get failed")
	}
	if _, ok := m.Get(30); ok {
		t.Fatalf("get non-existent should fail")
	}

	if !m.Contains(20) {
		t.Fatalf("contains failed")
	}
	if m.Size() != 2 {
		t.Fatalf("size failed")
	}

	if k, v, ok := m.First(); !ok || k != 10 || v != 100 {
		t.Fatalf("first failed")
	}
	if k, v, ok := m.Last(); !ok || k != 20 || v != 200 {
		t.Fatalf("last failed")
	}
	if k, v, ok := m.Next(10); !ok || k != 20 || v != 200 {
		t.Fatalf("next failed")
	}
	if k, v, ok := m.Prev(20); !ok || k != 10 || v != 100 {
		t.Fatalf("prev failed")
	}

	m.Delete(10)
	if m.Size() != 1 {
		t.Fatalf("delete failed")
	}
	m.Clear()
	if m.Size() != 0 {
		t.Fatalf("clear failed")
	}
}

func TestStrMap(t *testing.T) {
	m := NewStrMap()
	m.Set("hello", 100)
	if v, ok := m.Get("hello"); !ok || v != 100 {
		t.Fatalf("get failed")
	}
	if !m.Contains("hello") {
		t.Fatalf("contains failed")
	}
	m.Delete("hello")
	if m.Size() != 0 {
		t.Fatalf("delete failed")
	}
	m.Clear()
}

func TestBytesMap(t *testing.T) {
	m := NewBytesMap()
	m.Set([]byte{0, 1, 2}, 100)
	if v, ok := m.Get([]byte{0, 1, 2}); !ok || v != 100 {
		t.Fatalf("get failed")
	}
	if !m.Contains([]byte{0, 1, 2}) {
		t.Fatalf("contains failed")
	}
	m.Delete([]byte{0, 1, 2})
	if m.Size() != 0 {
		t.Fatalf("delete failed")
	}
	m.Clear()
}

func TestBlobMap(t *testing.T) {
	b := NewBlobMap(4096)
	b.Set(10, []byte("hello"), 1)
	b.Set(20, []byte("world"), 2)

	data, meta, ok := b.Get(10)
	if !ok || !bytes.Equal(data, []byte("hello")) || meta != 1 {
		t.Fatalf("get failed")
	}

	if b.Size() != 2 {
		t.Fatalf("size failed")
	}
	if !b.Contains(20) {
		t.Fatalf("contains failed")
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

func BenchmarkGoMap(b *testing.B) {
	m := make(map[uint64]uint64)
	for i := 0; i < b.N; i++ {
		m[uint64(i)] = uint64(i)
	}
	for i := 0; i < b.N; i++ {
		_ = m[uint64(i)]
	}
}

func BenchmarkExpanseMap(b *testing.B) {
	m := NewMap()
	for i := 0; i < b.N; i++ {
		m.Set(uint64(i), uint64(i))
	}
	for i := 0; i < b.N; i++ {
		m.Get(uint64(i))
	}
}
