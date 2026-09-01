//! # Expanse — clean-room Judy arrays for modern hardware
//!
//! Judy arrays (invented by Doug Baskins at Hewlett-Packard) are sparse,
//! dynamic associative data structures built as 256-ary digital tries that
//! partition keys by **expanse** (decoding keys byte by byte over fixed
//! digit ranges) rather than by population like comparison-based trees —
//! the defining term this project is named after. The original C
//! implementation (Judy IV, ~2002) achieves its speed through adaptive node
//! compression: linear, bitmap, and uncompressed branches; linear and
//! bitmap leaves; and keys stored immediately inside pointers.
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
//! - no upper-bit pointer stealing: the tag byte and the
//!   population/decode field live in word 1 of the
//!   16-byte `Edge`, so word 0 keeps the raw untruncated 64-bit pointer
//!   (correct under 57-bit LA57 and 52-bit ARM64 LVA);
//! - a modern allocation strategy instead of the custom 2001 chunk allocator;
//! - optimistic concurrency control for optimistic reads.
//!
//! The classic C `Judy.h` API (`Judy1*`, `JudyL*`, `JudySL*`) is provided as
//! a drop-in binary-compatible layer by the sibling `expanse-capi` crate
//! (`libexpanse`); this crate holds the core implementation and the native
//! Rust API (`ExpanseSet`, `ExpanseMap`, `ExpanseStrMap`, `ExpanseBytesMap`
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc as core_alloc;

#[cfg(not(any(target_pointer_width = "64", target_pointer_width = "32")))]
compile_error!("expanse supports 64-bit and 32-bit targets");

#[cfg(target_pointer_width = "64")]
mod algebra;
#[cfg(target_pointer_width = "64")]
mod algebra_build;
#[cfg(target_pointer_width = "64")]
pub mod alloc;
#[cfg(target_pointer_width = "64")]
pub mod bits;
#[cfg(target_pointer_width = "64")]
pub mod blobmap;
#[cfg(target_pointer_width = "64")]
pub mod bytesmap;
#[cfg(target_pointer_width = "64")]
pub mod codec;
#[cfg(target_pointer_width = "64")]
pub mod cursor;
#[cfg(target_pointer_width = "64")]
pub mod get;
#[cfg(target_pointer_width = "64")]
pub mod iter;
#[cfg(target_pointer_width = "64")]
pub mod leaf;
#[cfg(target_pointer_width = "64")]
pub mod map;
#[cfg(target_pointer_width = "64")]
pub mod mutate;
#[cfg(target_pointer_width = "64")]
mod mutate_map;
#[cfg(target_pointer_width = "64")]
mod nav;
#[cfg(target_pointer_width = "64")]
pub mod node;
#[cfg(target_pointer_width = "64")]
pub mod occ;
#[cfg(target_pointer_width = "64")]
pub mod occ_stats;
#[cfg(target_pointer_width = "64")]
pub mod set;
#[cfg(target_pointer_width = "64")]
pub mod slot;
#[cfg(target_pointer_width = "64")]
pub mod strmap;
#[cfg(all(target_pointer_width = "64", feature = "std"))]
pub mod sync;
#[cfg(target_pointer_width = "64")]
pub mod types;
#[cfg(target_pointer_width = "64")]
pub mod validate;

pub mod blobmap32;
pub mod cursor32;
pub mod map32;
pub mod node32;
pub mod occ32;
pub mod set32;
pub mod slot32;
mod trie32;
pub mod types32;

#[cfg(target_pointer_width = "64")]
pub use blobmap::{BlobArena, BlobView, ExpanseBlobMap};
#[cfg(target_pointer_width = "64")]
pub use cursor::{MapCursor, SetCursor};
#[cfg(target_pointer_width = "64")]
pub use map::ExpanseMap;
#[cfg(target_pointer_width = "64")]
pub use set::ExpanseSet;
#[cfg(target_pointer_width = "64")]
pub use slot::{SlotTag, ValueSlot};

#[cfg(target_pointer_width = "32")]
pub use blobmap32::ExpanseBlobMap32 as ExpanseBlobMap;
#[cfg(target_pointer_width = "32")]
pub use cursor32::{MapCursor32 as MapCursor, SetCursor32 as SetCursor};
#[cfg(target_pointer_width = "32")]
pub use map32::ExpanseMap32 as ExpanseMap;
#[cfg(target_pointer_width = "32")]
pub use set32::ExpanseSet32 as ExpanseSet;
#[cfg(target_pointer_width = "32")]
pub use slot32::ValueSlot32 as ValueSlot;

pub use blobmap32::{BlobMap32Error, BlobView32, ExpanseBlobMap32};
pub use cursor32::{MapCursor32, SetCursor32};
pub use map32::ExpanseMap32;
pub use occ32::SeqVersion32;
pub use set32::ExpanseSet32;
pub use slot32::{SlotTag32, ValueSlot32};
pub use types32::{Edge32, Key32, Tag32, Value32};
