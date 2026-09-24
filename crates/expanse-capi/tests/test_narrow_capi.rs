//! The 32-bit-only C ABI surface (`EXPANSE_WIDE_SURFACE == 0`). Runs on the
//! i686 CI job — the one host-runnable 32-bit target — where a 32-bit
//! `libexpanse` actually exists; on 64-bit hosts this file is empty.
#![cfg(target_pointer_width = "32")]

use core::ffi::c_void;
use expanse::modern::{
    expanse_map_for_each_range, expanse_map_free, expanse_map_insert, expanse_map_len,
    expanse_map_new, expanse_map_remove_range,
};

unsafe extern "C" fn collect(key: u32, value: u32, ctx: *mut c_void) {
    // SAFETY: the test passes a live `Vec<(u32, u32)>` as the context and
    // does not touch it while the call is in progress.
    let seen = unsafe { &mut *ctx.cast::<Vec<(u32, u32)>>() };
    seen.push((key, value));
}

#[test]
fn remove_range_reports_ascending_and_counts() {
    let mut seen: Vec<(u32, u32)> = Vec::new();
    // SAFETY: `m` is a live handle from `expanse_map_new` until the final
    // `expanse_map_free`; out-pointers are null; the callback's context is
    // `seen`, live for the whole block; null map / inverted range are
    // documented no-ops.
    unsafe {
        let m = expanse_map_new();
        for k in 0..1000u32 {
            assert!(expanse_map_insert(m, k * 3, !k, core::ptr::null_mut()));
        }
        let n = expanse_map_remove_range(m, 30, 300, Some(collect), (&raw mut seen).cast());
        // Multiples of 3 in [30, 300]: 30, 33, ..., 300.
        assert_eq!(n, 91);
        assert_eq!(seen.len(), 91);
        assert!(
            seen.windows(2).all(|w| w[0].0 < w[1].0),
            "ascending key order"
        );
        assert_eq!(seen[0], (30, !10));
        assert_eq!(seen[90], (300, !100));
        assert_eq!(expanse_map_len(m), 1000 - 91);

        // Inverted range, null callback, null map: all no-ops returning 0.
        assert_eq!(
            expanse_map_remove_range(m, 300, 30, None, core::ptr::null_mut()),
            0
        );
        assert_eq!(
            expanse_map_remove_range(
                core::ptr::null_mut(),
                0,
                u32::MAX,
                None,
                core::ptr::null_mut()
            ),
            0
        );
        assert_eq!(
            expanse_map_remove_range(m, 0, u32::MAX, None, core::ptr::null_mut()),
            1000 - 91
        );
        assert_eq!(expanse_map_len(m), 0);
        expanse_map_free(m);
    }
}

// ---- expanse_sync32_*: the 32-bit concurrent surface ----

