//! The validated batch cursor on the shared string map (#1143,
//! `docs/benchmarks/concurrency/METHODOLOGY.md` §33.3 G33.1, G33.5, G33.6
//! and §33.10).
//!
//! The tests whose names start with `tier1_` run in the per-pull-request
//! Miri filter (AGENTS.md §5), whose shard has little time to spare, so they
//! are the small ones that between them reach every route of the walk: the
//! exact descent, the drain of a root leaf and of a tree with branches,
//! suffix copies, the climbs, a refused admission's restarts, the end bound,
//! the spill buffer, a holder check and a cover check. The rest run under
//! Miri in the nightly `lib-sync` shard.

use super::*;
use crate::sync_str_cursor::test_hooks::{self as th, Control, Count, Site};
use crate::sync_str_cursor::{STR_BATCH_CAP, STR_KEY_ARENA, SyncStrMapCursor};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::vec;
use std::vec::Vec;

type Entries = Vec<(Vec<u8>, u64)>;

fn nf(b: &[u8]) -> &NulFreeStr {
    NulFreeStr::new(b).expect("test keys are NUL-free")
}

/// Every entry a cursor returns, in order.
fn drain(mut cur: SyncStrMapCursor<'_, '_>) -> Entries {
    let mut out = Vec::new();
    while let Some((k, v)) = cur.next() {
        out.push((k.to_vec(), v));
    }
    // An exhausted cursor stays exhausted.
    assert_eq!(cur.next(), None);
    assert_eq!(cur.current(), None);
    out
}

fn build(keys: &[Vec<u8>]) -> (SyncExpanseStrMap, BTreeMap<Vec<u8>, u64>) {
    let map = SyncExpanseStrMap::new();
    let mut model = BTreeMap::new();
    for (i, k) in keys.iter().enumerate() {
        map.insert(nf(k), i as u64);
        model.insert(k.clone(), i as u64);
    }
    (map, model)
}

fn model_range(model: &BTreeMap<Vec<u8>, u64>, lo: &[u8], hi: Option<&[u8]>) -> Entries {
    model
        .range(lo.to_vec()..)
        .take_while(|(k, _)| hi.is_none_or(|h| k.as_slice() < h))
        .map(|(k, &v)| (k.clone(), v))
        .collect()
}

/// The single-threaded `StrCursor` from `lo` on, under the writer lock.
fn locked_from(map: &SyncExpanseStrMap, lo: &[u8]) -> Entries {
    map.with_locked(|m| {
        let mut cur = m.cursor_at_or_after(nf(lo));
        let mut out = Vec::new();
        while let Some((k, v)) = cur.next_entry() {
            out.push((k.to_vec(), v));
        }
        out
    })
}

/// The single-threaded prefix cursor, under the writer lock.
fn locked_prefix(map: &SyncExpanseStrMap, p: &[u8]) -> Entries {
    map.with_locked(|m| {
        let mut cur = m.cursor_prefix(nf(p));
        let mut out = Vec::new();
        while let Some((k, v)) = cur.next_entry() {
            out.push((k.to_vec(), v));
        }
        out
    })
}

/// Runs `body` on a fresh thread with `controls` on, and returns its result
/// with that thread's counts.
fn on_thread<R: Send>(controls: &[Control], body: impl FnOnce() -> R + Send) -> (R, [u64; 8]) {
    std::thread::scope(|s| {
        s.spawn(|| {
            for &c in controls {
                th::set_control(c, true);
            }
            let r = body();
            (r, th::take_counts())
        })
        .join()
        .expect("reader thread")
    })
}

fn count(c: [u64; 8], which: Count) -> u64 {
    c[which as usize]
}

/// Arms `site` with `write` and returns how many times it ran.
fn arm(site: Site, write: impl FnOnce() + 'static) -> Arc<AtomicUsize> {
    let fired = Arc::new(AtomicUsize::new(0));
    let f = Arc::clone(&fired);
    th::arm_site(
        site,
        Box::new(move || {
            f.fetch_add(1, Ordering::SeqCst);
            write();
        }),
    );
    fired
}

/// Arms `site` so that `write` runs the `nth` time (from 1) the walk
/// reaches it.
fn arm_nth(site: Site, nth: usize, write: impl FnOnce() + 'static) -> Arc<AtomicUsize> {
    let fired = Arc::new(AtomicUsize::new(0));
    fn rearm(site: Site, left: usize, write: Box<dyn FnOnce()>, fired: Arc<AtomicUsize>) {
        th::arm_site(
            site,
            Box::new(move || {
                if left <= 1 {
                    fired.fetch_add(1, Ordering::SeqCst);
                    write();
                } else {
                    rearm(site, left - 1, write, fired);
                }
            }),
        );
    }
    rearm(site, nth, Box::new(write), Arc::clone(&fired));
    fired
}

