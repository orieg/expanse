//! The shared string map's C handle, `expanse_sync_strmap_*` (#1538): every
//! entry point through its `extern "C"` function, the NULL conventions, the
//! reader lifecycle, and the threaded contracts of `include/expanse.h`.
//!
//! The single-threaded tests are the ones the Miri and ASan lanes run
//! (`.github/workflows/ci.yml`, jobs `miri` and `test-asan`): they cover the
//! `Box` round trips and the `'static` lifetime extension of the reader
//! handle. The threaded tests are ignored under Miri, where they would run
//! for far longer than the lane allows, and run everywhere else.

use core::ffi::{CStr, c_char};
use expanse::modern::{
    ExpanseStrNavStatus, expanse_sync_strmap_reader_first, expanse_sync_strmap_reader_last,
    expanse_sync_strmap_reader_next_after, expanse_sync_strmap_reader_next_at_or_after,
    expanse_sync_strmap_reader_prev_at_or_before, expanse_sync_strmap_reader_prev_before,
};
use expanse::modern::{
    SyncStrMapReader, expanse_sync_strmap_contains, expanse_sync_strmap_free,
    expanse_sync_strmap_get, expanse_sync_strmap_insert, expanse_sync_strmap_len,
    expanse_sync_strmap_mem_held, expanse_sync_strmap_mem_used,
    expanse_sync_strmap_mem_used_estimate, expanse_sync_strmap_new,
    expanse_sync_strmap_reader_contains, expanse_sync_strmap_reader_free,
    expanse_sync_strmap_reader_get, expanse_sync_strmap_reader_new, expanse_sync_strmap_remove,
    expanse_sync_strmap_shrink_to_fit,
};
use expanse_trie::sync::SyncExpanseStrMap;
use std::ffi::CString;

const NULL_KEY: *const c_char = core::ptr::null();

fn key(s: &str) -> CString {
    CString::new(s).expect("test keys carry no NUL")
}

/// `get` through the container into a local, `None` when it returns false.
fn get(m: *const SyncExpanseStrMap, k: &CStr) -> Option<u64> {
    let mut v = 0xDEAD_u64;
    // SAFETY: `m` is live for every caller; `k` is a C string; `v` a local.
    unsafe { expanse_sync_strmap_get(m, k.as_ptr(), &raw mut v) }.then_some(v)
}

/// `get` through a reader into a local, `None` when it returns false.
fn reader_get(r: *const SyncStrMapReader, k: &CStr) -> Option<u64> {
    let mut v = 0xDEAD_u64;
    // SAFETY: `r` is live for every caller; `k` is a C string; `v` a local.
    unsafe { expanse_sync_strmap_reader_get(r, k.as_ptr(), &raw mut v) }.then_some(v)
}

#[test]
fn point_operations_and_return_conventions() {
    let m = expanse_sync_strmap_new();
    assert!(!m.is_null());
    let (apple, empty, app) = (key("apple"), key(""), key("app"));
    let long = key(&"x".repeat(40)); // several 8-byte hops
    let mut old = 0u64;
    // SAFETY: `m` is live until the free at the end; keys are C strings;
    // out-pointers are locals or null.
    unsafe {
        assert_eq!(expanse_sync_strmap_len(m), 0);
        assert!(expanse_sync_strmap_insert(
            m,
            apple.as_ptr(),
            1,
            &raw mut old
        ));
        assert_eq!(old, 0, "a new key writes nothing through old_out");
        assert!(!expanse_sync_strmap_insert(
            m,
            apple.as_ptr(),
            2,
            &raw mut old
        ));
        assert_eq!(old, 1, "a replace reports the old value");
        assert!(!expanse_sync_strmap_insert(
            m,
            apple.as_ptr(),
            3,
            core::ptr::null_mut()
        ));
        assert!(expanse_sync_strmap_insert(
            m,
            empty.as_ptr(),
            10,
            core::ptr::null_mut()
        ));
        assert!(expanse_sync_strmap_insert(
            m,
            long.as_ptr(),
            20,
            core::ptr::null_mut()
        ));
        assert_eq!(expanse_sync_strmap_len(m), 3);

        assert!(expanse_sync_strmap_contains(m, apple.as_ptr()));
        assert!(!expanse_sync_strmap_contains(m, app.as_ptr()));
        assert!(expanse_sync_strmap_get(
            m,
            apple.as_ptr(),
            core::ptr::null_mut()
        ));
    }
    assert_eq!(get(m, &apple), Some(3));
    assert_eq!(get(m, &empty), Some(10));
    assert_eq!(get(m, &long), Some(20));
    assert_eq!(get(m, &app), None);

    // SAFETY: as above.
    unsafe {
        assert!(expanse_sync_strmap_mem_used(m) > 0);
        assert!(expanse_sync_strmap_mem_held(m) >= expanse_sync_strmap_mem_used(m));
        assert_eq!(
            expanse_sync_strmap_mem_used_estimate(m),
            expanse_sync_strmap_mem_used(m),
            "equal with nothing in flight (SyncExpanseStrMap::mem_used_estimate)"
        );

        let mut v = 0xDEAD_u64;
        assert!(!expanse_sync_strmap_remove(m, app.as_ptr(), &raw mut v));
        assert_eq!(v, 0xDEAD, "a miss writes nothing");
        assert!(expanse_sync_strmap_remove(m, apple.as_ptr(), &raw mut v));
        assert_eq!(v, 3);
        assert!(expanse_sync_strmap_remove(
            m,
            long.as_ptr(),
            core::ptr::null_mut()
        ));
        assert_eq!(expanse_sync_strmap_len(m), 1);
        let _released = expanse_sync_strmap_shrink_to_fit(m);
        assert_eq!(get(m, &empty), Some(10));
        expanse_sync_strmap_free(m);
    }
}