mod sync32_surface {
    use core::ffi::CStr;
    use expanse::modern_sync32::{
        ExpanseSync32Stats, ExpanseSync32Status, expanse_sync32_map_free, expanse_sync32_map_new,
        expanse_sync32_map_reader, expanse_sync32_map_reader_try_first,
        expanse_sync32_map_reader_try_get, expanse_sync32_map_reader_try_last,
        expanse_sync32_map_reader_try_len, expanse_sync32_map_reader_try_next_after,
        expanse_sync32_map_reader_try_next_at_or_after,
        expanse_sync32_map_reader_try_prev_at_or_before, expanse_sync32_map_reader_try_prev_before,
        expanse_sync32_map_writer, expanse_sync32_map_writer_get, expanse_sync32_map_writer_stats,
        expanse_sync32_map_writer_try_insert, expanse_sync32_map_writer_try_reclaim,
        expanse_sync32_map_writer_try_remove, expanse_sync32_mutation_headroom,
        expanse_sync32_set_free, expanse_sync32_set_new, expanse_sync32_set_reader,
        expanse_sync32_set_reader_try_contains, expanse_sync32_set_writer,
        expanse_sync32_set_writer_contains, expanse_sync32_set_writer_try_insert,
        expanse_sync32_set_writer_try_remove, expanse_sync32_status_str,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn map_flow_status_codes_and_handles() {
        let headroom = expanse_sync32_mutation_headroom();
        assert!(headroom >= 1);
        // Below the headroom: refused at construction.
        assert!(expanse_sync32_map_new(headroom - 1, 1).is_null());
        // SAFETY: every handle comes from this container and is used from
        // one thread; the container is freed once, last.
        unsafe {
            let m = expanse_sync32_map_new(4096, 2);
            assert!(!m.is_null());
            let w = expanse_sync32_map_writer(m);
            assert!(!w.is_null());
            assert_eq!(
                expanse_sync32_map_writer(m),
                w,
                "writer accessor is idempotent"
            );
            let r0 = expanse_sync32_map_reader(m, 0);
            let r1 = expanse_sync32_map_reader(m, 1);
            assert!(!r0.is_null() && !r1.is_null() && r0 != r1);
            assert_eq!(
                expanse_sync32_map_reader(m, 0),
                r0,
                "reader accessor is idempotent"
            );
            assert!(
                expanse_sync32_map_reader(m, 2).is_null(),
                "out of range index"
            );

            let mut replaced = true;
            let mut old = 0u32;
            assert_eq!(
                expanse_sync32_map_writer_try_insert(w, 7, 70, &raw mut replaced, &raw mut old),
                ExpanseSync32Status::Ok
            );
            assert!(!replaced);
            assert_eq!(
                expanse_sync32_map_writer_try_insert(w, 7, 71, &raw mut replaced, &raw mut old),
                ExpanseSync32Status::Ok
            );
            assert!(replaced);
            assert_eq!(old, 70);

            let mut v = 0u32;
            assert!(expanse_sync32_map_writer_get(w, 7, &raw mut v));
            assert_eq!(v, 71);
            assert!(!expanse_sync32_map_writer_get(w, 8, &raw mut v));

            assert_eq!(
                expanse_sync32_map_reader_try_get(r0, 7, &raw mut v),
                ExpanseSync32Status::Ok
            );
            assert_eq!(v, 71);
            assert_eq!(
                expanse_sync32_map_reader_try_get(r1, 8, &raw mut v),
                ExpanseSync32Status::NotFound
            );
            let mut n = 0u64;
            assert_eq!(
                expanse_sync32_map_reader_try_len(r0, &raw mut n),
                ExpanseSync32Status::Ok
            );
            assert_eq!(n, 1);

            let mut st = ExpanseSync32Stats::default();
            assert_eq!(
                expanse_sync32_map_writer_stats(
                    w,
                    &raw mut st,
                    core::mem::size_of::<ExpanseSync32Stats>()
                ),
                ExpanseSync32Status::Ok
            );
            assert_eq!(st.len, 1);
            assert!(st.free_slots > headroom && st.mem_used > 0);
            // A caller compiled against a shorter prefix gets only that prefix.
            let mut short = ExpanseSync32Stats {
                len: 99,
                ..Default::default()
            };
            assert_eq!(
                expanse_sync32_map_writer_stats(w, &raw mut short, core::mem::size_of::<u64>()),
                ExpanseSync32Status::Ok
            );
            assert_eq!(short.len, 1);
            assert_eq!(short.free_slots, 0, "beyond the prefix is untouched");

            assert_eq!(
                expanse_sync32_map_writer_try_remove(w, 8, &raw mut old),
                ExpanseSync32Status::NotFound
            );
            assert_eq!(
                expanse_sync32_map_writer_try_remove(w, 7, &raw mut old),
                ExpanseSync32Status::Ok
            );
            assert_eq!(old, 71);
            assert_eq!(
                expanse_sync32_map_writer_try_reclaim(w),
                ExpanseSync32Status::Ok
            );

            // Null handles are a status, never a crash, in release builds.
            if cfg!(not(debug_assertions)) {
                assert_eq!(
                    expanse_sync32_map_reader_try_get(core::ptr::null_mut(), 1, &raw mut v),
                    ExpanseSync32Status::NullHandle
                );
            }
            expanse_sync32_map_free(m);
            expanse_sync32_map_free(core::ptr::null_mut());
        }
        for (code, name) in [
            (0, "ok"),
            (1, "not_found"),
            (2, "busy"),
            (16, "arena_full"),
            (17, "reclaim_backlog"),
            (32, "null_handle"),
            (99, "unknown"),
        ] {
            // SAFETY: static NUL-terminated strings.
            let s = unsafe { CStr::from_ptr(expanse_sync32_status_str(code)) };
            assert_eq!(s.to_str().unwrap(), name);
        }
    }

    /// Runs one ordered read into locals: `Some((key, value))` for `OK`,
    /// `None` for `NOT_FOUND`, and a panic for any other status.
    fn entry(read: impl FnOnce(*mut u32, *mut u32) -> ExpanseSync32Status) -> Option<(u32, u32)> {
        let (mut k, mut v) = (0u32, 0u32);
        match read(&raw mut k, &raw mut v) {
            ExpanseSync32Status::Ok => Some((k, v)),
            ExpanseSync32Status::NotFound => None,
            other => panic!("unexpected status {other:?}"),
        }
    }

    /// The six single-attempt ordered reads (#900) on a quiescent map: found,
    /// absent, both ends of the key space, NULL out-pointers and a NULL
    /// handle. No writer bracket is open here, so `BUSY` cannot occur; the
    /// churn test below exercises the same reads against a live writer.
    #[test]
    fn map_reader_ordered_reads() {
        let e = |key: u32| Some((key, !key));
        // SAFETY: `m` is live until the final free; reader 0 and the writer
        // are used from this thread only; out-pointers are live locals or null.
        unsafe {
            let m = expanse_sync32_map_new(4096, 1);
            assert!(!m.is_null());
            let w = expanse_sync32_map_writer(m);
            let r = expanse_sync32_map_reader(m, 0);

            // Empty: nothing in either direction, from either end.
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_first(r, k, v)),
                None
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_last(r, k, v)),
                None
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_at_or_after(r, 0, k, v)),
                None
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_prev_at_or_before(r, u32::MAX, k, v)),
                None
            );

            for key in [0u32, 10, 20, 30, u32::MAX] {
                assert_eq!(
                    expanse_sync32_map_writer_try_insert(
                        w,
                        key,
                        !key,
                        core::ptr::null_mut(),
                        core::ptr::null_mut()
                    ),
                    ExpanseSync32Status::Ok
                );
            }

            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_first(r, k, v)),
                e(0)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_last(r, k, v)),
                e(u32::MAX)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_at_or_after(r, 15, k, v)),
                e(20)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_at_or_after(r, 20, k, v)),
                e(20)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_after(r, 20, k, v)),
                e(30)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_after(r, 30, k, v)),
                e(u32::MAX)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_at_or_after(r, u32::MAX, k, v)),
                e(u32::MAX)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_after(r, u32::MAX, k, v)),
                None
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_prev_at_or_before(r, 25, k, v)),
                e(20)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_prev_at_or_before(r, 20, k, v)),
                e(20)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_prev_before(r, 20, k, v)),
                e(10)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_prev_at_or_before(r, 0, k, v)),
                e(0)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_prev_before(r, 0, k, v)),
                None
            );

            // Remove both ends: the searches that reached them now come up empty.
            for key in [0u32, u32::MAX] {
                assert_eq!(
                    expanse_sync32_map_writer_try_remove(w, key, core::ptr::null_mut()),
                    ExpanseSync32Status::Ok
                );
            }
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_prev_at_or_before(r, 5, k, v)),
                None
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_next_at_or_after(r, 31, k, v)),
                None
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_first(r, k, v)),
                e(10)
            );
            assert_eq!(
                entry(|k, v| expanse_sync32_map_reader_try_last(r, k, v)),
                e(30)
            );

            // NULL out-pointers: the status alone.
            assert_eq!(
                expanse_sync32_map_reader_try_next_after(
                    r,
                    10,
                    core::ptr::null_mut(),
                    core::ptr::null_mut()
                ),
                ExpanseSync32Status::Ok
            );

            // Null handles are a status, never a crash, in release builds.
            if cfg!(not(debug_assertions)) {
                assert_eq!(
                    entry(|k, v| expanse_sync32_map_reader_try_prev_before(
                        core::ptr::null_mut(),
                        30,
                        k,
                        v
                    ))
                    .map(|_| ()),
                    None
                );
            }
            expanse_sync32_map_free(m);
        }
    }

    #[test]
    fn set_flow() {
        // SAFETY: as in the map flow.
        unsafe {
            let s = expanse_sync32_set_new(4096, 1);
            let w = expanse_sync32_set_writer(s);
            let r = expanse_sync32_set_reader(s, 0);
            let mut inserted = false;
            assert_eq!(
                expanse_sync32_set_writer_try_insert(w, 5, &raw mut inserted),
                ExpanseSync32Status::Ok
            );
            assert!(inserted);
            assert_eq!(
                expanse_sync32_set_writer_try_insert(w, 5, &raw mut inserted),
                ExpanseSync32Status::Ok
            );
            assert!(!inserted);
            assert!(expanse_sync32_set_writer_contains(w, 5));
            assert_eq!(
                expanse_sync32_set_reader_try_contains(r, 5),
                ExpanseSync32Status::Ok
            );
            assert_eq!(
                expanse_sync32_set_reader_try_contains(r, 6),
                ExpanseSync32Status::NotFound
            );
            assert_eq!(
                expanse_sync32_set_writer_try_remove(w, 6),
                ExpanseSync32Status::NotFound
            );
            assert_eq!(
                expanse_sync32_set_writer_try_remove(w, 5),
                ExpanseSync32Status::Ok
            );
            expanse_sync32_set_free(s);
        }
    }

    /// The Rust stress shape through the C surface: a writer thread churns
    /// a disjoint key range while readers on their own handles read stable
    /// keys and must see exactly their values — or BUSY, never a torn one.
    #[test]
    fn readers_see_stable_keys_under_writer_churn() {
        const STABLE: u32 = 256;
        let m = expanse_sync32_map_new(16_384, 2);
        assert!(!m.is_null());
        // SAFETY: the writer handle is used from one thread, each reader
        // handle from one thread, all joined before the container is freed.
        unsafe {
            let w = expanse_sync32_map_writer(m);
            for k in 0..STABLE {
                assert_eq!(
                    expanse_sync32_map_writer_try_insert(
                        w,
                        k,
                        k ^ 0xABCD,
                        core::ptr::null_mut(),
                        core::ptr::null_mut()
                    ),
                    ExpanseSync32Status::Ok
                );
            }
        }
        let stop = AtomicBool::new(false);
        let m_addr = m as usize;
        std::thread::scope(|s| {
            for idx in 0..2usize {
                let stop = &stop;
                s.spawn(move || {
                    // SAFETY: reader `idx` belongs to this thread alone.
                    let r = unsafe { expanse_sync32_map_reader(m_addr as *mut _, idx) };
                    let (mut ok, mut busy) = (0u32, 0u32);
                    let mut k = 0u32;
                    while !stop.load(Ordering::Relaxed) || ok < 10_000 {
                        // Stable keys are 0..STABLE and the churn range starts
                        // at STABLE, so each read below has exactly one right
                        // answer whatever the writer is doing. The ordered
                        // reads (#900) rotate in with the point read; `k`
                        // steps by 7, so `k % 6` visits every arm.
                        let s = k % STABLE;
                        let (mut got, mut v) = (s, 0u32);
                        // SAFETY: handle contract above; out-pointers are locals.
                        let (status, want) = unsafe {
                            match k % 6 {
                                0 => (expanse_sync32_map_reader_try_get(r, s, &raw mut v), s),
                                1 => (
                                    expanse_sync32_map_reader_try_next_at_or_after(
                                        r,
                                        s,
                                        &raw mut got,
                                        &raw mut v,
                                    ),
                                    s,
                                ),
                                2 => {
                                    let t = s.min(STABLE - 2);
                                    (
                                        expanse_sync32_map_reader_try_next_after(
                                            r,
                                            t,
                                            &raw mut got,
                                            &raw mut v,
                                        ),
                                        t + 1,
                                    )
                                }
                                3 => (
                                    expanse_sync32_map_reader_try_prev_at_or_before(
                                        r,
                                        s,
                                        &raw mut got,
                                        &raw mut v,
                                    ),
                                    s,
                                ),
                                4 => {
                                    let u = s.max(1);
                                    (
                                        expanse_sync32_map_reader_try_prev_before(
                                            r,
                                            u,
                                            &raw mut got,
                                            &raw mut v,
                                        ),
                                        u - 1,
                                    )
                                }
                                _ => (
                                    expanse_sync32_map_reader_try_first(
                                        r,
                                        &raw mut got,
                                        &raw mut v,
                                    ),
                                    0,
                                ),
                            }
                        };
                        match status {
                            ExpanseSync32Status::Ok => {
                                assert_eq!((got, v), (want, want ^ 0xABCD), "stable entry torn");
                                ok += 1;
                            }
                            ExpanseSync32Status::Busy => busy += 1,
                            other => panic!("unexpected status {other:?}"),
                        }
                        k = k.wrapping_add(7);
                        if ok >= 10_000 && stop.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    assert!(busy < 100_000_000, "reader starved");
                });
            }
            // SAFETY: the writer handle is used from this thread only.
            unsafe {
                let w = expanse_sync32_map_writer(m_addr as *mut _);
                let mut refused = 0u32;
                for i in 0..40_000u32 {
                    let k = STABLE + (i.wrapping_mul(2_654_435_761) % 4096);
                    let st = if i % 3 == 2 {
                        expanse_sync32_map_writer_try_remove(w, k, core::ptr::null_mut())
                    } else {
                        expanse_sync32_map_writer_try_insert(
                            w,
                            k,
                            i,
                            core::ptr::null_mut(),
                            core::ptr::null_mut(),
                        )
                    };
                    match st {
                        ExpanseSync32Status::Ok | ExpanseSync32Status::NotFound => {}
                        ExpanseSync32Status::ArenaFull | ExpanseSync32Status::ReclaimBacklog => {
                            refused += 1;
                            expanse_sync32_map_writer_try_reclaim(w);
                        }
                        other => panic!("unexpected writer status {other:?}"),
                    }
                }
                assert!(refused < 40_000, "writer permanently refused");
            }
            stop.store(true, Ordering::Relaxed);
        });
        // SAFETY: every thread that held a handle has been joined.
        unsafe { expanse_sync32_map_free(m) };
    }
}