// ---- Boundaries (G33.6) ----------------------------------------------------

/// Keys around the 8-byte chunk edge: lengths 7, 8 and 9 sharing a prefix,
/// a terminal against a continuation byte (`abc` against `abc\x01`), runs of
/// `0xFF`, and keys that diverge only in a suffix leaf.
fn boundary_keys() -> Vec<Vec<u8>> {
    [
        &b"a"[..],
        b"abc",
        b"abc\x01",
        b"abc\x01\x01",
        b"abc\xFF",
        b"abc\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF",
        b"abcdefg",
        b"abcdefg\x01",
        b"abcdefgh",
        b"abcdefgh\x01",
        b"abcdefghi",
        b"abcdefghijklmnopq",
        b"abcdefghijklmnopr",
        b"abd",
        b"b",
        b"\xFF",
        b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF",
        b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x01",
    ]
    .iter()
    .map(|k| k.to_vec())
    .collect()
}

/// Probes around every key: the key, its cuts at the chunk edge, its last
/// byte moved either way, and its extensions by `0x01` and `0xFF`.
fn probes_around(keys: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(), vec![0x01], vec![0xFF; 20]];
    for k in keys {
        out.push(k.clone());
        for cut in [k.len().saturating_sub(1), k.len() / 8 * 8, 7, 8, 9] {
            if cut <= k.len() {
                out.push(k[..cut].to_vec());
            }
        }
        if let Some(&last) = k.last() {
            let mut up = k.clone();
            let mut down = k.clone();
            *up.last_mut().unwrap() = last.saturating_add(1);
            *down.last_mut().unwrap() = last.saturating_sub(1).max(1);
            out.push(up);
            out.push(down);
        }
        for ext in [0x01u8, 0xFF] {
            let mut e = k.clone();
            e.push(ext);
            out.push(e);
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn tier1_empty_map_and_empty_ranges() {
    let map = SyncExpanseStrMap::new();
    let rd = map.reader();
    assert_eq!(drain(rd.cursor()), Entries::new());
    assert_eq!(drain(rd.prefix_cursor(nf(b""))), Entries::new());
    map.insert(nf(b"abc"), 1);
    // lo >= hi is empty, as is lo == hi on a present key.
    assert_eq!(drain(rd.range_cursor(nf(b"b"), nf(b"a"))), Entries::new());
    assert_eq!(
        drain(rd.range_cursor(nf(b"abc"), nf(b"abc"))),
        Entries::new()
    );
    assert_eq!(
        drain(rd.range_cursor(nf(b"abc"), nf(b"abc\x01"))),
        vec![(b"abc".to_vec(), 1)]
    );
}

/// The prefix scan at every boundary G33.6 lists, against the
/// single-threaded prefix cursor and the model.
#[test]
fn tier1_prefix_scan_at_the_boundaries() {
    let keys = boundary_keys();
    let (map, model) = build(&keys);
    let rd = map.reader();
    let cases: [(&[u8], &str); 9] = [
        (b"", "the empty prefix scans the whole map"),
        (b"abcdefgh", "a prefix equal to a whole key"),
        (b"abz", "a prefix with no keys"),
        (b"abcdefg", "a prefix of 7 bytes"),
        (b"abcdefgh\x01", "a prefix of 9 bytes"),
        (b"abcdefghi", "a prefix of 9 bytes equal to a key"),
        (b"abc", "a terminal against continuation bytes"),
        (b"abc\x01", "a continuation byte as the prefix's last"),
        (b"\xFF", "an all-0xFF prefix, unbounded above"),
    ];
    for (p, what) in cases {
        let got = drain(rd.prefix_cursor(nf(p)));
        let want: Entries = model
            .iter()
            .filter(|(k, _)| k.starts_with(p))
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        assert_eq!(got, want, "{what}: prefix {p:x?}");
        assert_eq!(
            got,
            locked_prefix(&map, p),
            "{what}: against StrPrefixCursor"
        );
    }
    // NUL-terminal ordering: `abc` precedes its continuation `abc\x01`.
    let got = drain(rd.prefix_cursor(nf(b"abc")));
    let pos = |k: &[u8]| got.iter().position(|(g, _)| g == k).unwrap();
    assert!(pos(b"abc") < pos(b"abc\x01"));
    assert!(pos(b"abc\x01") < pos(b"abc\x01\x01"));
    assert!(pos(b"abc\x01\x01") < pos(b"abc\xFF"));
    assert!(pos(b"abcdefg") < pos(b"abcdefg\x01"));
    assert!(pos(b"abcdefg\x01") < pos(b"abcdefgh"));
}

/// `[lo, hi)` for every pair of boundary probes.
#[test]
fn range_scan_at_the_boundaries() {
    let keys = boundary_keys();
    let (map, model) = build(&keys);
    let rd = map.reader();
    let probes = probes_around(&keys);
    let step = if cfg!(miri) { 7 } else { 1 };
    for lo in probes.iter().step_by(step) {
        for hi in probes.iter().step_by(step) {
            let got = drain(rd.range_cursor(nf(lo), nf(hi)));
            let want = if lo < hi {
                model_range(&model, lo, Some(hi))
            } else {
                Entries::new()
            };
            assert_eq!(got, want, "[{lo:x?}, {hi:x?})");
        }
        assert_eq!(
            drain(SyncStrMapCursor::range(
                &map,
                &rd.reader,
                nf(lo),
                nf(&[0xFF; 30])
            )),
            model_range(&model, lo, Some(&[0xFF; 30])),
        );
    }
}

/// Keys whose remainders are suffix leaves, child nodes and terminal
/// chunks, so a scan descends, restarts into a child, passes a suffix below
/// the probe and ends a batch at a child.
#[test]
fn scan_crosses_levels_and_suffixes() {
    let mut keys = Vec::new();
    for a in [&b"aaaaaaaa"[..], b"aaaaaaab", b"bbbbbbbb"] {
        for t in [&b""[..], b"\x01", b"M", b"MM", b"MN", b"Z", b"ZZZZZZZZZ"] {
            let mut k = a.to_vec();
            k.extend_from_slice(t);
            keys.push(k);
        }
    }
    keys.push(b"aaaaaaaaQsuffix".to_vec());
    keys.push(b"aa".to_vec());
    let (map, model) = build(&keys);
    let rd = map.reader();
    assert_eq!(drain(rd.cursor()), model_range(&model, b"", None));
    for lo in probes_around(&keys) {
        assert_eq!(
            drain(SyncStrMapCursor::range(
                &map,
                &rd.reader,
                nf(&lo),
                nf(&[0xFF])
            )),
            model_range(&model, &lo, Some(&[0xFF])),
            "from {lo:x?}"
        );
    }
}

/// Every admission refused (§33.11.2 item 4): each attempt ends at its first
/// drained level, with a batch when it copied entries and otherwise with a
/// restart at the child's prefix or at the level's next chunk, and the scan
/// still returns exactly the model from every probe.
#[test]
fn tier1_refused_admission_restarts_by_key() {
    let mut keys = Vec::new();
    for a in [&b"aaaaaaaa"[..], b"aaaaaaab", b"bbbbbbbb"] {
        for t in [&b""[..], b"M", b"MM", b"Z", b"ZZZZZZZZZ"] {
            let mut k = a.to_vec();
            k.extend_from_slice(t);
            keys.push(k);
        }
    }
    keys.push(b"aa".to_vec());
    let (map, model) = build(&keys);
    let (got, c) = on_thread(&[Control::RefuseAdmission], || {
        let rd = map.reader();
        let all = drain(rd.cursor());
        for lo in [&b"aaaaaaaaZ"[..], b"aaaaaaab", b"ab"] {
            assert_eq!(
                drain(SyncStrMapCursor::range(
                    &map,
                    &rd.reader,
                    nf(lo),
                    nf(&[0xFF])
                )),
                model_range(&model, lo, Some(&[0xFF])),
                "from {lo:x?}"
            );
        }
        all
    });
    assert_eq!(got, model_range(&model, b"", None));
    assert!(count(c, Count::Restarts) >= 1, "{c:?}");
    assert_eq!(count(c, Count::Fallbacks), 0, "{c:?}");
}

/// A key longer than the arena is returned alone, from the spill buffer,
/// between batches that hold the keys around it.
#[test]
fn tier1_oversized_key_is_a_batch_of_one() {
    let mut long = vec![b'm'; STR_KEY_ARENA + 100];
    long[STR_KEY_ARENA] = b'x';
    let keys = vec![
        b"a".to_vec(),
        b"m".to_vec(),
        long.clone(),
        b"mz".to_vec(),
        b"z".to_vec(),
    ];
    let (map, model) = build(&keys);
    let rd = map.reader();
    assert_eq!(drain(rd.cursor()), model_range(&model, b"", None));
    let mut cur = rd.cursor();
    assert_eq!(
        cur.advance_to(nf(b"mm")).map(|(k, _)| k.len()),
        Some(long.len())
    );
    assert_eq!(cur.next().map(|(k, _)| k.to_vec()), Some(long));
    assert_eq!(cur.next().map(|(k, _)| k.to_vec()), Some(b"mz".to_vec()));
}

/// A root sub-map in tree state (more than `ROOT_LEAF_CAP` keys, with
/// branches under it), scanned whole, in a range that ends inside it, and
/// from a seek: the drain's branch arms and its end-bound stop.
#[test]
fn tier1_tree_state_drain() {
    let keys = wide_keys(80);
    let (map, model) = build(&keys);
    let rd = map.reader();
    assert_eq!(drain(rd.cursor()), model_range(&model, b"", None));
    let (lo, hi) = (&keys[20], &keys[50]);
    assert_eq!(
        drain(rd.range_cursor(nf(lo), nf(hi))),
        model_range(&model, lo, Some(hi))
    );
    let mut cur = rd.cursor();
    let got = cur.advance_to(nf(&keys[60])).map(|(k, v)| (k.to_vec(), v));
    assert_eq!(got, Some((keys[60].clone(), model[&keys[60]])));
}

// ---- The three defects of the map cursor (#1382, #1385) --------------------

/// 7-byte keys: each is one terminal chunk of the root's sub-map, so the
/// root is a tree of many branches and every scan is a depth-1 attempt.
fn wide_keys(n: usize) -> Vec<Vec<u8>> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut seen = std::collections::BTreeSet::new();
    while seen.len() < n {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let k: Vec<u8> = state.to_be_bytes()[..7].iter().map(|&b| b.max(1)).collect();
        seen.insert(k);
    }
    seen.into_iter().collect()
}

/// An empty range with the whole upper tree to its right, and a range of
/// one key, each sample at most one attempt's bound at depth 1 (14,
/// `scripts/str_cursor_bounds.py::str_cursor_drain_attempt_read_set_bound`).
/// The negative control lets the drain's branch loops walk past the end
/// bound, as the map cursor did before #1382: the walk then samples a branch
/// per subtree to the right until the read set is full.
fn bounded_probe(controls: &[Control]) -> ([u64; 8], [u64; 8], usize) {
    let keys = wide_keys(if cfg!(miri) { 320 } else { 20_000 });
    let (map, model) = build(&keys);
    let k = keys[keys.len() / 8].clone();
    // `k ++ 0x01 .. k ++ 0x02` holds no key and its end bound is not a key,
    // so the drain meets the next key above the bound without taking it.
    let (mut gap_lo, mut gap_hi) = (k.clone(), k.clone());
    gap_lo.push(1);
    gap_hi.push(2);
    let ((), empty) = on_thread(controls, || {
        let rd = map.reader();
        assert_eq!(
            drain(rd.range_cursor(nf(&gap_lo), nf(&gap_hi))),
            Entries::new()
        );
    });
    let ((), one) = on_thread(controls, || {
        let rd = map.reader();
        assert_eq!(
            drain(rd.range_cursor(nf(&k), nf(&gap_hi))),
            vec![(k.clone(), model[&k])]
        );
    });
    // A bounded scan of several batches still returns exactly the range.
    let (lo, hi) = (&keys[keys.len() / 4], &keys[keys.len() / 2]);
    let rd = map.reader();
    let got = drain(rd.range_cursor(nf(lo), nf(hi)));
    assert_eq!(got, model_range(&model, lo, Some(hi)));
    (empty, one, got.len())
}

#[test]
fn bounded_scan_does_not_walk_past_end() {
    let (empty, one, len) = bounded_probe(&[]);
    assert!(
        len > 2 * STR_BATCH_CAP || cfg!(miri),
        "the range must span batches"
    );
    for (what, c) in [("an empty range", empty), ("a one-key range", one)] {
        assert_eq!(count(c, Count::Fallbacks), 0, "{what}: {c:?}");
        assert!(
            count(c, Count::Samples) <= 14 * count(c, Count::Attempts),
            "{what} sampled {} versions in {} attempts: the walk went past the end bound",
            count(c, Count::Samples),
            count(c, Count::Attempts)
        );
    }
    assert_eq!(count(empty, Count::Attempts), 1, "{empty:?}");
}

/// The negative control of the end-bound stop: it fails the bound the test
/// above asserts.
#[test]
#[cfg_attr(miri, ignore = "20,000 inserts; the tier-1 test covers the bound")]
fn bounded_scan_that_walks_past_end_overflows_its_read_set() {
    let (empty, _, _) = bounded_probe(&[Control::WalkPastEnd]);
    assert!(
        count(empty, Count::Samples) > 14 * count(empty, Count::Attempts),
        "{empty:?}"
    );
    assert_eq!(count(empty, Count::Overflows), 1, "{empty:?}");
}

/// `advance_to` never rewinds: a target below the position returns the
/// current entry at every position, batch edges included, and a target
/// below the range's start does not leave the range.
#[test]
fn advance_to_never_rewinds() {
    let keys = wide_keys(if cfg!(miri) { 320 } else { 2_000 });
    let (map, model) = build(&keys);
    let rd = map.reader();
    let mut cur = rd.cursor();
    for (i, k) in keys.iter().enumerate() {
        assert_eq!(cur.next().map(|(g, _)| g.to_vec()), Some(k.clone()));
        let want = keys.get(i + 1).map(|n| (n.clone(), model[n]));
        let got = cur.advance_to(nf(b"")).map(|(g, v)| (g.to_vec(), v));
        assert_eq!(got, want, "advance_to(\"\") after {k:x?}");
        let got = cur.advance_to(nf(k)).map(|(g, v)| (g.to_vec(), v));
        assert_eq!(got, want, "advance_to({k:x?}) after {k:x?}");
    }
    let (lo, hi) = (&keys[10], &keys[20]);
    let mut cur = rd.range_cursor(nf(lo), nf(hi));
    assert_eq!(
        cur.advance_to(nf(&keys[3])).map(|(g, _)| g.to_vec()),
        Some(lo.clone())
    );
    assert_eq!(cur.next().map(|(g, _)| g.to_vec()), Some(lo.clone()));
}

/// A seek to or past the end bound exhausts the cursor: nothing buffered
/// stays readable, and a later seek below it does not revive it.
#[test]
fn advance_to_past_end_exhausts() {
    let keys: Vec<Vec<u8>> = (1..=100u32)
        .map(|i| format!("k{i:03}").into_bytes())
        .collect();
    let (map, _) = build(&keys);
    let rd = map.reader();
    let mut cur = rd.range_cursor(nf(b"k001"), nf(b"k051"));
    assert_eq!(cur.next().map(|(k, _)| k.to_vec()), Some(b"k001".to_vec()));
    assert_eq!(cur.advance_to(nf(b"k051")), None);
    assert_eq!(cur.current(), None);
    assert_eq!(cur.next(), None);
    assert_eq!(cur.advance_to(nf(b"k010")), None);
    // Past the end, from inside the buffer, of a prefix scan.
    let mut cur = rd.prefix_cursor(nf(b"k0"));
    assert_eq!(
        cur.advance_to(nf(b"k050")).map(|(k, _)| k.to_vec()),
        Some(b"k050".to_vec())
    );
    assert_eq!(cur.advance_to(nf(b"k1")), None);
    assert_eq!(cur.next(), None);
}

/// `next` and `advance_to` interleaved, against the model.
#[test]
fn advance_to_interleaved_with_next() {
    let keys = wide_keys(if cfg!(miri) { 320 } else { 3_000 });
    let (map, model) = build(&keys);
    let rd = map.reader();
    let mut cur = rd.cursor();
    // The smallest key the cursor may return next.
    let mut floor: Vec<u8> = Vec::new();
    for i in 0..keys.len() / 2 {
        let want;
        if i % 3 == 0 {
            let target = &keys[(i * 7) % keys.len()];
            if target > &floor {
                floor = target.clone();
            }
            want = model
                .range(floor.clone()..)
                .next()
                .map(|(k, &v)| (k.clone(), v));
            let got = cur.advance_to(nf(target)).map(|(k, v)| (k.to_vec(), v));
            assert_eq!(got, want, "step {i} advance_to");
            // `advance_to` does not consume: `next` returns the same entry.
            if let Some((k, _)) = &want {
                floor = k.clone();
            }
        } else {
            want = model
                .range(floor.clone()..)
                .next()
                .map(|(k, &v)| (k.clone(), v));
            let got = cur.next().map(|(k, v)| (k.to_vec(), v));
            assert_eq!(got, want, "step {i} next");
            if let Some((k, _)) = &want {
                floor = k.clone();
                floor.push(1);
            }
        }
        if want.is_none() {
            break;
        }
    }
}

// ---- Differential against StrCursor on quiescent maps (G33.5) --------------

/// `paths`-shaped keys (a 35-byte shared prefix and 12 hex digits, depth 5)
/// and `routes`-shaped keys (short shared segments, many child nodes).
fn generated_keys(n: usize, routes: bool) -> Vec<Vec<u8>> {
    let mut state = 0x5A71_C1A0_0000_0001u64;
    let mut seen = std::collections::BTreeSet::new();
    while seen.len() < n {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let k = if routes {
            format!(
                "/r{}/{}/i{}",
                state % 7,
                (state >> 8) % 300,
                (state >> 20) % 1000
            )
        } else {
            format!(
                "https://example.com/api/v2/objects/{:012x}",
                state & 0xFFFF_FFFF_FFFF
            )
        };
        seen.insert(k.into_bytes());
    }
    seen.into_iter().collect()
}

#[test]
#[cfg_attr(miri, ignore = "thousands of keys; the tier-1 tests cover each route")]
fn scan_matches_str_cursor_on_generated_keys() {
    for routes in [false, true] {
        let keys = generated_keys(5_000, routes);
        let (map, model) = build(&keys);
        let rd = map.reader();
        let (got, c) = on_thread(&[], || drain(map.reader().cursor()));
        assert_eq!(got, locked_from(&map, b""), "routes = {routes}");
        assert_eq!(got.len(), model.len());
        assert_eq!(count(c, Count::Fallbacks), 0, "{c:?}");
        for i in (0..keys.len()).step_by(97) {
            let lo = &keys[i];
            let hi = &keys[(i + 300).min(keys.len() - 1)];
            assert_eq!(
                drain(rd.range_cursor(nf(lo), nf(hi))),
                model_range(&model, lo, Some(hi))
            );
            let p = &lo[..lo.len().min(38)];
            assert_eq!(drain(rd.prefix_cursor(nf(p))), locked_prefix(&map, p));
        }
    }
}

/// Keys sharing a 1,096-byte prefix sit 138 levels deep: an attempt there
/// overflows the read set of 128 while it still validates, and the batch is
/// filled under the writer lock at once instead of after every retry.
#[test]
#[cfg_attr(miri, ignore = "1,096-byte keys; the tier-1 tests cover the fallback")]
fn deep_scan_overflow_falls_back_at_once() {
    let base = vec![b'p'; 1096];
    let keys: Vec<Vec<u8>> = [&b"a"[..], b"b", b"c"]
        .iter()
        .map(|t| {
            let mut k = base.clone();
            k.extend_from_slice(t);
            k
        })
        .collect();
    let (map, model) = build(&keys);
    let (got, c) = on_thread(&[], || drain(map.reader().cursor()));
    assert_eq!(got, model_range(&model, b"", None));
    assert!(count(c, Count::Overflows) >= 1, "{c:?}");
    assert_eq!(
        count(c, Count::Overflows),
        count(c, Count::Fallbacks),
        "{c:?}"
    );
}

/// The key sets of `sync_strmap_scan` (`path_keys` in
/// `crates/expanse/benches/instructions.rs`, the same generator and seed).
fn bench_path_keys(dense: bool, n: usize) -> Vec<Vec<u8>> {
    let mut x = 0x5A71_C1A0_0000_0001u64;
    let mut seen = std::collections::HashSet::with_capacity(n);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let id = if dense {
            let bucket = (x >> 28) % 52_428 * 20;
            (bucket << 28) | (x & 0xFFF_FFFF)
        } else {
            x & 0xFFFF_FFFF_FFFF
        };
        if seen.insert(id) {
            out.push(format!("https://example.com/api/v2/objects/{id:012x}").into_bytes());
        }
    }
    out
}

