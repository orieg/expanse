//! # expanse-capi — `libexpanse`, the drop-in C ABI for libjudy
//!
//! Exports the classic `Judy.h` surface (`Judy1*` backed by `ExpanseSet`,
//! `JudyL*` backed by `ExpanseMap`) with matching symbol names, calling
//! convention, and documented semantics, so existing consumers of the C
//! library link-swap or `LD_PRELOAD` `libexpanse` in place of `libJudy`
//! without source changes. The contract — target surface, guarantees,
//! non-goals, doc-gap resolutions, acceptance gates — is `docs/COMPAT.md`;
//! the shipped header is `include/Judy.h`.
//!
//! Array-handle convention: a `Pvoid_t` array word is null when the array
//! is empty and otherwise holds a pointer to the boxed tree. Removing the
//! last key returns the word to null, matching the classic representation
//! of an empty array.
//!
//! `JudySL*` is backed by `ExpanseStrMap` (a meta-trie of word maps);
//! `JudyHS*` lands with `ExpanseShortBytesMap`.

// The entry points below are C ABI exports whose safety contract is the
// documented Judy C API itself (docs/COMPAT.md); per-call obligations are
// stated in SAFETY comments inside each body rather than per-fn sections.
#![allow(clippy::missing_safety_doc)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc as core_alloc;

// Bare-metal support lives behind `not(std)`: the global allocator and the
// opt-in panic handler a staticlib link requires (#558).
#[cfg(not(feature = "std"))]
mod alloc_bridge;

use core::ffi::c_int;
#[cfg(target_pointer_width = "64")]
use core::ffi::c_void;
#[cfg(target_pointer_width = "64")]
pub mod blobmap;
#[cfg(target_pointer_width = "64")]
mod compat;
#[cfg(target_pointer_width = "64")]
mod judyl_small;
#[cfg(target_pointer_width = "64")]
pub use compat::*;
pub mod modern;
// The 32-bit-only concurrent surface: `sync32` needs no `std`, so this is
// present in every 32-bit build (see the `!EXPANSE_WIDE_SURFACE` block of
// `include/expanse.h`), and in `cfg(test)` so unit tests run on 64-bit hosts.
#[cfg(any(target_pointer_width = "32", test))]
pub mod modern_sync32;

#[cfg(target_pointer_width = "64")]
use expanse_trie::ExpanseMap;
#[cfg(target_pointer_width = "64")]
use expanse_trie::ExpanseSet;
#[cfg(target_pointer_width = "64")]
use expanse_trie::strmap::ExpanseStrMap;

/// `Word_t`: pointer-width unsigned integer (see COMPAT.md doc-gap D1).
pub type Word = usize;

/// The classic `JError_t` error record (see COMPAT.md doc-gap D2).
#[repr(C)]
pub struct JError {
    /// A `JU_ERRNO_*` code.
    pub je_errno: c_int,
    /// Internal error location id (always 0 here).
    pub je_err_id: c_int,
    /// Reserved words, kept for layout compatibility.
    pub je_reserved: [Word; 4],
}

/// No error.
pub const JU_ERRNO_NONE: c_int = 0;
/// Out of memory (see COMPAT.md doc-gap D3: allocation failure aborts).
pub const JU_ERRNO_NOMEM: c_int = 2;
/// A null `PPArray` was passed where one is required.
pub const JU_ERRNO_NULLPPARRAY: c_int = 3;
/// A null `PIndex` was passed where one is required.
pub const JU_ERRNO_NULLPINDEX: c_int = 5;

/// C-compatible counts of nodes by their specific form.
#[repr(C)]
pub struct CNodeCounts {
    /// NULL edges (empty slots).
    pub null: u64,
    /// Immed edges (key bytes stored inside the edge pointer).
    pub immed: u64,
    /// Packed linear leaf nodes.
    pub leaf_linear: u64,
    /// Bitmap leaf nodes.
    pub leaf_bitmap: u64,
    /// Linear branch nodes (up to 3 children).
    pub branch_l3: u64,
    /// Linear branch nodes (up to 7 children).
    pub branch_l7: u64,
    /// Bitmap branch nodes.
    pub branch_b: u64,
    /// Uncompressed branch nodes.
    pub branch_u: u64,
    /// Full expanse edges (set-flavor only).
    pub full_expanse: u64,
}

/// C-compatible diagnostic statistics for an Expanse trie.
#[repr(C)]
pub struct CExpanseStats {
    /// Counts of nodes by their specific form.
    pub node_counts: CNodeCounts,
    /// Histogram of node depths (0 to 8).
    pub depth_histogram: [u64; 9],
    /// Histogram of leaf populations (0 to 256).
    pub leaf_pop_histogram: [u64; 257],
}

/// Defensively validates a Judy1 or JudyL array structure.
///
/// `parray` is the pointer to the array (which is `*const c_void`).
/// `is_map` should be non-zero for JudyL arrays, and zero for Judy1 arrays.
///
/// Returns 1 if the array is valid (or empty/null), 0 if any structural corruption is detected.
#[cfg(target_pointer_width = "64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_validate(parray: *const c_void, is_map: c_int) -> c_int {
    if parray.is_null() {
        return 1;
    }
    if is_map != 0 {
        // SAFETY: a non-null JudyL array word.
        unsafe {
            compat::judyl_inspect(parray, |map| c_int::from(map.validate_defensive().is_ok()))
        }
    } else {
        // SAFETY: pointer is a valid ExpanseSet.
        let set = unsafe { &*parray.cast::<ExpanseSet>() };
        c_int::from(set.validate_defensive().is_ok())
    }
}

