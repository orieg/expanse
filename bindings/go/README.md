# expanse-go

Native Go 1.22+ bindings for `libexpanse`, providing zero-GC off-heap ordered maps, sets, and concurrent lock-free collections via CGO.

## Quickstart

```go
package main

import (
	"fmt"
	"github.com/orieg/expanse/bindings/go"
)

func main() {
	m := expanse.NewMap()
	m.Set(42, 100)
	val, ok := m.Get(42)
	fmt.Println(val, ok)
}
```