/// One single-threaded full scan of a benchmark key set: the entries it
/// returned against the model, and its counts.
fn bench_scan(dense: bool, controls: &[Control]) -> (usize, [u64; 8]) {
    let keys = bench_path_keys(dense, 50_000);
    let (map, model) = build(&keys);
    let (got, c) = on_thread(controls, || drain(map.reader().cursor()));
    assert_eq!(got, model_range(&model, b"", None));
    (got.len(), c)
}

/// P33.5 (§33.11.4): a single-threaded scan of each benchmark key set takes
/// no restart, no overflow and no fallback, and at least 32 (`paths`) and
/// 16 (`paths_dense`) keys per attempt.
#[test]
#[cfg_attr(miri, ignore = "50,000 keys per set; the tier-1 tests cover the walk")]
fn bench_key_sets_scan_in_few_attempts() {
    for (dense, floor) in [(false, 32), (true, 16)] {
        let (len, c) = bench_scan(dense, &[]);
        let attempts = count(c, Count::Attempts);
        std::eprintln!(
            "P33.5 {}: {len} keys, {attempts} attempts ({:.1} keys per attempt), {c:?}",
            if dense { "paths_dense" } else { "paths" },
            len as f64 / attempts as f64
        );
        assert_eq!(count(c, Count::Restarts), 0, "{c:?}");
        assert_eq!(count(c, Count::Overflows), 0, "{c:?}");
        assert_eq!(count(c, Count::Fallbacks), 0, "{c:?}");
        assert!(
            len as u64 >= floor * attempts,
            "{len} keys in {attempts} attempts: {c:?}"
        );
    }
}