/// Context for [`visit`]: the entries seen so far, and how many the walk is
/// allowed to take before it asks to stop (`usize::MAX` = never stop).
struct Walk {
    seen: Vec<(u32, u32)>,
    stop_after: usize,
}

unsafe extern "C" fn visit(key: u32, value: u32, ctx: *mut c_void) -> bool {
    // SAFETY: the test passes a live `Walk` as the context and does not
    // touch it while the call is in progress.
    let w = unsafe { &mut *ctx.cast::<Walk>() };
    w.seen.push((key, value));
    w.seen.len() < w.stop_after
}

#[test]
fn for_each_range_walks_ascending_and_honours_the_stop() {
    let mut w = Walk {
        seen: Vec::new(),
        stop_after: usize::MAX,
    };
    // SAFETY: `m` is a live handle from `expanse_map_new` until the final
    // `expanse_map_free`; the callback's context is `w`, live for the whole
    // block; null map / null callback / inverted range are documented
    // no-ops that report an exhausted walk.
    unsafe {
        let m = expanse_map_new();
        for k in 0..1000u32 {
            assert!(expanse_map_insert(m, k * 3, !k, core::ptr::null_mut()));
        }

        // A walk that never stops reports completion and visits the range.
        assert!(expanse_map_for_each_range(
            m,
            30,
            300,
            Some(visit),
            (&raw mut w).cast()
        ));
        // Multiples of 3 in [30, 300]: 30, 33, ..., 300.
        assert_eq!(w.seen.len(), 91);
        assert!(
            w.seen.windows(2).all(|a| a[0].0 < a[1].0),
            "ascending key order"
        );
        assert_eq!(w.seen[0], (30, !10));
        assert_eq!(w.seen[90], (300, !100));

        // Stopping mid-range reports the stop and visits exactly the prefix.
        w.seen.clear();
        w.stop_after = 5;
        assert!(!expanse_map_for_each_range(
            m,
            30,
            300,
            Some(visit),
            (&raw mut w).cast()
        ));
        assert_eq!(w.seen.len(), 5);
        assert_eq!(w.seen[4], (42, !14));

        // The walk is read-only: nothing was removed by any of it.
        assert_eq!(expanse_map_len(m), 1000);

        // Documented no-ops, each reporting an exhausted walk.
        w.seen.clear();
        w.stop_after = usize::MAX;
        assert!(expanse_map_for_each_range(
            m,
            300,
            30,
            Some(visit),
            (&raw mut w).cast()
        ));
        assert!(expanse_map_for_each_range(
            m,
            0,
            u32::MAX,
            None,
            (&raw mut w).cast()
        ));
        assert!(expanse_map_for_each_range(
            core::ptr::null(),
            0,
            u32::MAX,
            Some(visit),
            (&raw mut w).cast()
        ));
        assert!(w.seen.is_empty(), "no entry visited by any no-op form");

        expanse_map_free(m);
    }
}

