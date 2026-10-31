//! The shared blob map's C handle, `expanse_sync_blob_map_*` (#1538): every
//! entry point through its `extern "C"` function, the NULL conventions, the
//! reader lifecycle, the caller-buffer contract of `_get_into` on every value
//! representation, and the threaded contracts of `include/expanse.h`.
//!
//! The single-threaded tests are the ones the Miri lane runs
//! (`.github/workflows/ci.yml`, job `miri`); the ASan lane (job `test-asan`)
//! runs the whole target. The threaded tests are ignored under Miri, where
//! they would run for far longer than the lane allows, and run everywhere
//! else.

use expanse::blobmap::ExpanseBlobStatus;
use expanse::modern::{
    SyncBlobMapReader, expanse_sync_blob_map_compact, expanse_sync_blob_map_contains,
    expanse_sync_blob_map_free, expanse_sync_blob_map_get_into, expanse_sync_blob_map_get_meta,
    expanse_sync_blob_map_insert, expanse_sync_blob_map_len, expanse_sync_blob_map_mem_held,
    expanse_sync_blob_map_mem_held_estimate, expanse_sync_blob_map_mem_used,
    expanse_sync_blob_map_mem_used_estimate, expanse_sync_blob_map_new,
    expanse_sync_blob_map_new_with_capacity, expanse_sync_blob_map_reader_contains,
    expanse_sync_blob_map_reader_free, expanse_sync_blob_map_reader_get_into,
    expanse_sync_blob_map_reader_get_meta, expanse_sync_blob_map_reader_new,
    expanse_sync_blob_map_remove, expanse_sync_blob_map_set_reclaim_at_cap,
    expanse_sync_blob_map_shrink_to_fit,
};
use expanse_trie::blobmap::BlobView;
use expanse_trie::sync::SyncExpanseBlobMap;

type Map = SyncExpanseBlobMap;
type Status = ExpanseBlobStatus;

/// The smallest arena chunk (`BlobArena::new` clamps to 4096 bytes): it holds
/// every payload these tests store, and keeps the Miri lane's arena small.
const SMALL_CHUNK: usize = 4096;

/// The byte every test buffer is filled with before a call, so a write the
/// contract forbids shows.
const FILL: u8 = 0xA5;

fn insert(m: *const Map, k: u64, data: &[u8], meta: u32) -> Status {
    // SAFETY: `m` is live for every caller; `data` is readable for its length.
    unsafe { expanse_sync_blob_map_insert(m, k, data.as_ptr(), data.len(), meta) }
}

/// Which `_get_into` a read goes through.
#[derive(Clone, Copy, Debug)]
enum Via {
    Map(*const Map),
    Reader(*mut SyncBlobMapReader),
}

/// The outcome of one `_get_into` call: its status, the buffer as the call
/// left it, and the two out-values (`None` where the call left the
/// sentinel in place).
#[derive(Debug, PartialEq, Eq)]
struct Got {
    status: Status,
    buf: Vec<u8>,
    len: Option<usize>,
    meta: Option<u32>,
}

/// `_get_into` into a fresh `buf_len`-byte buffer filled with [`FILL`].
fn get_into(via: Via, k: u64, buf_len: usize) -> Got {
    const NO_LEN: usize = usize::MAX;
    const NO_META: u32 = u32::MAX;
    let mut buf = vec![FILL; buf_len];
    let (mut len, mut meta) = (NO_LEN, NO_META);
    // SAFETY: the handle is live for every caller and its reader is used on
    // this thread only; `buf` is writable for `buf_len` bytes; the outs are
    // locals.
    let status = unsafe {
        match via {
            Via::Map(m) => expanse_sync_blob_map_get_into(
                m,
                k,
                buf.as_mut_ptr(),
                buf_len,
                &raw mut len,
                &raw mut meta,
            ),
            Via::Reader(r) => expanse_sync_blob_map_reader_get_into(
                r,
                k,
                buf.as_mut_ptr(),
                buf_len,
                &raw mut len,
                &raw mut meta,
            ),
        }
    };
    Got {
        status,
        buf,
        len: (len != NO_LEN).then_some(len),
        meta: (meta != NO_META).then_some(meta),
    }
}

/// The length probe, `NULL` buffer of length 0.
fn probe(via: Via, k: u64) -> (Status, usize, u32) {
    let (mut len, mut meta) = (usize::MAX, u32::MAX);
    // SAFETY: as for `get_into`; a NULL buffer of length 0 is never written.
    let status = unsafe {
        match via {
            Via::Map(m) => expanse_sync_blob_map_get_into(
                m,
                k,
                core::ptr::null_mut(),
                0,
                &raw mut len,
                &raw mut meta,
            ),
            Via::Reader(r) => expanse_sync_blob_map_reader_get_into(
                r,
                k,
                core::ptr::null_mut(),
                0,
                &raw mut len,
                &raw mut meta,
            ),
        }
    };
    (status, len, meta)
}

/// How the engine stored a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Repr {
    /// In the slot word, uncompressed (7 bytes or fewer, metadata 0).
    Inline,
    /// In the slot word, compressed by the codec.
    Compressed,
    /// In the arena, with a 24-bit metadata field (any length with
    /// metadata, #1548).
    Arena,
}

fn repr(m: *const Map, k: u64) -> Option<Repr> {
    // SAFETY: `m` is live for every caller.
    let map = unsafe { &*m };
    map.with_locked(|bm| {
        bm.get(k).map(|(view, _)| match view {
            BlobView::Inline(_) => Repr::Inline,
            BlobView::CompressedInline { .. } => Repr::Compressed,
            BlobView::Arena(_) => Repr::Arena,
        })
    })
}

/// One value per representation, and the boundaries between them: the
/// payload, the metadata the insert passes, how it must be stored, and the
/// metadata it must read back.
fn representations() -> Vec<(Vec<u8>, u32, Repr, u32)> {
    vec![
        (Vec::new(), 0, Repr::Inline, 0),
        (b"abc".to_vec(), 0, Repr::Inline, 0),
        (b"seven!!".to_vec(), 0, Repr::Inline, 0),
        // At 7 bytes or fewer the slot word holds the payload and no
        // metadata field, so a short payload with metadata is an arena
        // record that keeps it (#1548, `ExpanseBlobMap::insert`).
        (Vec::new(), 1, Repr::Arena, 1),
        (b"seven!!".to_vec(), 9, Repr::Arena, 9),
        (b"tiny".to_vec(), 0x00FF_FFFF, Repr::Arena, 0x00FF_FFFF),
        // Eight bytes whose top byte is zero: the zero-trim codec.
        (vec![1, 2, 3, 4, 5, 6, 7, 0], 0, Repr::Compressed, 0),
        // ASCII digits, 8 to 14 of them: the nibble codec.
        (b"12345678".to_vec(), 0, Repr::Compressed, 0),
        (b"12345678901234".to_vec(), 0, Repr::Compressed, 0),
        // 8 or 9 alphanumerics: the alnum codec.
        (b"abcdEFG-_".to_vec(), 0, Repr::Compressed, 0),
        // A compressible payload with metadata goes to the arena with it.
        (b"12345678".to_vec(), 5, Repr::Arena, 5),
        // Just past the codecs: 15 digits, 8 bytes nothing compresses.
        (b"123456789012345".to_vec(), 0, Repr::Arena, 0),
        (vec![0xFF; 8], 0, Repr::Arena, 0),
        (vec![0xFF; 8], 0x00FF_FFFF, Repr::Arena, 0x00FF_FFFF),
        (
            (0..1000u32).map(|i| (i % 251) as u8).collect(),
            42,
            Repr::Arena,
            42,
        ),
    ]
}

