package expanse

// Word is a 64-bit value that may be absent. Absence is a flag, never a
// value, so every uint64 stays a legal value: the zero Word is absent, and
// Some(0) is present.
type Word struct {
	Value   uint64
	Present bool
}

// Some returns the present Word holding v.
func Some(v uint64) Word { return Word{Value: v, Present: true} }

// Update replaces the value for key with fn(current), where current and ok
// are the key's value and whether it is present. fn returns the new value and
// whether to store it: (v, true) stores v and (_, false) removes the key.
// Update returns the value it replaced as (previous, true), or (0, false) when
// the key was absent.
//
// It is a loop over CompareExchange, as SyncExpanseMap::update is in Rust: fn
// runs again with the observed word whenever a concurrent writer wins, so it
// may run several times for one call and must be free of side effects. A panic
// in fn propagates with the key unchanged. The C ABI exports no callback form.
func (m *SyncMap) Update(key uint64, fn func(current uint64, ok bool) (uint64, bool)) (previous uint64, ok bool) {
	v, present := m.Get(key)
	expected := Word{Value: v, Present: present}
	for {
		nv, store := fn(expected.Value, expected.Present)
		observed, stored := m.CompareExchange(key, expected, Word{Value: nv, Present: store})
		if stored {
			return observed.Value, observed.Present
		}
		expected = observed
	}
}
