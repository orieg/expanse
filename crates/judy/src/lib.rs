//! # judy-rs — clean-room Judy arrays for modern hardware
//!
//! Judy arrays (invented by Doug Baskins at Hewlett-Packard) are sparse,
//! dynamic associative data structures built as 256-ary digital tries that
//! partition keys by *expanse* (decoding keys byte by byte) rather than by
//! population like comparison-based trees. The original C implementation
//! (Judy IV, ~2002) achieves its speed through adaptive node compression:
//! linear, bitmap, and uncompressed branches; linear and bitmap leaves; and
//! keys stored immediately inside pointers.
//!
//! This crate is a **clean-room reimplementation**: no code from the LGPL
//! C library has been consulted or ported. The design derives from published
//! algorithm descriptions and modernizes the 2002 architecture for current
//! hardware:
//!
//! - node geometries sized to **64-byte cache lines** (the original assumed
//!   128-byte lines);
//! - hardware `popcnt`/`tzcnt` and SIMD byte scanning instead of SWAR bit
//!   hacks and unrolled scalar loops;
//! - tagged pointers exploiting 48-bit virtual addressing;
//! - a modern allocation strategy instead of the custom 2001 chunk allocator;
//! - optimistic concurrency control for lock-free reads.
//!
//! The classic C `Judy.h` API (`Judy1*`, `JudyL*`, `JudySL*`) is provided as
//! a drop-in binary-compatible layer by the sibling `judy-capi` crate; this
//! crate holds the core implementation and the native Rust API.
//!
//! Currently 64-bit platforms only (x86-64, AArch64).

#[cfg(not(target_pointer_width = "64"))]
compile_error!("judy-rs currently supports 64-bit targets only");

pub mod types;