#[test]
fn point_operations_and_return_conventions() {
    let m = expanse_sync_blob_map_new(0);
    assert!(!m.is_null());
    let mut meta = u32::MAX;
    // SAFETY: `m` is live until the free at the end; data is readable for
    // its length; out-pointers are locals or null.
    unsafe {
        assert_eq!(expanse_sync_blob_map_len(m), 0);
        assert_eq!(insert(m, 1, b"one", 0), Status::Ok);
        assert_eq!(insert(m, 2, &[7u8; 64], 3), Status::Ok);
        // NULL data with length 0 is the empty payload.
        assert_eq!(
            expanse_sync_blob_map_insert(m, 3, core::ptr::null(), 0, 0),
            Status::Ok
        );
        assert_eq!(insert(m, 1, b"replaced", 0), Status::Ok, "a replace");
        assert_eq!(expanse_sync_blob_map_len(m), 3);
        assert!(expanse_sync_blob_map_contains(m, 3));
        assert!(!expanse_sync_blob_map_contains(m, 4));
        assert!(expanse_sync_blob_map_get_meta(m, 2, &raw mut meta));
        assert_eq!(meta, 3);
        assert!(expanse_sync_blob_map_get_meta(m, 3, core::ptr::null_mut()));
        meta = u32::MAX;
        assert!(!expanse_sync_blob_map_get_meta(m, 4, &raw mut meta));
        assert_eq!(meta, u32::MAX, "a miss writes nothing");
    }
    let got = get_into(Via::Map(m), 1, 16);
    assert_eq!(
        (got.status, got.len, got.meta),
        (Status::Ok, Some(8), Some(0))
    );
    assert_eq!(&got.buf[..8], b"replaced");
    assert_eq!(get_into(Via::Map(m), 3, 0).len, Some(0), "an empty payload");

    // SAFETY: as above; no reader is live at the free.
    unsafe {
        let used = expanse_sync_blob_map_mem_used(m);
        assert!(used > 0);
        assert_eq!(
            expanse_sync_blob_map_mem_used_estimate(m),
            used,
            "equal with nothing in flight (SyncExpanseBlobMap::mem_used_estimate)"
        );
        let held = expanse_sync_blob_map_mem_held(m);
        assert!(held >= used);
        assert_eq!(
            expanse_sync_blob_map_mem_held_estimate(m),
            held,
            "equal with nothing in flight (SyncExpanseBlobMap::mem_held_estimate)"
        );
        assert!(expanse_sync_blob_map_remove(m, 2));
        assert!(!expanse_sync_blob_map_remove(m, 2));
        assert_eq!(expanse_sync_blob_map_compact(m), Status::Ok);
        let _released = expanse_sync_blob_map_shrink_to_fit(m);
        assert_eq!(expanse_sync_blob_map_len(m), 2);
        expanse_sync_blob_map_free(m);
    }
}

#[test]
fn every_representation_round_trips_through_both_get_intos() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    // SAFETY: `m` outlives the reader, which is freed first.
    let r = unsafe { expanse_sync_blob_map_reader_new(m) };
    assert!(!r.is_null());
    let reps = representations();
    for (k, (data, meta_in, want_repr, _)) in (0u64..).zip(&reps) {
        assert_eq!(insert(m, k, data, *meta_in), Status::Ok, "key {k}");
        assert_eq!(repr(m, k), Some(*want_repr), "key {k}: {data:?}");
    }
    for (k, (data, _, _, want_meta)) in (0u64..).zip(&reps) {
        for via in [Via::Map(m), Via::Reader(r)] {
            // Exact fit, and a buffer with spare room whose tail must stay.
            for spare in [0, 5] {
                let got = get_into(via, k, data.len() + spare);
                assert_eq!(got.status, Status::Ok, "key {k} {via:?}");
                assert_eq!(got.len, Some(data.len()));
                assert_eq!(got.meta, Some(*want_meta), "key {k} {via:?}");
                assert_eq!(&got.buf[..data.len()], &data[..]);
                assert!(got.buf[data.len()..].iter().all(|&b| b == FILL));
            }
        }
        let mut meta = u32::MAX;
        // SAFETY: `m` and `r` are live; `meta` is a local.
        unsafe {
            assert!(expanse_sync_blob_map_reader_get_meta(r, k, &raw mut meta));
            assert_eq!(meta, *want_meta);
            assert!(expanse_sync_blob_map_reader_contains(r, k));
        }
    }

    // One key moved across every representation boundary, in both
    // directions: each replace must read back as the value just stored.
    // A short payload moves between the slot word and an arena record by
    // its metadata alone (#1548).
    let walk: [(&[u8], u32, Repr); 8] = [
        (b"1234567", 0, Repr::Inline),
        (b"12345678", 0, Repr::Compressed),
        (&[0xEE; 300], 11, Repr::Arena),
        (b"12345678", 12, Repr::Arena),
        (b"abc", 13, Repr::Arena),
        (b"abc", 0, Repr::Inline),
        (b"abc", 14, Repr::Arena),
        (&[0xEE; 9], 0, Repr::Arena),
    ];
    for (data, meta_in, want) in walk {
        assert_eq!(insert(m, 99, data, meta_in), Status::Ok);
        assert_eq!(repr(m, 99), Some(want), "{data:?}");
        // Metadata reads back as stored, at every length.
        let want_meta = meta_in;
        let got = get_into(Via::Reader(r), 99, 300);
        assert_eq!(
            (got.status, got.len, got.meta),
            (Status::Ok, Some(data.len()), Some(want_meta))
        );
        assert_eq!(&got.buf[..data.len()], data);
    }
    // SAFETY: the reader is freed before its map.
    unsafe {
        expanse_sync_blob_map_reader_free(r);
        expanse_sync_blob_map_free(m);
    }
}

/// The plan's gate for the caller-buffer read (`docs/COMPAT.md`): absent,
/// exact fit, one byte short (buffer untouched, required length exact), and
/// the probe, on every representation that has bytes, through the map and
/// through a reader.
#[test]
fn get_into_buffer_contract() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    // SAFETY: `m` outlives the reader, which is freed first.
    let r = unsafe { expanse_sync_blob_map_reader_new(m) };
    let reps = representations();
    for (k, (data, meta_in, _, _)) in (0u64..).zip(&reps) {
        assert_eq!(insert(m, k, data, *meta_in), Status::Ok);
    }
    for via in [Via::Map(m), Via::Reader(r)] {
        let absent = get_into(via, 1_000, 64);
        assert_eq!(
            absent,
            Got {
                status: Status::NotFound,
                buf: vec![FILL; 64],
                len: None,
                meta: None
            },
            "{via:?}: an absent key writes nothing"
        );
        assert_eq!(probe(via, 1_000), (Status::NotFound, usize::MAX, u32::MAX));

        for (k, (data, _, _, want_meta)) in (0u64..).zip(&reps) {
            let n = data.len();
            let (st, len, meta) = probe(via, k);
            if n == 0 {
                assert_eq!(
                    (st, len, meta),
                    (Status::Ok, 0, *want_meta),
                    "the empty payload fits NULL"
                );
                continue;
            }
            assert_eq!(
                (st, len, meta),
                (Status::BufferTooSmall, n, *want_meta),
                "{via:?} key {k}: the probe"
            );
            // One byte short, and an empty non-NULL buffer: nothing copied.
            for short in [n - 1, 0] {
                let got = get_into(via, k, short);
                assert_eq!(
                    got,
                    Got {
                        status: Status::BufferTooSmall,
                        buf: vec![FILL; short],
                        len: Some(n),
                        meta: Some(*want_meta)
                    },
                    "{via:?} key {k}: {short} bytes for {n}"
                );
            }
            let exact = get_into(via, k, n);
            assert_eq!((exact.status, exact.len), (Status::Ok, Some(n)));
            assert_eq!(exact.buf, *data);
        }
    }

    // Usage errors write nothing; NULL outs are accepted.
    let mut buf = [FILL; 8];
    let (mut len, mut meta) = (usize::MAX, u32::MAX);
    // SAFETY: the NULL-buffer and oversized-length calls are refused before
    // any buffer is formed; the others pass a live handle and a local buffer.
    unsafe {
        assert_eq!(
            expanse_sync_blob_map_reader_get_into(
                r,
                1,
                core::ptr::null_mut(),
                4,
                &raw mut len,
                &raw mut meta
            ),
            Status::InvalidArgument,
            "a NULL buffer with a length"
        );
        assert_eq!(
            expanse_sync_blob_map_get_into(
                m,
                1,
                buf.as_mut_ptr(),
                usize::MAX,
                &raw mut len,
                &raw mut meta
            ),
            Status::InvalidArgument,
            "a length above PTRDIFF_MAX"
        );
        assert_eq!((len, meta), (usize::MAX, u32::MAX));
        assert_eq!(
            expanse_sync_blob_map_reader_get_into(
                r,
                1,
                buf.as_mut_ptr(),
                buf.len(),
                core::ptr::null_mut(),
                core::ptr::null_mut()
            ),
            Status::Ok
        );
        assert_eq!(&buf[..3], b"abc");
        expanse_sync_blob_map_reader_free(r);
        expanse_sync_blob_map_free(m);
    }
}

