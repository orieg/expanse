package expanse

import (
	"runtime"
	"testing"
	"time"
)

// collectFinalizers drives the collector until the sentinel finalizer set by
// the caller has had a chance to run, or the budget is exhausted. An object
// carrying a finalizer needs one cycle to be found unreachable and a second to
// be freed, and the finalizer itself runs on a separate goroutine, so a fixed
// pair of runtime.GC() calls is necessary but not sufficient to observe it.
func collectFinalizers(ran <-chan struct{}) bool {
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		runtime.GC()
		runtime.Gosched()
		select {
		case <-ran:
			return true
		case <-time.After(5 * time.Millisecond):
		}
	}
	select {
	case <-ran:
		return true
	default:
		return false
	}
}

// churn allocates on both sides of the FFI boundary so that a heap block freed
// by a premature finalizer is likely to be reused before it is read through a
// stale handle. It makes a use-after-free observable rather than latent; it is
// not what establishes the invariant.
func churn() {
	victims := make([]*SyncMap, 0, 64)
	for i := 0; i < 64; i++ {
		v := NewSyncMap()
		for k := uint64(0); k < 64; k++ {
			v.Set(k, k*0x9E3779B97F4A7C15)
		}
		victims = append(victims, v)
	}
	for _, v := range victims {
		v.Free()
	}
}

// A reader borrows from its map: expanse_sync_map_reader_new erases the
// borrow's lifetime, and the C contract requires the map to outlive every
// reader taken from it. Go has no lifetimes, so the binding must express that
// requirement as reachability -- the one ordering constraint the collector
// honours. These tests pin that the reference exists and does its job; without
// it the map is collectible the instant Reader() returns, and finalizer order
// between two unreachable objects is unspecified.

// newWatchedSyncMapReader builds the map and takes the reader inside its own
// frame, so the only reference to the map that can survive the return is one
// the binding itself holds. Assigning nil to a local in the test body is not
// enough: the compiler may leave the pointer in a dead stack slot, which the
// collector still scans, and the object then looks reachable by accident.
func newWatchedSyncMapReader(collected chan<- struct{}) *SyncMapReader {
	m := NewSyncMap()
	m.Set(7, 0xDEADBEEF)
	r := m.Reader()
	// Replaces the constructor's finalizer with one that reports collection
	// and then performs the same free, so the object under test is otherwise
	// unchanged. SetFinalizer throws if one is already set, hence the clear.
	runtime.SetFinalizer(m, nil)
	runtime.SetFinalizer(m, func(x *SyncMap) {
		close(collected)
		x.Free()
	})
	return r
}

func newWatchedSyncSetReader(collected chan<- struct{}) *SyncSetReader {
	s := NewSyncSet()
	s.Add(7)
	r := s.Reader()
	runtime.SetFinalizer(s, nil)
	runtime.SetFinalizer(s, func(x *SyncSet) {
		close(collected)
		x.Free()
	})
	return r
}

func TestSyncMapReaderKeepsParentMapReachable(t *testing.T) {
	collected := make(chan struct{})
	r := newWatchedSyncMapReader(collected)

	if collectFinalizers(collected) {
		t.Fatal("parent SyncMap was collected while a reader derived from it was still reachable")
	}

	if v, ok := r.Get(7); !ok || v != 0xDEADBEEF {
		t.Fatalf("reader lookup after GC: got (%d, %v), want (0xDEADBEEF, true)", v, ok)
	}
	runtime.KeepAlive(r)
}

func TestSyncSetReaderKeepsParentSetReachable(t *testing.T) {
	collected := make(chan struct{})
	r := newWatchedSyncSetReader(collected)

	if collectFinalizers(collected) {
		t.Fatal("parent SyncSet was collected while a reader derived from it was still reachable")
	}

	if !r.Contains(7) {
		t.Fatal("reader lookup after GC: key 7 reported absent")
	}
	runtime.KeepAlive(r)
}

