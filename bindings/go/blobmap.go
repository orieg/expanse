package expanse

// #include <stdlib.h>
// #include "expanse.h"
//
// extern bool goPrunePredicate(uint64_t key, uint32_t hot_meta, void *ctx);
//
// static inline size_t call_blobmap_scan_filtered(const ExpanseBlobMap *map, uint64_t start_key, uint64_t end_key, void *ctx) {
//     return expanse_blob_map_scan_filtered(map, start_key, end_key, goPrunePredicate, NULL, ctx);
// }
import "C"
import (
	"errors"
	"runtime"
	"runtime/cgo"
	"unsafe"
)

type BlobMap struct {
	ptr *C.ExpanseBlobMap
}

func NewBlobMap(chunkSize uint64) *BlobMap {
	m := &BlobMap{
		ptr: C.expanse_blob_map_new(C.size_t(chunkSize)),
	}
	runtime.SetFinalizer(m, (*BlobMap).Free)
	return m
}

func (b *BlobMap) Set(key uint64, data []byte, hotMeta uint32) {
	var cData *C.uint8_t
	if len(data) > 0 {
		cData = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	C.expanse_blob_map_insert(b.ptr, C.uint64_t(key), cData, C.size_t(len(data)), C.uint32_t(hotMeta))
}

func (b *BlobMap) Get(key uint64) ([]byte, uint32, bool) {
	var view C.ExpanseBlobView
	if bool(C.expanse_blob_map_get(b.ptr, C.uint64_t(key), &view)) {
		var data []byte
		if view.len > 0 {
			data = unsafe.Slice((*byte)(unsafe.Pointer(view.ptr)), view.len)
		}
		return data, uint32(view.hot_meta), true
	}
	return nil, 0, false
}

func (b *BlobMap) Delete(key uint64) bool {
	return bool(C.expanse_blob_map_remove(b.ptr, C.uint64_t(key)))
}

func (b *BlobMap) Contains(key uint64) bool {
	return bool(C.expanse_blob_map_contains_key(b.ptr, C.uint64_t(key)))
}

func (b *BlobMap) Size() uint64 {
	return uint64(C.expanse_blob_map_len(b.ptr))
}

func (b *BlobMap) Clear() {
	C.expanse_blob_map_clear(b.ptr)
}

//export goPrunePredicate
func goPrunePredicate(key C.uint64_t, meta C.uint32_t, ctx unsafe.Pointer) C.bool {
	h := *(*cgo.Handle)(ctx)
	predicate := h.Value().(func(uint64, uint32) bool)
	return C.bool(predicate(uint64(key), uint32(meta)))
}

func (b *BlobMap) Prune(predicate func(key uint64, hotMeta uint32) bool) int {
	var toRemove []uint64
	wrapper := func(key uint64, hotMeta uint32) bool {
		if predicate(key, hotMeta) {
			toRemove = append(toRemove, key)
		}
		return false
	}

	h := cgo.NewHandle(wrapper)
	defer h.Delete()

	C.call_blobmap_scan_filtered(b.ptr, 0, ^C.uint64_t(0), unsafe.Pointer(&h))

	count := 0
	for _, key := range toRemove {
		if b.Delete(key) {
			count++
		}
	}
	if count > 0 {
		C.expanse_blob_map_compact(b.ptr)
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
	if b.ptr != nil {
		C.expanse_blob_map_free(b.ptr)
		b.ptr = nil
	}
}
