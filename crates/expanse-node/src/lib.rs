//! Native Node.js / Bun / Deno N-API bindings for Expanse modern Judy arrays.
//!
//! Provides high-performance, cache-line-tuned Judy arrays, string/byte maps,
//! blob maps with inline value packing and slab arenas, and OCC concurrent structures.

#![allow(missing_docs)]

pub mod blobmap;
pub mod bytesmap;
pub mod common;
pub mod map;
pub mod set;
pub mod strmap;
pub mod sync;

pub use blobmap::ExpanseBlobMap;
pub use bytesmap::ExpanseBytesMap;
pub use common::{BlobMetaResult, BytesMapEntry, CompactionStatsResult, MapEntry, StrMapEntry};
pub use map::ExpanseMap;
pub use set::ExpanseSet;
pub use strmap::ExpanseStrMap;
pub use sync::{SyncExpanseMap, SyncExpanseSet};