#[test]
fn null_handles_write_nothing() {
    let null_map: *const Map = core::ptr::null();
    let null_reader: *mut SyncBlobMapReader = core::ptr::null_mut();
    let mut buf = [FILL; 8];
    let (mut len, mut meta) = (usize::MAX, u32::MAX);
    // SAFETY: every call is documented to accept a NULL handle.
    unsafe {
        assert_eq!(
            expanse_sync_blob_map_insert(null_map, 1, buf.as_ptr(), 1, 0),
            Status::InvalidArgument
        );
        assert!(!expanse_sync_blob_map_remove(null_map, 1));
        assert_eq!(
            expanse_sync_blob_map_get_into(
                null_map,
                1,
                buf.as_mut_ptr(),
                8,
                &raw mut len,
                &raw mut meta
            ),
            Status::InvalidArgument
        );
        assert!(!expanse_sync_blob_map_get_meta(null_map, 1, &raw mut meta));
        assert!(!expanse_sync_blob_map_contains(null_map, 1));
        assert_eq!(expanse_sync_blob_map_len(null_map), 0);
        assert_eq!(expanse_sync_blob_map_mem_used(null_map), 0);
        assert_eq!(expanse_sync_blob_map_mem_used_estimate(null_map), 0);
        assert_eq!(expanse_sync_blob_map_mem_held(null_map), 0);
        assert_eq!(expanse_sync_blob_map_mem_held_estimate(null_map), 0);
        assert_eq!(expanse_sync_blob_map_shrink_to_fit(null_map), 0);
        expanse_sync_blob_map_set_reclaim_at_cap(null_map, false);
        assert_eq!(
            expanse_sync_blob_map_compact(null_map),
            Status::InvalidArgument
        );
        assert!(expanse_sync_blob_map_reader_new(null_map).is_null());
        assert_eq!(
            expanse_sync_blob_map_reader_get_into(
                null_reader,
                1,
                buf.as_mut_ptr(),
                8,
                &raw mut len,
                &raw mut meta
            ),
            Status::InvalidArgument
        );
        assert!(!expanse_sync_blob_map_reader_get_meta(
            null_reader,
            1,
            &raw mut meta
        ));
        assert!(!expanse_sync_blob_map_reader_contains(null_reader, 1));
        expanse_sync_blob_map_reader_free(core::ptr::null_mut());
        expanse_sync_blob_map_free(core::ptr::null_mut());

        // A live map refuses NULL data with a length and stores nothing.
        let m = expanse_sync_blob_map_new(0);
        assert_eq!(
            expanse_sync_blob_map_insert(m, 1, core::ptr::null(), 8, 0),
            Status::InvalidArgument
        );
        assert_eq!(expanse_sync_blob_map_len(m), 0);
        expanse_sync_blob_map_free(m);
    }
    assert_eq!((buf, len, meta), ([FILL; 8], usize::MAX, u32::MAX));
}

/// The capacity cap, the reclaim switch, compaction and the refusals reach
/// C with `expanse_blob_map_insert_ex`'s statuses. One 4 KiB chunk fits
/// under a 6000-byte cap, as in `test_blobmap_capi.rs`.
#[test]
fn cap_reclaim_compact_and_refusals() {
    let rec = |k: u64, n: usize| vec![k as u8 | 0x80; n];
    for reclaim in [true, false] {
        let m = expanse_sync_blob_map_new_with_capacity(4096, 6000);
        // SAFETY: `m` is live until the free below.
        unsafe { expanse_sync_blob_map_set_reclaim_at_cap(m, reclaim) };
        for k in 0..4u64 {
            assert_eq!(insert(m, k, &rec(k, 1000), 1), Status::Ok);
        }
        let big = rec(9, 3100);
        assert_eq!(
            insert(m, 9, &big, 1),
            Status::CapRefused,
            "reclaim {reclaim}"
        );
        for k in 1..4u64 {
            // SAFETY: as above.
            assert!(unsafe { expanse_sync_blob_map_remove(m, k) });
        }
        // With the reclaim on, the insert compacts and the record still
        // does not fit beside the chunk's live tail; off, nothing compacts.
        let want = if reclaim {
            Status::ArenaFull
        } else {
            Status::CapRefused
        };
        assert_eq!(insert(m, 9, &big, 1), want, "reclaim {reclaim}");
        // An explicit compaction keeps the survivor's bytes and metadata.
        // SAFETY: as above.
        assert_eq!(unsafe { expanse_sync_blob_map_compact(m) }, Status::Ok);
        let got = get_into(Via::Map(m), 0, 1000);
        assert_eq!((got.status, got.meta), (Status::Ok, Some(1)));
        assert_eq!(got.buf, rec(0, 1000));

        assert_eq!(insert(m, 1, &rec(1, 100), 1 << 24), Status::MetaOverflow);
        assert_eq!(
            insert(m, 1, &rec(1, 5000), 1),
            Status::AllocationFailed,
            "a payload larger than a chunk"
        );
        // SAFETY: as above.
        unsafe {
            assert_eq!(expanse_sync_blob_map_len(m), 1, "refusals store nothing");
            expanse_sync_blob_map_free(m);
        }
    }
    // Zero selects the defaults.
    let d = expanse_sync_blob_map_new_with_capacity(0, 0);
    assert_eq!(insert(d, 1, &[3u8; 3000], 0), Status::Ok);
    // SAFETY: `d` is live.
    unsafe { expanse_sync_blob_map_free(d) };
}

#[test]
fn reader_sees_writes_and_is_freed_before_its_map() {
    const N: u64 = 48;
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    let val = |i: u64| vec![i as u8; (i as usize * 7) % 40];
    // SAFETY: `m` outlives both readers, which are freed before it.
    unsafe {
        let r = expanse_sync_blob_map_reader_new(m);
        assert!(!r.is_null());
        for i in 0..N {
            assert!(!expanse_sync_blob_map_reader_contains(r, i));
            assert_eq!(insert(m, i, &val(i), i as u32), Status::Ok);
            let got = get_into(Via::Reader(r), i, 64);
            assert_eq!(got.status, Status::Ok, "the reader sees a completed write");
            assert_eq!(&got.buf[..val(i).len()], &val(i)[..]);
        }
        let r2 = expanse_sync_blob_map_reader_new(m);
        for i in (0..N).step_by(2) {
            assert!(expanse_sync_blob_map_remove(m, i));
            assert_eq!(get_into(Via::Reader(r), i, 64).status, Status::NotFound);
            assert!(expanse_sync_blob_map_reader_contains(r2, i + 1));
        }
        expanse_sync_blob_map_reader_free(r2);
        expanse_sync_blob_map_reader_free(r);
        assert_eq!(expanse_sync_blob_map_len(m), N / 2);
        expanse_sync_blob_map_free(m);
    }
}

