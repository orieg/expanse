//! `vec_iter!`: the snapshot iterator classes every container returns.
//!
//! Each `keys()`/`values()`/`items()`/`__iter__`/range method collects its
//! results into a `Vec` and hands Python an iterator class that walks it. Those
//! classes differ only in their name, docstring, field name and item type, so
//! they are generated here rather than written out per container.
//!
//! Python sees the class docstring and the `__iter__`/`__next__`/`__len__`
//! slots; PyO3 gives slot methods CPython's generic docstrings, so the method
//! docs below are Rust-side only.

/// Generates a `#[pyclass]` iterator over a `Vec` snapshot.
///
/// ```ignore
/// vec_iter! {
///     /// Class docstring.
///     ExpanseSetIter { items: Vec<u64> }
/// }
/// ```
///
/// yields `Copy` elements as they are. A conversion is written after `yields`,
/// naming the lifetime of the `Python` token, the item type and a closure-like
/// expression over the token and a reference to the element:
///
/// ```ignore
/// vec_iter! {
///     /// Class docstring.
///     ExpanseBytesMapKeyIter { keys: Vec<Vec<u8>> }
///     yields<'py> Bound<'py, PyBytes> = |py, key| PyBytes::new(py, key)
/// }
/// ```
///
/// The fields stay `pub(crate)`, named as the caller names them, with an
/// `index` cursor, so construction sites read `Name { keys, index: 0 }`.
macro_rules! vec_iter {
    (
        $(#$attr:tt)*
        $name:ident { $field:ident: Vec<$elem:ty> }
    ) => {
        vec_iter! {
            $(#$attr)*
            $name { $field: Vec<$elem> }
            yields<'py> $elem = |_py, item| *item
        }
    };
    (
        $(#$attr:tt)*
        $name:ident { $field:ident: Vec<$elem:ty> }
        yields<$lt:lifetime> $ret:ty = |$py:ident, $item:ident| $conv:expr
    ) => {
        $(#$attr)*
        #[::pyo3::pyclass(unsendable, module = "expanse_trie._expanse")]
        pub struct $name {
            pub(crate) $field: Vec<$elem>,
            pub(crate) index: usize,
        }

        #[::pyo3::pymethods]
        impl $name {
            /// Returns the iterator instance.
            pub fn __iter__(slf: ::pyo3::PyRef<'_, Self>) -> ::pyo3::PyRef<'_, Self> {
                slf
            }

            /// Returns the next item, or ends the iteration.
            pub fn __next__<$lt>(&mut self, $py: ::pyo3::Python<$lt>) -> Option<$ret> {
                let $item = self.$field.get(self.index)?;
                self.index += 1;
                Some($conv)
            }

            /// Returns the number of items not yet yielded.
            pub fn __len__(&self) -> usize {
                self.$field.len().saturating_sub(self.index)
            }
        }
    };
}