// The map is unreachable the moment Reader() returns on this expression, which
// is the shape a caller writes without thinking about it.
func TestSyncMapReaderOutlivesUnreferencedMap(t *testing.T) {
	r := func() *SyncMapReader {
		m := NewSyncMap()
		for k := uint64(0); k < 256; k++ {
			m.Set(k, k+1000)
		}
		return m.Reader()
	}()

	runtime.GC()
	runtime.GC()
	churn()
	runtime.GC()

	for k := uint64(0); k < 256; k++ {
		v, ok := r.Get(k)
		if !ok || v != k+1000 {
			t.Fatalf("reader lookup for key %d after parent went out of scope: got (%d, %v), want (%d, true)", k, v, ok, k+1000)
		}
	}
	runtime.KeepAlive(r)
}

func TestSyncSetReaderOutlivesUnreferencedSet(t *testing.T) {
	r := func() *SyncSetReader {
		s := NewSyncSet()
		for k := uint64(0); k < 256; k++ {
			s.Add(k * 3)
		}
		return s.Reader()
	}()

	runtime.GC()
	runtime.GC()
	churn()
	runtime.GC()

	for k := uint64(0); k < 256; k++ {
		if !r.Contains(k * 3) {
			t.Fatalf("reader lookup for key %d after parent went out of scope: reported absent", k*3)
		}
	}
	runtime.KeepAlive(r)
}

// Freeing the map explicitly while a reader is still live is caller error the
// binding cannot prevent; freeing the reader first and then the map is the
// documented order and must stay sound.
func TestSyncReaderExplicitFreeOrder(t *testing.T) {
	m := NewSyncMap()
	m.Set(1, 2)
	r := m.Reader()
	if v, ok := r.Get(1); !ok || v != 2 {
		t.Fatalf("reader lookup: got (%d, %v), want (2, true)", v, ok)
	}
	r.Free()
	r.Free() // idempotent
	m.Free()
	m.Free() // idempotent

	s := NewSyncSet()
	s.Add(1)
	sr := s.Reader()
	if !sr.Contains(1) {
		t.Fatal("set reader lookup: key 1 reported absent")
	}
	sr.Free()
	sr.Free()
	s.Free()
	s.Free()
}

// Handles must survive collection across the call that uses them. The failure
// this guards -- the collector running a wrapper's finalizer while a call
// through that wrapper's raw handle is in flight -- has a window too narrow to
// provoke on demand, so this exercises the paths under continuous collection
// rather than claiming to falsify their absence.
func TestHandlesSurviveConcurrentCollection(t *testing.T) {
	stop := make(chan struct{})
	done := make(chan struct{})
	go func() {
		defer close(done)
		for {
			select {
			case <-stop:
				return
			default:
				runtime.GC()
			}
		}
	}()
	defer func() {
		close(stop)
		<-done
	}()

	for i := 0; i < 200; i++ {
		m := NewMap()
		s := NewSet()
		bm := NewBytesMap()
		sm := NewStrMap()
		blob := NewBlobMap(4096)
		for k := uint64(0); k < 32; k++ {
			m.Set(k, k)
			s.Add(k)
			bm.Set([]byte{byte(k), 0x5A}, k)
			sm.Set(string(rune('a'+k%26))+"key", k)
			blob.Set(k, []byte{byte(k), byte(k), byte(k)}, uint32(k))
		}
		for k := uint64(0); k < 32; k++ {
			if v, ok := m.Get(k); !ok || v != k {
				t.Fatalf("map lookup %d: got (%d, %v)", k, v, ok)
			}
			if !s.Contains(k) {
				t.Fatalf("set lookup %d: absent", k)
			}
			if v, ok := bm.Get([]byte{byte(k), 0x5A}); !ok || v != k {
				t.Fatalf("bytesmap lookup %d: got (%d, %v)", k, v, ok)
			}
			if data, _, ok := blob.Get(k); !ok || len(data) != 3 || data[0] != byte(k) {
				t.Fatalf("blobmap lookup %d: got (%v, %v)", data, ok, ok)
			}
		}
	}
}
