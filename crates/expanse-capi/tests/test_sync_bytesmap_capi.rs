//! The shared bytes and ordered-bytes maps' C handles,
//! `expanse_sync_bytesmap_*` and `expanse_sync_ordered_bytesmap_*` (#1538):
//! every entry point through its `extern "C"` function, the NULL conventions,
//! the reader lifecycle, the ordered reads' buffer contract, and the threaded
//! contracts of `include/expanse.h`.
//!
//! The two maps share their fourteen point, memory and reader entry points,
//! so those tests run once per map through a table of function pointers
//! ([`Family`]).
//!
//! The single-threaded tests are the ones the Miri lane runs
//! (`.github/workflows/ci.yml`, job `miri`); the ASan lane (job `test-asan`)
//! runs the whole target. The threaded tests are ignored under Miri, where
//! they would run for far longer than the lane allows, and run everywhere
//! else.

use core::ffi::c_void;
use expanse::modern::{
    ExpanseOrderedBytesNavStatus, SyncBytesMapReader, SyncOrderedBytesMapReader,
    expanse_sync_bytesmap_contains, expanse_sync_bytesmap_free, expanse_sync_bytesmap_get,
    expanse_sync_bytesmap_insert, expanse_sync_bytesmap_len, expanse_sync_bytesmap_mem_held,
    expanse_sync_bytesmap_mem_used, expanse_sync_bytesmap_new,
    expanse_sync_bytesmap_reader_contains, expanse_sync_bytesmap_reader_free,
    expanse_sync_bytesmap_reader_get, expanse_sync_bytesmap_reader_new,
    expanse_sync_bytesmap_remove, expanse_sync_bytesmap_shrink_to_fit,
};
use expanse::modern::{
    expanse_sync_ordered_bytesmap_contains, expanse_sync_ordered_bytesmap_first,
    expanse_sync_ordered_bytesmap_free, expanse_sync_ordered_bytesmap_get,
    expanse_sync_ordered_bytesmap_insert, expanse_sync_ordered_bytesmap_last,
    expanse_sync_ordered_bytesmap_len, expanse_sync_ordered_bytesmap_mem_held,
    expanse_sync_ordered_bytesmap_mem_used, expanse_sync_ordered_bytesmap_new,
    expanse_sync_ordered_bytesmap_next_after, expanse_sync_ordered_bytesmap_next_at_or_after,
    expanse_sync_ordered_bytesmap_prev_at_or_before, expanse_sync_ordered_bytesmap_prev_before,
    expanse_sync_ordered_bytesmap_reader_contains, expanse_sync_ordered_bytesmap_reader_free,
    expanse_sync_ordered_bytesmap_reader_get, expanse_sync_ordered_bytesmap_reader_new,
    expanse_sync_ordered_bytesmap_remove, expanse_sync_ordered_bytesmap_shrink_to_fit,
};
use expanse_trie::sync::{SyncExpanseBytesMap, SyncExpanseOrderedBytesMap};

const NULL_KEY: *const c_void = core::ptr::null();

/// The fourteen entry points both shared byte-keyed maps export, as function
/// pointers, so one test body covers both.
struct Family<M, R> {
    name: &'static str,
    new: extern "C" fn() -> *mut M,
    free: unsafe extern "C" fn(*mut M),
    insert: unsafe extern "C" fn(*const M, *const c_void, usize, u64, *mut u64) -> bool,
    get: unsafe extern "C" fn(*const M, *const c_void, usize, *mut u64) -> bool,
    contains: unsafe extern "C" fn(*const M, *const c_void, usize) -> bool,
    remove: unsafe extern "C" fn(*const M, *const c_void, usize, *mut u64) -> bool,
    len: unsafe extern "C" fn(*const M) -> u64,
    mem_used: unsafe extern "C" fn(*const M) -> usize,
    mem_held: unsafe extern "C" fn(*const M) -> usize,
    shrink_to_fit: unsafe extern "C" fn(*const M) -> usize,
    reader_new: unsafe extern "C" fn(*const M) -> *mut R,
    reader_free: unsafe extern "C" fn(*mut R),
    reader_get: unsafe extern "C" fn(*const R, *const c_void, usize, *mut u64) -> bool,
    reader_contains: unsafe extern "C" fn(*const R, *const c_void, usize) -> bool,
}

