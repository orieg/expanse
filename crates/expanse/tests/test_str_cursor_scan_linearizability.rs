//! The scan semantics of the validated string batch cursor (#1143,
//! `docs/benchmarks/concurrency/METHODOLOGY.md` §33.2 and §33.3 G33.3),
//! checked over concurrent histories.
//!
//! A writer inserts and removes "volatile" keys while readers scan; "stable"
//! keys are never written after the start. Each scan is checked against the
//! write history by these predicates, which together are §33.2's contract:
//!
//! - keys come back strictly ascending, so none twice;
//! - every stable key comes back, with its value;
//! - a volatile key that was present for the whole scan comes back;
//! - a volatile key that comes back was present at some instant of the
//!   scan, with the value its writer wrote.
//!
//! Volatile keys straddle the 8-byte chunk edge and share the stable keys'
//! prefixes, so the writes split suffix leaves, create and empty child
//! nodes, and the scans restart across levels.
// The nightly Miri lane runs `sync::str_cursor_tests` and the
// `str_batch_cursor_reader_writer` UB-site workload instead.
#![cfg(not(miri))]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Instant;

use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::SyncExpanseStrMap;

type Key = Vec<u8>;

#[derive(Clone)]
struct Write {
    key: Key,
    insert: bool,
    start: Instant,
    end: Instant,
}

struct Scan {
    /// Index into `FORMS`.
    form: usize,
    start: Instant,
    end: Instant,
    got: Vec<(Key, u64)>,
}

fn nf(b: &[u8]) -> &NulFreeStr {
    NulFreeStr::new(b).expect("NUL-free")
}