/// The values the writer of the threaded test cycles one key through, one
/// per representation, each with its metadata as stored.
fn cycle() -> [(Vec<u8>, u32); 3] {
    [
        (b"short".to_vec(), 0),
        (vec![b'L'; 300], 77),
        (b"12345678901".to_vec(), 0),
    ]
}

/// The get-into contract under a concurrent writer: one writer cycles keys
/// through an inline, an arena and a compressed value and an absence, and
/// compacts the arena, while readers on other threads read them into a
/// buffer that holds only the short values. Every `Ok` must be one whole
/// value with its own metadata, every `BufferTooSmall` must leave the buffer
/// untouched and report the long value's length and metadata together, and
/// stable keys must always read back exactly.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn concurrent_get_into_returns_whole_values_and_consistent_lengths() {
    const STABLE: u64 = 256;
    const CHURN: u64 = 64;
    const READERS: usize = 4;
    const BUF: usize = 32;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = expanse_sync_blob_map_new(64 * 1024);
    let stable = |i: u64| vec![(i % 200) as u8; 8 + (i as usize % 400)];
    for i in 0..STABLE {
        assert_eq!(insert(m, i, &stable(i), (i % 1000) as u32), Status::Ok);
    }
    let addr = m as usize; // raw pointers are not `Send`
    let stop = AtomicBool::new(false);
    let cycle = cycle();
    let seen = std::thread::scope(|sc| {
        let (stop, cycle) = (&stop, &cycle);
        let readers: Vec<_> = (0..READERS)
            .map(|_| {
                sc.spawn(move || {
                    let m = addr as *const Map;
                    // SAFETY: the map outlives the scope; this reader is used
                    // only on this thread and freed before the scope ends.
                    let r = unsafe { expanse_sync_blob_map_reader_new(m) };
                    let mut passes = 0u32;
                    // NotFound, Ok, BufferTooSmall outcomes of the churned keys.
                    let mut seen = [0u64; 3];
                    loop {
                        let done = stop.load(Ordering::Acquire);
                        for i in (0..STABLE).step_by(7) {
                            let want = stable(i);
                            let got = get_into(Via::Reader(r), i, 512);
                            assert_eq!(got.status, Status::Ok, "stable key {i}");
                            assert_eq!(got.meta, Some((i % 1000) as u32));
                            assert_eq!(&got.buf[..want.len()], &want[..]);
                        }
                        for k in STABLE..STABLE + CHURN {
                            let got = get_into(Via::Reader(r), k, BUF);
                            match got.status {
                                Status::NotFound => {
                                    seen[0] += 1;
                                    assert_eq!((got.len, got.meta), (None, None));
                                    assert!(got.buf.iter().all(|&b| b == FILL));
                                }
                                Status::Ok => {
                                    seen[1] += 1;
                                    let n = got.len.expect("Ok writes the length");
                                    let hit = cycle.iter().find(|(v, meta)| {
                                        v.len() == n
                                            && got.buf[..n] == v[..]
                                            && got.meta == Some(*meta)
                                    });
                                    assert!(
                                        hit.is_some(),
                                        "key {k}: a torn or mixed value {got:?}"
                                    );
                                    assert!(got.buf[n..].iter().all(|&b| b == FILL));
                                }
                                Status::BufferTooSmall => {
                                    seen[2] += 1;
                                    assert_eq!(
                                        (got.len, got.meta),
                                        (Some(300), Some(77)),
                                        "key {k}: length and metadata of one value"
                                    );
                                    assert!(got.buf.iter().all(|&b| b == FILL), "nothing copied");
                                }
                                other => panic!("key {k}: {other:?}"),
                            }
                            // The probe-and-retry loop the header describes ends
                            // in Ok or NotFound.
                            let mut tries = 0;
                            loop {
                                let (st, len, _) = probe(Via::Reader(r), k);
                                let st = if st == Status::BufferTooSmall {
                                    get_into(Via::Reader(r), k, len).status
                                } else {
                                    st
                                };
                                tries += 1;
                                if st == Status::Ok || st == Status::NotFound {
                                    break;
                                }
                                assert_eq!(st, Status::BufferTooSmall);
                                assert!(tries < 1_000_000);
                            }
                        }
                        passes += 1;
                        if done {
                            break;
                        }
                    }
                    // SAFETY: no call on `r` is in progress; the map is live.
                    unsafe { expanse_sync_blob_map_reader_free(r) };
                    assert!(passes > 0);
                    seen
                })
            })
            .collect();
        sc.spawn(move || {
            let m = addr as *const Map;
            for round in 0..400u64 {
                for k in STABLE..STABLE + CHURN {
                    match (round + k) % 4 {
                        3 => {
                            // SAFETY: the map outlives the scope.
                            unsafe { expanse_sync_blob_map_remove(m, k) };
                        }
                        c => {
                            let (v, meta) = &cycle[c as usize];
                            assert_eq!(insert(m, k, v, *meta), Status::Ok);
                        }
                    }
                }
                if round % 8 == 7 {
                    // SAFETY: as above.
                    assert_eq!(unsafe { expanse_sync_blob_map_compact(m) }, Status::Ok);
                }
            }
            stop.store(true, Ordering::Release);
        });
        readers
            .into_iter()
            .map(|h| h.join().unwrap())
            .fold([0u64; 3], |acc, s| {
                [acc[0] + s[0], acc[1] + s[1], acc[2] + s[2]]
            })
    });
    // Every reader makes a pass after the writer's last round, which leaves
    // keys in each state, so each outcome is observed at least then.
    assert!(seen.iter().all(|&n| n > 0), "outcomes seen: {seen:?}");
    // SAFETY: every reader was freed inside the scope.
    unsafe {
        let len = expanse_sync_blob_map_len(m);
        assert!((STABLE..=STABLE + CHURN).contains(&len));
        expanse_sync_blob_map_free(m);
    }
}