/// The negative control of admission: without it the walk enters children
/// until the read set is full, and the attempt overflows into the writer
/// lock.
#[test]
#[cfg_attr(miri, ignore = "50,000 keys; the tier-1 tests cover the walk")]
fn bench_key_set_scan_without_admission_overflows() {
    let (_, c) = bench_scan(true, &[Control::SkipAdmission]);
    assert!(count(c, Count::Overflows) >= 1, "{c:?}");
}

// ---- Park points with negative controls (G33.1) ----------------------------

/// A writer inserts into the drained level before the final validation.
/// The batch must describe one instant: the validation fails, the attempt
/// retries and the batch holds the new key. Without the final validation the
/// batch is the drain's stale copy.
fn park_final(controls: &[Control]) -> (Entries, [u64; 8]) {
    let keys: Vec<Vec<u8>> = (0..40u32)
        .map(|i| format!("key{:03}", 2 * i).into_bytes())
        .collect();
    let map = Arc::new(build(&keys).0);
    let (got, c) = on_thread(controls, || {
        let w = Arc::clone(&map);
        let fired = arm(Site::Final, move || {
            assert_eq!(w.insert(nf(b"key011"), 99), None);
        });
        let got = drain(map.reader().cursor());
        assert_eq!(fired.load(Ordering::SeqCst), 1);
        got
    });
    (got, c)
}

