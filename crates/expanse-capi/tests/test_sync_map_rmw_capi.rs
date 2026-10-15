//! `expanse_sync_map_compare_exchange` and `expanse_sync_map_get_or_insert`
//! (#1194): every branch of the exchange, the C `update` loop documented in
//! `include/expanse.h` run from several threads, and the NULL conventions.

use expanse::modern::{
    expanse_sync_map_compare_exchange, expanse_sync_map_free, expanse_sync_map_get,
    expanse_sync_map_get_or_insert, expanse_sync_map_new,
};
use expanse_trie::sync::SyncExpanseMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One exchange into locals: `(exchanged, observed)`.
fn cx(
    m: *mut SyncExpanseMap,
    key: u64,
    expected: Option<u64>,
    new: Option<u64>,
) -> (bool, Option<u64>) {
    let mut seen_present = expected.is_none();
    let mut seen = 0xDEAD_u64;
    // SAFETY: `m` is a live handle for every caller; outputs are live locals.
    let ok = unsafe {
        expanse_sync_map_compare_exchange(
            m,
            key,
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

fn get(m: *mut SyncExpanseMap, key: u64) -> Option<u64> {
    let mut v = 0;
    // SAFETY: `m` is a live handle; `v` is a live local.
    unsafe { expanse_sync_map_get(m, key, &raw mut v) }.then_some(v)
}

#[test]
fn compare_exchange_every_branch() {
    let m = expanse_sync_map_new();

    // Absent -> insert (expected absent, new present).
    assert_eq!(cx(m, 1, None, Some(10)), (true, None));
    assert_eq!(get(m, 1), Some(10));
    // Expected-absent on a present key: mismatch, observed value, untouched.
    assert_eq!(cx(m, 1, None, Some(99)), (false, Some(10)));
    assert_eq!(get(m, 1), Some(10));
    // Present -> replace.
    assert_eq!(cx(m, 1, Some(10), Some(11)), (true, Some(10)));
    assert_eq!(get(m, 1), Some(11));
    // Mismatch on the value: observed returned, untouched.
    assert_eq!(cx(m, 1, Some(10), Some(12)), (false, Some(11)));
    assert_eq!(get(m, 1), Some(11));
    // Expected-present on an absent key: mismatch, observed absent.
    assert_eq!(cx(m, 2, Some(0), Some(5)), (false, None));
    assert_eq!(get(m, 2), None);
    // Present -> remove.
    assert_eq!(cx(m, 1, Some(11), None), (true, Some(11)));
    assert_eq!(get(m, 1), None);
    // Remove with a wrong expected value leaves the key.
    assert_eq!(cx(m, 3, None, Some(7)), (true, None));
    assert_eq!(cx(m, 3, Some(8), None), (false, Some(7)));
    assert_eq!(get(m, 3), Some(7));
    // Absent expected and absent new: succeeds on an absent key, fails on a present one.
    assert_eq!(cx(m, 4, None, None), (true, None));
    assert_eq!(cx(m, 3, None, None), (false, Some(7)));
    // Every u64 is a legal value, 0 and MAX included.
    assert_eq!(cx(m, 5, None, Some(0)), (true, None));
    assert_eq!(cx(m, 5, Some(0), Some(u64::MAX)), (true, Some(0)));
    assert_eq!(cx(m, 5, Some(u64::MAX), Some(0)), (true, Some(u64::MAX)));
    assert_eq!(get(m, 5), Some(0));

    // SAFETY: `m` is live; NULL outputs are documented as accepted.
    unsafe {
        let null_out = |new_value: u64| {
            expanse_sync_map_compare_exchange(
                m,
                6,
                false,
                0,
                true,
                new_value,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        assert!(null_out(1));
        assert!(!null_out(2));
        assert_eq!(get(m, 6), Some(1));
    }

    // SAFETY: a NULL map is a documented `false` that writes nothing; the
    // outputs are live locals.
    unsafe {
        let (mut sp, mut sv) = (true, 77u64);
        assert!(!expanse_sync_map_compare_exchange(
            core::ptr::null(),
            1,
            false,
            0,
            true,
            1,
            &raw mut sp,
            &raw mut sv
        ));
        assert!(sp && sv == 77);
    }
    // SAFETY: `m` came from `expanse_sync_map_new` and is not used again.
    unsafe { expanse_sync_map_free(m) };
}

#[test]
fn get_or_insert_every_branch() {
    let m = expanse_sync_map_new();
    let mut existing = 0u64;
    // SAFETY: `m` is live until the free; `existing` is a live local; NULL
    // map and NULL output are documented.
    unsafe {
        assert!(expanse_sync_map_get_or_insert(m, 100, 1, &raw mut existing));
        assert_eq!(existing, 0, "no write on insert");
        assert!(!expanse_sync_map_get_or_insert(
            m,
            100,
            2,
            &raw mut existing
        ));
        assert_eq!(existing, 1);
        assert_eq!(get(m, 100), Some(1));
        assert!(!expanse_sync_map_get_or_insert(
            m,
            100,
            3,
            core::ptr::null_mut()
        ));
        assert!(!expanse_sync_map_get_or_insert(
            core::ptr::null(),
            100,
            3,
            &raw mut existing
        ));
        assert_eq!(existing, 1);
        expanse_sync_map_free(m);
    }
}

struct Handle(*mut SyncExpanseMap);
// SAFETY: the shared map handle is callable from any thread (include/expanse.h).
unsafe impl Send for Handle {}
// SAFETY: as above; the handle is only ever used through `&`.
unsafe impl Sync for Handle {}

/// The loop from the `expanse.h` comment, from several threads on one shared
/// handle, loses no increment; racing `get_or_insert` callers on one absent
/// key elect exactly one winner.
#[test]
fn update_loop_is_atomic_across_threads() {
    const THREADS: u64 = 4;
    const N: u64 = 5_000;
    let h = Handle(expanse_sync_map_new());
    let winners = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                let h = &h;
                // SAFETY: the handle is live until after the scope; outputs are locals.
                unsafe {
                    for _ in 0..N {
                        let mut v = 0u64;
                        let mut present = expanse_sync_map_get(h.0, 7, &raw mut v);
                        loop {
                            let nv = v + 1;
                            let (mut sp, mut sv) = (false, 0u64);
                            if expanse_sync_map_compare_exchange(
                                h.0,
                                7,
                                present,
                                v,
                                true,
                                nv,
                                &raw mut sp,
                                &raw mut sv,
                            ) {
                                break;
                            }
                            present = sp;
                            v = sv;
                        }
                    }
                    if expanse_sync_map_get_or_insert(h.0, 8, 1, core::ptr::null_mut()) {
                        winners.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    let mut total = 0u64;
    // SAFETY: all threads joined; `h.0` is live until the free.
    unsafe {
        assert!(expanse_sync_map_get(h.0, 7, &raw mut total));
        expanse_sync_map_free(h.0);
    }
    assert_eq!(total, THREADS * N);
    assert_eq!(winners.load(Ordering::Relaxed), 1);
}