#[test]
fn null_handles_and_null_keys_write_nothing() {
    let null_map: *const SyncExpanseStrMap = core::ptr::null();
    let null_reader: *const SyncStrMapReader = core::ptr::null();
    let k = key("k");
    let mut v = 0xDEAD_u64;
    // SAFETY: every call is documented to accept a NULL handle and a NULL key;
    // the live map below outlives its reader.
    unsafe {
        assert!(!expanse_sync_strmap_insert(
            null_map,
            k.as_ptr(),
            1,
            &raw mut v
        ));
        assert!(!expanse_sync_strmap_get(null_map, k.as_ptr(), &raw mut v));
        assert!(!expanse_sync_strmap_contains(null_map, k.as_ptr()));
        assert!(!expanse_sync_strmap_remove(
            null_map,
            k.as_ptr(),
            &raw mut v
        ));
        assert_eq!(expanse_sync_strmap_len(null_map), 0);
        assert_eq!(expanse_sync_strmap_mem_used(null_map), 0);
        assert_eq!(expanse_sync_strmap_mem_used_estimate(null_map), 0);
        assert_eq!(expanse_sync_strmap_mem_held(null_map), 0);
        assert_eq!(expanse_sync_strmap_shrink_to_fit(null_map), 0);
        assert!(expanse_sync_strmap_reader_new(null_map).is_null());
        assert!(!expanse_sync_strmap_reader_get(
            null_reader,
            k.as_ptr(),
            &raw mut v
        ));
        assert!(!expanse_sync_strmap_reader_contains(
            null_reader,
            k.as_ptr()
        ));
        expanse_sync_strmap_reader_free(core::ptr::null_mut());
        expanse_sync_strmap_free(core::ptr::null_mut());

        let m = expanse_sync_strmap_new();
        assert!(expanse_sync_strmap_insert(
            m,
            k.as_ptr(),
            7,
            core::ptr::null_mut()
        ));
        assert!(!expanse_sync_strmap_insert(m, NULL_KEY, 1, &raw mut v));
        assert!(!expanse_sync_strmap_get(m, NULL_KEY, &raw mut v));
        assert!(!expanse_sync_strmap_contains(m, NULL_KEY));
        assert!(!expanse_sync_strmap_remove(m, NULL_KEY, &raw mut v));
        let r = expanse_sync_strmap_reader_new(m);
        assert!(!expanse_sync_strmap_reader_get(r, NULL_KEY, &raw mut v));
        assert!(!expanse_sync_strmap_reader_contains(r, NULL_KEY));
        expanse_sync_strmap_reader_free(r);
        assert_eq!(expanse_sync_strmap_len(m), 1, "a NULL key stored nothing");
        expanse_sync_strmap_free(m);
    }
    assert_eq!(
        v, 0xDEAD,
        "no NULL-handle or NULL-key call wrote through an out-pointer"
    );
}

