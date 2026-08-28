//go:build (darwin || linux || freebsd || openbsd || netbsd) && (!cgo || expanse_purego)

package expanse

import (
	"fmt"

	"github.com/ebitengine/purego"
)

func openPlatformLibrary(path string) (*LibraryHandle, error) {
	handle, err := purego.Dlopen(path, purego.RTLD_NOW|purego.RTLD_GLOBAL)
	if err != nil {
		return nil, fmt.Errorf("dlopen(%q) failed: %w", path, err)
	}
	return &LibraryHandle{handle: handle}, nil
}

func (h *LibraryHandle) registerFunc(fptr any, name string) error {
	sym, err := purego.Dlsym(h.handle, name)
	if err != nil || sym == 0 {
		return fmt.Errorf("symbol %q not found in libexpanse: %w", name, err)
	}
	purego.RegisterFunc(fptr, sym)
	return nil
}