/// Reader handles created and used on worker threads, then freed on the main
/// thread while a writer runs, before the map (`docs/COMPAT.md`, reader-handle
/// ownership).
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn reader_handles_freed_on_another_thread() {
    const THREADS: usize = 6;
    const N: u64 = 256;
    use std::sync::atomic::{AtomicBool, Ordering};

    let m = expanse_sync_blob_map_new(0);
    let val = |i: u64| vec![(i % 256) as u8; 1 + (i as usize % 64)];
    for i in 0..N {
        assert_eq!(insert(m, i, &val(i), 0), Status::Ok);
    }
    let addr = m as usize;
    let handles: Vec<usize> = std::thread::scope(|sc| {
        let workers: Vec<_> = (0..THREADS)
            .map(|_| {
                sc.spawn(move || {
                    let m = addr as *const Map;
                    // SAFETY: the map outlives every handle; the handle is
                    // used only on this thread until returned, idle after.
                    let r = unsafe { expanse_sync_blob_map_reader_new(m) };
                    assert!(!r.is_null());
                    for i in 0..N {
                        let got = get_into(Via::Reader(r), i, 64);
                        assert_eq!(got.status, Status::Ok);
                        assert_eq!(&got.buf[..val(i).len()], &val(i)[..]);
                    }
                    r as usize
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).collect()
    });
    let stop = AtomicBool::new(false);
    std::thread::scope(|sc| {
        let stop = &stop;
        sc.spawn(move || {
            let m = addr as *const Map;
            let mut i = N;
            while !stop.load(Ordering::Acquire) {
                assert_eq!(insert(m, i, &val(i), 0), Status::Ok);
                i += 1;
            }
        });
        for h in handles {
            // SAFETY: each handle is idle (its thread joined) and its map
            // live; freeing from this thread is the contract under test.
            unsafe { expanse_sync_blob_map_reader_free(h as *mut SyncBlobMapReader) };
        }
        stop.store(true, Ordering::Release);
    });
    // SAFETY: every reader was freed.
    unsafe {
        assert!(expanse_sync_blob_map_len(m) >= N);
        expanse_sync_blob_map_free(m);
    }
}

// ---- Read-modify-write: `_compare_exchange`, `_compare_exchange_meta` and
// `_get_or_insert` ----

use expanse::modern::{
    expanse_sync_blob_map_compare_exchange, expanse_sync_blob_map_compare_exchange_meta,
    expanse_sync_blob_map_get_or_insert,
};

/// The outcome of one blob read-modify-write: its status, the buffer as the
/// call left it, and the out-values (`None` where the call left the sentinel).
#[derive(Debug, PartialEq, Eq)]
struct Rmw {
    status: Status,
    present: Option<bool>,
    buf: Vec<u8>,
    len: Option<usize>,
    meta: Option<u32>,
}

const NO_LEN: usize = usize::MAX;
const NO_META: u32 = u32::MAX;

impl Rmw {
    /// What a call that writes nothing leaves behind.
    fn untouched(status: Status, buf_len: usize) -> Self {
        Rmw {
            status,
            present: None,
            buf: vec![FILL; buf_len],
            len: None,
            meta: None,
        }
    }

    /// A reported present value, copied (`buf_len >= data.len()`) or not.
    fn observed(data: &[u8], meta: u32, buf_len: usize, with_present: bool) -> Self {
        let mut buf = vec![FILL; buf_len];
        let status = if data.len() <= buf_len {
            buf[..data.len()].copy_from_slice(data);
            Status::Mismatch
        } else {
            Status::BufferTooSmall
        };
        Rmw {
            status,
            present: with_present.then_some(true),
            buf,
            len: Some(data.len()),
            meta: Some(meta),
        }
    }
}

type Val<'a> = Option<(&'a [u8], u32)>;

/// `_compare_exchange` into a fresh `buf_len`-byte buffer filled with [`FILL`].
fn cx(m: *const Map, k: u64, expected: Val<'_>, new: Val<'_>, buf_len: usize) -> Rmw {
    let mut buf = vec![FILL; buf_len];
    let (mut present, mut len, mut meta) = (2u8, NO_LEN, NO_META);
    let (ep, ed, el, em) = split(expected);
    let (np, nd, nl, nm) = split(new);
    // SAFETY: `m` is live for every caller; the payloads are readable for
    // their lengths; `buf` is writable for `buf_len` bytes; the outs are
    // locals (`present` read back below only as a byte, see `as_present`).
    let status = unsafe {
        expanse_sync_blob_map_compare_exchange(
            m,
            k,
            ep,
            ed,
            el,
            em,
            np,
            nd,
            nl,
            nm,
            (&raw mut present).cast::<bool>(),
            buf.as_mut_ptr(),
            buf_len,
            &raw mut len,
            &raw mut meta,
        )
    };
    Rmw {
        status,
        present: as_present(present),
        buf,
        len: (len != NO_LEN).then_some(len),
        meta: (meta != NO_META).then_some(meta),
    }
}

/// A value argument as the C call takes it.
fn split(v: Val<'_>) -> (bool, *const u8, usize, u32) {
    match v {
        Some((d, meta)) => (true, d.as_ptr(), d.len(), meta),
        None => (false, core::ptr::null(), 0, 0),
    }
}

/// A `bool` out-parameter pre-filled with the byte 2, which is no `bool`:
/// `None` when the call left it, else the `bool` it wrote.
fn as_present(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        2 => None,
        other => panic!("a bool out-parameter holds {other}"),
    }
}

/// `_get_or_insert` into a fresh `buf_len`-byte buffer filled with [`FILL`].
fn goi(m: *const Map, k: u64, data: &[u8], hot_meta: u32, buf_len: usize) -> Rmw {
    let mut buf = vec![FILL; buf_len];
    let (mut len, mut meta) = (NO_LEN, NO_META);
    // SAFETY: as for `cx`.
    let status = unsafe {
        expanse_sync_blob_map_get_or_insert(
            m,
            k,
            data.as_ptr(),
            data.len(),
            hot_meta,
            buf.as_mut_ptr(),
            buf_len,
            &raw mut len,
            &raw mut meta,
        )
    };
    Rmw {
        status,
        present: None,
        buf,
        len: (len != NO_LEN).then_some(len),
        meta: (meta != NO_META).then_some(meta),
    }
}

/// `_compare_exchange_meta` into locals: `(status, observed)`, `observed`
/// `None` when the call left the outputs alone, else `Some(presence and
/// metadata)`.
fn cx_meta(
    m: *const Map,
    k: u64,
    expected: Option<u32>,
    new: Option<u32>,
) -> (Status, Option<Option<u32>>) {
    let (mut present, mut meta) = (2u8, NO_META);
    // SAFETY: `m` is live for every caller; the outs are locals.
    let status = unsafe {
        expanse_sync_blob_map_compare_exchange_meta(
            m,
            k,
            expected.is_some(),
            expected.unwrap_or(0),
            new.is_some(),
            new.unwrap_or(0),
            (&raw mut present).cast::<bool>(),
            &raw mut meta,
        )
    };
    let observed = as_present(present).map(|p| {
        assert_eq!(p, meta != NO_META, "metadata written iff present");
        p.then_some(meta)
    });
    (status, observed)
}

/// The current value through `_get_into`, `None` when absent.
fn value(m: *const Map, k: u64) -> Option<(Vec<u8>, u32)> {
    let (status, len, _) = probe(Via::Map(m), k);
    match status {
        Status::NotFound => None,
        Status::Ok | Status::BufferTooSmall => {
            let got = get_into(Via::Map(m), k, len);
            assert_eq!(got.status, Status::Ok);
            Some((got.buf, got.meta.expect("written on Ok")))
        }
        other => panic!("get_into: {other:?}"),
    }
}

#[test]
fn compare_exchange_every_branch_and_the_buffer_contract() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    let a: Vec<u8> = (0..20u8).collect();
    let b = b"hi".to_vec();
    let c = vec![0xC3; 40];

    // Absent -> stored: OK, nothing written.
    assert_eq!(
        cx(m, 1, None, Some((&a, 7)), 64),
        Rmw::untouched(Status::Ok, 64)
    );
    assert_eq!(value(m, 1), Some((a.clone(), 7)));
    // Expected-absent on a present key: MISMATCH with the value, untouched.
    assert_eq!(
        cx(m, 1, None, Some((&b, 0)), 64),
        Rmw::observed(&a, 7, 64, true)
    );
    // The same with a buffer one byte short, and the NULL length probe:
    // nothing copied, length and metadata reported.
    assert_eq!(
        cx(m, 1, None, Some((&b, 0)), 19),
        Rmw::observed(&a, 7, 19, true)
    );
    assert_eq!(
        cx(m, 1, None, Some((&b, 0)), 0),
        Rmw::observed(&a, 7, 0, true)
    );
    // The exact size fits.
    assert_eq!(
        cx(m, 1, None, Some((&b, 0)), 20),
        Rmw::observed(&a, 7, 20, true)
    );
    // Metadata is compared with the bytes.
    assert_eq!(
        cx(m, 1, Some((&a, 8)), Some((&b, 0)), 64),
        Rmw::observed(&a, 7, 64, true)
    );
    assert_eq!(
        cx(m, 1, Some((&a[..19], 7)), Some((&b, 0)), 64),
        Rmw::observed(&a, 7, 64, true)
    );
    assert_eq!(
        value(m, 1),
        Some((a.clone(), 7)),
        "mismatches change nothing"
    );
    // A matching expected replaces, arena value by an inline one and back.
    assert_eq!(
        cx(m, 1, Some((&a, 7)), Some((&b, 0)), 64),
        Rmw::untouched(Status::Ok, 64)
    );
    assert_eq!(value(m, 1), Some((b.clone(), 0)));
    assert_eq!(
        cx(m, 1, Some((&b, 0)), Some((&c, 3)), 64),
        Rmw::untouched(Status::Ok, 64)
    );
    assert_eq!(value(m, 1), Some((c.clone(), 3)));
    // Expected-present on an absent key: MISMATCH, observed absent, 0 and 0.
    let absent = Rmw {
        status: Status::Mismatch,
        present: Some(false),
        buf: vec![FILL; 64],
        len: Some(0),
        meta: Some(0),
    };
    assert_eq!(cx(m, 2, Some((&a, 7)), Some((&b, 0)), 64), absent);
    assert_eq!(value(m, 2), None);
    // Remove only on a matching value.
    assert_eq!(
        cx(m, 1, Some((&c, 4)), None, 64),
        Rmw::observed(&c, 3, 64, true)
    );
    assert_eq!(
        cx(m, 1, Some((&c, 3)), None, 64),
        Rmw::untouched(Status::Ok, 64)
    );
    assert_eq!(value(m, 1), None);
    // (absent, absent): a validated absence check that never writes.
    assert_eq!(cx(m, 1, None, None, 64), Rmw::untouched(Status::Ok, 64));
    assert_eq!(
        cx(m, 3, None, Some((&[][..], 0)), 8),
        Rmw::untouched(Status::Ok, 8)
    );
    assert_eq!(cx(m, 3, None, None, 8), Rmw::observed(&[], 0, 8, true));
    // An empty payload is present and fits a NULL buffer of length 0.
    assert_eq!(cx(m, 3, None, None, 0), Rmw::observed(&[], 0, 0, true));

    // Payloads of 7 bytes or fewer keep the metadata they are stored with
    // (#1548): expecting it matches, and expecting 0 is a mismatch on it.
    assert_eq!(
        cx(m, 5, None, Some((&b"seven!!"[..], 9)), 16),
        Rmw::untouched(Status::Ok, 16)
    );
    assert_eq!(
        cx(m, 5, Some((&b"seven!!"[..], 0)), Some((&b"x"[..], 0)), 16),
        Rmw::observed(b"seven!!", 9, 16, true)
    );
    assert_eq!(
        cx(m, 5, Some((&b"seven!!"[..], 9)), Some((&b"x"[..], 0)), 16),
        Rmw::untouched(Status::Ok, 16)
    );
    // Metadata wider than 24 bits is refused for a short payload too.
    assert_eq!(
        cx(m, 5, Some((&b"x"[..], 0)), Some((&b"y"[..], 1 << 24)), 16),
        Rmw::untouched(Status::MetaOverflow, 16)
    );

    // SAFETY: `m` is live until the free below.
    unsafe {
        assert_eq!(expanse_sync_blob_map_len(m), 2, "keys 3 and 5");
        expanse_sync_blob_map_free(m);
    }
}