#[test]
fn reader_sees_writes_and_is_freed_before_its_map() {
    const N: u64 = 64;
    let m = expanse_sync_strmap_new();
    let keys: Vec<CString> = (0..N).map(|i| key(&format!("key/{i:03}/suffix"))).collect();
    // SAFETY: `m` outlives both readers, which are freed before it.
    unsafe {
        let r = expanse_sync_strmap_reader_new(m);
        assert!(!r.is_null());
        for (i, k) in (0u64..).zip(&keys) {
            assert!(!expanse_sync_strmap_reader_contains(r, k.as_ptr()));
            assert!(expanse_sync_strmap_insert(
                m,
                k.as_ptr(),
                i,
                core::ptr::null_mut()
            ));
            assert_eq!(
                reader_get(r, k),
                Some(i),
                "the reader sees a completed write"
            );
            assert!(expanse_sync_strmap_reader_contains(r, k.as_ptr()));
        }
        // A second reader on the same map, on the same thread.
        let r2 = expanse_sync_strmap_reader_new(m);
        for (i, k) in (0u64..).zip(&keys).step_by(2) {
            assert!(expanse_sync_strmap_remove(
                m,
                k.as_ptr(),
                core::ptr::null_mut()
            ));
            assert_eq!(reader_get(r, k), None);
            assert_eq!(reader_get(r2, &keys[(i + 1) as usize]), Some(i + 1));
        }
        expanse_sync_strmap_reader_free(r2);
        expanse_sync_strmap_reader_free(r);
        assert_eq!(expanse_sync_strmap_len(m), N / 2);
        expanse_sync_strmap_free(m);
    }
}

/// One writer churns keys while readers on other threads read stable keys,
/// each of which must come back with its exact value on every read, and
/// churned keys, each of which must be absent or carry its writer's value.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn concurrent_readers_never_see_a_wrong_value() {
    const STABLE: u64 = 1024;
    const CHURN: u64 = 512;
    const READERS: usize = 4;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = expanse_sync_strmap_new();
    let stable: Vec<CString> = (0..STABLE)
        .map(|i| key(&format!("stable/{i:05}")))
        .collect();
    let churn: Vec<CString> = (0..CHURN)
        .map(|i| key(&format!("churn/{i:05}/a-longer-tail")))
        .collect();
    for (i, k) in (0u64..).zip(&stable) {
        // SAFETY: live handle; C-string key.
        assert!(unsafe { expanse_sync_strmap_insert(m, k.as_ptr(), i, core::ptr::null_mut()) });
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
                    let r = expanse_sync_strmap_reader_new(addr as *const _);
                    let mut passes = 0u32;
                    loop {
                        let done = stop.load(Ordering::Acquire);
                        for (i, k) in (0u64..).zip(stable) {
                            assert_eq!(reader_get(r, k), Some(i), "stable key {i}");
                        }
                        for (i, k) in (0u64..).zip(churn) {
                            if let Some(v) = reader_get(r, k) {
                                assert_eq!(v, 1_000_000 + i, "churned key {i}");
                            }
                        }
                        passes += 1;
                        if done {
                            break;
                        }
                    }
                    expanse_sync_strmap_reader_free(r);
                    assert!(passes > 0);
                }
            });
        }
        sc.spawn(move || {
            for round in 0..40u64 {
                for (i, k) in (0u64..).zip(churn) {
                    // SAFETY: live handle; writes serialize internally.
                    unsafe {
                        if round.is_multiple_of(2) {
                            expanse_sync_strmap_insert(
                                addr as *const _,
                                k.as_ptr(),
                                1_000_000 + i,
                                core::ptr::null_mut(),
                            );
                        } else {
                            expanse_sync_strmap_remove(
                                addr as *const _,
                                k.as_ptr(),
                                core::ptr::null_mut(),
                            );
                        }
                    }
                }
            }
            stop.store(true, Ordering::Release);
        });
    });
    // SAFETY: every reader was freed inside the scope.
    unsafe {
        assert_eq!(
            expanse_sync_strmap_len(m),
            STABLE,
            "the last round removed every churned key"
        );
        expanse_sync_strmap_free(m);
    }
}

