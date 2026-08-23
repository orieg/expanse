//! Buffer protocol and key/value extraction utilities for zero-copy and bulk ingestion.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{
    PyByteArray, PyByteArrayMethods, PyBytes, PyBytesMethods, PyString, PyStringMethods,
};
use std::borrow::Cow;

/// Extracts a string or byte slice as a NUL-free byte slice for `ExpanseStrMap`.
pub fn extract_str_key<'a>(key: &'a Bound<'_, PyAny>) -> PyResult<Cow<'a, [u8]>> {
    if let Ok(s) = key.cast::<PyString>() {
        let text = s.to_cow()?;
        let bytes = text.as_bytes();
        if bytes.contains(&0) {
            return Err(PyValueError::new_err(
                "NUL bytes are not allowed in ExpanseStrMap keys",
            ));
        }
        return Ok(Cow::Owned(bytes.to_vec()));
    }
    if let Ok(b) = key.cast::<PyBytes>() {
        let bytes = b.as_bytes();
        if bytes.contains(&0) {
            return Err(PyValueError::new_err(
                "NUL bytes are not allowed in ExpanseStrMap keys",
            ));
        }
        return Ok(Cow::Borrowed(bytes));
    }
    if let Ok(ba) = key.cast::<PyByteArray>() {
        // SAFETY: `ba` is a valid PyByteArray reference whose backing bytes remain valid during this call.
        let bytes = unsafe { ba.as_bytes() };
        if bytes.contains(&0) {
            return Err(PyValueError::new_err(
                "NUL bytes are not allowed in ExpanseStrMap keys",
            ));
        }
        return Ok(Cow::Owned(bytes.to_vec()));
    }
    Err(PyTypeError::new_err(
        "Expected str, bytes, or bytearray for key",
    ))
}

/// Extracts arbitrary byte slices for `ExpanseBytesMap` (allows NUL bytes).
pub fn extract_bytes_key<'a>(key: &'a Bound<'_, PyAny>) -> PyResult<Cow<'a, [u8]>> {
    if let Ok(s) = key.cast::<PyString>() {
        let text = s.to_cow()?;
        return Ok(Cow::Owned(text.into_owned().into_bytes()));
    }
    if let Ok(b) = key.cast::<PyBytes>() {
        return Ok(Cow::Borrowed(b.as_bytes()));
    }
    if let Ok(ba) = key.cast::<PyByteArray>() {
        // SAFETY: `ba` is a valid PyByteArray reference whose backing bytes remain valid during this call.
        return Ok(Cow::Owned(unsafe { ba.as_bytes() }.to_vec()));
    }
    Err(PyTypeError::new_err(
        "Expected bytes, bytearray, or str for key",
    ))
}

/// Iterates through key-value pairs from Python iterables or sequences.
pub fn for_each_u64_pair(
    keys_obj: &Bound<'_, PyAny>,
    vals_obj: &Bound<'_, PyAny>,
    mut f: impl FnMut(u64, u64),
) -> PyResult<usize> {
    if let (Ok(k_bytes), Ok(v_bytes)) = (keys_obj.cast::<PyBytes>(), vals_obj.cast::<PyBytes>()) {
        let k_slice = k_bytes.as_bytes();
        let v_slice = v_bytes.as_bytes();
        if k_slice.len() % 8 == 0 && v_slice.len() % 8 == 0 {
            let count = (k_slice.len() / 8).min(v_slice.len() / 8);
            for i in 0..count {
                let k = u64::from_ne_bytes(k_slice[i * 8..(i + 1) * 8].try_into().unwrap());
                let v = u64::from_ne_bytes(v_slice[i * 8..(i + 1) * 8].try_into().unwrap());
                f(k, v);
            }
            return Ok(count);
        }
    }

    let mut count = 0;
    let k_iter = keys_obj.try_iter()?;
    let v_iter = vals_obj.try_iter()?;
    for (k_item, v_item) in k_iter.zip(v_iter) {
        let k: u64 = k_item?.extract()?;
        let v: u64 = v_item?.extract()?;
        f(k, v);
        count += 1;
    }
    Ok(count)
}

/// Iterates through keys from a Python iterable or sequence.
pub fn for_each_u64_key(keys_obj: &Bound<'_, PyAny>, mut f: impl FnMut(u64)) -> PyResult<usize> {
    if let Ok(k_bytes) = keys_obj.cast::<PyBytes>() {
        let k_slice = k_bytes.as_bytes();
        if k_slice.len() % 8 == 0 {
            let count = k_slice.len() / 8;
            for i in 0..count {
                let k = u64::from_ne_bytes(k_slice[i * 8..(i + 1) * 8].try_into().unwrap());
                f(k);
            }
            return Ok(count);
        }
    }

    let mut count = 0;
    for k_item in keys_obj.try_iter()? {
        let k: u64 = k_item?.extract()?;
        f(k);
        count += 1;
    }
    Ok(count)
}