const BYTES: Family<SyncExpanseBytesMap, SyncBytesMapReader> = Family {
    name: "bytesmap",
    new: expanse_sync_bytesmap_new,
    free: expanse_sync_bytesmap_free,
    insert: expanse_sync_bytesmap_insert,
    get: expanse_sync_bytesmap_get,
    contains: expanse_sync_bytesmap_contains,
    remove: expanse_sync_bytesmap_remove,
    len: expanse_sync_bytesmap_len,
    mem_used: expanse_sync_bytesmap_mem_used,
    mem_held: expanse_sync_bytesmap_mem_held,
    shrink_to_fit: expanse_sync_bytesmap_shrink_to_fit,
    reader_new: expanse_sync_bytesmap_reader_new,
    reader_free: expanse_sync_bytesmap_reader_free,
    reader_get: expanse_sync_bytesmap_reader_get,
    reader_contains: expanse_sync_bytesmap_reader_contains,
};

const ORDERED: Family<SyncExpanseOrderedBytesMap, SyncOrderedBytesMapReader> = Family {
    name: "ordered_bytesmap",
    new: expanse_sync_ordered_bytesmap_new,
    free: expanse_sync_ordered_bytesmap_free,
    insert: expanse_sync_ordered_bytesmap_insert,
    get: expanse_sync_ordered_bytesmap_get,
    contains: expanse_sync_ordered_bytesmap_contains,
    remove: expanse_sync_ordered_bytesmap_remove,
    len: expanse_sync_ordered_bytesmap_len,
    mem_used: expanse_sync_ordered_bytesmap_mem_used,
    mem_held: expanse_sync_ordered_bytesmap_mem_held,
    shrink_to_fit: expanse_sync_ordered_bytesmap_shrink_to_fit,
    reader_new: expanse_sync_ordered_bytesmap_reader_new,
    reader_free: expanse_sync_ordered_bytesmap_reader_free,
    reader_get: expanse_sync_ordered_bytesmap_reader_get,
    reader_contains: expanse_sync_ordered_bytesmap_reader_contains,
};

fn ptr(k: &[u8]) -> *const c_void {
    k.as_ptr().cast()
}

impl<M, R> Family<M, R> {
    fn insert(&self, m: *const M, k: &[u8], v: u64) -> bool {
        // SAFETY: `m` is live for every caller; `k` is readable for its length.
        unsafe { (self.insert)(m, ptr(k), k.len(), v, core::ptr::null_mut()) }
    }

    fn remove(&self, m: *const M, k: &[u8]) -> bool {
        // SAFETY: as for `insert`.
        unsafe { (self.remove)(m, ptr(k), k.len(), core::ptr::null_mut()) }
    }

    /// `get` through the container into a local, `None` when it returns false.
    fn get(&self, m: *const M, k: &[u8]) -> Option<u64> {
        let mut v = 0xDEAD_u64;
        // SAFETY: as for `insert`; `v` is a local.
        unsafe { (self.get)(m, ptr(k), k.len(), &raw mut v) }.then_some(v)
    }

    /// `get` through a reader into a local, `None` when it returns false.
    fn reader_get(&self, r: *const R, k: &[u8]) -> Option<u64> {
        let mut v = 0xDEAD_u64;
        // SAFETY: `r` is live for every caller; `k` readable; `v` a local.
        unsafe { (self.reader_get)(r, ptr(k), k.len(), &raw mut v) }.then_some(v)
    }
}

/// Keys that a C string cannot carry: embedded and leading NULs, 0x01 (the
/// ordered map's escape byte), and the empty key.
const ODD_KEYS: [&[u8]; 6] = [
    b"",
    b"\0",
    b"a\0b",
    b"a\x01b",
    b"\x01\x01\0\0",
    b"a long key past several eight-byte hops\0with a tail",
];

