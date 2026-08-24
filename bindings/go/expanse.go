package expanse

// #cgo CFLAGS: -I../../include
// #cgo LDFLAGS: -L../../target/release -L../../target/debug -lexpanse -lpthread -ldl -lm
// #include <stdlib.h>
// #include "expanse.h"
import "C"