/// Reader handles created and used on worker threads, then freed on the main
/// thread while a writer runs, before the map (`docs/COMPAT.md`, reader-handle
/// ownership).
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn reader_handles_freed_on_another_thread() {
    const THREADS: usize = 6;
    const N: u64 = 512;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = expanse_sync_strmap_new();
    let keys: Vec<CString> = (0..N).map(|i| key(&format!("k{i}"))).collect();
    for (i, k) in (0u64..).zip(&keys) {
        // SAFETY: live handle; C-string key.
        unsafe { expanse_sync_strmap_insert(m, k.as_ptr(), i * 7, core::ptr::null_mut()) };
    }
    let addr = m as usize;
    let handles: Vec<usize> = std::thread::scope(|sc| {
        let keys = &keys;
        let workers: Vec<_> = (0..THREADS)
            .map(|_| {
                sc.spawn(move || {
                    // SAFETY: the map outlives every handle; the handle is used
                    // only on this thread until returned, idle afterwards.
                    unsafe {
                        let r = expanse_sync_strmap_reader_new(addr as *const _);
                        assert!(!r.is_null());
                        for (i, k) in (0u64..).zip(keys) {
                            assert_eq!(reader_get(r, k), Some(i * 7));
                        }
                        r as usize
                    }
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
                let k = key(&format!("churn{}", i % 2048));
                // SAFETY: live handle; writes serialize internally.
                unsafe {
                    if i.is_multiple_of(2) {
                        expanse_sync_strmap_insert(
                            addr as *const _,
                            k.as_ptr(),
                            i,
                            core::ptr::null_mut(),
                        );
                    } else {
                        expanse_sync_strmap_remove(
                            addr as *const _,
                            k.as_ptr(),
                            core::ptr::null_mut(),
                        );
                    }
                }
                i += 1;
            }
        });
        for h in &handles {
            // SAFETY: each handle came from `_reader_new`, no call on it is in
            // progress, and the map is still alive.
            unsafe { expanse_sync_strmap_reader_free(*h as *mut SyncStrMapReader) };
        }
        stop.store(true, Ordering::Relaxed);
    });
    // SAFETY: every reader handle was freed above.
    unsafe {
        for (i, k) in (0u64..).zip(&keys) {
            assert_eq!(get(m, k), Some(i * 7));
        }
        expanse_sync_strmap_free(m);
    }
}

// ---- Ordered reads through the reader handle ----

type ByKeyNav = unsafe extern "C" fn(
    *const SyncStrMapReader,
    *const c_char,
    *mut c_char,
    usize,
    *mut usize,
    *mut u64,
) -> ExpanseStrNavStatus;

/// One by-key ordered read into a buffer of `buf_len` bytes: the status, the
/// key written (on `Ok`), `*required_len` and `*value_out`, each starting from
/// a sentinel so an unwritten output is visible.
fn nav(
    f: ByKeyNav,
    r: *const SyncStrMapReader,
    k: &CStr,
    buf_len: usize,
) -> (ExpanseStrNavStatus, Option<Vec<u8>>, usize, u64) {
    let mut buf = vec![0x7F_u8; buf_len.max(1)];
    let (mut req, mut v) = (usize::MAX, 0xDEAD_u64);
    // SAFETY: `r` is null or live for every caller; `k` a C string; `buf`
    // holds at least `buf_len` bytes; the outputs are locals.
    let st = unsafe {
        f(
            r,
            k.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf_len,
            &raw mut req,
            &raw mut v,
        )
    };
    let written = (st == ExpanseStrNavStatus::Ok).then(|| {
        // SAFETY: on `Ok` the key and its NUL are in `buf`.
        unsafe { CStr::from_ptr(buf.as_ptr().cast()) }
            .to_bytes()
            .to_vec()
    });
    if st != ExpanseStrNavStatus::Ok {
        assert!(
            buf.iter().all(|&b| b == 0x7F),
            "{st:?} wrote into the buffer"
        );
    }
    (st, written, req, v)
}

#[test]
fn ordered_reads_and_the_buffer_contract() {
    use ExpanseStrNavStatus::{BufferTooSmall, NotFound, Ok};
    let m = expanse_sync_strmap_new();
    let long = "apple pie, a long key over several hops";
    let entries: [(&str, u64); 5] = [
        ("", 1),
        ("apple", 2),
        (long, 3),
        ("banana", 4),
        ("cherry", 5),
    ];
    for (k, v) in entries {
        let k = key(k);
        // SAFETY: live handle; C-string key.
        assert!(unsafe { expanse_sync_strmap_insert(m, k.as_ptr(), v, core::ptr::null_mut()) });
    }
    // SAFETY: the map outlives the reader, freed below before it.
    let r = unsafe { expanse_sync_strmap_reader_new(m) };
    let (mut buf, mut req, mut v) = ([0x7F_u8; 64], 0usize, 0u64);
    let out: *mut c_char = buf.as_mut_ptr().cast();

    // first / last, the length probe, and all-NULL outputs.
    // SAFETY: live reader; `out` holds 64 bytes; the outputs are locals.
    unsafe {
        let st = expanse_sync_strmap_reader_first(r, out, 64, &raw mut req, &raw mut v);
        assert_eq!(
            (st, req, v, *out),
            (Ok, 1, 1, 0),
            "the empty key is the smallest"
        );
        let st = expanse_sync_strmap_reader_last(r, out, 64, &raw mut req, &raw mut v);
        assert_eq!((st, req, v), (Ok, 7, 5));
        assert_eq!(CStr::from_ptr(out).to_bytes(), b"cherry");
        v = 0xDEAD;
        let st =
            expanse_sync_strmap_reader_last(r, core::ptr::null_mut(), 0, &raw mut req, &raw mut v);
        assert_eq!(
            (st, req, v),
            (BufferTooSmall, 7, 0xDEAD),
            "a probe writes only required_len"
        );
        let null_out = core::ptr::null_mut();
        let st = expanse_sync_strmap_reader_first(r, null_out, 0, null_out.cast(), null_out.cast());
        assert_eq!(st, BufferTooSmall);
    }

    let (apple, banana, b, z, empty) = (key("apple"), key("banana"), key("b"), key("z"), key(""));
    let n: ByKeyNav = expanse_sync_strmap_reader_next_at_or_after;
    let na: ByKeyNav = expanse_sync_strmap_reader_next_after;
    let p: ByKeyNav = expanse_sync_strmap_reader_prev_at_or_before;
    let pb: ByKeyNav = expanse_sync_strmap_reader_prev_before;
    let lk = long.as_bytes().to_vec();
    assert_eq!(nav(n, r, &apple, 64), (Ok, Some(b"apple".to_vec()), 6, 2));
    assert_eq!(nav(n, r, &b, 64), (Ok, Some(b"banana".to_vec()), 7, 4));
    assert_eq!(
        nav(na, r, &apple, 64),
        (Ok, Some(lk.clone()), long.len() + 1, 3)
    );
    assert_eq!(nav(na, r, &z, 64), (NotFound, None, usize::MAX, 0xDEAD));
    assert_eq!(nav(p, r, &banana, 64), (Ok, Some(b"banana".to_vec()), 7, 4));
    assert_eq!(nav(p, r, &b, 64), (Ok, Some(lk), long.len() + 1, 3));
    assert_eq!(nav(pb, r, &apple, 64), (Ok, Some(Vec::new()), 1, 1));
    assert_eq!(nav(pb, r, &empty, 64), (NotFound, None, usize::MAX, 0xDEAD));

    // One byte short (no room for the NUL): nothing written, required length
    // exact; then the exact length succeeds.
    let short = nav(na, r, &apple, long.len());
    assert_eq!(short, (BufferTooSmall, None, long.len() + 1, 0xDEAD));
    assert_eq!(nav(na, r, &apple, long.len() + 1).0, Ok);

    // NULL reader and NULL key: NotFound, nothing written.
    let null_r: *const SyncStrMapReader = core::ptr::null();
    assert_eq!(
        nav(n, null_r, &apple, 64),
        (NotFound, None, usize::MAX, 0xDEAD)
    );
    req = usize::MAX;
    // SAFETY: a NULL reader and a NULL key are documented inputs; `out` holds
    // 64 bytes; the outputs are locals.
    unsafe {
        let st = expanse_sync_strmap_reader_first(null_r, out, 64, &raw mut req, &raw mut v);
        assert_eq!((st, req), (NotFound, usize::MAX));
        let st = n(r, NULL_KEY, out, 64, &raw mut req, &raw mut v);
        assert_eq!((st, req), (NotFound, usize::MAX));
    }

    // An empty map: every read is NotFound.
    // SAFETY: `r` is freed before `m`; `er` before `e`.
    unsafe {
        expanse_sync_strmap_reader_free(r);
        expanse_sync_strmap_free(m);
        let e = expanse_sync_strmap_new();
        let er = expanse_sync_strmap_reader_new(e);
        let st = expanse_sync_strmap_reader_first(er, out, 64, &raw mut req, &raw mut v);
        assert_eq!(st, NotFound);
        assert_eq!(nav(p, er, &z, 64).0, NotFound);
        expanse_sync_strmap_reader_free(er);
        expanse_sync_strmap_free(e);
    }
}

/// Walks the map with `_reader_first` and `_reader_next_after` into a buffer
/// that starts too small, growing it from `required_len` on every
/// `BufferTooSmall`. Returns the keys in walk order and how many reads came
/// back too small.
fn walk(r: *const SyncStrMapReader) -> (Vec<Vec<u8>>, usize) {
    let mut buf = vec![0u8; 4];
    let (mut out, mut too_small) = (Vec::new(), 0usize);
    let mut prev: Option<CString> = None;
    loop {
        let mut req = 0usize;
        let (dst, len, value) = (buf.as_mut_ptr().cast(), buf.len(), core::ptr::null_mut());
        // SAFETY: `r` is live; `dst` holds `len` bytes; `prev` is a C string;
        // `req` is a local.
        let st = unsafe {
            match &prev {
                None => expanse_sync_strmap_reader_first(r, dst, len, &raw mut req, value),
                Some(k) => expanse_sync_strmap_reader_next_after(
                    r,
                    k.as_ptr(),
                    dst,
                    len,
                    &raw mut req,
                    value,
                ),
            }
        };
        match st {
            ExpanseStrNavStatus::NotFound => return (out, too_small),
            ExpanseStrNavStatus::BufferTooSmall => {
                too_small += 1;
                assert!(req > buf.len());
                buf.resize(req, 0);
            }
            ExpanseStrNavStatus::Ok => {
                // SAFETY: on `Ok` the key and its NUL are in `buf`.
                let k = unsafe { CStr::from_ptr(buf.as_ptr().cast()) }.to_owned();
                assert_eq!(k.as_bytes().len() + 1, req);
                out.push(k.as_bytes().to_vec());
                prev = Some(k);
            }
        }
    }
}

/// The gate of #1538 step 2: a walk built from `_reader_next_after` over a
/// map under concurrent inserts and removes returns every key present
/// throughout, in strictly increasing order, with the too-small retry taken.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn ordered_walk_under_concurrent_writes_sees_every_stable_key_in_order() {
    const STABLE: usize = 600;
    const CHURN: usize = 600;
    const READERS: usize = 3;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = expanse_sync_strmap_new();
    // Stable keys of varied length, so a walk outgrows its buffer repeatedly.
    let stable: Vec<CString> = (0..STABLE)
        .map(|i| key(&format!("s{i:04}{}", "-".repeat(i % 37))))
        .collect();
    // Churned keys interleave with the stable ones in key order.
    let churn: Vec<CString> = (0..CHURN).map(|i| key(&format!("s{i:04}~churn"))).collect();
    for (i, k) in (0u64..).zip(&stable) {
        // SAFETY: live handle; C-string key.
        assert!(unsafe { expanse_sync_strmap_insert(m, k.as_ptr(), i, core::ptr::null_mut()) });
    }
    let addr = m as usize;
    let stop = AtomicBool::new(false);
    std::thread::scope(|sc| {
        let (stable, churn, stop) = (&stable, &churn, &stop);
        for _ in 0..READERS {
            sc.spawn(move || {
                // SAFETY: the map outlives the scope; this thread's reader is
                // used only here and freed before the scope ends.
                let r = unsafe { expanse_sync_strmap_reader_new(addr as *const _) };
                let (mut walks, mut retried) = (0u32, 0usize);
                loop {
                    let done = stop.load(Ordering::Acquire);
                    let (keys, too_small) = walk(r);
                    retried += too_small;
                    assert!(keys.windows(2).all(|w| w[0] < w[1]), "walk out of order");
                    let mut it = keys.iter();
                    for s in stable {
                        let seen = it.any(|k| k.as_slice() == s.as_bytes());
                        assert!(seen, "walk missed stable key {s:?}");
                    }
                    walks += 1;
                    if done {
                        break;
                    }
                }
                // SAFETY: no call on `r` is in progress; the map is alive.
                unsafe { expanse_sync_strmap_reader_free(r) };
                assert!(walks > 0);
                assert!(retried > 0, "the too-small retry was never taken");
            });
        }
        sc.spawn(move || {
            for round in 0..200u64 {
                for (i, k) in (0u64..).zip(churn) {
                    let m = addr as *const SyncExpanseStrMap;
                    // SAFETY: live handle; writes serialize internally.
                    unsafe {
                        if round.is_multiple_of(2) {
                            expanse_sync_strmap_insert(m, k.as_ptr(), i, core::ptr::null_mut());
                        } else {
                            expanse_sync_strmap_remove(m, k.as_ptr(), core::ptr::null_mut());
                        }
                    }
                }
            }
            stop.store(true, Ordering::Release);
        });
    });
    // SAFETY: every reader was freed inside the scope.
    unsafe { expanse_sync_strmap_free(m) };
}