fn point_operations_and_return_conventions<M, R>(f: &Family<M, R>) {
    let m = (f.new)();
    assert!(!m.is_null(), "{}", f.name);
    let mut old = 0u64;
    // SAFETY: `m` is live until the free at the end; every key is readable
    // for its length; out-pointers are locals or null.
    unsafe {
        assert_eq!((f.len)(m), 0);
        assert!((f.insert)(m, ptr(b"apple"), 5, 1, &raw mut old));
        assert_eq!(old, 0, "a new key writes nothing through old_out");
        assert!(!(f.insert)(m, ptr(b"apple"), 5, 2, &raw mut old));
        assert_eq!(old, 1, "a replace reports the old value");
        assert!(!(f.insert)(m, ptr(b"apple"), 5, 3, core::ptr::null_mut()));
        // NULL with length 0 is the empty key.
        assert!((f.insert)(m, NULL_KEY, 0, 10, core::ptr::null_mut()));
    }
    assert_eq!(
        f.get(m, b""),
        Some(10),
        "{}: NULL, 0 is the empty key",
        f.name
    );
    for (i, k) in (100u64..).zip(&ODD_KEYS[1..]) {
        assert!(f.insert(m, k, i), "{}: {k:?} is new", f.name);
    }
    // SAFETY: as above.
    unsafe {
        assert_eq!((f.len)(m), 2 + ODD_KEYS.len() as u64 - 1);
        assert!((f.contains)(m, ptr(b"apple"), 5));
        assert!(!(f.contains)(m, ptr(b"app"), 3));
        assert!((f.get)(m, ptr(b"apple"), 5, core::ptr::null_mut()));
    }
    assert_eq!(f.get(m, b"apple"), Some(3));
    for (i, k) in (100u64..).zip(&ODD_KEYS[1..]) {
        assert_eq!(f.get(m, k), Some(i), "{}: {k:?}", f.name);
    }
    assert_eq!(f.get(m, b"a"), None, "a key is matched on all its bytes");
    assert_eq!(f.get(m, b"a\0"), None);

    // SAFETY: as above.
    unsafe {
        assert!((f.mem_used)(m) > 0);
        assert!((f.mem_held)(m) >= (f.mem_used)(m));

        let mut v = 0xDEAD_u64;
        assert!(!(f.remove)(m, ptr(b"app"), 3, &raw mut v));
        assert_eq!(v, 0xDEAD, "a miss writes nothing");
        assert!((f.remove)(m, ptr(b"apple"), 5, &raw mut v));
        assert_eq!(v, 3);
        assert!((f.remove)(m, ptr(b"a\0b"), 3, core::ptr::null_mut()));
        let _released = (f.shrink_to_fit)(m);
    }
    assert_eq!(f.get(m, b"a\0b"), None);
    assert_eq!(f.get(m, b"a\x01b"), Some(102), "a neighbour survives");
    // SAFETY: no reader is live.
    unsafe {
        assert_eq!((f.len)(m), ODD_KEYS.len() as u64 - 1);
        (f.free)(m);
    }
}

#[test]
fn bytesmap_point_operations_and_return_conventions() {
    point_operations_and_return_conventions(&BYTES);
}

#[test]
fn ordered_bytesmap_point_operations_and_return_conventions() {
    point_operations_and_return_conventions(&ORDERED);
}

fn null_handles_and_null_keys_write_nothing<M, R>(f: &Family<M, R>) {
    let null_map: *const M = core::ptr::null();
    let null_reader: *const R = core::ptr::null();
    let mut v = 0xDEAD_u64;
    // SAFETY: every call is documented to accept a NULL handle, and a NULL
    // key with a non-zero length is refused before it is read; the live map
    // below outlives its reader.
    unsafe {
        assert!(!(f.insert)(null_map, ptr(b"k"), 1, 1, &raw mut v));
        assert!(!(f.get)(null_map, ptr(b"k"), 1, &raw mut v));
        assert!(!(f.contains)(null_map, ptr(b"k"), 1));
        assert!(!(f.remove)(null_map, ptr(b"k"), 1, &raw mut v));
        assert_eq!((f.len)(null_map), 0);
        assert_eq!((f.mem_used)(null_map), 0);
        assert_eq!((f.mem_held)(null_map), 0);
        assert_eq!((f.shrink_to_fit)(null_map), 0);
        assert!((f.reader_new)(null_map).is_null());
        assert!(!(f.reader_get)(null_reader, ptr(b"k"), 1, &raw mut v));
        assert!(!(f.reader_contains)(null_reader, ptr(b"k"), 1));
        (f.reader_free)(core::ptr::null_mut());
        (f.free)(core::ptr::null_mut());

        let m = (f.new)();
        assert!((f.insert)(m, ptr(b"k"), 1, 7, core::ptr::null_mut()));
        assert!((f.insert)(m, NULL_KEY, 0, 8, core::ptr::null_mut()));
        assert!(!(f.insert)(m, NULL_KEY, 1, 1, &raw mut v));
        assert!(!(f.get)(m, NULL_KEY, 1, &raw mut v));
        assert!(!(f.contains)(m, NULL_KEY, 1));
        assert!(!(f.remove)(m, NULL_KEY, 1, &raw mut v));
        let r = (f.reader_new)(m);
        assert!(!(f.reader_get)(r, NULL_KEY, 1, &raw mut v));
        assert!(!(f.reader_contains)(r, NULL_KEY, 1));
        assert!(
            (f.reader_contains)(r, NULL_KEY, 0),
            "NULL, 0 is the empty key"
        );
        (f.reader_free)(r);
        assert_eq!((f.len)(m), 2, "a NULL key with a length stored nothing");
        (f.free)(m);
    }
    assert_eq!(
        v, 0xDEAD,
        "{}: no NULL-handle or NULL-key call wrote through an out-pointer",
        f.name
    );
}