/// The 32-bit map and set keep their node arena's tables after a drain;
/// `_shrink_to_fit` returns exactly `_mem_held - _mem_used` (#1135).
#[test]
fn mem_held_and_shrink_to_fit_on_the_narrow_surface() {
    use expanse::modern::{
        expanse_map_mem_held, expanse_map_mem_used, expanse_map_remove, expanse_map_shrink_to_fit,
        expanse_set_free, expanse_set_insert, expanse_set_mem_held, expanse_set_mem_used,
        expanse_set_new, expanse_set_remove, expanse_set_shrink_to_fit,
    };
    // SAFETY: `m` and `s` are live handles from their constructors until the
    // final frees; out-pointers are null.
    unsafe {
        assert_eq!(expanse_map_mem_held(core::ptr::null()), 0);
        assert_eq!(expanse_map_shrink_to_fit(core::ptr::null_mut()), 0);
        let m = expanse_map_new();
        let s = expanse_set_new();
        for k in 0..20_000u32 {
            let key = k.wrapping_mul(0x9E37_79B9);
            expanse_map_insert(m, key, k, core::ptr::null_mut());
            expanse_set_insert(s, key);
        }
        for k in 0..20_000u32 {
            let key = k.wrapping_mul(0x9E37_79B9);
            assert!(expanse_map_remove(m, key, core::ptr::null_mut()));
            assert!(expanse_set_remove(s, key));
        }
        let (held, used) = (expanse_map_mem_held(m), expanse_map_mem_used(m));
        assert_eq!(used, 0);
        assert!(held > 0, "a drained 32-bit map keeps its arena tables");
        assert_eq!(expanse_map_shrink_to_fit(m), held - used);
        assert_eq!(expanse_map_mem_held(m), expanse_map_mem_used(m));
        assert_eq!(expanse_map_shrink_to_fit(m), 0);
        let (held, used) = (expanse_set_mem_held(s), expanse_set_mem_used(s));
        assert_eq!(used, 0);
        assert!(held > 0, "a drained 32-bit set keeps its arena tables");
        assert_eq!(expanse_set_shrink_to_fit(s), held - used);
        assert_eq!(expanse_set_mem_held(s), expanse_set_mem_used(s));
        expanse_map_free(m);
        expanse_set_free(s);
    }
}
