package expanse

import "unsafe"

// Shared-map plumbing common to the cgo and purego builds: the status
// vocabulary of the shared string, bytes, ordered-bytes and blob maps, the
// buffer retry loops, and the update loop. The two builds define only the
// handle types and the one-call native shims (the n* methods); everything that
// decides a result lives here, so the two builds cannot disagree.

// expanse_str_nav_status, expanse_ordered_bytes_nav_status and
// expanse_blob_status_t values (include/expanse.h) the wrappers branch on.
const (
	syncNavOK       int32 = 0
	syncNavNotFound int32 = 1
	syncNavTooSmall int32 = 2

	syncBlobOK       int32 = 0
	syncBlobNotFound int32 = 1
	syncBlobTooSmall int32 = 2
	syncBlobMismatch int32 = 3
)

// syncNavBufLen and syncBlobBufLen are the sizes of the first buffer an ordered
// read or a blob read uses. A larger result is fetched by growing the buffer to
// the length the native call reports and reading again.
const (
	syncNavBufLen  = 256
	syncBlobBufLen = 256
)

// navOp names one ordered read of a shared map.
type navOp int

const (
	navFirst navOp = iota
	navLast
	navNextAtOrAfter
	navNextAfter
	navPrevAtOrBefore
	navPrevBefore
)

// syncStrKey returns key as a NUL-terminated byte string for a C call. The
// caller has rejected a key containing NUL (see nulIndex).
func syncStrKey(key string) []byte {
	b := make([]byte, len(key)+1)
	copy(b, key)
	return b
}

// cptr is the address of b for a C call, nil for an empty slice (NULL with
// length zero is accepted by the C ABI).
func cptr(b []byte) unsafe.Pointer {
	if len(b) == 0 {
		return nil
	}
	return unsafe.Pointer(&b[0])
}

// grownBuf returns a buffer for the retry after a BUFFER_TOO_SMALL that
// reported need bytes. It is never empty and always larger than the buffer it
// replaces, so a native call that misreports cannot spin the loop in place.
func grownBuf(old int, need uintptr) []byte {
	n := int(need)
	if n <= old {
		n = old * 2
	}
	if n < 1 {
		n = 1
	}
	return make([]byte, n)
}

// syncNav runs one ordered read, growing the buffer to the length the native
// call reports and reading again until the call is not BUFFER_TOO_SMALL. A
// retry is a new read, so under concurrent writers it can return a different
// key than the read that was too small; the loop ends because each pass
// either finds a key that fits or finds none. It returns a copy of the key.
// nulTerminated is true for the string map, whose required length counts the
// terminating NUL.
func syncNav(call func(buf []byte) (status int32, required uintptr, value uint64), nulTerminated bool) ([]byte, uint64, bool) {
	buf := make([]byte, syncNavBufLen)
	for {
		status, req, val := call(buf)
		switch status {
		case syncNavOK:
			n := int(req)
			if nulTerminated {
				n--
			}
			key := make([]byte, n)
			copy(key, buf)
			return key, val, true
		case syncNavTooSmall:
			buf = grownBuf(len(buf), req)
		default:
			return nil, 0, false
		}
	}
}

// BlobValue is a blob map value that may be absent: its payload and its 24-bit
// hot metadata, with Present telling a present value from an absent one. The
// zero BlobValue is absent; SomeBlob builds a present one. Data and Meta of an
// absent value are ignored.
//
// An absent value is the "expected" side's "only if the key is absent" and the
// "new" side's "remove the key", exactly as Word is for the word-valued maps.
//
// A payload of 7 bytes or fewer, or one the map compresses into its slot word,
// is stored without metadata and reads back Meta 0 whatever Meta it was stored
// with, so expecting the metadata such a value was inserted with reports a
// mismatch with Meta 0 (#1548).
type BlobValue struct {
	Data    []byte
	Meta    uint32
	Present bool
}

// SomeBlob returns the present BlobValue holding data and meta.
func SomeBlob(data []byte, meta uint32) BlobValue {
	return BlobValue{Data: data, Meta: meta, Present: true}
}

// BlobMeta is a hot-metadata word that may be absent, the operand of
// (*SyncBlobMap).CompareExchangeMeta. The zero BlobMeta is absent.
type BlobMeta struct {
	Meta    uint32
	Present bool
}

// SomeBlobMeta returns the present BlobMeta holding meta.
func SomeBlobMeta(meta uint32) BlobMeta { return BlobMeta{Meta: meta, Present: true} }

// blobData is the payload a C call receives for v: nil for an absent value or
// an empty payload (the C ABI accepts NULL with length zero), else its bytes.
func blobData(v BlobValue) []byte {
	if !v.Present || len(v.Data) == 0 {
		return nil
	}
	return v.Data
}

// syncBlobGet runs one blob read (a map or reader get_into), growing the buffer
// until the payload fits. found is false when the key is absent. A retry is a
// new read: if a writer replaced the value in between, the loop reports the new
// state, so it ends on OK or NOT_FOUND.
func syncBlobGet(call func(buf []byte) (status int32, n uintptr, meta uint32)) (data []byte, meta uint32, found bool, err error) {
	buf := make([]byte, syncBlobBufLen)
	for {
		status, n, m := call(buf)
		switch status {
		case syncBlobOK:
			return copyPayload(buf, n), m, true, nil
		case syncBlobNotFound:
			return nil, 0, false, nil
		case syncBlobTooSmall:
			buf = grownBuf(len(buf), n)
		default:
			return nil, 0, false, blobStatusErr(status)
		}
	}
}

// syncBlobRMW runs one blob read-modify-write (compare_exchange or
// get_or_insert) and maps its status. OK is a store: observed is then ok, the
// value the caller already knows (nothing is written back). MISMATCH is no
// store, with the current value copied out; BUFFER_TOO_SMALL grows the buffer
// and compares again, which is a new step. Any other status is a refusal or a
// usage error and comes back as the error, with the map unchanged.
func syncBlobRMW(ok BlobValue, call func(buf []byte) (status int32, present bool, n uintptr, meta uint32)) (observed BlobValue, stored bool, err error) {
	buf := make([]byte, syncBlobBufLen)
	for {
		status, present, n, m := call(buf)
		switch status {
		case syncBlobOK:
			return ok, true, nil
		case syncBlobMismatch:
			if !present {
				return BlobValue{}, false, nil
			}
			return BlobValue{Data: copyPayload(buf, n), Meta: m, Present: true}, false, nil
		case syncBlobTooSmall:
			buf = grownBuf(len(buf), n)
		default:
			return BlobValue{}, false, blobStatusErr(status)
		}
	}
}

// copyPayload copies the first n bytes of buf. An empty payload is nil.
func copyPayload(buf []byte, n uintptr) []byte {
	if n == 0 {
		return nil
	}
	out := make([]byte, n)
	copy(out, buf)
	return out
}

// updateWord is the retry loop behind Update on the word-valued shared maps: it
// applies fn to the observed word and compare-exchanges, retrying on the word a
// concurrent writer left. It returns the value it replaced.
func updateWord(current Word, cas func(expected, newValue Word) (Word, bool), fn func(current uint64, ok bool) (uint64, bool)) (previous uint64, ok bool) {
	expected := current
	for {
		nv, store := fn(expected.Value, expected.Present)
		observed, stored := cas(expected, Word{Value: nv, Present: store})
		if stored {
			return observed.Value, observed.Present
		}
		expected = observed
	}
}
