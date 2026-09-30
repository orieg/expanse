package expanse_test

// The Go usage example on the project website is this file. scripts/build_pages.py
// renders everything from the import block down, as `package main` with
// Example renamed to main and its `// Output:` block dropped, so `go test`
// compiles and runs exactly what the page shows. Keep it self-contained.

import (
	"errors"
	"fmt"
	"log"

	expanse "github.com/orieg/expanse/bindings/go"
)

func Example() {
	m := expanse.NewMap()
	defer m.Free()

	m.Set(42, 100)
	if val, ok := m.Get(42); ok {
		fmt.Printf("Found key 42 -> %d\n", val)
	}

	// Off-heap blob map with hot metadata; 0 selects the default chunk size.
	bm := expanse.NewBlobMap(0)
	defer bm.Free()
	if err := bm.Set(1001, []byte("payload data"), 0x2A); err != nil {
		log.Fatal(err)
	}
	if data, meta, ok := bm.Get(1001); ok {
		fmt.Printf("Blob: %s, meta: 0x%X\n", string(data), meta)
	}

	// A refused insert returns an error and leaves the key unchanged: here,
	// hot metadata wider than 24 bits on a payload longer than 7 bytes.
	err := bm.Set(1002, []byte("sixteen byte val"), 1<<24)
	if errors.Is(err, expanse.ErrBlobInsertRefused) {
		fmt.Println("Insert refused; key 1002 present:", bm.Contains(1002))
	}
	// Output:
	// Found key 42 -> 100
	// Blob: payload data, meta: 0x2A
	// Insert refused; key 1002 present: false
}