/// Gathers structural statistics of the trie.
///
/// Returns 1 on success (with stats written to `out`), 0 on null pointer arguments.
#[cfg(target_pointer_width = "64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn expanse_stats(
    parray: *const c_void,
    is_map: c_int,
    out: *mut CExpanseStats,
) -> c_int {
    if parray.is_null() || out.is_null() {
        return 0;
    }
    let stats = if is_map != 0 {
        // SAFETY: a non-null JudyL array word.
        unsafe { compat::judyl_inspect(parray, ExpanseMap::stats) }
    } else {
        // SAFETY: pointer is a valid ExpanseSet.
        let set = unsafe { &*parray.cast::<ExpanseSet>() };
        set.stats()
    };

    // SAFETY: out checked non-null.
    unsafe {
        (*out).node_counts.null = stats.node_counts.null as u64;
        (*out).node_counts.immed = stats.node_counts.immed as u64;
        (*out).node_counts.leaf_linear = stats.node_counts.leaf_linear as u64;
        (*out).node_counts.leaf_bitmap = stats.node_counts.leaf_bitmap as u64;
        (*out).node_counts.branch_l3 = stats.node_counts.branch_l3 as u64;
        (*out).node_counts.branch_l7 = stats.node_counts.branch_l7 as u64;
        (*out).node_counts.branch_b = stats.node_counts.branch_b as u64;
        (*out).node_counts.branch_u = stats.node_counts.branch_u as u64;
        (*out).node_counts.full_expanse = stats.node_counts.full_expanse as u64;

        // Index form kept so codegen is unchanged; reviewed in #1724.
        #[allow(clippy::needless_range_loop)]
        for i in 0..9 {
            (*out).depth_histogram[i] = stats.depth_histogram[i] as u64;
        }
        // Index form kept so codegen is unchanged; reviewed in #1724.
        #[allow(clippy::needless_range_loop)]
        for i in 0..257 {
            (*out).leaf_pop_histogram[i] = stats.leaf_pop_histogram[i] as u64;
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr::null_mut;

    fn arr() -> *mut c_void {
        null_mut()
    }

    #[test]
    fn judyhs_smoke() {
        // SAFETY: valid pointers throughout; handles managed per contract.
        unsafe {
            let mut a = arr();
            let key = b"hello\0world";
            let kp = key.as_ptr().cast::<c_void>();
            let slot = JudyHSIns(&raw mut a, kp, key.len(), null_mut()).cast::<Word>();
            assert_eq!(*slot, 0);
            *slot = 777;
            // Same bytes, different buffer: hash + byte-compare match.
            let copy = key.to_vec();
            let got = JudyHSGet(a, copy.as_ptr().cast(), copy.len()).cast::<Word>();
            assert_eq!(*got, 777);
            // A prefix is a different key.
            assert!(JudyHSGet(a, kp, 5).is_null());
            // Zero-length key is a valid key (null index allowed at 0).
            let empty = JudyHSIns(&raw mut a, null_mut(), 0, null_mut()).cast::<Word>();
            *empty = 1;
            assert_eq!(*JudyHSGet(a, null_mut(), 0).cast::<Word>(), 1);
            assert_eq!(JudyHSDel(&raw mut a, kp, key.len(), null_mut()), 1);
            assert_eq!(JudyHSDel(&raw mut a, kp, key.len(), null_mut()), 0);
            assert!(JudyHSFreeArray(&raw mut a, null_mut()) > 0);
            assert!(a.is_null());
        }
    }

    #[test]
    fn judy1_smoke() {
        // SAFETY: valid pointers throughout; handles managed per contract.
        unsafe {
            let mut a = arr();
            assert_eq!(Judy1Set(&raw mut a, 42, null_mut()), 1);
            assert_eq!(Judy1Set(&raw mut a, 42, null_mut()), 0);
            assert_eq!(Judy1Set(&raw mut a, 7, null_mut()), 1);
            assert_eq!(Judy1Set(&raw mut a, usize::MAX, null_mut()), 1);
            assert_eq!(Judy1Test(a, 42, null_mut()), 1);
            assert_eq!(Judy1Test(a, 43, null_mut()), 0);
            assert_eq!(Judy1Count(a, 0, usize::MAX, null_mut()), 3);
            assert_eq!(Judy1Count(a, 8, 42, null_mut()), 1);

            let mut idx: Word = 0;
            assert_eq!(Judy1First(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 7);
            assert_eq!(Judy1Next(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 42);
            idx = usize::MAX;
            assert_eq!(Judy1Last(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, usize::MAX);
            assert_eq!(Judy1Prev(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 42);

            let mut nth: Word = 0;
            assert_eq!(Judy1ByCount(a, 2, &raw mut nth, null_mut()), 1);
            assert_eq!(nth, 42);
            assert_eq!(Judy1ByCount(a, 4, &raw mut nth, null_mut()), 0);

            idx = 7;
            assert_eq!(Judy1FirstEmpty(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 8);
            idx = usize::MAX;
            assert_eq!(Judy1LastEmpty(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, usize::MAX - 1);

            assert_eq!(Judy1Unset(&raw mut a, 43, null_mut()), 0);
            assert_eq!(Judy1Unset(&raw mut a, 42, null_mut()), 1);
            assert!(Judy1MemUsed(a) > 0);
            let freed = Judy1FreeArray(&raw mut a, null_mut());
            assert!(freed > 0);
            assert!(a.is_null());

            // Emptying by Unset also nulls the handle.
            assert_eq!(Judy1Set(&raw mut a, 5, null_mut()), 1);
            assert_eq!(Judy1Unset(&raw mut a, 5, null_mut()), 1);
            assert!(a.is_null());
        }
    }

    #[test]
    fn judyl_smoke() {
        // SAFETY: valid pointers throughout; slots written before any
        // further structural mutation, per the JudyL contract.
        unsafe {
            let mut a = arr();
            let slot = JudyLIns(&raw mut a, 100, null_mut()).cast::<Word>();
            assert!(!slot.is_null() && slot != PJERR.cast());
            assert_eq!(*slot, 0, "new slots are zero-initialized");
            *slot = 12345;

            // Re-insert keeps the existing value.
            let slot2 = JudyLIns(&raw mut a, 100, null_mut()).cast::<Word>();
            assert_eq!(*slot2, 12345);

            let got = JudyLGet(a, 100, null_mut()).cast::<Word>();
            assert_eq!(*got, 12345);
            assert!(JudyLGet(a, 101, null_mut()).is_null());

            *JudyLIns(&raw mut a, 5, null_mut()).cast::<Word>() = 55;
            *JudyLIns(&raw mut a, usize::MAX, null_mut()).cast::<Word>() = 99;

            let mut idx: Word = 0;
            let first = JudyLFirst(a, &raw mut idx, null_mut()).cast::<Word>();
            assert_eq!((idx, *first), (5, 55));
            let next = JudyLNext(a, &raw mut idx, null_mut()).cast::<Word>();
            assert_eq!((idx, *next), (100, 12345));

            assert_eq!(JudyLCount(a, 0, usize::MAX, null_mut()), 3);
            let mut nth: Word = 0;
            let bc = JudyLByCount(a, 3, &raw mut nth, null_mut()).cast::<Word>();
            assert_eq!((nth, *bc), (usize::MAX, 99));

            idx = 5;
            assert_eq!(JudyLFirstEmpty(a, &raw mut idx, null_mut()), 1);
            assert_eq!(idx, 6);

            assert_eq!(JudyLDel(&raw mut a, 100, null_mut()), 1);
            assert_eq!(JudyLDel(&raw mut a, 100, null_mut()), 0);
            let freed = JudyLFreeArray(&raw mut a, null_mut());
            assert!(freed > 0);
            assert!(a.is_null());
        }
    }

    #[test]
    fn judysl_smoke() {
        // SAFETY: valid pointers; slots written before further mutation.
        unsafe {
            let mut a = arr();
            let s1 = JudySLIns(&raw mut a, c"hello".as_ptr().cast(), null_mut()).cast::<Word>();
            *s1 = 11;
            *JudySLIns(&raw mut a, c"help".as_ptr().cast(), null_mut()).cast::<Word>() = 22;
            *JudySLIns(
                &raw mut a,
                c"a-much-longer-key-crossing-chunks".as_ptr().cast(),
                null_mut(),
            )
            .cast::<Word>() = 33;
            *JudySLIns(&raw mut a, c"".as_ptr().cast(), null_mut()).cast::<Word>() = 44;

            assert_eq!(
                *JudySLGet(a, c"hello".as_ptr().cast(), null_mut()).cast::<Word>(),
                11
            );
            assert!(JudySLGet(a, c"hell".as_ptr().cast(), null_mut()).is_null());
            // Re-insert keeps the value.
            assert_eq!(
                *JudySLIns(&raw mut a, c"hello".as_ptr().cast(), null_mut()).cast::<Word>(),
                11
            );

            // Ordered sweep with the documented buffer contract.
            let mut buf = [0u8; 64];
            let mut got: Vec<(Vec<u8>, Word)> = Vec::new();
            buf[0] = 0;
            let mut slot = JudySLFirst(a, buf.as_mut_ptr(), null_mut());
            while !slot.is_null() {
                let len = buf.iter().position(|&b| b == 0).unwrap();
                got.push((buf[..len].to_vec(), *slot.cast::<Word>()));
                slot = JudySLNext(a, buf.as_mut_ptr(), null_mut());
            }
            let expected: Vec<(Vec<u8>, Word)> = vec![
                (b"".to_vec(), 44),
                (b"a-much-longer-key-crossing-chunks".to_vec(), 33),
                (b"hello".to_vec(), 11),
                (b"help".to_vec(), 22),
            ];
            assert_eq!(got, expected, "byte-lexicographic sweep");

            // Backward from the top.
            buf[..2].copy_from_slice(b"z\0");
            let slot = JudySLLast(a, buf.as_mut_ptr(), null_mut());
            assert!(!slot.is_null());
            let len = buf.iter().position(|&b| b == 0).unwrap();
            assert_eq!(&buf[..len], b"help");

            assert_eq!(
                JudySLDel(&raw mut a, c"hello".as_ptr().cast(), null_mut()),
                1
            );
            assert_eq!(
                JudySLDel(&raw mut a, c"hello".as_ptr().cast(), null_mut()),
                0
            );
            let freed = JudySLFreeArray(&raw mut a, null_mut());
            assert!(freed > 0);
            assert!(a.is_null());
        }
    }

    #[test]
    fn null_argument_errors() {
        // SAFETY: exercising the documented error paths.
        unsafe {
            let mut err = JError {
                je_errno: JU_ERRNO_NONE,
                je_err_id: 0,
                je_reserved: [0; 4],
            };
            assert_eq!(Judy1Set(null_mut(), 1, &raw mut err), JERR_INT);
            assert_eq!(err.je_errno, JU_ERRNO_NULLPPARRAY);
            assert_eq!(JudyLIns(null_mut(), 1, &raw mut err), PJERR);
            let a = arr();
            assert_eq!(Judy1First(a, null_mut(), &raw mut err), JERR_INT);
            assert_eq!(err.je_errno, JU_ERRNO_NULLPINDEX);
        }
    }
}

/// Differential oracle against a stock libjudy, loaded with `dlopen` so
/// our exported `Judy*` symbols never collide with the reference library's
/// at link time (COMPAT.md gate G1). Enabled by the `oracle` feature; the
/// CI job installs `libjudy-dev` (locally: Homebrew `judy` on macOS).
#[cfg(all(
    test,
    feature = "oracle",
    any(target_os = "linux", target_os = "macos")
))]
mod oracle {
    use super::*;
    use core::ptr::null_mut;

    unsafe extern "C" {
        fn dlopen(filename: *const u8, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    type F2i = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> c_int;
    type F2t = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> c_int;
    type FCount = unsafe extern "C" fn(*const c_void, Word, Word, *mut c_void) -> Word;
    type FNav = unsafe extern "C" fn(*const c_void, *mut Word, *mut c_void) -> c_int;
    type FByC = unsafe extern "C" fn(*const c_void, Word, *mut Word, *mut c_void) -> c_int;
    type FFree = unsafe extern "C" fn(*mut *mut c_void, *mut c_void) -> Word;
    type FpIns = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> *mut c_void;
    type FpGet = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> *mut c_void;
    type FpNav = unsafe extern "C" fn(*const c_void, *mut Word, *mut c_void) -> *mut c_void;
    type FpByC = unsafe extern "C" fn(*const c_void, Word, *mut Word, *mut c_void) -> *mut c_void;

    struct Lib(*mut c_void);
    impl Lib {
        fn open() -> Self {
            let names: [&core::ffi::CStr; 4] = [
                c"libJudy.so.1",
                c"libJudy.so",
                c"/opt/homebrew/opt/judy/lib/libJudy.dylib",
                c"libJudy.dylib",
            ];
            for name in names.map(core::ffi::CStr::to_bytes_with_nul) {
                // SAFETY: valid NUL-terminated name.
                let h = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
                if !h.is_null() {
                    return Self(h);
                }
            }
            panic!("stock libjudy not found — install libjudy-dev");
        }
        fn sym<T: Copy>(&self, name: &core::ffi::CStr) -> T {
            // SAFETY: valid handle + NUL-terminated name; the caller names
            // the correct fn-pointer type for the symbol.
            let p = unsafe { dlsym(self.0, name.to_bytes_with_nul().as_ptr()) };
            assert!(!p.is_null(), "missing symbol {name:?}");
            // SAFETY: fn-pointer transmute of a resolved symbol.
            unsafe { core::mem::transmute_copy(&p) }
        }
    }

    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    fn keygen(rng: &mut XorShift) -> Word {
        (match rng.next() % 5 {
            0 => rng.next() % 512,
            1 => 0xAB_CD00_0000 + (rng.next() % 300),
            2 => (rng.next() % 1024) << 48,
            // 256-aligned run at a fixed deep base: exercises the
            // narrow-pointer (skip-carrying leaf) paths against stock.
            3 => 0x99_8877_6655_4400 + (rng.next() % 256),
            _ => rng.next(),
        }) as Word
    }

    type FSlIns = unsafe extern "C" fn(*mut *mut c_void, *const u8, *mut c_void) -> *mut c_void;
    type FSlDel = unsafe extern "C" fn(*mut *mut c_void, *const u8, *mut c_void) -> c_int;
    type FSlGet = unsafe extern "C" fn(*const c_void, *const u8, *mut c_void) -> *mut c_void;
    type FSlNav = unsafe extern "C" fn(*const c_void, *mut u8, *mut c_void) -> *mut c_void;

    /// Longest key the oracle generates, and therefore the sweep-buffer
    /// size. `ExpanseStrMap` is a meta-trie over 8-byte chunks, so key
    /// length *is* tree depth: 4 KiB is ~512 levels, which is deep
    /// enough to exercise the chunk-chain paths that short keys never
    /// reach, while staying well inside any reasonable stack for the
    /// stock library too (we cannot inspect its recursion depth —
    /// clean-room — so the extreme 64 KiB cases live in our own unit
    /// tests, not in a differential comparison).
    const ORACLE_MAX_KEY: usize = 4096;

    fn strgen(rng: &mut XorShift) -> std::ffi::CString {
        const PREFIXES: [&[u8]; 4] = [b"", b"user:profile:", b"shared/deep/path/", b"x"];
        let p = PREFIXES[(rng.next() % 4) as usize];
        // Mostly short keys (the common shape), with a deliberate tail of
        // long ones: without a chunk-chain of real depth this comparison
        // cannot see the class of bug that lives there.
        let len = match rng.next() % 16 {
            0 => (rng.next() as usize % (ORACLE_MAX_KEY - 64)) + 64, // deep
            1 => (rng.next() as usize % 200) + 56,                   // medium
            _ => (rng.next() % 18) as usize,                         // short
        };
        let mut k = p.to_vec();
        for _ in 0..len {
            k.push((rng.next() % 255 + 1) as u8);
        }
        k.truncate(ORACLE_MAX_KEY - 1); // leave room for the NUL
        std::ffi::CString::new(k).expect("NUL-free")
    }

    #[test]
    fn judysl_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_ins: FSlIns = lib.sym(c"JudySLIns");
        let o_del: FSlDel = lib.sym(c"JudySLDel");
        let o_get: FSlGet = lib.sym(c"JudySLGet");
        let o_first: FSlNav = lib.sym(c"JudySLFirst");
        let o_next: FSlNav = lib.sym(c"JudySLNext");
        let o_free: FFree = lib.sym(c"JudySLFreeArray");

        for seed in [0x51u64, 0x52, 0x53] {
            let mut rng = XorShift(seed | 1);
            let mut ours: *mut c_void = null_mut();
            let mut theirs: *mut c_void = null_mut();
            // SAFETY: both stacks driven per the C contract with valid
            // NUL-terminated keys and adequate navigation buffers.
            unsafe {
                for _ in 0..2500 {
                    let k = strgen(&mut rng);
                    let kp = k.as_ptr().cast::<u8>();
                    match rng.next() % 5 {
                        0..=1 => {
                            let v = rng.next() as Word;
                            let s1 = JudySLIns(&raw mut ours, kp, null_mut()).cast::<Word>();
                            let s2 = o_ins(&raw mut theirs, kp, null_mut()).cast::<Word>();
                            assert_eq!(*s1, *s2, "ins existing {k:?}");
                            *s1 = v;
                            *s2 = v;
                        }
                        2 => assert_eq!(
                            JudySLDel(&raw mut ours, kp, null_mut()),
                            o_del(&raw mut theirs, kp, null_mut()),
                            "del {k:?}"
                        ),
                        _ => {
                            let s1 = JudySLGet(ours, kp, null_mut()).cast::<Word>();
                            let s2 = o_get(theirs, kp, null_mut()).cast::<Word>();
                            assert_eq!(s1.is_null(), s2.is_null(), "get null-ness {k:?}");
                            if !s1.is_null() {
                                assert_eq!(*s1, *s2, "get value {k:?}");
                            }
                        }
                    }
                }
                // Full lexicographic sweep must agree byte for byte.
                let (mut b1, mut b2) = ([0u8; ORACLE_MAX_KEY], [0u8; ORACLE_MAX_KEY]);
                let mut s1 = JudySLFirst(ours, b1.as_mut_ptr(), null_mut());
                let mut s2 = o_first(theirs, b2.as_mut_ptr(), null_mut());
                let mut n = 0u32;
                while !s1.is_null() || !s2.is_null() {
                    assert_eq!(s1.is_null(), s2.is_null(), "sweep null-ness at {n}");
                    assert_eq!(b1, b2, "sweep key at {n}");
                    assert_eq!(*s1.cast::<Word>(), *s2.cast::<Word>(), "sweep value at {n}");
                    n += 1;
                    s1 = JudySLNext(ours, b1.as_mut_ptr(), null_mut());
                    s2 = o_next(theirs, b2.as_mut_ptr(), null_mut());
                }
                assert!(JudySLFreeArray(&raw mut ours, null_mut()) > 0);
                assert!(o_free(&raw mut theirs, null_mut()) > 0);
                assert!(ours.is_null() && theirs.is_null());
            }
        }
    }

    type FHsIns =
        unsafe extern "C" fn(*mut *mut c_void, *const c_void, Word, *mut c_void) -> *mut c_void;
    type FHsDel = unsafe extern "C" fn(*mut *mut c_void, *const c_void, Word, *mut c_void) -> c_int;
    type FHsGet = unsafe extern "C" fn(*const c_void, *const c_void, Word) -> *mut c_void;

    fn bytesgen(rng: &mut XorShift) -> Vec<u8> {
        // Arbitrary bytes (NULs included), lengths 0..=32 with repeats.
        let len = (rng.next() % 33) as usize;
        (0..len).map(|_| (rng.next() % 5) as u8 * 0x3F).collect()
    }

    #[test]
    fn judyhs_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_ins: FHsIns = lib.sym(c"JudyHSIns");
        let o_del: FHsDel = lib.sym(c"JudyHSDel");
        let o_get: FHsGet = lib.sym(c"JudyHSGet");
        let o_free: FFree = lib.sym(c"JudyHSFreeArray");

        for seed in [0x61u64, 0x62, 0x63] {
            let mut rng = XorShift(seed | 1);
            let mut ours: *mut c_void = null_mut();
            let mut theirs: *mut c_void = null_mut();
            // SAFETY: both stacks driven per the C contract with valid
            // (pointer, length) byte strings.
            unsafe {
                for _ in 0..2500 {
                    let k = bytesgen(&mut rng);
                    let (kp, kl) = (k.as_ptr().cast::<c_void>(), k.len() as Word);
                    match rng.next() % 5 {
                        0..=1 => {
                            let v = rng.next() as Word;
                            let s1 = JudyHSIns(&raw mut ours, kp, kl, null_mut()).cast::<Word>();
                            let s2 = o_ins(&raw mut theirs, kp, kl, null_mut()).cast::<Word>();
                            assert_eq!(*s1, *s2, "ins existing {k:02x?}");
                            *s1 = v;
                            *s2 = v;
                        }
                        2 => assert_eq!(
                            JudyHSDel(&raw mut ours, kp, kl, null_mut()),
                            o_del(&raw mut theirs, kp, kl, null_mut()),
                            "del {k:02x?}"
                        ),
                        _ => {
                            let s1 = JudyHSGet(ours, kp, kl).cast::<Word>();
                            let s2 = o_get(theirs, kp, kl).cast::<Word>();
                            assert_eq!(s1.is_null(), s2.is_null(), "get null-ness {k:02x?}");
                            if !s1.is_null() {
                                assert_eq!(*s1, *s2, "get value {k:02x?}");
                            }
                        }
                    }
                }
                // JudyHS has no navigation surface; the byte totals of
                // FreeArray are implementation-defined (doc-gap D4) —
                // only emptiness semantics must agree.
                assert!(JudyHSFreeArray(&raw mut ours, null_mut()) > 0);
                assert!(o_free(&raw mut theirs, null_mut()) > 0);
                assert!(ours.is_null() && theirs.is_null());
            }
        }
    }

    /// Black-box probe (#1681, docs/COMPAT.md D10): does a JudyHS value slot
    /// stay at one address across unrelated inserts? Each library takes a
    /// probe key, then 20,000 other keys of the same length class, and the
    /// probe key's slot address is compared before and after. Prints both
    /// libraries' answer per class; asserts only that the value survives.
    #[test]
    fn judyhs_slot_address_across_unrelated_inserts() {
        let lib = Lib::open();
        let o_ins: FHsIns = lib.sym(c"JudyHSIns");
        let o_get: FHsGet = lib.sym(c"JudyHSGet");
        let o_free: FFree = lib.sym(c"JudyHSFreeArray");
        unsafe extern "C" fn our_ins(
            pp: *mut *mut c_void,
            k: *const c_void,
            n: Word,
            pj: *mut c_void,
        ) -> *mut c_void {
            // SAFETY: forwarded unchanged.
            unsafe { JudyHSIns(pp, k, n, pj.cast()) }
        }
        unsafe extern "C" fn our_get(p: *const c_void, k: *const c_void, n: Word) -> *mut c_void {
            // SAFETY: forwarded unchanged.
            unsafe { JudyHSGet(p, k, n) }
        }
        for (class, len) in [("8-byte", 8usize), ("40-byte", 40)] {
            let key = |i: u64| -> Vec<u8> {
                let mut k = vec![0u8; len];
                k[len - 8..].copy_from_slice(&i.to_be_bytes());
                k
            };
            let mut moved = Vec::new();
            let libs: [(&str, FHsIns, FHsGet); 2] =
                [("libexpanse", our_ins, our_get), ("stock", o_ins, o_get)];
            for (name, ins, get) in libs {
                let mut arr: *mut c_void = null_mut();
                let probe = key(u64::MAX / 3);
                // SAFETY: standard JudyHS usage with live byte strings; the
                // slot address is compared, and dereferenced only through a
                // fresh JudyHSGet.
                unsafe {
                    let p0 = ins(&raw mut arr, probe.as_ptr().cast(), len as Word, null_mut());
                    *p0.cast::<Word>() = 0xC0FFEE;
                    for i in 0..20_000u64 {
                        let k = key(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                        ins(&raw mut arr, k.as_ptr().cast(), len as Word, null_mut());
                    }
                    let p1 = get(arr, probe.as_ptr().cast(), len as Word);
                    assert_eq!(*p1.cast::<Word>(), 0xC0FFEE, "{name} {class}: value lost");
                    moved.push((name, p0 != p1));
                    if name == "stock" {
                        o_free(&raw mut arr, null_mut());
                    } else {
                        JudyHSFreeArray(&raw mut arr, null_mut());
                    }
                }
            }
            println!("JudyHS slot moved across 20,000 unrelated inserts ({class} keys): {moved:?}");
        }
    }

    /// zmap's dedup window (#1672): JudyHS as a fixed-capacity LRU index.
    /// Each arrival is tested with `JudyHSGet`; a miss evicts the oldest key
    /// with `JudyHSDel` once the window is full and inserts the arrival, so
    /// evicted keys recur and are inserted again. Both libraries run the same
    /// sequence in lockstep against one model, and every return is compared:
    /// `Get` presence and value, `Del`'s 0/1, the fresh slot `Ins` hands
    /// back, and whether the array word is null. Key shapes: zmap's 8-byte
    /// `target_t {ip, port, status}` (libexpanse's short-key tries), 0..=8
    /// byte keys differing only in trailing zeros, and 16- and 40-byte keys
    /// (the hashed path). The integration test
    /// `tests/test_judyhs_lru.rs` runs the same pattern on libexpanse alone.
    #[test]
    fn judyhs_lru_evict_reinsert_matches_stock_libjudy() {
        use std::collections::{BTreeMap, VecDeque};
        let lib = Lib::open();
        let o_ins: FHsIns = lib.sym(c"JudyHSIns");
        let o_del: FHsDel = lib.sym(c"JudyHSDel");
        let o_get: FHsGet = lib.sym(c"JudyHSGet");
        let o_free: FFree = lib.sym(c"JudyHSFreeArray");

        fn target_t(i: u64) -> Vec<u8> {
            let ip = 0x0A00_0000u32 + (i / 3) as u32;
            let port = [80u16, 443, 0][(i % 3) as usize];
            let status = u16::from(i.is_multiple_of(7));
            let mut k = ip.to_ne_bytes().to_vec();
            k.extend_from_slice(&port.to_ne_bytes());
            k.extend_from_slice(&status.to_ne_bytes());
            k
        }
        fn zero_padded(i: u64) -> Vec<u8> {
            let Some(j) = i.checked_sub(1) else {
                return Vec::new();
            };
            let mut k = vec![0u8; 1 + (j % 8) as usize];
            k[0] = (j / 8) as u8;
            k
        }
        fn target_t_16(i: u64) -> Vec<u8> {
            let mut k = target_t(i);
            k.resize(16, 0);
            k
        }
        fn wide_40(i: u64) -> Vec<u8> {
            let mut k = vec![0u8; 40];
            k[32..].copy_from_slice(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_be_bytes());
            k
        }
        /// Every slot read and write goes through these two, so the null
        /// check sits with the dereference. `p` is null or a live slot with
        /// no change to its array since the call that returned it.
        unsafe fn read(p: *mut c_void) -> Option<Word> {
            assert_ne!(p as usize, usize::MAX, "PJERR returned");
            // SAFETY: per the contract above; `as_ref` maps null to `None`.
            unsafe { p.cast::<Word>().as_ref().copied() }
        }
        unsafe fn write(p: *mut c_void, v: Word) {
            assert_ne!(p as usize, usize::MAX, "PJERR returned");
            // SAFETY: per the contract above; `as_mut` maps null to `None`.
            let slot = unsafe { p.cast::<Word>().as_mut() }.expect("JudyHSIns returned null");
            *slot = v;
        }
        type KeyFn = fn(u64) -> Vec<u8>;
        let shapes: [(&str, KeyFn, u64); 4] = [
            ("target_t", target_t, 3000),
            ("zero_padded", zero_padded, 1 + 8 * 256),
            ("target_t_16", target_t_16, 3000),
            ("wide_40", wide_40, 3000),
        ];
        for (shape, key, pool) in shapes {
            let keys: Vec<Vec<u8>> = (0..pool).map(key).collect();
            let wide = keys.len() / 3;
            for (capacity, seed) in [(1usize, 0x1672u64), (37, 0x1673), (wide, 0x1674)] {
                let mut rng = XorShift(seed | 1);
                let mut ours: *mut c_void = null_mut();
                let mut theirs: *mut c_void = null_mut();
                let mut model: BTreeMap<&[u8], Word> = BTreeMap::new();
                let mut order: VecDeque<&[u8]> = VecDeque::new();
                let mut reinserts = 0usize;
                let mut evicted: BTreeMap<&[u8], ()> = BTreeMap::new();
                let at = |s: &str, step: usize| format!("{shape} cap {capacity} step {step}: {s}");
                // SAFETY: both stacks driven per the C contract with live
                // (pointer, length) byte strings; a slot pointer is used
                // only before the next call that changes its array.
                unsafe {
                    for step in 0..20_000usize {
                        let k: &[u8] = &keys[(rng.next() % pool) as usize];
                        let (kp, kl) = (k.as_ptr().cast::<c_void>(), k.len() as Word);
                        let g1 = read(JudyHSGet(ours, kp, kl));
                        let g2 = read(o_get(theirs, kp, kl));
                        assert_eq!(g1, g2, "{}", at("get", step));
                        let want = model.get(k).copied();
                        assert_eq!(g1, want, "{}", at("get vs model", step));
                        if want.is_some() {
                            continue;
                        }
                        if order.len() == capacity {
                            let old = order.pop_front().expect("window is full");
                            let (op, ol) = (old.as_ptr().cast::<c_void>(), old.len() as Word);
                            let d1 = JudyHSDel(&raw mut ours, op, ol, null_mut());
                            let d2 = o_del(&raw mut theirs, op, ol, null_mut());
                            assert_eq!((d1, d2), (1, 1), "{}", at("evict", step));
                            assert_eq!(ours.is_null(), theirs.is_null(), "{}", at("word", step));
                            let r1 = JudyHSDel(&raw mut ours, op, ol, null_mut());
                            let r2 = o_del(&raw mut theirs, op, ol, null_mut());
                            assert_eq!((r1, r2), (0, 0), "{}", at("re-delete", step));
                            model.remove(old);
                            evicted.insert(old, ());
                        }
                        if evicted.remove(k).is_some() {
                            reinserts += 1;
                        }
                        let s1 = JudyHSIns(&raw mut ours, kp, kl, null_mut());
                        let s2 = o_ins(&raw mut theirs, kp, kl, null_mut());
                        let fresh = (read(s1), read(s2));
                        assert_eq!(fresh, (Some(0), Some(0)), "{}", at("fresh slot", step));
                        let v = (step as Word) << 1 | 1;
                        write(s1, v);
                        write(s2, v);
                        model.insert(k, v);
                        order.push_back(k);
                    }
                    assert!(reinserts > 0, "{shape} cap {capacity}: no re-insert");
                    // Drain oldest first: both words empty to null together.
                    while let Some(old) = order.pop_front() {
                        let (op, ol) = (old.as_ptr().cast::<c_void>(), old.len() as Word);
                        let d1 = JudyHSDel(&raw mut ours, op, ol, null_mut());
                        let d2 = o_del(&raw mut theirs, op, ol, null_mut());
                        assert_eq!((d1, d2), (1, 1), "{shape} cap {capacity}: drain");
                        assert_eq!(ours.is_null(), theirs.is_null(), "{shape}: drain word");
                    }
                    assert!(ours.is_null() && theirs.is_null(), "{shape}: emptied word");
                    // Refill and free whole; byte totals are D4, only
                    // emptiness semantics are compared.
                    for k in keys.iter().take(capacity) {
                        let (kp, kl) = (k.as_ptr().cast::<c_void>(), k.len() as Word);
                        write(JudyHSIns(&raw mut ours, kp, kl, null_mut()), 7);
                        write(o_ins(&raw mut theirs, kp, kl, null_mut()), 7);
                    }
                    assert!(JudyHSFreeArray(&raw mut ours, null_mut()) > 0);
                    assert!(o_free(&raw mut theirs, null_mut()) > 0);
                    assert!(ours.is_null() && theirs.is_null());
                    assert_eq!(JudyHSFreeArray(&raw mut ours, null_mut()), 0);
                    assert_eq!(o_free(&raw mut theirs, null_mut()), 0);
                }
            }
        }
    }

    #[test]
    fn judy1_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_set: F2i = lib.sym(c"Judy1Set");
        let o_unset: F2i = lib.sym(c"Judy1Unset");
        let o_test: F2t = lib.sym(c"Judy1Test");
        let o_count: FCount = lib.sym(c"Judy1Count");
        let o_byc: FByC = lib.sym(c"Judy1ByCount");
        let o_first: FNav = lib.sym(c"Judy1First");
        let o_next: FNav = lib.sym(c"Judy1Next");
        let _o_last: FNav = lib.sym(c"Judy1Last");
        let o_prev: FNav = lib.sym(c"Judy1Prev");
        let o_fe: FNav = lib.sym(c"Judy1FirstEmpty");
        let o_le: FNav = lib.sym(c"Judy1LastEmpty");
        let o_free: FFree = lib.sym(c"Judy1FreeArray");

        for seed in [0xA1u64, 0xB2, 0xC3] {
            let mut rng = XorShift(seed | 1);
            let mut ours: *mut c_void = null_mut();
            let mut theirs: *mut c_void = null_mut();
            // SAFETY: both stacks driven through their C contracts with
            // valid pointers; sequences identical on both sides.
            unsafe {
                for _ in 0..4000 {
                    let k = keygen(&mut rng);
                    match rng.next() % 8 {
                        0..=2 => assert_eq!(
                            Judy1Set(&raw mut ours, k, null_mut()),
                            o_set(&raw mut theirs, k, null_mut()),
                            "set {k:#x}"
                        ),
                        3 => assert_eq!(
                            Judy1Unset(&raw mut ours, k, null_mut()),
                            o_unset(&raw mut theirs, k, null_mut()),
                            "unset {k:#x}"
                        ),
                        4 => assert_eq!(
                            Judy1Test(ours, k, null_mut()),
                            o_test(theirs, k, null_mut()),
                            "test {k:#x}"
                        ),
                        5 => {
                            let k2 = keygen(&mut rng);
                            let (a, b) = (k.min(k2), k.max(k2));
                            assert_eq!(
                                Judy1Count(ours, a, b, null_mut()),
                                o_count(theirs, a, b, null_mut()),
                                "count {a:#x}..={b:#x}"
                            );
                        }
                        6 => {
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1Next(ours, &raw mut i1, null_mut());
                            let r2 = o_next(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, i2), "next {k:#x}");
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1Prev(ours, &raw mut i1, null_mut());
                            let r2 = o_prev(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, i2), "prev {k:#x}");
                        }
                        _ => {
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1FirstEmpty(ours, &raw mut i1, null_mut());
                            let r2 = o_fe(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, i2), "fe {k:#x}");
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1LastEmpty(ours, &raw mut i1, null_mut());
                            let r2 = o_le(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, i2), "le {k:#x}");
                        }
                    }
                }
                // Full ordered sweep must agree in both directions.
                let (mut i1, mut i2) = (0 as Word, 0 as Word);
                let (mut r1, mut r2) = (
                    Judy1First(ours, &raw mut i1, null_mut()),
                    o_first(theirs, &raw mut i2, null_mut()),
                );
                let mut n = 0u64;
                while r1 == 1 || r2 == 1 {
                    assert_eq!((r1, i1), (r2, i2), "sweep step {n}");
                    n += 1;
                    r1 = Judy1Next(ours, &raw mut i1, null_mut());
                    r2 = o_next(theirs, &raw mut i2, null_mut());
                }
                // Rank agreement over the whole population.
                for nth in [1 as Word, 2, n as Word / 2, n as Word] {
                    let (mut i1, mut i2) = (0 as Word, 0 as Word);
                    let r1 = Judy1ByCount(ours, nth, &raw mut i1, null_mut());
                    let r2 = o_byc(theirs, nth, &raw mut i2, null_mut());
                    assert_eq!(
                        (r1, i1),
                        (r2, if r2 == 1 { i2 } else { i1 }),
                        "bycount {nth}"
                    );
                }
                assert!(Judy1FreeArray(&raw mut ours, null_mut()) > 0);
                assert!(o_free(&raw mut theirs, null_mut()) > 0);
                assert!(ours.is_null() && theirs.is_null());
            }
        }
    }

    /// The small form of a JudyL array and its promotion at 32 keys, against
    /// stock: every entry point is compared after each insert from 1 to 40
    /// keys, after each delete back to empty, and while the population
    /// crosses 31/32 in both directions.
    #[test]
    fn judyl_small_form_boundary_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_ins: FpIns = lib.sym(c"JudyLIns");
        let o_del: F2i = lib.sym(c"JudyLDel");
        let o_get: FpGet = lib.sym(c"JudyLGet");
        let o_count: FCount = lib.sym(c"JudyLCount");
        let o_byc: FpByC = lib.sym(c"JudyLByCount");
        let o_free: FFree = lib.sym(c"JudyLFreeArray");
        let o_nav: [FpNav; 4] = [
            lib.sym(c"JudyLFirst"),
            lib.sym(c"JudyLNext"),
            lib.sym(c"JudyLLast"),
            lib.sym(c"JudyLPrev"),
        ];
        let o_empty: [FNav; 4] = [
            lib.sym(c"JudyLFirstEmpty"),
            lib.sym(c"JudyLNextEmpty"),
            lib.sym(c"JudyLLastEmpty"),
            lib.sym(c"JudyLPrevEmpty"),
        ];
        type Nav = unsafe extern "C" fn(*const c_void, *mut Word, *mut JError) -> *mut c_void;
        type Empty = unsafe extern "C" fn(*const c_void, *mut Word, *mut JError) -> c_int;
        let nav: [Nav; 4] = [JudyLFirst, JudyLNext, JudyLLast, JudyLPrev];
        let empty: [Empty; 4] = [
            JudyLFirstEmpty,
            JudyLNextEmpty,
            JudyLLastEmpty,
            JudyLPrevEmpty,
        ];

        // Every read-only entry point, on both arrays, over probes at and
        // beside each present key and at both ends of the key space.
        let compare = |ours: *const c_void, theirs: *const c_void, keys: &[Word], at: &str| {
            let mut probes = vec![0, 1, Word::MAX - 1, Word::MAX];
            for &k in keys {
                probes.extend([k.wrapping_sub(1), k, k.wrapping_add(1)]);
            }
            // SAFETY: both arrays are live and neither is mutated here; a
            // returned slot is read before the next call.
            unsafe {
                for &p in &probes {
                    let (s1, s2) = (
                        JudyLGet(ours, p, null_mut()).cast::<Word>(),
                        o_get(theirs, p, null_mut()).cast::<Word>(),
                    );
                    assert_eq!(s1.is_null(), s2.is_null(), "{at}: get {p:#x}");
                    if !s1.is_null() {
                        assert_eq!(*s1, *s2, "{at}: get value {p:#x}");
                    }
                    for f in 0..4 {
                        let (mut i1, mut i2) = (p, p);
                        let s1 = nav[f](ours, &raw mut i1, null_mut()).cast::<Word>();
                        let s2 = o_nav[f](theirs, &raw mut i2, null_mut()).cast::<Word>();
                        assert_eq!(
                            (s1.is_null(), i1),
                            (s2.is_null(), i2),
                            "{at}: nav {f} from {p:#x}"
                        );
                        if !s1.is_null() {
                            assert_eq!(*s1, *s2, "{at}: nav {f} value from {p:#x}");
                        }
                        let (mut i1, mut i2) = (p, p);
                        let r1 = empty[f](ours, &raw mut i1, null_mut());
                        let r2 = o_empty[f](theirs, &raw mut i2, null_mut());
                        assert_eq!((r1, i1), (r2, i2), "{at}: empty {f} from {p:#x}");
                    }
                    for &q in &[0, p, Word::MAX] {
                        assert_eq!(
                            JudyLCount(ours, p.min(q), p.max(q), null_mut()),
                            o_count(theirs, p.min(q), p.max(q), null_mut()),
                            "{at}: count {p:#x} {q:#x}"
                        );
                    }
                }
                for nth in 0..=keys.len() as Word + 1 {
                    let (mut i1, mut i2) = (7 as Word, 7 as Word);
                    let s1 = JudyLByCount(ours, nth, &raw mut i1, null_mut()).cast::<Word>();
                    let s2 = o_byc(theirs, nth, &raw mut i2, null_mut()).cast::<Word>();
                    assert_eq!(s1.is_null(), s2.is_null(), "{at}: bycount {nth}");
                    if !s1.is_null() {
                        assert_eq!((i1, *s1), (i2, *s2), "{at}: bycount {nth}");
                    }
                }
            }
        };

        type Shape = (&'static str, fn(usize) -> Word);
        let shapes: [Shape; 4] = [
            ("dense", |i| i as Word),
            ("spread", |i| {
                (i as Word + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            }),
            ("top", |i| Word::MAX - 3 * i as Word),
            ("run", |i| 0x99_8877_6655_4400 + (i as Word * 5) % 256),
        ];
        for (shape, key) in shapes {
            let mut ours: *mut c_void = null_mut();
            let mut theirs: *mut c_void = null_mut();
            let mut keys: Vec<Word> = Vec::new();
            // SAFETY: standard JudyL usage on both libraries; each slot is
            // written before the next call that mutates its array.
            unsafe {
                let put = |ours: &mut *mut c_void, theirs: &mut *mut c_void, k: Word| {
                    let s1 = JudyLIns(ours, k, null_mut()).cast::<Word>();
                    let s2 = o_ins(theirs, k, null_mut()).cast::<Word>();
                    if s1.is_null() || s2.is_null() || s1 == PJERR.cast() || s2 == PJERR.cast() {
                        panic!("{shape}: ins {k:#x} returned no slot");
                    }
                    assert_eq!(*s1, *s2, "{shape}: ins existing value {k:#x}");
                    *s1 = !k;
                    *s2 = !k;
                };
                let del = |ours: &mut *mut c_void, theirs: &mut *mut c_void, k: Word| {
                    assert_eq!(
                        JudyLDel(ours, k, null_mut()),
                        o_del(theirs, k, null_mut()),
                        "{shape}: del {k:#x}"
                    );
                    assert_eq!(ours.is_null(), theirs.is_null(), "{shape}: null after del");
                };
                compare(ours, theirs, &keys, shape);
                for i in 0..40 {
                    let k = key(i);
                    put(&mut ours, &mut theirs, k);
                    keys.push(k);
                    // A second insert of a present key, at every population.
                    put(&mut ours, &mut theirs, keys[i / 2]);
                    compare(ours, theirs, &keys, &format!("{shape} up {}", i + 1));
                }
                // Down through 32 and 31, in an order unlike the insertion's.
                keys.rotate_left(13);
                while keys.len() > 28 {
                    let k = keys.pop().unwrap();
                    del(&mut ours, &mut theirs, k);
                    del(&mut ours, &mut theirs, k);
                    compare(ours, theirs, &keys, &format!("{shape} down {}", keys.len()));
                }
                // Back up across the boundary, then down to empty.
                for i in 40..46 {
                    let k = key(i);
                    put(&mut ours, &mut theirs, k);
                    keys.push(k);
                    compare(
                        ours,
                        theirs,
                        &keys,
                        &format!("{shape} again {}", keys.len()),
                    );
                }
                while let Some(k) = keys.pop() {
                    del(&mut ours, &mut theirs, k);
                    compare(
                        ours,
                        theirs,
                        &keys,
                        &format!("{shape} drain {}", keys.len()),
                    );
                }
                assert!(ours.is_null() && theirs.is_null(), "{shape}: emptied");
                // A fresh small array, freed while small.
                for i in 0..31 {
                    put(&mut ours, &mut theirs, key(i));
                }
                assert!(JudyLFreeArray(&raw mut ours, null_mut()) > 0);
                assert!(o_free(&raw mut theirs, null_mut()) > 0);
                assert!(ours.is_null() && theirs.is_null());
            }
        }
    }

    #[test]
    fn judyl_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_ins: FpIns = lib.sym(c"JudyLIns");
        let o_del: F2i = lib.sym(c"JudyLDel");
        let o_get: FpGet = lib.sym(c"JudyLGet");
        let o_count: FCount = lib.sym(c"JudyLCount");
        let o_byc: FpByC = lib.sym(c"JudyLByCount");
        let o_first: FpNav = lib.sym(c"JudyLFirst");
        let o_next: FpNav = lib.sym(c"JudyLNext");
        let o_free: FFree = lib.sym(c"JudyLFreeArray");

        for seed in [0x11u64, 0x22, 0x33] {
            let mut rng = XorShift(seed | 1);
            let mut ours: *mut c_void = null_mut();
            let mut theirs: *mut c_void = null_mut();
            // SAFETY: as in the Judy1 oracle; slots written immediately
            // after the call that returned them.
            unsafe {
                for _ in 0..4000 {
                    let k = keygen(&mut rng);
                    match rng.next() % 6 {
                        0..=2 => {
                            let v = rng.next() as Word;
                            let s1 = JudyLIns(&raw mut ours, k, null_mut()).cast::<Word>();
                            let s2 = o_ins(&raw mut theirs, k, null_mut()).cast::<Word>();
                            assert_eq!(*s1, *s2, "ins existing value {k:#x}");
                            *s1 = v;
                            *s2 = v;
                        }
                        3 => assert_eq!(
                            JudyLDel(&raw mut ours, k, null_mut()),
                            o_del(&raw mut theirs, k, null_mut()),
                            "del {k:#x}"
                        ),
                        4 => {
                            let s1 = JudyLGet(ours, k, null_mut()).cast::<Word>();
                            let s2 = o_get(theirs, k, null_mut()).cast::<Word>();
                            assert_eq!(s1.is_null(), s2.is_null(), "get null-ness {k:#x}");
                            if !s1.is_null() {
                                assert_eq!(*s1, *s2, "get value {k:#x}");
                            }
                        }
                        _ => {
                            let k2 = keygen(&mut rng);
                            let (a, b) = (k.min(k2), k.max(k2));
                            assert_eq!(
                                JudyLCount(ours, a, b, null_mut()),
                                o_count(theirs, a, b, null_mut()),
                                "count {a:#x}..={b:#x}"
                            );
                        }
                    }
                }
                // Ordered sweep with value agreement.
                let (mut i1, mut i2) = (0 as Word, 0 as Word);
                let (mut s1, mut s2) = (
                    JudyLFirst(ours, &raw mut i1, null_mut()),
                    o_first(theirs, &raw mut i2, null_mut()),
                );
                let mut n: Word = 0;
                while !s1.is_null() || !s2.is_null() {
                    assert_eq!(s1.is_null(), s2.is_null(), "sweep null-ness at {n}");
                    assert_eq!(i1, i2, "sweep key at {n}");
                    assert_eq!(*s1.cast::<Word>(), *s2.cast::<Word>(), "sweep value at {n}");
                    n += 1;
                    s1 = JudyLNext(ours, &raw mut i1, null_mut());
                    s2 = o_next(theirs, &raw mut i2, null_mut());
                }
                for nth in [1 as Word, n / 2, n] {
                    let (mut i1, mut i2) = (0 as Word, 0 as Word);
                    let s1 = o_byc(theirs, nth, &raw mut i2, null_mut());
                    let s2 = JudyLByCount(ours, nth, &raw mut i1, null_mut());
                    assert_eq!(s2.is_null(), s1.is_null(), "bycount null-ness {nth}");
                    if !s1.is_null() {
                        assert_eq!(i1, i2, "bycount key {nth}");
                        assert_eq!(
                            *s2.cast::<Word>(),
                            *s1.cast::<Word>(),
                            "bycount value {nth}"
                        );
                    }
                }
                assert!(JudyLFreeArray(&raw mut ours, null_mut()) > 0);
                assert!(o_free(&raw mut theirs, null_mut()) > 0);
                assert!(ours.is_null() && theirs.is_null());
            }
        }
    }

    /// What `First`/`Next`/`Last`/`Prev` leave in `*PIndex` on a miss, for
    /// Judy1 and JudyL. The man pages define `*PIndex` only on a hit; this
    /// compares the unmasked `(found, index)` pair of every call, so the
    /// miss path is settled here (COMPAT.md D8): on a non-empty array stock
    /// writes one step past the probe in the search direction, wrapping,
    /// and on a null array it writes nothing. The arrays include one
    /// emptied back to null by removal, and probes at both ends of the key
    /// space, where the step wraps.
    #[test]
    fn nav_miss_index_matches_stock_libjudy() {
        let lib = Lib::open();
        let o_set: F2i = lib.sym(c"Judy1Set");
        let o_unset: F2i = lib.sym(c"Judy1Unset");
        let o_free1: FFree = lib.sym(c"Judy1FreeArray");
        let o_ins: FpIns = lib.sym(c"JudyLIns");
        let o_del: F2i = lib.sym(c"JudyLDel");
        let o_freel: FFree = lib.sym(c"JudyLFreeArray");
        let o_nav1: [FNav; 4] = [
            lib.sym(c"Judy1First"),
            lib.sym(c"Judy1Next"),
            lib.sym(c"Judy1Last"),
            lib.sym(c"Judy1Prev"),
        ];
        let o_navl: [FpNav; 4] = [
            lib.sym(c"JudyLFirst"),
            lib.sym(c"JudyLNext"),
            lib.sym(c"JudyLLast"),
            lib.sym(c"JudyLPrev"),
        ];
        unsafe extern "C" fn adapt1<const F: usize>(
            p: *const c_void,
            i: *mut Word,
            pj: *mut c_void,
        ) -> c_int {
            // SAFETY: forwarded unchanged; `pj` is null or a live JError.
            unsafe {
                match F {
                    0 => Judy1First(p, i, pj.cast()),
                    1 => Judy1Next(p, i, pj.cast()),
                    2 => Judy1Last(p, i, pj.cast()),
                    _ => Judy1Prev(p, i, pj.cast()),
                }
            }
        }
        unsafe extern "C" fn adaptl<const F: usize>(
            p: *const c_void,
            i: *mut Word,
            pj: *mut c_void,
        ) -> *mut c_void {
            // SAFETY: as above.
            unsafe {
                match F {
                    0 => JudyLFirst(p, i, pj.cast()),
                    1 => JudyLNext(p, i, pj.cast()),
                    2 => JudyLLast(p, i, pj.cast()),
                    _ => JudyLPrev(p, i, pj.cast()),
                }
            }
        }
        let our_nav1: [FNav; 4] = [adapt1::<0>, adapt1::<1>, adapt1::<2>, adapt1::<3>];
        let our_navl: [FpNav; 4] = [adaptl::<0>, adaptl::<1>, adaptl::<2>, adaptl::<3>];

        let shapes: [&[Word]; 6] = [
            &[],
            &[100],
            &[100, 200, 300],
            &[0],
            &[Word::MAX],
            &[0x99_8877_6655_4400, 0x99_8877_6655_4410],
        ];
        let mut probes: Vec<Word> = vec![0, 1, 99, 100, 101, 199, 200, 201, 299, 300, 301];
        probes.extend([1 << 32, (1 << 63) - 1, 1 << 63, Word::MAX - 1, Word::MAX]);
        probes.extend([
            0x99_8877_6655_43FF,
            0x99_8877_6655_4405,
            0x99_8877_6655_4411,
        ]);
        let mut rng = XorShift(0x5EED_D8D8);
        // A larger array of every key shape, so a miss can end at any depth.
        let wide: Vec<Word> = (0..3000).map(|_| keygen(&mut rng)).collect();
        probes.extend((0..400).map(|_| keygen(&mut rng)));

        // One transcript per library: (shape, probe, function, found, index).
        let transcript = |set: [F2i; 2],
                          ins: FpIns,
                          del: F2i,
                          free: [FFree; 2],
                          nav1: [FNav; 4],
                          navl: [FpNav; 4]| {
            let mut out = Vec::new();
            let all = shapes.iter().copied().chain([wide.as_slice()]);
            for (shape, keys) in all.enumerate() {
                let (mut a1, mut al) = (null_mut::<c_void>(), null_mut::<c_void>());
                // SAFETY: both arrays are owned here and driven through
                // their C contracts; each slot is written right after the
                // call that returned it.
                unsafe {
                    for &k in keys {
                        set[0](&raw mut a1, k, null_mut());
                        *ins(&raw mut al, k, null_mut()).cast::<Word>() = k;
                    }
                    for &p in &probes {
                        for f in 0..4 {
                            let mut i = p;
                            let r = nav1[f](a1, &raw mut i, null_mut());
                            out.push((shape, p, f, r == 1, i));
                            let mut i = p;
                            let r = navl[f](al, &raw mut i, null_mut());
                            out.push((shape, p, 4 + f, !r.is_null(), i));
                        }
                    }
                    if shape == 1 {
                        // Emptied back to null by removal, then probed again.
                        assert_eq!(set[1](&raw mut a1, 100, null_mut()), 1);
                        assert_eq!(del(&raw mut al, 100, null_mut()), 1);
                        assert!(a1.is_null() && al.is_null(), "emptied array is null");
                        for &p in &probes {
                            for f in 0..4 {
                                let mut i = p;
                                let r = nav1[f](a1, &raw mut i, null_mut());
                                out.push((99, p, f, r == 1, i));
                                let mut i = p;
                                let r = navl[f](al, &raw mut i, null_mut());
                                out.push((99, p, 4 + f, !r.is_null(), i));
                            }
                        }
                    }
                    free[0](&raw mut a1, null_mut());
                    free[1](&raw mut al, null_mut());
                }
            }
            out
        };
        unsafe extern "C" fn our_set(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
            // SAFETY: forwarded unchanged.
            unsafe { Judy1Set(p, k, pj.cast()) }
        }
        unsafe extern "C" fn our_unset(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
            // SAFETY: forwarded unchanged.
            unsafe { Judy1Unset(p, k, pj.cast()) }
        }
        unsafe extern "C" fn our_ins(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> *mut c_void {
            // SAFETY: forwarded unchanged.
            unsafe { JudyLIns(p, k, pj.cast()) }
        }
        unsafe extern "C" fn our_del(p: *mut *mut c_void, k: Word, pj: *mut c_void) -> c_int {
            // SAFETY: forwarded unchanged.
            unsafe { JudyLDel(p, k, pj.cast()) }
        }
        unsafe extern "C" fn our_free1(p: *mut *mut c_void, pj: *mut c_void) -> Word {
            // SAFETY: forwarded unchanged.
            unsafe { Judy1FreeArray(p, pj.cast()) }
        }
        unsafe extern "C" fn our_freel(p: *mut *mut c_void, pj: *mut c_void) -> Word {
            // SAFETY: forwarded unchanged.
            unsafe { JudyLFreeArray(p, pj.cast()) }
        }
        let ours = transcript(
            [our_set, our_unset],
            our_ins,
            our_del,
            [our_free1, our_freel],
            our_nav1,
            our_navl,
        );
        let theirs = transcript(
            [o_set, o_unset],
            o_ins,
            o_del,
            [o_free1, o_freel],
            o_nav1,
            o_navl,
        );
        assert_eq!(ours.len(), theirs.len());
        let misses = theirs.iter().filter(|t| !t.3).count();
        assert!(
            misses > 1000,
            "the probes must reach the miss path: {misses}"
        );
        for (o, t) in ours.iter().zip(&theirs) {
            assert_eq!(
                o, t,
                "(shape, probe, fn 0-3 Judy1 F/N/L/P 4-7 JudyL, found, index)"
            );
        }
    }

    /// What `Judy1Count`/`JudyLCount` write to `*PJError`, case by case.
    /// The man pages (`Judy1_funcs(3)`) document only that a 0 return
    /// with `JU_ERRNO_NONE` is a real empty count; whether a nonzero count
    /// or a reversed range writes it is settled here (COMPAT.md D7).
    /// Each call starts from a sentinel errno, so "left untouched" and
    /// "written" are told apart.
    #[test]
    fn count_errno_matches_stock_libjudy() {
        const SENTINEL: c_int = 99;
        let lib = Lib::open();
        let o_set: F2i = lib.sym(c"Judy1Set");
        let o_count1: FCount = lib.sym(c"Judy1Count");
        let o_free1: FFree = lib.sym(c"Judy1FreeArray");
        let o_ins: FpIns = lib.sym(c"JudyLIns");
        let o_countl: FCount = lib.sym(c"JudyLCount");
        let o_freel: FFree = lib.sym(c"JudyLFreeArray");
        let ranges: [(Word, Word); 7] = [
            (0, Word::MAX),
            (10, 30),
            (11, 19),
            (25, 25),
            (31, Word::MAX),
            (30, 10),
            (Word::MAX, 0),
        ];
        // Our entry points take `*mut JError`; the probe takes the stock
        // library's untyped signature.
        unsafe extern "C" fn our_count1(
            p: *const c_void,
            a: Word,
            b: Word,
            pj: *mut c_void,
        ) -> Word {
            // SAFETY: forwarded unchanged; `pj` is null or a live JError.
            unsafe { Judy1Count(p, a, b, pj.cast()) }
        }
        unsafe extern "C" fn our_countl(
            p: *const c_void,
            a: Word,
            b: Word,
            pj: *mut c_void,
        ) -> Word {
            // SAFETY: as above.
            unsafe { JudyLCount(p, a, b, pj.cast()) }
        }
        let probe = |count: FCount, array: *const c_void, a: Word, b: Word| {
            let mut je = JError {
                je_errno: SENTINEL,
                je_err_id: 0,
                je_reserved: [0; 4],
            };
            // SAFETY: `array` is null or a live array of the family
            // `count` belongs to; `je` is a live, writable JError.
            let rc = unsafe { count(array, a, b, (&raw mut je).cast()) };
            // SAFETY: a null PJError is part of the documented contract.
            let rc_null = unsafe { count(array, a, b, null_mut()) };
            assert_eq!(rc, rc_null, "count {a:#x}..={b:#x} with a null PJError");
            (a, b, rc, je.je_errno)
        };
        // SAFETY: both libraries' entry points driven through their C
        // contracts with identical sequences and valid pointers.
        unsafe {
            for populated in [false, true] {
                let (mut ours1, mut theirs1) = (null_mut::<c_void>(), null_mut::<c_void>());
                let (mut oursl, mut theirsl) = (null_mut::<c_void>(), null_mut::<c_void>());
                if populated {
                    for k in [10, 20, 30] {
                        assert_eq!(Judy1Set(&raw mut ours1, k, null_mut()), 1);
                        assert_eq!(o_set(&raw mut theirs1, k, null_mut()), 1);
                        *JudyLIns(&raw mut oursl, k, null_mut()).cast::<Word>() = k;
                        *o_ins(&raw mut theirsl, k, null_mut()).cast::<Word>() = k;
                    }
                }
                let ours: Vec<_> = ranges
                    .iter()
                    .flat_map(|&(a, b)| {
                        [
                            probe(our_count1, ours1, a, b),
                            probe(our_countl, oursl, a, b),
                        ]
                    })
                    .collect();
                let theirs: Vec<_> = ranges
                    .iter()
                    .flat_map(|&(a, b)| {
                        [
                            probe(o_count1, theirs1, a, b),
                            probe(o_countl, theirsl, a, b),
                        ]
                    })
                    .collect();
                assert_eq!(
                    ours, theirs,
                    "(index1, index2, count, errno) per call, Judy1 then JudyL; populated = {populated}"
                );
                if populated {
                    assert!(Judy1FreeArray(&raw mut ours1, null_mut()) > 0);
                    assert!(o_free1(&raw mut theirs1, null_mut()) > 0);
                    assert!(JudyLFreeArray(&raw mut oursl, null_mut()) > 0);
                    assert!(o_freel(&raw mut theirsl, null_mut()) > 0);
                }
            }
        }
    }
}