#[test]
fn bytesmap_null_handles_and_null_keys_write_nothing() {
    null_handles_and_null_keys_write_nothing(&BYTES);
}

#[test]
fn ordered_bytesmap_null_handles_and_null_keys_write_nothing() {
    null_handles_and_null_keys_write_nothing(&ORDERED);
}

fn reader_sees_writes_and_is_freed_before_its_map<M, R>(f: &Family<M, R>) {
    const N: u64 = 64;
    let m = (f.new)();
    let keys: Vec<Vec<u8>> = (0..N)
        .map(|i| format!("key\0{i:03}\x01suffix").into_bytes())
        .collect();
    // SAFETY: `m` outlives both readers, which are freed before it.
    unsafe {
        let r = (f.reader_new)(m);
        assert!(!r.is_null());
        for (i, k) in (0u64..).zip(&keys) {
            assert!(!(f.reader_contains)(r, ptr(k), k.len()));
            assert!(f.insert(m, k, i));
            assert_eq!(
                f.reader_get(r, k),
                Some(i),
                "the reader sees a completed write"
            );
            assert!((f.reader_contains)(r, ptr(k), k.len()));
        }
        // A second reader on the same map, on the same thread.
        let r2 = (f.reader_new)(m);
        for (i, k) in (0u64..).zip(&keys).step_by(2) {
            assert!(f.remove(m, k));
            assert_eq!(f.reader_get(r, k), None);
            assert_eq!(f.reader_get(r2, &keys[(i + 1) as usize]), Some(i + 1));
        }
        (f.reader_free)(r2);
        (f.reader_free)(r);
        assert_eq!((f.len)(m), N / 2);
        (f.free)(m);
    }
}

#[test]
fn bytesmap_reader_sees_writes_and_is_freed_before_its_map() {
    reader_sees_writes_and_is_freed_before_its_map(&BYTES);
}

#[test]
fn ordered_bytesmap_reader_sees_writes_and_is_freed_before_its_map() {
    reader_sees_writes_and_is_freed_before_its_map(&ORDERED);
}

/// One writer churns keys while readers on other threads read stable keys,
/// each of which must come back with its exact value on every read, and
/// churned keys, each of which must be absent or carry its writer's value.
fn concurrent_readers_never_see_a_wrong_value<M, R>(f: &Family<M, R>) {
    const STABLE: u64 = 1024;
    const CHURN: u64 = 512;
    const READERS: usize = 4;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = (f.new)();
    let stable: Vec<Vec<u8>> = (0..STABLE)
        .map(|i| format!("stable\0{i:05}").into_bytes())
        .collect();
    let churn: Vec<Vec<u8>> = (0..CHURN)
        .map(|i| format!("churn\x01{i:05}\0a-longer-tail").into_bytes())
        .collect();
    for (i, k) in (0u64..).zip(&stable) {
        assert!(f.insert(m, k, i));
    }
    let addr = m as usize; // raw pointers are not `Send`
    let stop = AtomicBool::new(false);
    std::thread::scope(|sc| {
        let (stable, churn, stop) = (&stable, &churn, &stop);
        for _ in 0..READERS {
            sc.spawn(move || {
                // SAFETY: the map outlives the scope; this thread's reader is
                // used only here and freed before the scope ends.
                unsafe {
                    let r = (f.reader_new)(addr as *const M);
                    let mut passes = 0u32;
                    loop {
                        let done = stop.load(Ordering::Acquire);
                        for (i, k) in (0u64..).zip(stable) {
                            assert_eq!(f.reader_get(r, k), Some(i), "stable key {i}");
                        }
                        for (i, k) in (0u64..).zip(churn) {
                            if let Some(v) = f.reader_get(r, k) {
                                assert_eq!(v, 1_000_000 + i, "churned key {i}");
                            }
                        }
                        passes += 1;
                        if done {
                            break;
                        }
                    }
                    (f.reader_free)(r);
                    assert!(passes > 0);
                }
            });
        }
        sc.spawn(move || {
            for round in 0..40u64 {
                for (i, k) in (0u64..).zip(churn) {
                    if round.is_multiple_of(2) {
                        f.insert(addr as *const M, k, 1_000_000 + i);
                    } else {
                        f.remove(addr as *const M, k);
                    }
                }
            }
            stop.store(true, Ordering::Release);
        });
    });
    // SAFETY: every reader was freed inside the scope.
    unsafe {
        assert_eq!(
            (f.len)(m),
            STABLE,
            "the last round removed every churned key"
        );
        (f.free)(m);
    }
}