// ---- Read-modify-write: `_compare_exchange` and `_get_or_insert` ----

use expanse::modern::{expanse_sync_strmap_compare_exchange, expanse_sync_strmap_get_or_insert};

/// One exchange into locals: `(exchanged, observed)`, with sentinels that
/// show which outputs the call wrote.
fn cx(
    m: *const SyncExpanseStrMap,
    k: &CStr,
    expected: Option<u64>,
    new: Option<u64>,
) -> (bool, Option<u64>) {
    let mut seen_present = expected.is_none();
    let mut seen = 0xDEAD_u64;
    // SAFETY: `m` is live for every caller; `k` is a C string; outputs are
    // locals.
    let ok = unsafe {
        expanse_sync_strmap_compare_exchange(
            m,
            k.as_ptr(),
            expected.is_some(),
            expected.unwrap_or(0),
            new.is_some(),
            new.unwrap_or(0),
            &raw mut seen_present,
            &raw mut seen,
        )
    };
    (ok, seen_present.then_some(seen))
}

/// `get_or_insert` into a local: `(stored, existing)`, `existing` the
/// sentinel-free value written when the call returned false.
fn goi(m: *const SyncExpanseStrMap, k: &CStr, v: u64) -> (bool, Option<u64>) {
    let mut existing = 0xDEAD_u64;
    // SAFETY: as for `cx`.
    let stored = unsafe { expanse_sync_strmap_get_or_insert(m, k.as_ptr(), v, &raw mut existing) };
    (stored, (existing != 0xDEAD).then_some(existing))
}

