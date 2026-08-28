//go:build windows && (!cgo || expanse_purego)

package expanse

import (
	"fmt"
	"syscall"

	"github.com/ebitengine/purego"
)

func openPlatformLibrary(path string) (*LibraryHandle, error) {
	dll := syscall.NewLazyDLL(path)
	if err := dll.Load(); err != nil {
		return nil, fmt.Errorf("LoadLibrary(%q) failed: %w", path, err)
	}
	return &LibraryHandle{handle: dll.Handle()}, nil
}

func (h *LibraryHandle) registerFunc(fptr any, name string) error {
	proc, err := syscall.GetProcAddress(syscall.Handle(h.handle), name)
	if err != nil || proc == 0 {
		return fmt.Errorf("symbol %q not found in expanse.dll: %w", name, err)
	}
	purego.RegisterFunc(fptr, proc)
	return nil
}
