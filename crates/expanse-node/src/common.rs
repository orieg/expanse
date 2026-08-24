//! Common types, converters, and entry records for Expanse Node bindings.

use napi::bindgen_prelude::*;
use napi_derive::napi;

/// A 64-bit integer key input: either a JS BigInt or a JS Number.
pub type KeyInput = Either<BigInt, f64>;

/// Arbitrary byte slice input: either a Node Buffer, a Uint8Array, or a UTF-8 string.
pub type BytesInput = Either3<Buffer, Uint8Array, String>;

/// Converts a JS BigInt or Number to a Rust `u64`.
pub fn key_to_u64(key: KeyInput) -> Result<u64> {
    match key {
        Either::A(bi) => {
            if bi.sign_bit {
                return Err(Error::new(
                    Status::InvalidArg,
                    "Key must be an unsigned 64-bit integer (received negative BigInt)",
                ));
            }
            // `lossless` is false when the BigInt does not fit in a single u64 word;
            // ignoring it silently truncates, e.g. 2n**64n + 1n would store 1.
            let (_, val, lossless) = bi.get_u64();
            if !lossless {
                return Err(Error::new(
                    Status::InvalidArg,
                    "Key BigInt does not fit in an unsigned 64-bit integer (value out of u64 range)",
                ));
            }
            Ok(val)
        }
        Either::B(num) => {
            if num < 0.0 || num.is_nan() || num.is_infinite() {
                return Err(Error::new(
                    Status::InvalidArg,
                    "Key must be a non-negative finite number",
                ));
            }
            if num.fract() != 0.0 {
                return Err(Error::new(
                    Status::InvalidArg,
                    "Key number must be an integer (received a fractional value)",
                ));
            }
            // A JS Number (f64) can only represent integers exactly up to 2^53; past
            // that, distinct u64 keys collapse to the same value. Require a BigInt.
            const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_992.0; // 2^53
            if num > MAX_SAFE_INTEGER {
                return Err(Error::new(
                    Status::InvalidArg,
                    "Key number exceeds 2^53 (JS integer-precision limit); pass a BigInt for larger keys",
                ));
            }
            Ok(num as u64)
        }
    }
}

/// Converts a `BytesInput` to a borrowed byte slice.
pub fn bytes_input_to_slice(input: &BytesInput) -> &[u8] {
    match input {
        Either3::A(buf) => buf.as_ref(),
        Either3::B(u8arr) => u8arr.as_ref(),
        Either3::C(s) => s.as_bytes(),
    }
}

/// Validates that a string contains no NUL (`\0`) bytes and returns its byte slice.
pub fn str_to_nul_free_bytes(s: &str) -> Result<&[u8]> {
    let bytes = s.as_bytes();
    if bytes.contains(&0) {
        return Err(Error::new(
            Status::InvalidArg,
            "NUL bytes ('\\0') are not allowed in ExpanseStrMap keys",
        ));
    }
    Ok(bytes)
}

#[napi(object)]
/// A 64-bit integer key-value entry.
pub struct MapEntry {
    /// 64-bit unsigned integer key.
    pub key: BigInt,
    /// 64-bit unsigned integer value.
    pub value: BigInt,
}

#[napi(object)]
/// A string key-value entry.
pub struct StrMapEntry {
    /// String key.
    pub key: String,
    /// 64-bit unsigned integer value.
    pub value: BigInt,
}

#[napi(object)]
/// A byte-key value entry.
pub struct BytesMapEntry {
    /// Raw byte key as a Node Buffer.
    pub key: Buffer,
    /// 64-bit unsigned integer value.
    pub value: BigInt,
}

#[napi(object)]
/// Metadata and payload returned by BlobMap lookup.
pub struct BlobMetaResult {
    /// The blob payload bytes as a Node Buffer.
    pub payload: Buffer,
    /// 32-bit hot metadata word stored in the trie index.
    pub hot_meta: u32,
    /// True if payload was stored inline in the 64-bit value slot.
    pub is_inline: bool,
}

#[napi(object)]
/// Compaction statistics returned by BlobMap garbage collection.
pub struct CompactionStatsResult {
    /// Live payload bytes before compaction.
    pub live_bytes_before: BigInt,
    /// Live payload bytes after compaction.
    pub live_bytes_after: BigInt,
    /// Total arena bytes allocated before compaction.
    pub total_allocated_before: BigInt,
    /// Total arena bytes allocated after compaction.
    pub total_allocated_after: BigInt,
}