#[test]
fn compare_exchange_refusals_hold_only_when_the_comparison_does() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    let a = vec![0x11; 20];
    let big = vec![0x22; SMALL_CHUNK + 1];
    assert_eq!(insert(m, 1, &a, 7), Status::Ok);
    // A comparison that holds, with a value that cannot be stored: the
    // refusal, nothing written, nothing changed.
    assert_eq!(
        cx(m, 1, Some((&a, 7)), Some((&a, 1 << 24)), 64),
        Rmw::untouched(Status::MetaOverflow, 64)
    );
    assert_eq!(
        cx(m, 1, Some((&a, 7)), Some((&big, 0)), 64),
        Rmw::untouched(Status::AllocationFailed, 64)
    );
    assert_eq!(
        cx(m, 2, None, Some((&a, 1 << 24)), 64),
        Rmw::untouched(Status::MetaOverflow, 64)
    );
    // The same values against a comparison that fails: the mismatch wins.
    assert_eq!(
        cx(m, 1, Some((&a, 6)), Some((&a, 1 << 24)), 64),
        Rmw::observed(&a, 7, 64, true)
    );
    assert_eq!(
        cx(m, 1, None, Some((&big, 0)), 64),
        Rmw::observed(&a, 7, 64, true)
    );
    assert_eq!(value(m, 1), Some((a.clone(), 7)));
    assert_eq!(value(m, 2), None);

    // The header's retry loop stops on a refusal: a loop raising the
    // metadata past 24 bits ends with META_OVERFLOW, value unchanged.
    let mut cur = (a.clone(), 7u32);
    let mut rounds = 0;
    let st = loop {
        rounds += 1;
        let next = (cur.0.clone(), cur.1 << 12);
        let got = cx(m, 1, Some((&cur.0, cur.1)), Some((&next.0, next.1)), 64);
        if got.status != Status::Mismatch {
            break got.status;
        }
        let len = got.len.unwrap();
        cur = (got.buf[..len].to_vec(), got.meta.unwrap());
    };
    assert_eq!((st, rounds), (Status::Ok, 1), "7 << 12 fits 24 bits");
    cur.1 <<= 12;
    let mut rounds = 0;
    let st = loop {
        rounds += 1;
        assert!(rounds < 10, "the loop did not stop on a refusal");
        let got = cx(m, 1, Some((&cur.0, cur.1)), Some((&cur.0, cur.1 << 12)), 64);
        if got.status != Status::Mismatch {
            break got.status;
        }
        cur = (got.buf[..got.len.unwrap()].to_vec(), got.meta.unwrap());
    };
    assert_eq!(st, Status::MetaOverflow);
    assert_eq!(value(m, 1), Some((a.clone(), 7 << 12)));
    // SAFETY: `m` is live.
    unsafe { expanse_sync_blob_map_free(m) };
}

/// A retry loop that reuses one buffer: the observed value lands in the
/// buffer the caller passes as `expected_data` (and as `data` for
/// `get_or_insert`), as the header allows.
#[test]
fn rmw_buffer_may_alias_the_expected_payload() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    let a: Vec<u8> = (100..132u8).collect();
    assert_eq!(insert(m, 1, &a, 5), Status::Ok);
    let mut buf = vec![0u8; 64];
    let p = buf.as_mut_ptr();
    let (mut present, mut len, mut meta) = (false, 0usize, 0u32);
    // SAFETY: `m` is live; `p` is readable and writable for 64 bytes and
    // serves as both the expected payload and the output buffer; the outs
    // are locals.
    unsafe {
        // A stale expected (32 zero bytes) fails and writes the observed
        // value over itself.
        let st = expanse_sync_blob_map_compare_exchange(
            m,
            1,
            true,
            p,
            32,
            5,
            true,
            p,
            32,
            6,
            &raw mut present,
            p,
            64,
            &raw mut len,
            &raw mut meta,
        );
        assert_eq!((st, present, len, meta), (Status::Mismatch, true, 32, 5));
        // Retry from the same buffer: it now matches.
        let st = expanse_sync_blob_map_compare_exchange(
            m,
            1,
            true,
            p,
            len,
            meta,
            true,
            p,
            len,
            6,
            &raw mut present,
            p,
            64,
            &raw mut len,
            &raw mut meta,
        );
        assert_eq!(st, Status::Ok);
        // get_or_insert with `data` and `buf` the same bytes, on a present key.
        core::ptr::write_bytes(p, 0, 64);
        let st =
            expanse_sync_blob_map_get_or_insert(m, 1, p, 40, 0, p, 64, &raw mut len, &raw mut meta);
        assert_eq!((st, len, meta), (Status::Mismatch, 32, 6));
    }
    assert_eq!(&buf[..32], &a[..]);
    assert_eq!(value(m, 1), Some((a, 6)));
    // SAFETY: `m` is live.
    unsafe { expanse_sync_blob_map_free(m) };
}