#[test]
fn park_insert_into_the_drained_level_before_validation() {
    let (got, c) = park_final(&[]);
    assert!(
        got.iter().any(|(k, v)| k == b"key011" && *v == 99),
        "{got:?}"
    );
    assert!(count(c, Count::Attempts) >= 2, "{c:?}");
}

#[test]
fn park_without_final_validation_returns_the_stale_batch() {
    let (got, _) = park_final(&[Control::SkipFinalValidation]);
    assert!(!got.iter().any(|(k, _)| k == b"key011"), "{got:?}");
}

/// The climb's validation: the probe `aaaaaaaaZ` descends into C under
/// `aaaaaaaa`, finds nothing there at or after `Z`, and climbs to the root
/// past C (§33.11.2). A writer inserts `aaaaaaaaZZ` into C at that attempt's
/// final validation. The climb asserts C held nothing past the probe, so the
/// validation fails and the retry returns the new key first; without it the
/// cursor skips C and starts at S.
fn park_restart(controls: &[Control]) -> (Entries, [u64; 8]) {
    let keys = [
        &b"aaaaaaaaAA"[..],
        b"aaaaaaaaAB",
        b"bbbbbbbbM",
        b"bbbbbbbbZ",
    ];
    let keys: Vec<Vec<u8>> = keys.iter().map(|k| k.to_vec()).collect();
    let map = Arc::new(build(&keys).0);
    let root_and_c = {
        // SAFETY: no writer runs on the map here.
        let m = unsafe { &*map.shared.tree_ptr() };
        (
            m.test_cover_at(&[]).expect("root"),
            m.test_cover_at(&[u64::from_be_bytes(*b"aaaaaaaa")])
                .expect("C is a child node"),
        )
    };
    let version = |vp: *const u32| {
        // SAFETY: the nodes are live for the whole test and not replaced.
        unsafe { crate::occ::version_cell(vp) }.load(Ordering::Acquire)
    };
    let before = (version(root_and_c.0), version(root_and_c.1));
    let (got, c) = on_thread(controls, || {
        let w = Arc::clone(&map);
        let fired = arm(Site::Final, move || {
            assert_eq!(w.insert(nf(b"aaaaaaaaZZ"), 10), None);
        });
        let rd = map.reader();
        let got = drain(rd.range_cursor(nf(b"aaaaaaaaZ"), nf(b"c")));
        assert_eq!(fired.load(Ordering::SeqCst), 1);
        got
    });
    let after = (version(root_and_c.0), version(root_and_c.1));
    assert_eq!(after.0, before.0, "the root's cover must not move");
    assert_ne!(after.1, before.1, "the insert must move C's cover");
    (got, c)
}