/// The value every write of `k` stores.
fn value_of(k: &[u8]) -> u64 {
    k.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

fn stable_keys() -> Vec<Key> {
    let mut out = Vec::new();
    for p in [
        &b"aaaaaaa"[..],
        b"aaaaaaaa",
        b"abcdefghij",
        b"m",
        b"zzzzzzzzzz",
    ] {
        for i in 0..24u32 {
            let mut k = p.to_vec();
            k.extend_from_slice(format!("{i:02}").as_bytes());
            out.push(k);
        }
    }
    out
}

fn volatile_keys() -> Vec<Key> {
    let mut out = Vec::new();
    for p in [&b"aaaaaaa"[..], b"aaaaaaaa", b"abcdefghij", b"m", b"q"] {
        for i in 0..12u32 {
            let mut k = p.to_vec();
            k.extend_from_slice(format!("{i:02}x").as_bytes());
            out.push(k);
        }
    }
    // Whole child nodes appear and disappear under these.
    for i in 0..8u32 {
        out.push(format!("cccccccc{i}").into_bytes());
    }
    out
}

/// Whether `k` was present when the scan started, if the history decides
/// it: `None` when a write of `k` overlaps the start.
fn state_at(writes: &[Write], initial: bool, at: Instant) -> Option<bool> {
    let mut state = initial;
    for w in writes {
        if w.end < at {
            state = w.insert;
        } else if w.start < at {
            return None;
        }
    }
    Some(state)
}

fn check(scan: &Scan, stable: &BTreeMap<Key, u64>, history: &BTreeMap<Key, Vec<Write>>) {
    assert!(
        scan.got.windows(2).all(|w| w[0].0 < w[1].0),
        "keys must be strictly ascending"
    );
    let got: BTreeMap<&Key, u64> = scan.got.iter().map(|(k, v)| (k, *v)).collect();
    for (k, v) in stable {
        assert_eq!(got.get(k), Some(v), "stable key {k:?} must be returned");
    }
    for (k, writes) in history {
        let during = |w: &&Write| w.start < scan.end && w.end > scan.start;
        let at_start = state_at(writes, false, scan.start);
        match got.get(k) {
            Some(&v) => {
                assert_eq!(v, value_of(k), "{k:?} carries its writer's value");
                let inserted = writes.iter().filter(during).any(|w| w.insert);
                assert!(
                    at_start != Some(false) || inserted,
                    "{k:?} was absent for the whole scan and was returned"
                );
            }
            None => {
                let untouched = !writes.iter().any(|w| during(&w));
                assert!(
                    !(at_start == Some(true) && untouched),
                    "{k:?} was present for the whole scan and was not returned"
                );
            }
        }
    }
    for k in got.keys() {
        assert!(
            stable.contains_key(*k) || history.contains_key(*k),
            "{k:?} was never written"
        );
    }
}

/// The scans the readers alternate: `[lo, hi)`, `None` unbounded above.
const FORMS: [(&[u8], Option<&[u8]>); 3] = [
    (b"", None),
    (b"aaaaaaa", Some(b"aaaaaab")),
    (b"abc", Some(b"n")),
];

#[test]
fn str_cursor_scans_satisfy_the_scan_contract() {
    const SCANS: usize = 200;
    const READERS: usize = 2;
    let map = Arc::new(SyncExpanseStrMap::new());
    let mut stable = BTreeMap::new();
    for k in stable_keys() {
        map.insert(nf(&k), value_of(&k));
        stable.insert(k.clone(), value_of(&k));
    }
    let volatile = volatile_keys();
    let done = Arc::new(AtomicBool::new(false));
    let overlapped = Arc::new(AtomicU64::new(0));
    let writes_done = Arc::new(AtomicU64::new(0));
    let start = Arc::new(Barrier::new(READERS + 1));
    let writes = Arc::new(Mutex::new(Vec::<Write>::new()));
    let scans = Arc::new(Mutex::new(Vec::<Scan>::new()));

    let writer = {
        let (map, done, start, writes, writes_done) = (
            Arc::clone(&map),
            Arc::clone(&done),
            Arc::clone(&start),
            Arc::clone(&writes),
            Arc::clone(&writes_done),
        );
        thread::spawn(move || {
            let mut local = Vec::new();
            let mut present = vec![false; volatile.len()];
            let mut rng = 0x1234_5678_9ABC_DEF1u64;
            start.wait();
            // Bounded, so the checker's work is bounded whatever the schedule.
            while !done.load(Ordering::Acquire) && local.len() < 20_000 {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                let i = (rng % volatile.len() as u64) as usize;
                let k = &volatile[i];
                let insert = !present[i];
                let t0 = Instant::now();
                if insert {
                    assert_eq!(map.insert(nf(k), value_of(k)), None);
                } else {
                    assert_eq!(map.remove(nf(k)), Some(value_of(k)));
                }
                let t1 = Instant::now();
                present[i] = insert;
                local.push(Write {
                    key: k.clone(),
                    insert,
                    start: t0,
                    end: t1,
                });
                writes_done.fetch_add(1, Ordering::Release);
            }
            writes.lock().unwrap().extend(local);
        })
    };
    let readers: Vec<_> = (0..READERS)
        .map(|r| {
            let (map, start, scans, overlapped, writes_done) = (
                Arc::clone(&map),
                Arc::clone(&start),
                Arc::clone(&scans),
                Arc::clone(&overlapped),
                Arc::clone(&writes_done),
            );
            thread::spawn(move || {
                let rd = map.reader();
                let mut local = Vec::new();
                start.wait();
                for i in 0..SCANS {
                    let before = writes_done.load(Ordering::Acquire);
                    let t0 = Instant::now();
                    let mut got = Vec::new();
                    // Alternate full, prefix and range scans.
                    let form = (i + r) % FORMS.len();
                    let mut cur = match form {
                        0 => rd.cursor(),
                        1 => rd.prefix_cursor(nf(b"aaaaaaa")),
                        _ => rd.range_cursor(nf(b"abc"), nf(b"n")),
                    };
                    while let Some((k, v)) = cur.next() {
                        got.push((k.to_vec(), v));
                    }
                    let t1 = Instant::now();
                    if writes_done.load(Ordering::Acquire) != before {
                        overlapped.fetch_add(1, Ordering::Relaxed);
                    }
                    local.push(Scan {
                        form,
                        start: t0,
                        end: t1,
                        got,
                    });
                }
                scans.lock().unwrap().extend(local);
            })
        })
        .collect();
    for h in readers {
        h.join().unwrap();
    }
    done.store(true, Ordering::Release);
    writer.join().unwrap();

    let writes = writes.lock().unwrap();
    let mut history: BTreeMap<Key, Vec<Write>> = BTreeMap::new();
    for w in writes.iter() {
        history.entry(w.key.clone()).or_default().push(w.clone());
    }
    let scans = scans.lock().unwrap();
    assert!(
        overlapped.load(Ordering::Relaxed) > 0,
        "no scan overlapped a write ({} writes): the history checks nothing concurrent",
        writes.len()
    );
    for scan in scans.iter() {
        let (lo, hi) = FORMS[scan.form];
        let within = |k: &Key| k.as_slice() >= lo && hi.is_none_or(|h| k.as_slice() < h);
        let stable_in: BTreeMap<Key, u64> = stable
            .iter()
            .filter(|(k, _)| within(k))
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let history_in: BTreeMap<Key, Vec<Write>> = history
            .iter()
            .filter(|(k, _)| within(k))
            .map(|(k, w)| (k.clone(), w.clone()))
            .collect();
        check(scan, &stable_in, &history_in);
    }
}