#[test]
fn compare_exchange_meta_every_branch() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    let a = vec![0x5A; 24];
    assert_eq!(insert(m, 1, &a, 7), Status::Ok);
    // Swap in place; a stale expected reports the current metadata.
    assert_eq!(cx_meta(m, 1, Some(7), Some(8)), (Status::Ok, None));
    assert_eq!(
        cx_meta(m, 1, Some(7), Some(9)),
        (Status::Mismatch, Some(Some(8)))
    );
    assert_eq!(
        cx_meta(m, 1, None, Some(9)),
        (Status::Mismatch, Some(Some(8)))
    );
    assert_eq!(
        value(m, 1),
        Some((a.clone(), 8)),
        "the payload is untouched"
    );
    // Refusals hold only against a matching comparison.
    assert_eq!(
        cx_meta(m, 1, Some(8), Some(1 << 24)),
        (Status::MetaOverflow, None)
    );
    assert_eq!(
        cx_meta(m, 1, Some(9), Some(1 << 24)),
        (Status::Mismatch, Some(Some(8)))
    );
    assert_eq!(
        cx_meta(m, 1, Some(8), Some(0x00FF_FFFF)),
        (Status::Ok, None)
    );
    // An absent key: metadata alone cannot create it.
    assert_eq!(cx_meta(m, 2, None, Some(1)), (Status::NoMetaField, None));
    assert_eq!(cx_meta(m, 2, None, Some(0)), (Status::NoMetaField, None));
    assert_eq!(
        cx_meta(m, 2, Some(0), Some(1)),
        (Status::Mismatch, Some(None))
    );
    assert_eq!(cx_meta(m, 2, None, None), (Status::Ok, None));
    assert_eq!(value(m, 2), None);
    // An inline payload reads metadata 0 and has no field to write.
    assert_eq!(insert(m, 3, b"abc", 0), Status::Ok);
    assert_eq!(cx_meta(m, 3, Some(0), Some(3)), (Status::NoMetaField, None));
    assert_eq!(cx_meta(m, 3, Some(0), Some(0)), (Status::Ok, None));
    // Payloads of 7 bytes or fewer keep the metadata they were inserted with
    // (#1548): it reads back, and swaps in place.
    assert_eq!(insert(m, 4, b"seven!!", 9), Status::Ok);
    assert_eq!(
        cx_meta(m, 4, Some(0), Some(10)),
        (Status::Mismatch, Some(Some(9)))
    );
    assert_eq!(cx_meta(m, 4, Some(9), Some(10)), (Status::Ok, None));
    assert_eq!(value(m, 4), Some((b"seven!!".to_vec(), 10)));
    // Removal on a matching metadata.
    assert_eq!(cx_meta(m, 3, Some(0), None), (Status::Ok, None));
    assert_eq!(
        cx_meta(m, 1, Some(1), None),
        (Status::Mismatch, Some(Some(0x00FF_FFFF)))
    );
    assert_eq!(cx_meta(m, 1, Some(0x00FF_FFFF), None), (Status::Ok, None));
    assert_eq!(value(m, 1), None);
    assert_eq!(value(m, 3), None);
    // SAFETY: `m` is live.
    unsafe {
        assert_eq!(expanse_sync_blob_map_len(m), 1, "key 4");
        expanse_sync_blob_map_free(m);
    }
}

#[test]
fn get_or_insert_every_branch_and_the_buffer_contract() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    let a: Vec<u8> = (0..30u8).collect();
    let b = vec![0xB0; 12];
    // Absent: stored, nothing written.
    assert_eq!(goi(m, 1, &a, 4, 64), Rmw::untouched(Status::Ok, 64));
    // Present: untouched, the stored value reported, whatever is offered.
    assert_eq!(goi(m, 1, &b, 0, 64), Rmw::observed(&a, 4, 64, false));
    assert_eq!(goi(m, 1, &b, 1 << 24, 64), Rmw::observed(&a, 4, 64, false));
    // Too small: nothing copied, nothing stored, length and metadata reported.
    assert_eq!(goi(m, 1, &b, 0, 29), Rmw::observed(&a, 4, 29, false));
    assert_eq!(goi(m, 1, &b, 0, 0), Rmw::observed(&a, 4, 0, false));
    assert_eq!(value(m, 1), Some((a.clone(), 4)));
    // Absent and unstorable: the refusal, the key stays absent.
    assert_eq!(
        goi(m, 2, &a, 1 << 24, 64),
        Rmw::untouched(Status::MetaOverflow, 64)
    );
    assert_eq!(
        goi(m, 2, &vec![1u8; SMALL_CHUNK + 1], 0, 64),
        Rmw::untouched(Status::AllocationFailed, 64)
    );
    assert_eq!(value(m, 2), None);
    // Payloads of 7 bytes or fewer: metadata wider than 24 bits is refused,
    // and metadata in range is stored and reported (#1548).
    assert_eq!(
        goi(m, 3, b"tiny", 1 << 24, 8),
        Rmw::untouched(Status::MetaOverflow, 8)
    );
    assert_eq!(value(m, 3), None);
    assert_eq!(goi(m, 3, b"tiny", 6, 8), Rmw::untouched(Status::Ok, 8));
    assert_eq!(
        goi(m, 3, b"other", 0, 8),
        Rmw::observed(b"tiny", 6, 8, false)
    );
    // The empty payload.
    assert_eq!(goi(m, 4, &[], 0, 0), Rmw::untouched(Status::Ok, 0));
    assert_eq!(goi(m, 4, &a, 0, 0), Rmw::observed(&[], 0, 0, false));
    // SAFETY: `m` is live.
    unsafe {
        assert_eq!(expanse_sync_blob_map_len(m), 3);
        expanse_sync_blob_map_free(m);
    }
}