#[test]
fn compare_exchange_and_get_or_insert_every_branch() {
    let m = expanse_sync_strmap_new();
    let (a, b, c, d, z) = (
        key("a"),
        key("b/longer-than-a-hop"),
        key("c"),
        key(""),
        key("z"),
    );

    // Absent -> insert; expected-absent on a present key fails with the word.
    assert_eq!(cx(m, &a, None, Some(10)), (true, None));
    assert_eq!(cx(m, &a, None, Some(99)), (false, Some(10)));
    assert_eq!(get(m, &a), Some(10));
    // Replace, then a stale expected fails with the current word.
    assert_eq!(cx(m, &a, Some(10), Some(11)), (true, Some(10)));
    assert_eq!(cx(m, &a, Some(10), Some(12)), (false, Some(11)));
    assert_eq!(get(m, &a), Some(11));
    // Expected-present on an absent key: observed absent, nothing stored.
    assert_eq!(cx(m, &b, Some(0), Some(5)), (false, None));
    assert_eq!(get(m, &b), None);
    // Remove only on a matching word.
    assert_eq!(cx(m, &a, Some(12), None), (false, Some(11)));
    assert_eq!(cx(m, &a, Some(11), None), (true, Some(11)));
    assert_eq!(get(m, &a), None);
    // (absent, absent): a validated absence check that never writes.
    assert_eq!(cx(m, &c, None, None), (true, None));
    assert_eq!(cx(m, &c, None, Some(3)), (true, None));
    assert_eq!(cx(m, &c, None, None), (false, Some(3)));
    // Every u64 is a legal value, and the empty string a legal key.
    assert_eq!(cx(m, &d, None, Some(0)), (true, None));
    assert_eq!(cx(m, &d, Some(0), Some(u64::MAX)), (true, Some(0)));
    assert_eq!(get(m, &d), Some(u64::MAX));

    // get_or_insert: stores once, then reports the stored word untouched.
    assert_eq!(goi(m, &b, 7), (true, None));
    assert_eq!(goi(m, &b, 8), (false, Some(7)));
    assert_eq!(get(m, &b), Some(7));
    assert_eq!(goi(m, &z, 0), (true, None));
    assert_eq!(goi(m, &z, 1), (false, Some(0)));

    let null_map: *const SyncExpanseStrMap = core::ptr::null();
    let (mut present, mut seen, mut existing) = (true, 0xDEAD_u64, 0xDEAD_u64);
    // SAFETY: NULL handles and NULL keys are documented to write nothing;
    // NULL outputs are documented as accepted; `m` is live.
    unsafe {
        for (map, k) in [(null_map, a.as_ptr()), (m.cast_const(), NULL_KEY)] {
            assert!(!expanse_sync_strmap_compare_exchange(
                map,
                k,
                false,
                0,
                true,
                1,
                &raw mut present,
                &raw mut seen
            ));
            assert!(!expanse_sync_strmap_get_or_insert(
                map,
                k,
                1,
                &raw mut existing
            ));
        }
        assert!(present && seen == 0xDEAD && existing == 0xDEAD);
        assert!(expanse_sync_strmap_compare_exchange(
            m,
            b.as_ptr(),
            true,
            7,
            true,
            9,
            core::ptr::null_mut(),
            core::ptr::null_mut()
        ));
        assert!(!expanse_sync_strmap_get_or_insert(
            m,
            b.as_ptr(),
            1,
            core::ptr::null_mut()
        ));
        assert_eq!(expanse_sync_strmap_len(m), 4, "b, c, the empty key and z");
        expanse_sync_strmap_free(m);
    }
}

