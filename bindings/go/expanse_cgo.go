//go:build cgo && !expanse_purego

package expanse

/*
#cgo CFLAGS: -I${SRCDIR}/../../include
#cgo !windows LDFLAGS: ${SRCDIR}/../../target/release/libexpanse.a -lpthread -ldl -lm
#cgo windows LDFLAGS: -L${SRCDIR}/../../target/release -lexpanse
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include "expanse.h"
*/
import "C"

// Version returns the libexpanse build version.
func Version() string {
	return C.GoString(C.expanse_version())
}
