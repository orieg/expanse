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
//! `JudyHS*` lands with `ExpanseBytesMap`.

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
pub use compat::*;
pub mod modern;

#[cfg(target_pointer_width = "64")]
use expanse_trie::ExpanseMap;
#[cfg(target_pointer_width = "64")]
use expanse_trie::ExpanseSet;
#[cfg(target_pointer_width = "64")]
use expanse_trie::bytesmap::ExpanseBytesMap;
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
        // SAFETY: pointer is a valid ExpanseMap.
        let map = unsafe { &*parray.cast::<ExpanseMap>() };
        c_int::from(map.validate_defensive().is_ok())
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
        // SAFETY: pointer is a valid ExpanseMap.
        let map = unsafe { &*parray.cast::<ExpanseMap>() };
        map.stats()
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

        for i in 0..9 {
            (*out).depth_histogram[i] = stats.depth_histogram[i] as u64;
        }
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
                            assert_eq!(
                                (r1, i1),
                                (r2, if r2 == 1 { i2 } else { i1 }),
                                "next {k:#x}"
                            );
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1Prev(ours, &raw mut i1, null_mut());
                            let r2 = o_prev(theirs, &raw mut i2, null_mut());
                            assert_eq!(
                                (r1, i1),
                                (r2, if r2 == 1 { i2 } else { i1 }),
                                "prev {k:#x}"
                            );
                        }
                        _ => {
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1FirstEmpty(ours, &raw mut i1, null_mut());
                            let r2 = o_fe(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, if r2 == 1 { i2 } else { i1 }), "fe {k:#x}");
                            let (mut i1, mut i2) = (k, k);
                            let r1 = Judy1LastEmpty(ours, &raw mut i1, null_mut());
                            let r2 = o_le(theirs, &raw mut i2, null_mut());
                            assert_eq!((r1, i1), (r2, if r2 == 1 { i2 } else { i1 }), "le {k:#x}");
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
}
