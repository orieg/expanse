//! # judy-capi — drop-in C ABI for libjudy
//!
//! This crate builds `judy_capi` as a `cdylib` and `staticlib` exposing the
//! classic `Judy.h` surface (`Judy1*`, `JudyL*`, `JudySL*` functions with
//! matching symbol names, calling conventions, and semantics), backed by the
//! clean-room `judy-rs` core. The goal is that existing consumers of the C
//! library — php-judy first among them — can link-swap or `LD_PRELOAD` this
//! library in place of `libJudy` without source changes.
//!
//! The compatibility contract (target surface, guarantees, non-goals, and
//! the differential-oracle test obligations) is defined in `docs/COMPAT.md`
//! at the repository root; the shipped header lives in `include/Judy.h`.
//!
//! The exported surface lands once the core read/write paths exist
//! (Phases 4 and 6). Until then this crate is a compiling stub so the
//! packaging, linkage, and CI artifact checks are exercised from day one.

// Reference the core crate so the dependency edge is built and checked even
// while the stub exports nothing.
use judy_rs as _;