#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn bytesmap_concurrent_readers_never_see_a_wrong_value() {
    concurrent_readers_never_see_a_wrong_value(&BYTES);
}

#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn ordered_bytesmap_concurrent_readers_never_see_a_wrong_value() {
    concurrent_readers_never_see_a_wrong_value(&ORDERED);
}

/// Reader handles created and used on worker threads, then freed on the main
/// thread while a writer runs, before the map (`docs/COMPAT.md`, reader-handle
/// ownership).
fn reader_handles_freed_on_another_thread<M, R>(f: &Family<M, R>) {
    const THREADS: usize = 6;
    const N: u64 = 512;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = (f.new)();
    let keys: Vec<Vec<u8>> = (0..N).map(|i| format!("k\0{i}").into_bytes()).collect();
    for (i, k) in (0u64..).zip(&keys) {
        f.insert(m, k, i * 7);
    }
    let addr = m as usize;
    let handles: Vec<usize> = std::thread::scope(|sc| {
        let keys = &keys;
        let workers: Vec<_> = (0..THREADS)
            .map(|_| {
                sc.spawn(move || {
                    // SAFETY: the map outlives every handle; the handle is used
                    // only on this thread until returned, idle afterwards.
                    let r = unsafe { (f.reader_new)(addr as *const M) };
                    assert!(!r.is_null());
                    for (i, k) in (0u64..).zip(keys) {
                        assert_eq!(f.reader_get(r, k), Some(i * 7));
                    }
                    r as usize
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("reader thread"))
            .collect()
    });

    let stop = AtomicBool::new(false);
    std::thread::scope(|sc| {
        let stop = &stop;
        sc.spawn(move || {
            let mut i = 0u64;
            while !stop.load(Ordering::Relaxed) || i < 20_000 {
                let k = format!("churn\0{}", i % 2048).into_bytes();
                if i.is_multiple_of(2) {
                    f.insert(addr as *const M, &k, i);
                } else {
                    f.remove(addr as *const M, &k);
                }
                i += 1;
            }
        });
        for h in &handles {
            // SAFETY: each handle came from `_reader_new`, no call on it is in
            // progress, and the map is still alive.
            unsafe { (f.reader_free)(*h as *mut R) };
        }
        stop.store(true, Ordering::Relaxed);
    });
    for (i, k) in (0u64..).zip(&keys) {
        assert_eq!(f.get(m, k), Some(i * 7));
    }
    // SAFETY: every reader handle was freed above.
    unsafe { (f.free)(m) };
}

#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn bytesmap_reader_handles_freed_on_another_thread() {
    reader_handles_freed_on_another_thread(&BYTES);
}

#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn ordered_bytesmap_reader_handles_freed_on_another_thread() {
    reader_handles_freed_on_another_thread(&ORDERED);
}

// ---- Ordered reads on the ordered-bytes map's handle ----

type Map = SyncExpanseOrderedBytesMap;
type Extreme = unsafe extern "C" fn(
    *const Map,
    *mut c_void,
    usize,
    *mut usize,
    *mut u64,
) -> ExpanseOrderedBytesNavStatus;
type ByKey = unsafe extern "C" fn(
    *const Map,
    *const c_void,
    usize,
    *mut c_void,
    usize,
    *mut usize,
    *mut u64,
) -> ExpanseOrderedBytesNavStatus;

/// What one ordered read did: the status, the key written (on `Ok`),
/// `*required_len` and `*value_out`. Each output starts from a sentinel, so an
/// unwritten one is visible, and a read that is not `Ok` is checked to have
/// left the buffer untouched.
type Read = (ExpanseOrderedBytesNavStatus, Option<Vec<u8>>, usize, u64);

const SENTINEL: u8 = 0x7F;

fn finish(st: ExpanseOrderedBytesNavStatus, buf: &[u8], req: usize, v: u64) -> Read {
    let written = (st == ExpanseOrderedBytesNavStatus::Ok).then(|| buf[..req].to_vec());
    let untouched_from = if st == ExpanseOrderedBytesNavStatus::Ok {
        req
    } else {
        0
    };
    assert!(
        buf[untouched_from..].iter().all(|&b| b == SENTINEL),
        "{st:?} wrote into the buffer outside the key it returned"
    );
    (st, written, req, v)
}

fn extreme(f: Extreme, m: *const Map, buf_len: usize) -> Read {
    let mut buf = vec![SENTINEL; buf_len.max(1)];
    let (mut req, mut v) = (usize::MAX, 0xDEAD_u64);
    // SAFETY: `m` null or live; `buf` holds at least `buf_len` bytes; the
    // outputs are locals.
    let st = unsafe {
        f(
            m,
            buf.as_mut_ptr().cast(),
            buf_len,
            &raw mut req,
            &raw mut v,
        )
    };
    finish(st, &buf, req, v)
}

fn by_key(f: ByKey, m: *const Map, k: &[u8], buf_len: usize) -> Read {
    let mut buf = vec![SENTINEL; buf_len.max(1)];
    let (mut req, mut v) = (usize::MAX, 0xDEAD_u64);
    // SAFETY: as for `extreme`; `k` is readable for its length.
    let st = unsafe {
        f(
            m,
            ptr(k),
            k.len(),
            buf.as_mut_ptr().cast(),
            buf_len,
            &raw mut req,
            &raw mut v,
        )
    };
    finish(st, &buf, req, v)
}

const FIRST: Extreme = expanse_sync_ordered_bytesmap_first;
const LAST: Extreme = expanse_sync_ordered_bytesmap_last;
const NEXT_AT: ByKey = expanse_sync_ordered_bytesmap_next_at_or_after;
const NEXT: ByKey = expanse_sync_ordered_bytesmap_next_after;
const PREV_AT: ByKey = expanse_sync_ordered_bytesmap_prev_at_or_before;
const PREV: ByKey = expanse_sync_ordered_bytesmap_prev_before;

/// The gate of #1538 step 3: ordered-bytes keys with embedded NULs (and the
/// 0x01 escape byte) round-trip through insert, get and every ordered read.
#[test]
fn ordered_keys_with_nuls_round_trip_through_every_ordered_read() {
    use ExpanseOrderedBytesNavStatus::{NotFound, Ok};
    let m = expanse_sync_ordered_bytesmap_new();
    // Byte order: "" < "\0" < "\0\0" < "\0\x01" < "\x01" < "a" < "a\0" < "a\0b" < "a\x01" < "b"
    let sorted: [&[u8]; 10] = [
        b"", b"\0", b"\0\0", b"\0\x01", b"\x01", b"a", b"a\0", b"a\0b", b"a\x01", b"b",
    ];
    // Insert out of order.
    for i in [3usize, 9, 0, 6, 1, 8, 4, 2, 7, 5] {
        assert!(ORDERED.insert(m, sorted[i], i as u64));
    }
    for (i, k) in sorted.iter().enumerate() {
        assert_eq!(ORDERED.get(m, k), Some(i as u64), "get {k:?}");
    }
    let entry = |i: usize| (Ok, Some(sorted[i].to_vec()), sorted[i].len(), i as u64);
    assert_eq!(extreme(FIRST, m, 16), entry(0));
    assert_eq!(extreme(LAST, m, 16), entry(9));
    for i in 0..sorted.len() {
        let k = sorted[i];
        assert_eq!(
            by_key(NEXT_AT, m, k, 16),
            entry(i),
            "next_at_or_after {k:?}"
        );
        assert_eq!(
            by_key(PREV_AT, m, k, 16),
            entry(i),
            "prev_at_or_before {k:?}"
        );
        let next = if i + 1 < sorted.len() {
            entry(i + 1)
        } else {
            (NotFound, None, usize::MAX, 0xDEAD)
        };
        assert_eq!(by_key(NEXT, m, k, 16), next, "next_after {k:?}");
        let prev = if i > 0 {
            entry(i - 1)
        } else {
            (NotFound, None, usize::MAX, 0xDEAD)
        };
        assert_eq!(by_key(PREV, m, k, 16), prev, "prev_before {k:?}");
    }
    // A walk forward with next_after, reusing the output buffer as the search
    // key (the header allows `key` and `key_out` to be one buffer).
    let mut buf = [SENTINEL; 16];
    let (mut req, mut v) = (0usize, 0u64);
    let mut seen = Vec::new();
    // SAFETY: `m` is live; `buf` holds 16 bytes and is both the search key
    // (its first `req` bytes) and the output; the outputs are locals.
    unsafe {
        let mut st = FIRST(m, buf.as_mut_ptr().cast(), 16, &raw mut req, &raw mut v);
        while st == Ok {
            seen.push((buf[..req].to_vec(), v));
            let p = buf.as_mut_ptr();
            st = NEXT(
                m,
                p.cast_const().cast(),
                req,
                p.cast(),
                16,
                &raw mut req,
                &raw mut v,
            );
        }
        assert_eq!(st, NotFound);
    }
    let want: Vec<(Vec<u8>, u64)> = (0u64..).zip(sorted).map(|(i, k)| (k.to_vec(), i)).collect();
    assert_eq!(seen, want, "the in-place walk returns every key in order");
    // SAFETY: no reader is live.
    unsafe { expanse_sync_ordered_bytesmap_free(m) };
}

#[test]
fn ordered_reads_and_the_buffer_contract() {
    use ExpanseOrderedBytesNavStatus::{BufferTooSmall, NotFound, Ok};
    let m = expanse_sync_ordered_bytesmap_new();
    // A key with escaped bytes throughout, so a decode into a short buffer
    // would write the prefix that fits before it ran out of room.
    let escaped: &[u8] = b"\0\x01\0\x01\0\x01\0\x01";
    let plain: &[u8] = b"plain-key";
    assert!(ORDERED.insert(m, escaped, 1));
    assert!(ORDERED.insert(m, plain, 2));

    // The length probe: NULL with 0 writes only required_len.
    let mut req = usize::MAX;
    let mut v = 0xDEAD_u64;
    // SAFETY: live map; NULL with 0 is the documented probe; locals.
    unsafe {
        let st = FIRST(m, core::ptr::null_mut(), 0, &raw mut req, &raw mut v);
        assert_eq!((st, req, v), (BufferTooSmall, escaped.len(), 0xDEAD));
        let st = LAST(m, core::ptr::null_mut(), 0, &raw mut req, &raw mut v);
        assert_eq!((st, req, v), (BufferTooSmall, plain.len(), 0xDEAD));
        // A non-NULL buffer with buf_len 0 is a probe too.
        let mut one = [SENTINEL; 1];
        let st = FIRST(m, one.as_mut_ptr().cast(), 0, &raw mut req, &raw mut v);
        assert_eq!((st, one[0]), (BufferTooSmall, SENTINEL));
        // A non-zero buf_len with a NULL buffer is too small, not a write.
        let st = FIRST(m, core::ptr::null_mut(), 64, &raw mut req, &raw mut v);
        assert_eq!(st, BufferTooSmall);
        // Every output may be NULL.
        let null = core::ptr::null_mut();
        assert_eq!(FIRST(m, null, 0, null.cast(), null.cast()), BufferTooSmall);
    }

    // One byte short: nothing written (`finish` checks the buffer), required
    // length exact; then the exact length succeeds.
    let short = escaped.len() - 1;
    assert_eq!(
        extreme(FIRST, m, short),
        (BufferTooSmall, None, escaped.len(), 0xDEAD)
    );
    assert_eq!(
        by_key(PREV, m, plain, short),
        (BufferTooSmall, None, escaped.len(), 0xDEAD)
    );
    assert_eq!(
        extreme(FIRST, m, escaped.len()),
        (Ok, Some(escaped.to_vec()), escaped.len(), 1)
    );
    assert_eq!(
        by_key(NEXT, m, escaped, plain.len()),
        (Ok, Some(plain.to_vec()), plain.len(), 2)
    );
    assert_eq!(
        by_key(NEXT, m, escaped, plain.len() - 1),
        (BufferTooSmall, None, plain.len(), 0xDEAD)
    );

    // The empty key needs no buffer: a NULL, 0 probe that finds it is Ok.
    assert!(ORDERED.insert(m, b"", 3));
    // SAFETY: live map; NULL with 0; locals.
    unsafe {
        let st = FIRST(m, core::ptr::null_mut(), 0, &raw mut req, &raw mut v);
        assert_eq!((st, req, v), (Ok, 0, 3));
    }
    assert_eq!(
        by_key(PREV, m, b"", 16),
        (NotFound, None, usize::MAX, 0xDEAD)
    );
    assert_eq!(
        by_key(NEXT, m, plain, 16),
        (NotFound, None, usize::MAX, 0xDEAD)
    );

    // NULL map, and a NULL search key with a length: NotFound, nothing written.
    let null_m: *const Map = core::ptr::null();
    assert_eq!(
        extreme(FIRST, null_m, 16),
        (NotFound, None, usize::MAX, 0xDEAD)
    );
    assert_eq!(
        by_key(NEXT_AT, null_m, b"a", 16),
        (NotFound, None, usize::MAX, 0xDEAD)
    );
    let mut buf = [SENTINEL; 16];
    // SAFETY: a NULL key with a length is refused before it is read; `buf`
    // holds 16 bytes; locals.
    unsafe {
        req = usize::MAX;
        let st = NEXT_AT(
            m,
            NULL_KEY,
            1,
            buf.as_mut_ptr().cast(),
            16,
            &raw mut req,
            &raw mut v,
        );
        assert_eq!((st, req), (NotFound, usize::MAX));
        // NULL with 0 is the empty search key.
        let st = NEXT_AT(
            m,
            NULL_KEY,
            0,
            buf.as_mut_ptr().cast(),
            16,
            &raw mut req,
            &raw mut v,
        );
        assert_eq!((st, req, v), (Ok, 0, 3));
    }
    assert!(buf.iter().all(|&b| b == SENTINEL));

    // An empty map: every read is NotFound.
    // SAFETY: no reader is live on either map.
    unsafe {
        expanse_sync_ordered_bytesmap_free(m);
        let e = expanse_sync_ordered_bytesmap_new();
        assert_eq!(extreme(FIRST, e, 16).0, NotFound);
        assert_eq!(extreme(LAST, e, 16).0, NotFound);
        assert_eq!(by_key(PREV_AT, e, b"z", 16).0, NotFound);
        expanse_sync_ordered_bytesmap_free(e);
    }
}

/// Walks the map with `_first` and `_next_after` into a buffer that starts
/// too small, growing it from `required_len` on every `BufferTooSmall`.
/// Returns the keys in walk order and how many reads came back too small.
fn walk(m: *const Map) -> (Vec<Vec<u8>>, usize) {
    let mut buf = vec![0u8; 2];
    let (mut out, mut too_small) = (Vec::<Vec<u8>>::new(), 0usize);
    loop {
        let mut req = 0usize;
        let (dst, len, value) = (buf.as_mut_ptr().cast(), buf.len(), core::ptr::null_mut());
        // SAFETY: `m` is live; `dst` holds `len` bytes; the previous key is
        // readable; `req` is a local.
        let st = unsafe {
            match out.last() {
                None => FIRST(m, dst, len, &raw mut req, value),
                Some(k) => NEXT(m, ptr(k), k.len(), dst, len, &raw mut req, value),
            }
        };
        match st {
            ExpanseOrderedBytesNavStatus::NotFound => return (out, too_small),
            ExpanseOrderedBytesNavStatus::BufferTooSmall => {
                too_small += 1;
                assert!(req > buf.len());
                buf.resize(req, 0);
            }
            ExpanseOrderedBytesNavStatus::Ok => out.push(buf[..req].to_vec()),
        }
    }
}

/// An ordered walk built from `_next_after` over a map under concurrent
/// inserts and removes returns every key present throughout, in strictly
/// increasing order, with the too-small retry taken.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn ordered_walk_under_concurrent_writes_sees_every_stable_key_in_order() {
    const STABLE: usize = 600;
    const CHURN: usize = 600;
    const READERS: usize = 3;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = expanse_sync_ordered_bytesmap_new();
    // Stable keys of varied length with NULs inside, so a walk outgrows its
    // buffer repeatedly; churned keys interleave with them in key order.
    let stable: Vec<Vec<u8>> = (0..STABLE)
        .map(|i| format!("s{i:04}\0{}", "-".repeat(i % 37)).into_bytes())
        .collect();
    let churn: Vec<Vec<u8>> = (0..CHURN)
        .map(|i| format!("s{i:04}\x01churn").into_bytes())
        .collect();
    for (i, k) in (0u64..).zip(&stable) {
        assert!(ORDERED.insert(m, k, i));
    }
    let addr = m as usize;
    let stop = AtomicBool::new(false);
    std::thread::scope(|sc| {
        let (stable, churn, stop) = (&stable, &churn, &stop);
        for _ in 0..READERS {
            sc.spawn(move || {
                let (mut walks, mut retried) = (0u32, 0usize);
                loop {
                    let done = stop.load(Ordering::Acquire);
                    let (keys, too_small) = walk(addr as *const Map);
                    retried += too_small;
                    assert!(keys.windows(2).all(|w| w[0] < w[1]), "walk out of order");
                    let mut it = keys.iter();
                    for s in stable {
                        assert!(it.any(|k| k == s), "walk missed stable key {s:?}");
                    }
                    walks += 1;
                    if done {
                        break;
                    }
                }
                assert!(walks > 0);
                assert!(retried > 0, "the too-small retry was never taken");
            });
        }
        sc.spawn(move || {
            for round in 0..200u64 {
                for (i, k) in (0u64..).zip(churn) {
                    if round.is_multiple_of(2) {
                        ORDERED.insert(addr as *const Map, k, i);
                    } else {
                        ORDERED.remove(addr as *const Map, k);
                    }
                }
            }
            stop.store(true, Ordering::Release);
        });
    });
    // SAFETY: no reader handle was taken.
    unsafe {
        assert_eq!(expanse_sync_ordered_bytesmap_len(m), STABLE as u64);
        expanse_sync_ordered_bytesmap_free(m);
    }
}
