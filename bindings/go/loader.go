//go:build !cgo || expanse_purego

package expanse

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync"
)

type LibraryHandle struct {
	handle uintptr
}

var (
	loadOnce  sync.Once
	loadErr   error
	libHandle *LibraryHandle
)

func platformLibName() string {
	switch runtime.GOOS {
	case "darwin":
		return "libexpanse.dylib"
	case "windows":
		return "expanse.dll"
	default:
		return "libexpanse.so"
	}
}

func findLibrary() (string, error) {
	libName := platformLibName()

	// 1. Check EXPANSE_LIBRARY env var (file path or directory)
	if env := os.Getenv("EXPANSE_LIBRARY"); env != "" {
		if fi, err := os.Stat(env); err == nil {
			if fi.IsDir() {
				target := filepath.Join(env, libName)
				if _, err := os.Stat(target); err == nil {
					return target, nil
				}
			} else {
				return env, nil
			}
		}
	}

	// 2. Check EXPANSE_LIBRARY_PATH env var
	if env := os.Getenv("EXPANSE_LIBRARY_PATH"); env != "" {
		if fi, err := os.Stat(env); err == nil {
			if fi.IsDir() {
				target := filepath.Join(env, libName)
				if _, err := os.Stat(target); err == nil {
					return target, nil
				}
			} else {
				return env, nil
			}
		}
	}

	// 3. Check EXPANSE_LIB_DIR env var
	if env := os.Getenv("EXPANSE_LIB_DIR"); env != "" {
		target := filepath.Join(env, libName)
		if _, err := os.Stat(target); err == nil {
			return target, nil
		}
	}

	// 4. Check relative development paths
	devPaths := []string{
		filepath.Join(".", "target", "release", libName),
		filepath.Join("..", "target", "release", libName),
		filepath.Join("..", "..", "target", "release", libName),
		filepath.Join("..", "..", "..", "target", "release", libName),
	}
	for _, p := range devPaths {
		if _, err := os.Stat(p); err == nil {
			if abs, err := filepath.Abs(p); err == nil {
				return abs, nil
			}
			return p, nil
		}
	}

	// 5. Check standard system library directories
	sysDirs := []string{
		"/usr/local/lib",
		"/usr/lib",
		"/opt/homebrew/lib",
		"/opt/local/lib",
	}
	for _, d := range sysDirs {
		p := filepath.Join(d, libName)
		if _, err := os.Stat(p); err == nil {
			return p, nil
		}
	}

	// 6. Default to bare library name for system loader search
	return libName, nil
}

func ensureLoaded() {
	loadOnce.Do(func() {
		path, err := findLibrary()
		if err != nil {
			loadErr = fmt.Errorf("could not find libexpanse (%s): %w\nHint: Set EXPANSE_LIBRARY=/path/to/%s or build with 'cargo build --release -p expanse-capi'", platformLibName(), err, platformLibName())
			return
		}
		h, err := openPlatformLibrary(path)
		if err != nil {
			loadErr = fmt.Errorf("failed to load %q: %w\nHint: Verify the binary architecture matches %s/%s or set EXPANSE_LIBRARY to the correct shared library path", path, err, runtime.GOOS, runtime.GOARCH)
			return
		}
		libHandle = h
		if err := bindSymbols(h); err != nil {
			loadErr = fmt.Errorf("failed to bind symbols from %q: %w\nHint: Verify libexpanse was compiled from the matching version of expanse-capi", path, err)
			return
		}
		initCallbacks()
	})
	if loadErr != nil {
		panic(fmt.Sprintf("expanse: failed to initialize purego native library:\n%v", loadErr))
	}
}
