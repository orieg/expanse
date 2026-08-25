package expanse

import (
	"math/rand"
	"testing"
)

func generateBenchmarkKeys(n int) []uint64 {
	r := rand.New(rand.NewSource(0x0DDB_1A5E_5EED_0001))
	keys := make([]uint64, n)
	for i := 0; i < n; i++ {
		keys[i] = r.Uint64()
	}
	return keys
}

func BenchmarkExpanseMap_Insert_Random(b *testing.B) {
	keys := generateBenchmarkKeys(b.N)
	m := NewMap()
	defer m.Free()

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		m.Set(keys[i], keys[i]^0x55)
	}
}

func BenchmarkGoMap_Insert_Random(b *testing.B) {
	keys := generateBenchmarkKeys(b.N)
	m := make(map[uint64]uint64, b.N)

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		m[keys[i]] = keys[i] ^ 0x55
	}
}

func BenchmarkExpanseMap_Lookup_Random(b *testing.B) {
	const n = 100_000
	keys := generateBenchmarkKeys(n)
	m := NewMap()
	defer m.Free()
	for i := 0; i < n; i++ {
		m.Set(keys[i], keys[i]^0x55)
	}

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		k := keys[i%n]
		v, ok := m.Get(k)
		if !ok || v != k^0x55 {
			b.Fatalf("lookup failed for key %d", k)
		}
	}
}

func BenchmarkGoMap_Lookup_Random(b *testing.B) {
	const n = 100_000
	keys := generateBenchmarkKeys(n)
	m := make(map[uint64]uint64, n)
	for i := 0; i < n; i++ {
		m[keys[i]] = keys[i] ^ 0x55
	}

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		k := keys[i%n]
		v, ok := m[k]
		if !ok || v != k^0x55 {
			b.Fatalf("lookup failed for key %d", k)
		}
	}
}

func BenchmarkExpanseSet_Insert_Random(b *testing.B) {
	keys := generateBenchmarkKeys(b.N)
	s := NewSet()
	defer s.Free()

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		s.Add(keys[i])
	}
}

func BenchmarkGoSet_Insert_Random(b *testing.B) {
	keys := generateBenchmarkKeys(b.N)
	s := make(map[uint64]struct{}, b.N)

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		s[keys[i]] = struct{}{}
	}
}

func BenchmarkExpanseSet_Contains_Random(b *testing.B) {
	const n = 100_000
	keys := generateBenchmarkKeys(n)
	s := NewSet()
	defer s.Free()
	for i := 0; i < n; i++ {
		s.Add(keys[i])
	}

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		k := keys[i%n]
		if !s.Contains(k) {
			b.Fatalf("contains failed for key %d", k)
		}
	}
}

func BenchmarkGoSet_Contains_Random(b *testing.B) {
	const n = 100_000
	keys := generateBenchmarkKeys(n)
	s := make(map[uint64]struct{}, n)
	for i := 0; i < n; i++ {
		s[keys[i]] = struct{}{}
	}

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		k := keys[i%n]
		if _, ok := s[k]; !ok {
			b.Fatalf("contains failed for key %d", k)
		}
	}
}

func BenchmarkExpanseSet_CountRange(b *testing.B) {
	const n = 100_000
	keys := generateBenchmarkKeys(n)
	s := NewSet()
	defer s.Free()
	for i := 0; i < n; i++ {
		s.Add(keys[i])
	}

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		start := keys[i%n]
		end := start + 1000
		_ = s.CountRange(start, end)
	}
}
