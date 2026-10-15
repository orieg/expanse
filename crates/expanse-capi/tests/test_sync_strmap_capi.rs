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
    SyncStrMapReader, expanse_sync_strmap_contains, expanse_sync_strmap_free,
    expanse_sync_strmap_get, expanse_sync_strmap_insert, expanse_sync_strmap_len,
    expanse_sync_strmap_mem_held, expanse_sync_strmap_mem_used, expanse_sync_strmap_new,
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