#[test]
fn tier1_park_restart_past_a_level_that_changed() {
    let (got, c) = park_restart(&[]);
    assert_eq!(got.first(), Some(&(b"aaaaaaaaZZ".to_vec(), 10)), "{got:?}");
    assert!(count(c, Count::Climbs) >= 1, "{c:?}");
    assert!(count(c, Count::Attempts) >= 2, "{c:?}");
    assert_eq!(count(c, Count::Fallbacks), 0, "{c:?}");
}

#[test]
fn park_restart_without_validation_skips_the_level() {
    let (got, _) = park_restart(&[Control::SkipFinalValidation]);
    assert_eq!(got.first(), Some(&(b"bbbbbbbbM".to_vec(), 2)), "{got:?}");
}

/// T4, a suffix split: the drain copies the root's pair for `kkkkkkkk`, a
/// suffix leaf, and a writer then inserts a second key under that chunk,
/// which replaces the suffix with a child node and retires it. The holder
/// check before the suffix is read must fail and retry; without it the walk
/// reads the retired suffix through a holder that no longer validates.
fn park_suffix_split(controls: &[Control]) -> (Entries, [u64; 8]) {
    let keys = [&b"a"[..], b"kkkkkkkkSUFFIX1", b"z"];
    let keys: Vec<Vec<u8>> = keys.iter().map(|k| k.to_vec()).collect();
    let map = Arc::new(build(&keys).0);
    on_thread(controls, || {
        let w = Arc::clone(&map);
        let fired = arm(Site::Drained, move || {
            assert_eq!(w.insert(nf(b"kkkkkkkkSUFFIX2"), 7), None);
        });
        let got = drain(map.reader().cursor());
        assert_eq!(fired.load(Ordering::SeqCst), 1);
        got
    })
}

