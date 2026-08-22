//! Python PyO3 bindings for Expanse modern Judy arrays and digital tries.
//!
//! Provides Python classes for cache-line-tuned Judy arrays and optimistic
//! concurrency control (OCC) structures with GIL-free multithreaded queries.

#![warn(missing_docs)]

pub mod buffer;
pub mod bytesmap;
pub mod map;
pub mod set;
pub mod strmap;
pub mod sync;

use pyo3::prelude::*;

pub use bytesmap::{
    ExpanseBytesMap, ExpanseBytesMapItemIter, ExpanseBytesMapKeyIter, ExpanseBytesMapValueIter,
};
pub use map::{
    ExpanseMap, ExpanseMapItemIter, ExpanseMapKeyIter, ExpanseMapRangeIter, ExpanseMapValueIter,
};
pub use set::{ExpanseSet, ExpanseSetIter, ExpanseSetRangeIter};
pub use strmap::{
    ExpanseStrMap, ExpanseStrMapItemIter, ExpanseStrMapKeyIter, ExpanseStrMapValueIter,
};
pub use sync::{
    SyncExpanseMap, SyncExpanseMapItemIter, SyncExpanseMapKeyIter, SyncExpanseMapRangeIter,
    SyncExpanseMapValueIter, SyncExpanseSet, SyncExpanseSetIter, SyncExpanseSetRangeIter,
};

/// The `_expanse` PyO3 module.
#[pymodule]
fn _expanse(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Main collection types
    m.add_class::<ExpanseMap>()?;
    m.add_class::<ExpanseSet>()?;
    m.add_class::<SyncExpanseMap>()?;
    m.add_class::<SyncExpanseSet>()?;
    m.add_class::<ExpanseStrMap>()?;
    m.add_class::<ExpanseBytesMap>()?;

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

    Ok(())
}
