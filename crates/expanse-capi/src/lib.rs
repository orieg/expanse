//! # expanse-capi — `libexpanse`, the drop-in C ABI for libjudy
//!
//! This crate builds `libexpanse` (`libexpanse.so` / `expanse.dll` /
//! `libexpanse.a`) exposing two C surfaces:
//!
//! - **Legacy compat**: the classic `Judy.h` API (`Judy1*`, `JudyL*`,
//!   `JudySL*` functions with matching symbol names, calling conventions,
//!   and semantics) so existing consumers of the C library — php-judy first
//!   among them — can link-swap or `LD_PRELOAD` this library in place of
//!   `libJudy` without source changes (a `libjudy-compat` package provides
//!   the `libJudy.so.1` soname symlink and `Judy.h` alias).
//! - **Modern API**: `expanse_*`-prefixed functions in `expanse.h` exposing
//!   the new capabilities; never a semantic change to a `Judy*` symbol.
//!
//! The compatibility contract (target surface, guarantees, non-goals, and
//! the differential-oracle test obligations) is defined in `docs/COMPAT.md`
//! at the repository root; the shipped headers live in `include/`.
//!
//! The exported surface lands once the core read/write paths exist
//! (Phases 4 and 6). Until then this crate is a compiling stub so the
//! packaging, linkage, and CI artifact checks are exercised from day one.

// Reference the core crate so the dependency edge is built and checked even
// while the stub exports nothing.
use expanse_trie as _;