/// Threads incrementing shared counters through the `compare_exchange` retry
/// loop of `include/expanse.h`: every increment lands exactly once.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn concurrent_compare_exchange_increments_lose_no_update() {
    const THREADS: u64 = 4;
    const PER_THREAD: u64 = 2000;
    let m = expanse_sync_strmap_new();
    // Two counters on keys sharing a long prefix, so the walks contend.
    let keys = [
        key("counter/shared-prefix/0"),
        key("counter/shared-prefix/1"),
    ];
    let addr = m as usize; // raw pointers are not `Send`
    let barrier = std::sync::Barrier::new(THREADS as usize);
    std::thread::scope(|sc| {
        for t in 0..THREADS {
            let (keys, barrier) = (&keys, &barrier);
            sc.spawn(move || {
                let m = addr as *const SyncExpanseStrMap;
                barrier.wait();
                for i in 0..PER_THREAD {
                    let k = &keys[((t + i) % 2) as usize];
                    let mut cur = get(m, k);
                    loop {
                        match cx(m, k, cur, Some(cur.unwrap_or(0) + 1)) {
                            (true, _) => break,
                            (false, seen) => cur = seen,
                        }
                    }
                }
            });
        }
    });
    let total: u64 = keys.iter().map(|k| get(m, k).unwrap_or(0)).sum();
    assert_eq!(total, THREADS * PER_THREAD, "an increment was lost");
    // SAFETY: no call is in progress and no reader exists.
    unsafe { expanse_sync_strmap_free(m) };
}