#[test]
fn rmw_null_and_invalid_arguments_do_nothing() {
    let m = expanse_sync_blob_map_new(SMALL_CHUNK);
    let null_map: *const Map = core::ptr::null();
    let a = vec![0x33; 16];
    assert_eq!(insert(m, 1, &a, 2), Status::Ok);
    let mut buf = [FILL; 8];
    let (mut present, mut len, mut meta) = (true, NO_LEN, NO_META);
    let (pp, lp, mp) = (&raw mut present, &raw mut len, &raw mut meta);
    let null = core::ptr::null::<u8>();
    let big = isize::MAX as usize + 1;
    // SAFETY: each call is documented to reject these arguments before it
    // reads or writes anything; `m` is live; the outs are locals.
    unsafe {
        let cx_with = |map, ep, ed, el, np, nd, nl, b: *mut u8, bl| {
            expanse_sync_blob_map_compare_exchange(
                map, 1, ep, ed, el, 2, np, nd, nl, 3, pp, b, bl, lp, mp,
            )
        };
        let bp = buf.as_mut_ptr();
        let ap = a.as_ptr();
        for (what, st) in [
            (
                "NULL map",
                cx_with(null_map, true, ap, 16, true, ap, 16, bp, 8),
            ),
            (
                "NULL expected data",
                cx_with(m, true, null, 1, true, ap, 16, bp, 8),
            ),
            (
                "NULL new data",
                cx_with(m, true, ap, 16, true, null, 1, bp, 8),
            ),
            (
                "expected len",
                cx_with(m, true, ap, big, true, ap, 16, bp, 8),
            ),
            (
                "NULL buf",
                cx_with(m, true, ap, 16, true, ap, 16, core::ptr::null_mut(), 1),
            ),
            ("buf_len", cx_with(m, true, ap, 16, true, ap, 16, bp, big)),
            (
                "goi NULL map",
                expanse_sync_blob_map_get_or_insert(null_map, 1, ap, 16, 0, bp, 8, lp, mp),
            ),
            (
                "goi NULL data",
                expanse_sync_blob_map_get_or_insert(m, 1, null, 1, 0, bp, 8, lp, mp),
            ),
            (
                "goi NULL buf",
                expanse_sync_blob_map_get_or_insert(
                    m,
                    1,
                    ap,
                    16,
                    0,
                    core::ptr::null_mut(),
                    1,
                    lp,
                    mp,
                ),
            ),
            (
                "meta NULL map",
                expanse_sync_blob_map_compare_exchange_meta(null_map, 1, true, 2, true, 3, pp, mp),
            ),
        ] {
            assert_eq!(st, Status::InvalidArgument, "{what}");
        }
        assert_eq!(
            (present, len, meta, buf),
            (true, NO_LEN, NO_META, [FILL; 8])
        );
        // A value whose flag is clear is not read: NULL data with any length.
        let st = cx_with(m, false, null, 5, false, null, 9, bp, 8);
        assert_eq!(
            (st, present, len, meta),
            (Status::BufferTooSmall, true, 16, 2),
            "the 16-byte value does not fit 8 bytes"
        );
        // NULL outputs are accepted.
        assert_eq!(
            expanse_sync_blob_map_compare_exchange(
                m,
                1,
                true,
                ap,
                16,
                2,
                true,
                ap,
                16,
                3,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                0,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            ),
            Status::Ok
        );
        assert_eq!(
            expanse_sync_blob_map_get_or_insert(
                m,
                1,
                ap,
                16,
                0,
                core::ptr::null_mut(),
                0,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            ),
            Status::BufferTooSmall
        );
        assert_eq!(
            expanse_sync_blob_map_compare_exchange_meta(
                m,
                1,
                false,
                0,
                true,
                1,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            ),
            Status::Mismatch
        );
        assert_eq!(expanse_sync_blob_map_len(m), 1);
        expanse_sync_blob_map_free(m);
    }
}

/// A counter payload: 8 little-endian bytes and 8 bytes no codec compresses,
/// so it is stored in the arena with its metadata.
fn counter(n: u64) -> Vec<u8> {
    let mut v = n.to_le_bytes().to_vec();
    v.extend_from_slice(&[0xFF; 8]);
    v
}

/// Threads incrementing shared counters through the `compare_exchange` retry
/// loop, reusing one buffer for the observed and the expected value, and
/// through `compare_exchange_meta`: every increment lands exactly once.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn concurrent_compare_exchange_increments_lose_no_update() {
    const THREADS: u64 = 4;
    const PER_THREAD: u64 = 1000;
    let m = expanse_sync_blob_map_new(0);
    let addr = m as usize; // raw pointers are not `Send`
    let barrier = std::sync::Barrier::new(THREADS as usize);
    std::thread::scope(|sc| {
        for t in 0..THREADS {
            let barrier = &barrier;
            sc.spawn(move || {
                let m = addr as *const Map;
                barrier.wait();
                for i in 0..PER_THREAD {
                    // Payload counters on keys 0 and 1, the payload and its
                    // metadata both counting.
                    let k = (t + i) % 2;
                    let mut cur = value(m, k);
                    loop {
                        let n = cur
                            .as_ref()
                            .map_or(0, |(d, _)| u64::from_le_bytes(d[..8].try_into().unwrap()));
                        let next = counter(n + 1);
                        let exp = cur.as_ref().map(|(d, meta)| (d.as_slice(), *meta));
                        let got = cx(m, k, exp, Some((&next, (n + 1) as u32)), 16);
                        match got.status {
                            Status::Ok => break,
                            Status::Mismatch => {
                                cur = (got.present == Some(true)).then(|| {
                                    (got.buf[..got.len.unwrap()].to_vec(), got.meta.unwrap())
                                });
                            }
                            other => panic!("compare_exchange: {other:?}"),
                        }
                    }
                    // A metadata counter on key 2 (its payload is stored by
                    // whichever thread gets there first).
                    let st = goi(m, 2, &counter(0), 0, 16).status;
                    assert!(matches!(st, Status::Ok | Status::Mismatch), "{st:?}");
                    let mut meta = 0u32;
                    loop {
                        match cx_meta(m, 2, Some(meta), Some(meta + 1)) {
                            (Status::Ok, None) => break,
                            (Status::Mismatch, Some(Some(seen))) => meta = seen,
                            other => panic!("compare_exchange_meta: {other:?}"),
                        }
                    }
                }
            });
        }
    });
    let mut total = 0;
    for k in 0..2 {
        let (d, meta) = value(m, k).expect("both counters exist");
        let n = u64::from_le_bytes(d[..8].try_into().unwrap());
        assert_eq!(
            u64::from(meta),
            n,
            "key {k}: payload and metadata moved together"
        );
        total += n;
    }
    assert_eq!(total, THREADS * PER_THREAD, "a payload increment was lost");
    assert_eq!(
        value(m, 2).map(|(_, meta)| u64::from(meta)),
        Some(THREADS * PER_THREAD),
        "a metadata increment was lost"
    );
    // SAFETY: no call is in progress and no reader exists.
    unsafe { expanse_sync_blob_map_free(m) };
}

/// Threads racing `get_or_insert` on the same absent keys: exactly one call
/// per key stores, and every other call reports the winner's payload and
/// metadata.
#[test]
#[cfg_attr(miri, ignore = "threaded; the single-threaded tests are the Miri lane")]
fn concurrent_get_or_insert_agrees_on_one_winner() {
    const THREADS: u64 = 4;
    const KEYS: u64 = 1024;
    let payload = |t: u64, k: u64| {
        let mut v = counter(k);
        v.extend_from_slice(&t.to_le_bytes());
        v
    };
    let m = expanse_sync_blob_map_new(0);
    let addr = m as usize; // raw pointers are not `Send`
    let barrier = std::sync::Barrier::new(THREADS as usize);
    let results: Vec<Vec<Rmw>> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let barrier = &barrier;
                sc.spawn(move || {
                    let m = addr as *const Map;
                    (0..KEYS)
                        .map(|k| {
                            // Every thread reaches each key together.
                            barrier.wait();
                            goi(m, k, &payload(t, k), (t + 1) as u32, 64)
                        })
                        .collect()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for k in 0..KEYS {
        let winners: Vec<u64> = (0..THREADS)
            .filter(|&t| results[t as usize][k as usize].status == Status::Ok)
            .collect();
        assert_eq!(winners.len(), 1, "key {k}: winners {winners:?}");
        let w = winners[0];
        for t in 0..THREADS {
            if t != w {
                assert_eq!(
                    results[t as usize][k as usize],
                    Rmw::observed(&payload(w, k), (w + 1) as u32, 64, false),
                    "key {k}, thread {t}"
                );
            }
        }
        assert_eq!(value(m, k), Some((payload(w, k), (w + 1) as u32)));
    }
    // SAFETY: no call is in progress and no reader exists.
    unsafe { expanse_sync_blob_map_free(m) };
}
