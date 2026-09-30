package expanse

import (
	"errors"
	"strings"
)

// ErrNulInKey is returned by (*StrMap).Set when the key contains a NUL byte.
//
// ExpanseStrMap's key domain is NUL-free: the C ABI takes a key as a
// NUL-terminated `const char*`, so a Go string such as "a\x00b" would reach
// the engine as "a" and overwrite that entry. Set refuses it instead, and the
// map is left unchanged.
var ErrNulInKey = errors.New("expanse: StrMap key contains a NUL byte (StrMap keys are NUL-free)")

// nulIndex returns the index of the first NUL byte in key, or -1.
//
// The read-only calls use it to answer for the key as given rather than for
// its truncation. A NUL-bearing key can never be stored, so Get, Contains and
// Delete report it absent. For navigation, write key = p + "\x00" + rest with
// p NUL-free. Every stored (NUL-free) key k satisfies k > key exactly when
// k > p, and k < key exactly when k <= p, so:
//
//	Next(key), NextAtOrAfter(key)  == Next(p)
//	Prev(key), PrevAtOrBefore(key) == PrevAtOrBefore(p)
func nulIndex(key string) int {
	return strings.IndexByte(key, 0)
}
