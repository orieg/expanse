//go:build !cgo || expanse_purego

package expanse

import (
	"errors"
	"runtime"
	"unsafe"
)

type BlobMap struct {
	ptr uintptr
}

func NewBlobMap(chunkSize uint64) *BlobMap {
	ensureLoaded()
	m := &BlobMap{
		ptr: expanse_blob_map_new(uintptr(chunkSize)),
	}
	runtime.SetFinalizer(m, (*BlobMap).Free)
	return m
}

func (b *BlobMap) Set(key uint64, data []byte, hotMeta uint32) {
	var cData unsafe.Pointer
	if len(data) > 0 {
		cData = unsafe.Pointer(&data[0])
	}
	expanse_blob_map_insert(b.ptr, key, cData, uintptr(len(data)), hotMeta)
}

func (b *BlobMap) Get(key uint64) ([]byte, uint32, bool) {
	var view blobView
	if expanse_blob_map_get(b.ptr, key, &view) {
		var data []byte
		if view.len > 0 && view.ptr != nil {
			data = unsafe.Slice((*byte)(view.ptr), view.len)
		}
		return data, view.hotMeta, true
	}
	return nil, 0, false
}

func (b *BlobMap) Delete(key uint64) bool {
	return expanse_blob_map_remove(b.ptr, key)
}

func (b *BlobMap) Contains(key uint64) bool {
	return expanse_blob_map_contains_key(b.ptr, key)
}

func (b *BlobMap) Size() uint64 {
	return expanse_blob_map_len(b.ptr)
}

func (b *BlobMap) MemoryUsed() uint64 {
	return uint64(expanse_blob_map_mem_used(b.ptr))
}

func (b *BlobMap) Clear() {
	expanse_blob_map_clear(b.ptr)
}

func (b *BlobMap) Compact() bool {
	return expanse_blob_map_compact(b.ptr)
}

func (b *BlobMap) Prune(predicate func(key uint64, hotMeta uint32) bool) int {
	ctx := &pruneContext{
		predicate: predicate,
	}
	expanse_blob_map_scan_filtered(b.ptr, 0, ^uint64(0), prunePredicateCallbackPtr, 0, unsafe.Pointer(ctx))

	count := 0
	for _, key := range ctx.toRemove {
		if b.Delete(key) {
			count++
		}
	}
	if count > 0 {
		b.Compact()
	}
	return count
}

func (b *BlobMap) SaveImage(path string) error {
	return errors.New("SaveImage not implemented in C API")
}

func OpenBlobMapImage(path string, mmap bool) (*BlobMap, error) {
	return nil, errors.New("OpenBlobMapImage not implemented in C API")
}

func (b *BlobMap) Free() {
	if b.ptr != 0 {
		expanse_blob_map_free(b.ptr)
		b.ptr = 0
	}
}
