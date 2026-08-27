//! Buffer protocol and key/value extraction utilities for zero-copy and bulk ingestion.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{
    PyByteArray, PyByteArrayMethods, PyBytes, PyBytesMethods, PyMemoryView, PyString,
    PyStringMethods,
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

/// If `obj` is a contiguous byte buffer (`bytes`, `bytearray`, or `memoryview`),
/// returns a copy of its raw bytes; otherwise `Ok(None)`.
///
/// (We copy rather than borrow because the buffer-protocol C API is unavailable under
/// the limited/abi3 ABI this extension is built with, so the "zero-copy" fast path is
/// not possible here — correctness over a micro-optimization.)
fn packed_byte_source(obj: &Bound<'_, PyAny>) -> PyResult<Option<Vec<u8>>> {
    if let Ok(b) = obj.cast::<PyBytes>() {
        return Ok(Some(b.as_bytes().to_vec()));
    }
    if let Ok(ba) = obj.cast::<PyByteArray>() {
        // SAFETY: the bytes are copied out immediately while the GIL is held; the
        // bytearray cannot be resized/mutated concurrently before the copy completes.
        return Ok(Some(unsafe { ba.as_bytes() }.to_vec()));
    }
    if let Ok(mv) = obj.cast::<PyMemoryView>() {
        // The buffer protocol isn't reachable under abi3, so materialize via tobytes().
        let bytes_obj = mv.call_method0("tobytes")?;
        let b = bytes_obj.cast::<PyBytes>()?;
        return Ok(Some(b.as_bytes().to_vec()));
    }
    Ok(None)
}

/// Decodes a packed byte buffer as NATIVE-endian `u64` values, erroring (rather than
/// silently reinterpreting) if the length is not a whole number of 8-byte elements.
fn bytes_as_u64_vec(bytes: &[u8], what: &str) -> PyResult<Vec<u64>> {
    if !bytes.len().is_multiple_of(8) {
        return Err(PyValueError::new_err(format!(
            "insert_many: {what} byte buffer length ({}) is not a multiple of 8; a packed \
             u64 buffer must contain whole 8-byte elements",
            bytes.len()
        )));
    }
    // Length is an exact multiple of 8 (checked above), so `as_chunks` leaves an
    // empty remainder and every chunk is a `[u8; 8]` — no fallible conversion.
    Ok(bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| u64::from_ne_bytes(*c))
        .collect())
}

/// Materializes a keys/values argument as a `Vec<u64>`.
///
/// A `bytes`/`bytearray`/`memoryview` is read as packed **native-endian** `u64` (its
/// byte length must be a multiple of 8, else `ValueError`); anything else (lists, numpy
/// uint64 arrays, generators, ...) is extracted element-by-element. This is the fix for
/// the old behaviour where a `bytes` with `len % 8 != 0`, or any `bytearray`/`memoryview`,
/// silently fell back to per-BYTE iteration and stored 0..255 byte values as keys.
fn collect_u64(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<Vec<u64>> {
    if let Some(bytes) = packed_byte_source(obj)? {
        return bytes_as_u64_vec(&bytes, what);
    }
    let mut out = Vec::new();
    for item in obj.try_iter()? {
        out.push(item?.extract::<u64>()?);
    }
    Ok(out)
}

/// Applies `f` to key-value pairs from Python iterables, sequences, or byte buffers.
///
/// Both arguments are interpreted independently (see [`collect_u64`]); their element
/// counts must match, otherwise a `ValueError` is raised instead of silently truncating
/// to the shorter one. Packed byte buffers are read as **native-endian** `u64`.
pub fn for_each_u64_pair(
    keys_obj: &Bound<'_, PyAny>,
    vals_obj: &Bound<'_, PyAny>,
    mut f: impl FnMut(u64, u64),
) -> PyResult<usize> {
    let keys = collect_u64(keys_obj, "keys")?;
    let vals = collect_u64(vals_obj, "values")?;
    if keys.len() != vals.len() {
        return Err(PyValueError::new_err(format!(
            "insert_many: keys ({}) and values ({}) must have the same number of elements",
            keys.len(),
            vals.len()
        )));
    }
    for (&k, &v) in keys.iter().zip(vals.iter()) {
        f(k, v);
    }
    Ok(keys.len())
}

/// Applies `f` to keys from a Python iterable, sequence, or byte buffer.
///
/// A packed byte buffer is read as **native-endian** `u64` (length must be a multiple
/// of 8, else `ValueError`); other iterables are extracted element-by-element.
pub fn for_each_u64_key(keys_obj: &Bound<'_, PyAny>, mut f: impl FnMut(u64)) -> PyResult<usize> {
    if let Some(bytes) = packed_byte_source(keys_obj)? {
        let keys = bytes_as_u64_vec(&bytes, "keys")?;
        for &k in &keys {
            f(k);
        }
        return Ok(keys.len());
    }

    let mut count = 0;
    for k_item in keys_obj.try_iter()? {
        let k: u64 = k_item?.extract()?;
        f(k);
        count += 1;
    }
    Ok(count)
}
