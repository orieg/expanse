//! The 32-bit-only C ABI surface (`EXPANSE_WIDE_SURFACE == 0`). Runs on the
//! i686 CI job — the one host-runnable 32-bit target — where a 32-bit
//! `libexpanse` actually exists; on 64-bit hosts this file is empty.
#![cfg(target_pointer_width = "32")]

use core::ffi::c_void;
use expanse::modern::{
    expanse_map_free, expanse_map_insert, expanse_map_len, expanse_map_new,
    expanse_map_remove_range,
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