#[test]
fn tier1_park_suffix_split_retries_before_the_dereference() {
    let (got, c) = park_suffix_split(&[]);
    assert!(got.iter().any(|(k, _)| k == b"kkkkkkkkSUFFIX2"), "{got:?}");
    assert!(count(c, Count::HolderRetries) >= 1, "{c:?}");
    assert_eq!(count(c, Count::UnvalidatedDerefs), 0, "{c:?}");
}

#[test]
fn park_suffix_split_without_the_holder_check_reads_a_retired_suffix() {
    let (got, c) = park_suffix_split(&[Control::SkipHolderCheck]);
    // The final validation still catches the change; the control shows the
    // dereference the holder check exists to prevent.
    assert!(got.iter().any(|(k, _)| k == b"kkkkkkkkSUFFIX2"), "{got:?}");
    assert_eq!(count(c, Count::UnvalidatedDerefs), 1, "{c:?}");
    assert_eq!(count(c, Count::HolderRetries), 0, "{c:?}");
}

/// T8/T9, a child emptied and pruned: the walk enters C under `cccccccc`
/// and a writer removes every key under it at C's level entry, which
/// unlinks C and marks it obsolete. The cover check must fail; without it,
/// and without the final validation, the cursor returns the removed keys
/// from C's stale root state.
fn park_prune(controls: &[Control]) -> (Entries, [u64; 8]) {
    let gone = [&b"ccccccccA"[..], b"ccccccccB"];
    let keys = [&b"a"[..], gone[0], gone[1], b"z"];
    let keys: Vec<Vec<u8>> = keys.iter().map(|k| k.to_vec()).collect();
    let map = Arc::new(build(&keys).0);
    on_thread(controls, || {
        let w = Arc::clone(&map);
        // The root's entry is the first, C's the second.
        let fired = arm_nth(Site::LevelEntry, 2, move || {
            for k in gone {
                assert!(w.remove(nf(k)).is_some());
            }
        });
        let got = drain(map.reader().range_cursor(nf(b"cccccccc"), nf(b"d")));
        assert_eq!(fired.load(Ordering::SeqCst), 1);
        got
    })
}

