//! The operation tape of MariaDB OQGraph's two-bit colour map (#1682),
//! shared by `benches/vs_stock.rs` (Callgrind) and `examples/colour_map.rs`
//! (wall clock and `perf stat`). Not a harness: no entry point.
//!
//! OQGraph keeps a breadth-first search's vertex colours in two Judy1
//! arrays, bit 0 in one and bit 1 in the other, keyed by vertex id: white 0,
//! gray 1, black 3. Reading a colour tests both arrays; writing one sets or
//! unsets a bit in each, so painting a vertex gray unsets bit 1 even though
//! it was never set. The search ends by destroying both arrays.
//!
//! The tape is that call sequence, recorded once from a search that keeps
//! its colours in a plain `Vec<u8>`, so replaying it drives either library
//! through exactly the same calls in the same order and nothing but the
//! calls. Each entry packs the vertex id in the low 32 bits, the array in
//! bit 32 and the operation in bits 33..35.

// Each harness that includes this module uses a different subset of it.
#![allow(dead_code)]

use core::ffi::{c_int, c_void};

/// `Word_t`.
pub type Word = usize;

/// Read one colour bit: `Judy1Test`.
pub const OP_TEST: u64 = 0;
/// Set one colour bit: `Judy1Set`.
pub const OP_SET: u64 = 1;
/// Clear one colour bit: `Judy1Unset`.
pub const OP_UNSET: u64 = 2;
/// Destroy one array: `Judy1FreeArray`.
pub const OP_FREE: u64 = 3;

/// Out-edges per vertex: three pseudo-random targets and `v + 1`, so every
/// vertex is reachable from 0 and the search visits all `n`.
pub const DEGREE: u64 = 4;

/// The `e`-th out-edge of `v` in an `n`-vertex graph. The same graph as
/// `smoke/colour_map_smoke.c`.
#[inline]
pub fn edge(v: u64, e: u64, n: u64) -> u64 {
    if e == 3 {
        return (v + 1) % n;
    }
    let x = v
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(e.wrapping_mul(1_442_695_040_888_963_407))
        .wrapping_add(1);
    (x >> 17) % n
}

#[inline]
fn entry(op: u64, array: u64, v: u64) -> u64 {
    (op << 33) | (array << 32) | v
}

/// Splits a tape entry into `(op, array, vertex)`.
#[inline(always)]
pub fn decode(t: u64) -> (u64, usize, u64) {
    (t >> 33, ((t >> 32) & 1) as usize, t & 0xFFFF_FFFF)
}

/// The tape of one breadth-first search from vertex 0 over `n` vertices,
/// keys `0..n` in visit order, ending with both arrays destroyed.
pub fn tape(n: u64) -> Vec<u64> {
    assert!(n > 0 && n <= u32::MAX as u64, "vertex ids fit 32 bits");
    let mut colour = vec![0u8; n as usize];
    let mut queue = Vec::with_capacity(n as usize);
    let mut out = Vec::with_capacity(n as usize * 14 + 2);
    let paint = |out: &mut Vec<u64>, colour: &mut [u8], v: u64, c: u8| {
        for bit in 0..2u64 {
            let op = if c >> bit & 1 == 1 { OP_SET } else { OP_UNSET };
            out.push(entry(op, bit, v));
        }
        colour[v as usize] = c;
    };
    paint(&mut out, &mut colour, 0, 1);
    queue.push(0u64);
    let mut head = 0;
    while head < queue.len() {
        let v = queue[head];
        head += 1;
        for e in 0..DEGREE {
            let w = edge(v, e, n);
            out.push(entry(OP_TEST, 0, w));
            out.push(entry(OP_TEST, 1, w));
            if colour[w as usize] == 0 {
                paint(&mut out, &mut colour, w, 1);
                queue.push(w);
            }
        }
        paint(&mut out, &mut colour, v, 3);
    }
    assert_eq!(queue.len() as u64, n, "every vertex is reachable from 0");
    out.push(entry(OP_FREE, 0, 0));
    out.push(entry(OP_FREE, 1, 0));
    out
}

/// Calls per kind in `tape`: `(tests, sets, unsets, frees)`.
pub fn census(tape: &[u64]) -> [usize; 4] {
    let mut c = [0; 4];
    for &t in tape {
        c[decode(t).0 as usize] += 1;
    }
    c
}

/// `Judy1Set` / `Judy1Unset`'s shape.
pub type SetFn = unsafe extern "C" fn(*mut *mut c_void, Word, *mut c_void) -> c_int;
/// `Judy1Test`'s shape.
pub type TestFn = unsafe extern "C" fn(*const c_void, Word, *mut c_void) -> c_int;
/// `Judy1FreeArray`'s shape.
pub type FreeFn = unsafe extern "C" fn(*mut *mut c_void, *mut c_void) -> Word;

/// One library's four Judy1 entry points.
#[derive(Clone, Copy)]
pub struct Judy1 {
    /// `Judy1Test`.
    pub test: TestFn,
    /// `Judy1Set`.
    pub set: SetFn,
    /// `Judy1Unset`.
    pub unset: SetFn,
    /// `Judy1FreeArray`.
    pub free: FreeFn,
}

/// Replays `tape` against two fresh arrays and returns the sum of every
/// test, set and unset result, so no call's work is dead and two libraries
/// that agree return the same sum. `Judy1FreeArray`'s byte count is
/// implementation-defined (`docs/COMPAT.md` D4), so it is kept live with
/// `black_box` and left out of the sum. Both arrays are null at the end.
///
/// # Safety
///
/// `j`'s pointers are one library's Judy1 entry points.
#[inline(always)]
pub unsafe fn replay(tape: &[u64], j: Judy1) -> Word {
    let mut arrays: [*mut c_void; 2] = [core::ptr::null_mut(); 2];
    let mut sink: Word = 0;
    for &t in tape {
        let (op, a, v) = decode(t);
        let v = v as Word;
        // SAFETY: `arrays[a]` is null or an array of `j`'s library, owned
        // here; Judy1 accepts a null PJError.
        unsafe {
            sink = sink.wrapping_add(match op {
                OP_TEST => (j.test)(arrays[a], v, core::ptr::null_mut()) as Word,
                OP_SET => (j.set)(&raw mut arrays[a], v, core::ptr::null_mut()) as Word,
                OP_UNSET => (j.unset)(&raw mut arrays[a], v, core::ptr::null_mut()) as Word,
                _ => {
                    core::hint::black_box((j.free)(&raw mut arrays[a], core::ptr::null_mut()));
                    0
                }
            });
        }
    }
    debug_assert!(arrays.iter().all(|a| a.is_null()));
    sink
}
