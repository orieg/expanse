package expanse

import (
	"bytes"
	"testing"
)

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

	first, ok := s.First()
	if !ok || first != 10 {
		t.Fatalf("first should be 10, got %d", first)
	}

	next, ok := s.Next(10)
	if !ok || next != 20 {
		t.Fatalf("next after 10 should be 20, got %d", next)
	}

	last, ok := s.Last()
	if !ok || last != 30 {
		t.Fatalf("last should be 30, got %d", last)
	}

	prev, ok := s.Prev(30)
	if !ok || prev != 20 {
		t.Fatalf("prev before 30 should be 20, got %d", prev)
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

	firstK, firstV, ok := m.First()
	if !ok || firstK != 1 || firstV != 100 {
		t.Fatalf("first failed")
	}

	nextK, nextV, ok := m.Next(1)
	if !ok || nextK != 2 || nextV != 200 {
		t.Fatalf("next failed")
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
