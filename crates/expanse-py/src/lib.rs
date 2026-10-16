//! Python PyO3 bindings for Expanse modern Judy arrays and digital tries.
//!
//! Provides Python classes for cache-line-tuned Judy arrays and optimistic
//! concurrency control (OCC) structures with GIL-free multithreaded queries.

#![warn(missing_docs)]

pub mod blobmap;
pub mod buffer;
pub mod bytesmap;
pub mod map;
pub mod ordered_bytesmap;
pub(crate) mod reader_cache;
pub mod set;
pub mod strmap;
pub mod sync;
pub mod sync_blobmap;
pub mod sync_bytesmap;
pub mod sync_ordered_bytesmap;
pub mod sync_strmap;

use pyo3::prelude::*;

pub use blobmap::ExpanseBlobMap;
pub use bytesmap::{
    ExpanseBytesMap, ExpanseBytesMapItemIter, ExpanseBytesMapKeyIter, ExpanseBytesMapValueIter,
};
pub use map::{
    ExpanseMap, ExpanseMapItemIter, ExpanseMapKeyIter, ExpanseMapRangeIter, ExpanseMapValueIter,
};
pub use ordered_bytesmap::{
    ExpanseOrderedBytesMap, ExpanseOrderedBytesMapItemIter, ExpanseOrderedBytesMapKeyIter,
    ExpanseOrderedBytesMapValueIter,
};
pub use set::{ExpanseSet, ExpanseSetIter, ExpanseSetRangeIter};
pub use strmap::{
    ExpanseStrMap, ExpanseStrMapItemIter, ExpanseStrMapKeyIter, ExpanseStrMapValueIter,
};
pub use sync::{
    SyncExpanseMap, SyncExpanseMapItemIter, SyncExpanseMapKeyIter, SyncExpanseMapRangeIter,
    SyncExpanseMapValueIter, SyncExpanseSet, SyncExpanseSetIter, SyncExpanseSetRangeIter,
};
pub use sync_blobmap::SyncExpanseBlobMap;
pub use sync_bytesmap::SyncExpanseBytesMap;
pub use sync_ordered_bytesmap::SyncExpanseOrderedBytesMap;
pub use sync_strmap::SyncExpanseStrMap;

// abi-parity: expanse_version
/// The `_expanse` PyO3 module.
#[pymodule]
fn _expanse(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Main collection types
    m.add_class::<ExpanseMap>()?;
    m.add_class::<ExpanseSet>()?;
    m.add_class::<SyncExpanseMap>()?;
    m.add_class::<SyncExpanseSet>()?;
    m.add_class::<SyncExpanseStrMap>()?;
    m.add_class::<SyncExpanseBytesMap>()?;
    m.add_class::<SyncExpanseOrderedBytesMap>()?;
    m.add_class::<SyncExpanseBlobMap>()?;
    m.add_class::<ExpanseStrMap>()?;
    m.add_class::<ExpanseBytesMap>()?;
    m.add_class::<ExpanseBlobMap>()?;
    m.add_class::<ExpanseOrderedBytesMap>()?;

    // Iterator types for ExpanseMap
    m.add_class::<ExpanseMapKeyIter>()?;
    m.add_class::<ExpanseMapValueIter>()?;
    m.add_class::<ExpanseMapItemIter>()?;
    m.add_class::<ExpanseMapRangeIter>()?;

    // Iterator types for ExpanseSet
    m.add_class::<ExpanseSetIter>()?;
    m.add_class::<ExpanseSetRangeIter>()?;

    // Iterator types for SyncExpanseMap
    m.add_class::<SyncExpanseMapKeyIter>()?;
    m.add_class::<SyncExpanseMapValueIter>()?;
    m.add_class::<SyncExpanseMapItemIter>()?;
    m.add_class::<SyncExpanseMapRangeIter>()?;

    // Iterator types for SyncExpanseSet
    m.add_class::<SyncExpanseSetIter>()?;
    m.add_class::<SyncExpanseSetRangeIter>()?;

    // Iterator types for ExpanseStrMap
    m.add_class::<ExpanseStrMapKeyIter>()?;
    m.add_class::<ExpanseStrMapValueIter>()?;
    m.add_class::<ExpanseStrMapItemIter>()?;

    // Iterator types for ExpanseBytesMap
    m.add_class::<ExpanseBytesMapKeyIter>()?;
    m.add_class::<ExpanseBytesMapValueIter>()?;
    m.add_class::<ExpanseBytesMapItemIter>()?;

    // Iterator types for ExpanseOrderedBytesMap
    m.add_class::<ExpanseOrderedBytesMapKeyIter>()?;
    m.add_class::<ExpanseOrderedBytesMapValueIter>()?;
    m.add_class::<ExpanseOrderedBytesMapItemIter>()?;

    Ok(())
}
