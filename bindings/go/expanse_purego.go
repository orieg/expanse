//go:build !cgo || expanse_purego

package expanse

// Version returns the libexpanse build version.
func Version() string {
	ensureLoaded()
	return expanse_version()
}