#[test]
fn tier1_park_pruned_child_is_retried() {
    let (got, c) = park_prune(&[]);
    assert_eq!(got, Entries::new());
    assert!(count(c, Count::Attempts) >= 2, "{c:?}");
    assert_eq!(count(c, Count::UnvalidatedDerefs), 0, "{c:?}");
}

#[test]
fn park_pruned_child_without_the_cover_check_returns_removed_keys() {
    let (got, c) = park_prune(&[Control::SkipCoverCheck, Control::SkipFinalValidation]);
    assert_eq!(
        got,
        vec![(b"ccccccccA".to_vec(), 1), (b"ccccccccB".to_vec(), 2)]
    );
    assert_eq!(count(c, Count::UnvalidatedDerefs), 1, "{c:?}");
}

/// The cursor under a running writer, on several threads: every key that
/// no writer touches is returned, in strictly ascending order, once.
#[test]
#[cfg_attr(miri, ignore = "concurrent; the UB-site workload covers it under Miri")]
fn scan_under_writers_returns_every_stable_key() {
    let stable: Vec<Vec<u8>> = generated_keys(3_000, true);
    let map = Arc::new(build(&stable).0);
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            for round in 0..20u64 {
                for i in 0..200u64 {
                    let k = format!("/r{}/{}/v{}", i % 7, i % 300, round);
                    map.insert(nf(k.as_bytes()), i);
                    if i % 2 == 0 {
                        map.remove(nf(k.as_bytes()));
                    }
                }
            }
            done.store(true, Ordering::Release);
        });
        for _ in 0..2 {
            s.spawn(|| {
                let rd = map.reader();
                loop {
                    let finished = done.load(Ordering::Acquire);
                    let got = drain(rd.cursor());
                    assert!(got.windows(2).all(|w| w[0].0 < w[1].0), "ascending, once");
                    let mut it = got.iter().peekable();
                    for k in &stable {
                        while it.peek().is_some_and(|(g, _)| g < k) {
                            it.next();
                        }
                        assert_eq!(
                            it.peek().map(|(g, _)| g),
                            Some(k),
                            "a stable key is missing"
                        );
                    }
                    if finished {
                        break;
                    }
                }
            });
        }
    });
}
