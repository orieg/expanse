package expanse

import "fmt"

// The refusals (*BlobMap).Set names, one per expanse_blob_status_t value in
// expanse.h. Each wraps ErrBlobInsertRefused. The map's contents are
// unchanged after any of them.
var (
	// ErrBlobMetaOverflow: hotMeta needs more than the 24 bits an arena slot holds.
	ErrBlobMetaOverflow = fmt.Errorf("%w: hot_meta above 24 bits", ErrBlobInsertRefused)
	// ErrBlobAllocationFailed: the payload is larger than a chunk, or memory
	// allocation failed.
	ErrBlobAllocationFailed = fmt.Errorf("%w: payload larger than a chunk, or allocation failure", ErrBlobInsertRefused)
	// ErrBlobCapRefused: the capacity cap refused a chunk and nothing was
	// compacted (the reclaim rule declined, or it is off). Dead bytes may
	// remain: ArenaStats().AllocatedBytes - LiveBytes bounds what Compact frees.
	ErrBlobCapRefused = fmt.Errorf("%w: arena capacity cap reached, nothing compacted", ErrBlobInsertRefused)
	// ErrBlobArenaFull: the insert compacted the arena and the record still
	// does not fit under the cap; only a removal makes room.
	ErrBlobArenaFull = fmt.Errorf("%w: arena full after compacting", ErrBlobInsertRefused)
	// ErrBlobInvalidArgument: a nil handle (a freed map).
	ErrBlobInvalidArgument = fmt.Errorf("%w: invalid argument", ErrBlobInsertRefused)
)

// BlobArenaStats is (*BlobMap).ArenaStats' result; its layout is
// expanse_blob_arena_stats_t's.
type BlobArenaStats struct {
	// LiveBytes counts live payload bytes plus an 8-byte header per record.
	LiveBytes uint64
	// AllocatedBytes counts chunk bytes, dead and live: what the cap counts.
	AllocatedBytes uint64
	// MaxCapacity is the capacity cap, as clamped.
	MaxCapacity uint64
	// ChunkSize is the arena's chunk size.
	ChunkSize uint64
	// ReclaimAtCap is 1 if an insert the cap refuses may compact the arena.
	ReclaimAtCap uint64
}

// blobStatusErr maps an expanse_blob_status_t to Set's error.
func blobStatusErr(status int32) error {
	switch status {
	case 0:
		return nil
	case 16:
		return ErrBlobMetaOverflow
	case 17:
		return ErrBlobAllocationFailed
	case 18:
		return ErrBlobCapRefused
	case 19:
		return ErrBlobArenaFull
	case 32:
		return ErrBlobInvalidArgument
	default:
		return fmt.Errorf("%w: status %d", ErrBlobInsertRefused, status)
	}
}