/// Threads racing `get_or_insert` on the same absent keys: exactly one call
/// per key stores, and every other call reports the winner's value.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn concurrent_get_or_insert_agrees_on_one_winner() {
    const THREADS: u64 = 4;
    const KEYS: u64 = 1024;
    let m = expanse_sync_strmap_new();
    let keys: Vec<CString> = (0..KEYS).map(|i| key(&format!("race/{i:05}"))).collect();
    let addr = m as usize; // raw pointers are not `Send`
    let barrier = std::sync::Barrier::new(THREADS as usize);
    let results: Vec<Vec<(bool, Option<u64>)>> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let (keys, barrier) = (&keys, &barrier);
                sc.spawn(move || {
                    let m = addr as *const SyncExpanseStrMap;
                    (0..KEYS)
                        .map(|i| {
                            // Every thread reaches each key together.
                            barrier.wait();
                            goi(m, &keys[i as usize], t * 1_000_000 + i)
                        })
                        .collect()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for i in 0..KEYS as usize {
        let winners: Vec<u64> = (0..THREADS).filter(|&t| results[t as usize][i].0).collect();
        assert_eq!(winners.len(), 1, "key {i}: winners {winners:?}");
        let value = winners[0] * 1_000_000 + i as u64;
        for (t, per_thread) in results.iter().enumerate() {
            if !per_thread[i].0 {
                assert_eq!(per_thread[i].1, Some(value), "key {i}, thread {t}");
            }
        }
        assert_eq!(get(m, &keys[i]), Some(value));
    }
    // SAFETY: no call is in progress and no reader exists.
    unsafe { expanse_sync_strmap_free(m) };
}
