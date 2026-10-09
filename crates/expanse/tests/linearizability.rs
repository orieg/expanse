//! Concurrent OCC linearizability verification test harness.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};
use expanse_trie::types::{BITMAP_TO_UNCOMPRESSED_THRESHOLD, BRANCHU_TO_B_DOWN};

#[path = "../benches/ycsb_common/mod.rs"]
mod ycsb_common;

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Insert(u64, u64),
    Remove(u64),
    Get(u64),
    CompareExchange(u64, Option<u64>, Option<u64>),
    Update(u64, i64),
    Contains(u64),
}

#[derive(Clone, Debug, PartialEq)]
enum Ret {
    Insert(Option<u64>),
    Remove(Option<u64>),
    Get(Option<u64>),
    CompareExchange(Result<Option<u64>, Option<u64>>),
    Update(Option<u64>),
    Contains(bool),
}

impl Op {
    fn key(&self) -> u64 {
        match self {
            Op::Insert(k, _) => *k,
            Op::Remove(k) => *k,
            Op::Get(k) => *k,
            Op::CompareExchange(k, _, _) => *k,
            Op::Update(k, _) => *k,
            Op::Contains(k) => *k,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Event {
    op: Op,
    ret: Ret,
    start: Instant,
    end: Instant,
}

fn update_state(cur: Option<u64>, delta: i64) -> Option<u64> {
    if delta == 0 {
        None
    } else if delta > 0 {
        Some(cur.unwrap_or(0).wrapping_add(delta as u64))
    } else {
        match cur {
            Some(v) if (v as i64) + delta > 0 => Some(((v as i64) + delta) as u64),
            _ => None,
        }
    }
}

fn is_valid_transition(state: &Option<u64>, op: &Op, ret: &Ret) -> (bool, Option<u64>) {
    match (op, ret) {
        (Op::Insert(_, v), Ret::Insert(old)) => (old == state, Some(*v)),
        (Op::Remove(_), Ret::Remove(old)) => (old == state, None),
        (Op::Get(_), Ret::Get(val)) => (val == state, *state),
        (Op::Contains(_), Ret::Contains(present)) => (*present == state.is_some(), *state),
        (Op::CompareExchange(_, expected, new), Ret::CompareExchange(res)) => match res {
            Ok(prev) => {
                if state == expected && prev == expected {
                    (true, *new)
                } else {
                    (false, None)
                }
            }
            Err(seen) => {
                if state != expected && state == seen {
                    (true, *state)
                } else {
                    (false, None)
                }
            }
        },
        (Op::Update(_, delta), Ret::Update(old)) => {
            if old == state {
                (true, update_state(*state, *delta))
            } else {
                (false, None)
            }
        }
        _ => (false, None),
    }
}

fn check_linearizability_for_key(events: &[Event]) -> bool {
    // A backtracking search over the orders real time allows, remembering
    // every configuration it has already failed from.
    //
    // A configuration is the set of events placed so far and the key's state
    // after them; whether the rest can be placed depends on nothing else, so
    // a configuration that failed once fails again. Without that memory the
    // search revisits the same configurations through every interleaving that
    // reaches them: with 8 threads on one key it ran past a CI job's limit on
    // a history that is linearizable.
    fn search(
        events: &[Event],
        used: &mut Vec<bool>,
        state: Option<u64>,
        completed: usize,
        failed: &mut HashSet<(Vec<u64>, Option<u64>)>,
    ) -> bool {
        if completed == events.len() {
            return true;
        }
        let mut placed = vec![0u64; events.len().div_ceil(64)];
        for (i, &u) in used.iter().enumerate() {
            if u {
                placed[i / 64] |= 1 << (i % 64);
            }
        }
        let config = (placed, state);
        if failed.contains(&config) {
            return false;
        }

        // The earliest end among the events not yet placed: an event that
        // started after it cannot be placed before the one that ended.
        let mut min_end = None;
        for (i, e) in events.iter().enumerate() {
            if !used[i] && min_end.is_none_or(|me| e.end < me) {
                min_end = Some(e.end);
            }
        }

        for i in 0..events.len() {
            if !used[i] {
                let e = &events[i];
                if let Some(me) = min_end
                    && me < e.start
                {
                    continue;
                }

                let (valid, next_state) = is_valid_transition(&state, &e.op, &e.ret);
                if valid {
                    used[i] = true;
                    if search(events, used, next_state, completed + 1, failed) {
                        return true;
                    }
                    used[i] = false;
                }
            }
        }
        failed.insert(config);
        false
    }

    let mut used = vec![false; events.len()];
    search(events, &mut used, None, 0, &mut HashSet::new())
}

/// The checker terminates on a history built to make an unmemoised search
/// explode: many concurrent operations that leave the state unchanged, so
/// every order of them reaches the same configuration, followed by one
/// operation no order can satisfy.
#[test]
fn test_linearizability_checker_rejects_without_exhausting_orders() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + std::time::Duration::from_millis(ms);
    // 12 overlapping reads of an absent key: 12! (479,001,600) orders, and
    // 4,096 configurations once failed ones are remembered.
    let mut events: Vec<Event> = (0..12)
        .map(|_| Event {
            op: Op::Get(1),
            ret: Ret::Get(None),
            start: at(0),
            end: at(10),
        })
        .collect();
    // Then a read that saw a value nobody wrote.
    events.push(Event {
        op: Op::Get(1),
        ret: Ret::Get(Some(7)),
        start: at(20),
        end: at(30),
    });
    assert!(!check_linearizability_for_key(&events));

    // The same reads with nothing impossible after them are accepted.
    events.pop();
    assert!(check_linearizability_for_key(&events));
}

#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_linearizability() {
    let map = Arc::new(SyncExpanseMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 4;
    let ops_per_thread = 50;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let keys = [1, 2, 3]; // small key space to encourage contention

            for i in 0..ops_per_thread {
                let key = keys[(t_id + i) % keys.len()];

                let op = match (t_id + i) % 3 {
                    0 => Op::Insert(key, (t_id * 100 + i) as u64),
                    1 => Op::Remove(key),
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(*k, *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(*k)),
                    Op::Get(k) => Ret::Get(map_clone.get(*k)),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(*k, *exp, *new))
                    }
                    Op::Update(..) | Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    // Group by key
    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        println!("Verifying key {} with {} events", key, events.len());
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for key {}",
            key
        );
    }
}

/// Concurrent OCC linearizability verification on a Zipfian key sample (METHODOLOGY §20.12 item 8, §20.15).
///
/// Under Zipfian key choice (θ = 0.99 over 256 keys), operations collide on the
/// lowest ranks with high probability, exercising real-time concurrency boundaries
/// on hot keys while verifying that every key's history admits a valid sequential
/// linearization via `check_linearizability_for_key`.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_linearizability_zipfian() {
    let map = Arc::new(SyncExpanseMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    const NUM_KEYS: u64 = 256;
    let zipf = ycsb_common::ZipfianGenerator::new(NUM_KEYS, ycsb_common::ZIPFIAN_THETA);

    let num_threads = 4;
    let ops_per_thread = 50;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);
        let zipf_clone = zipf.clone();

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let mut rng = ycsb_common::XorShift64::new(
                0x717F_0000 ^ (t_id as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15),
            );

            for i in 0..ops_per_thread {
                let rank = zipf_clone.next(rng.next_f64());
                let key = rank;

                let op = match (t_id + i) % 3 {
                    0 => Op::Insert(key, (t_id * 1000 + i + 1) as u64),
                    1 => Op::Remove(key),
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(*k, *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(*k)),
                    Op::Get(k) => Ret::Get(map_clone.get(*k)),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(*k, *exp, *new))
                    }
                    Op::Update(..) | Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();
    assert_eq!(history.len(), num_threads * ops_per_thread);

    // Group by key
    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    let mut total_verified = 0;
    for (key, events) in by_key {
        total_verified += events.len();
        assert!(
            check_linearizability_for_key(&events),
            "Zipfian linearizability violation for key {key} with {} events",
            events.len()
        );
    }
    assert_eq!(total_verified, num_threads * ops_per_thread);
}

/// Single-threaded, Miri-safe companion to [`test_sync_map_linearizability`].
///
/// The multi-threaded version above is `#[ignore]`d under Miri because the
/// seqlock reader path performs deliberate racy plain loads that Miri's
/// data-race detector (correctly) flags — see the `sync.rs` module docs.
/// This variant drives a sequential interleaving of insert/remove/get
/// through BOTH the writer API and a registered `reader()` handle, feeding
/// the resulting history through the same Wing-Gong linearizability checker.
/// No two operations overlap in real time, so the reader path, the reader
/// handle, and the checker wiring are all exercised without any cross-thread
/// race for Miri to reject. Every sequential history is trivially
/// linearizable, so a failure here means a genuine reader/writer disagreement
/// with the sequential specification, not a scheduling artifact.
#[test]
fn test_sync_map_linearizability_single_threaded() {
    let map = SyncExpanseMap::new();
    let reader = map.reader();

    // Deterministic op stream over a tiny key space, so every key sees a
    // mix of inserts, removes, and gets through both read paths.
    let keys = [1u64, 2, 3];
    let mut events: Vec<Event> = Vec::new();

    for i in 0..90usize {
        let key = keys[i % keys.len()];
        let op = match i % 3 {
            0 => Op::Insert(key, (i as u64) + 1),
            1 => Op::Remove(key),
            // Alternate the get between the one-shot API and the reader
            // handle so both validated-read entry points are covered.
            _ => Op::Get(key),
        };

        let start = Instant::now();
        let ret = match &op {
            Op::Insert(k, v) => Ret::Insert(map.insert(*k, *v)),
            Op::Remove(k) => Ret::Remove(map.remove(*k)),
            Op::Get(k) => {
                // Route half the gets through the persistent reader handle
                // and half through the one-shot API.
                let v = if i % 2 == 0 {
                    reader.get(*k)
                } else {
                    map.get(*k)
                };
                Ret::Get(v)
            }
            Op::CompareExchange(..) => unreachable!(),
            Op::Update(..) | Op::Contains(_) => unreachable!(),
        };
        let end = Instant::now();

        events.push(Event {
            op,
            ret,
            start,
            end,
        });
    }

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in events {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Sequential linearizability violation for key {}",
            key
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SetOp {
    Insert(u64),
    Remove(u64),
    Contains(u64),
}

#[derive(Clone, Debug, PartialEq)]
enum SetRet {
    Insert(bool),
    Remove(bool),
    Contains(bool),
}

impl SetOp {
    fn key(&self) -> u64 {
        match self {
            SetOp::Insert(k) => *k,
            SetOp::Remove(k) => *k,
            SetOp::Contains(k) => *k,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct SetEvent {
    op: SetOp,
    ret: SetRet,
    start: Instant,
    end: Instant,
}

fn is_valid_set_transition(state: bool, op: &SetOp, ret: &SetRet) -> (bool, bool) {
    match (op, ret) {
        (SetOp::Insert(_), SetRet::Insert(was_absent)) => (*was_absent == !state, true),
        (SetOp::Remove(_), SetRet::Remove(was_present)) => (*was_present == state, false),
        (SetOp::Contains(_), SetRet::Contains(present)) => (*present == state, state),
        _ => (false, state),
    }
}

fn check_set_linearizability_for_key(events: &[SetEvent]) -> bool {
    fn search(events: &[SetEvent], used: &mut Vec<bool>, state: bool, completed: usize) -> bool {
        if completed == events.len() {
            return true;
        }

        let mut min_end = None;
        for (i, e) in events.iter().enumerate() {
            if !used[i] && min_end.is_none_or(|me| e.end < me) {
                min_end = Some(e.end);
            }
        }

        for i in 0..events.len() {
            if !used[i] {
                let e = &events[i];

                if let Some(me) = min_end
                    && me < e.start
                {
                    continue;
                }

                let (valid, next_state) = is_valid_set_transition(state, &e.op, &e.ret);
                if valid {
                    used[i] = true;
                    if search(events, used, next_state, completed + 1) {
                        return true;
                    }
                    used[i] = false;
                }
            }
        }
        false
    }

    let mut used = vec![false; events.len()];
    search(events, &mut used, false, 0)
}

#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_set_linearizability() {
    let set = Arc::new(SyncExpanseSet::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 4;
    let ops_per_thread = 50;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let set_clone = Arc::clone(&set);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let keys = [1, 2, 3];

            for i in 0..ops_per_thread {
                let key = keys[(t_id + i) % keys.len()];

                let op = match (t_id + i) % 3 {
                    0 => SetOp::Insert(key),
                    1 => SetOp::Remove(key),
                    _ => SetOp::Contains(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    SetOp::Insert(k) => SetRet::Insert(set_clone.insert(*k)),
                    SetOp::Remove(k) => SetRet::Remove(set_clone.remove(*k)),
                    SetOp::Contains(k) => SetRet::Contains(set_clone.contains(*k)),
                };
                let end = Instant::now();

                local_events.push(SetEvent {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    let mut by_key: HashMap<u64, Vec<SetEvent>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        println!("Verifying set key {} with {} events", key, events.len());
        assert!(
            check_set_linearizability_for_key(&events),
            "Linearizability violation for set key {}",
            key
        );
    }
}

/// Single-threaded, Miri-safe companion to [`test_sync_set_linearizability`].
#[test]
fn test_sync_set_linearizability_single_threaded() {
    let set = SyncExpanseSet::new();
    let reader = set.reader();

    let keys = [1u64, 2, 3];
    let mut events: Vec<SetEvent> = Vec::new();

    for i in 0..90usize {
        let key = keys[i % keys.len()];
        let op = match i % 3 {
            0 => SetOp::Insert(key),
            1 => SetOp::Remove(key),
            _ => SetOp::Contains(key),
        };

        let start = Instant::now();
        let ret = match &op {
            SetOp::Insert(k) => SetRet::Insert(set.insert(*k)),
            SetOp::Remove(k) => SetRet::Remove(set.remove(*k)),
            SetOp::Contains(k) => {
                let v = if i % 2 == 0 {
                    reader.contains(*k)
                } else {
                    set.contains(*k)
                };
                SetRet::Contains(v)
            }
        };
        let end = Instant::now();

        events.push(SetEvent {
            op,
            ret,
            start,
            end,
        });
    }

    let mut by_key: HashMap<u64, Vec<SetEvent>> = HashMap::new();
    for e in events {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_set_linearizability_for_key(&events),
            "Sequential linearizability violation for set key {}",
            key
        );
    }
}

/// Verifies concurrent multi-writer parallel progress on disjoint expanses and
/// census convergence (S7): ancestor `pop0(e) + 1 == |keys under e|` across the
/// entire trie once writers drain.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_multi_writer_parallel_disjoint_and_census() {
    let map = Arc::new(SyncExpanseMap::new());
    let set = Arc::new(SyncExpanseSet::new());

    // Pre-populate keys so root transitions to Root::Tree.
    const PREPOP: u64 = 64;
    for b in 1..=PREPOP {
        map.insert(b << 56, b);
        set.insert(b << 56);
    }

    let num_threads = 4;
    let keys_per_thread = 200;

    let mut map_handles: Vec<thread::JoinHandle<()>> = vec![];
    let mut set_handles = vec![];

    for t_id in 0..num_threads {
        let m = Arc::clone(&map);
        map_handles.push(thread::spawn(move || {
            let base = ((t_id as u64) + 1) << 48;
            for i in 0..keys_per_thread {
                let k = base | (i as u64);
                assert_eq!(m.insert(k, k ^ 0xDEAD_BEEF), None);
            }
            for i in 0..keys_per_thread {
                let k = base | (i as u64);
                assert_eq!(m.insert(k, k ^ 0xCAFE_BABE), Some(k ^ 0xDEAD_BEEF));
            }
        }));

        let s = Arc::clone(&set);
        set_handles.push(thread::spawn(move || {
            let base = ((t_id as u64) + 1) << 48;
            for i in 0..keys_per_thread {
                let k = base | (i as u64);
                assert!(s.insert(k));
            }
        }));
    }

    for h in map_handles {
        h.join().unwrap();
    }
    for h in set_handles {
        h.join().unwrap();
    }

    map.with_locked(|inner| {
        inner.validate();
        assert_eq!(
            inner.len(),
            PREPOP + (num_threads as u64) * (keys_per_thread as u64)
        );
        for t_id in 0..num_threads {
            let base = ((t_id as u64) + 1) << 48;
            for i in 0..keys_per_thread {
                let k = base | (i as u64);
                assert_eq!(inner.get(k), Some(k ^ 0xCAFE_BABE));
            }
        }
    });

    set.with_locked(|inner| {
        inner.validate();
        assert_eq!(
            inner.len(),
            PREPOP + (num_threads as u64) * (keys_per_thread as u64)
        );
        for t_id in 0..num_threads {
            let base = ((t_id as u64) + 1) << 48;
            for i in 0..keys_per_thread {
                let k = base | (i as u64);
                assert!(inner.contains(k));
            }
        }
    });
}

/// Exercises the Stage B multi-writer OLC execution path (`olc_insert_map`, `olc_remove_map`)
/// under concurrent W = 4 writers and readers, verifying Wing-Gong linearizability
/// on a tree-rooted trie (`Root::Tree`) across a widened key set spanning disjoint
/// and contended expanses (Docs/ARCHITECTURE.md §4.2 Table row S6).
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_linearizability_tree_rooted() {
    let map = Arc::new(SyncExpanseMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    // Pre-populate keys so root transitions permanently to Root::Tree.
    const PREPOP: u64 = 64;
    for b in 1..=PREPOP {
        map.insert(b << 56, b);
    }

    let num_threads = 4;
    let ops_per_thread = 40;

    // Widened key set spanning multiple branches and leaves:
    let keys = [
        (1u64 << 48) | 1,
        (1u64 << 48) | 2,
        (1u64 << 48) | 3,
        (1u64 << 48) | 4,
        (2u64 << 48) | 1,
        (2u64 << 48) | 2,
        (2u64 << 48) | 3,
        (2u64 << 48) | 4,
        (3u64 << 48) | 1,
        (3u64 << 48) | 2,
        (3u64 << 48) | 3,
        (3u64 << 48) | 4,
        (4u64 << 48) | 1,
        (4u64 << 48) | 2,
        (4u64 << 48) | 3,
        (4u64 << 48) | 4,
    ];

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);

            for i in 0..ops_per_thread {
                // Each thread accesses its dedicated branch keys and shares contention on adjacent keys:
                let key = keys[(t_id * 4 + i) % keys.len()];

                let op = match (t_id + i) % 3 {
                    0 => Op::Insert(key, (t_id * 1000 + i) as u64),
                    1 => Op::Remove(key),
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(*k, *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(*k)),
                    Op::Get(k) => Ret::Get(map_clone.get(*k)),
                    Op::CompareExchange(..) => unreachable!(),
                    Op::Update(..) | Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    // Group by key
    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation on tree-rooted OLC path for key {}",
            key
        );
    }

    // Census and structural validation
    map.with_locked(|inner| {
        inner.validate();
    });
}

/// Exercises the Stage B multi-writer OLC execution path (`olc_insert_set`, `olc_remove_set`)
/// under concurrent W = 4 writers and readers, verifying Wing-Gong linearizability
/// on a tree-rooted trie (`Root::Tree`) across a widened key set spanning disjoint
/// and contended expanses (Docs/ARCHITECTURE.md §4.2 Table row S6).
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_set_linearizability_tree_rooted() {
    let set = Arc::new(SyncExpanseSet::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    const PREPOP: u64 = 64;
    for b in 1..=PREPOP {
        set.insert(b << 56);
    }

    let num_threads = 4;
    let ops_per_thread = 40;

    let keys = [
        (1u64 << 48) | 1,
        (1u64 << 48) | 2,
        (1u64 << 48) | 3,
        (1u64 << 48) | 4,
        (2u64 << 48) | 1,
        (2u64 << 48) | 2,
        (2u64 << 48) | 3,
        (2u64 << 48) | 4,
        (3u64 << 48) | 1,
        (3u64 << 48) | 2,
        (3u64 << 48) | 3,
        (3u64 << 48) | 4,
        (4u64 << 48) | 1,
        (4u64 << 48) | 2,
        (4u64 << 48) | 3,
        (4u64 << 48) | 4,
    ];

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let set_clone = Arc::clone(&set);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);

            for i in 0..ops_per_thread {
                let key = keys[(t_id * 4 + i) % keys.len()];

                let op = match (t_id + i) % 3 {
                    0 => SetOp::Insert(key),
                    1 => SetOp::Remove(key),
                    _ => SetOp::Contains(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    SetOp::Insert(k) => SetRet::Insert(set_clone.insert(*k)),
                    SetOp::Remove(k) => SetRet::Remove(set_clone.remove(*k)),
                    SetOp::Contains(k) => SetRet::Contains(set_clone.contains(*k)),
                };
                let end = Instant::now();

                local_events.push(SetEvent {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    let mut by_key: HashMap<u64, Vec<SetEvent>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_set_linearizability_for_key(&events),
            "Linearizability violation on tree-rooted OLC set path for key {}",
            key
        );
    }

    set.with_locked(|inner| {
        inner.validate();
    });
}

// ---- Ordered queries: a whole-map model (#900) ----------------------------
//
// The checkers above partition a history by key, which is sound only for
// point operations: a predecessor's answer depends on keys other than its
// argument, so an ordered history is checked against one sequential map. The
// search is Wing & Gong's, memoised on (operations applied, map state); it is
// exponential in the worst case, so a history holds at most 64 events.

#[derive(Clone, Debug, PartialEq)]
enum OrdOp {
    Insert(u64, u64),
    Remove(u64),
    PrevAtOrBefore(u64),
    NextAtOrAfter(u64),
}

#[derive(Clone, Debug, PartialEq)]
enum OrdRet {
    Insert(Option<u64>),
    Remove(Option<u64>),
    Found(Option<(u64, u64)>),
}

#[derive(Clone, Debug)]
struct OrdEvent {
    op: OrdOp,
    ret: OrdRet,
    start: Instant,
    end: Instant,
}

/// The map after `op`, if the sequential map in `state` returns `ret` for it.
fn ord_apply(state: &BTreeMap<u64, u64>, op: &OrdOp, ret: &OrdRet) -> Option<BTreeMap<u64, u64>> {
    match (op, ret) {
        (OrdOp::Insert(k, v), OrdRet::Insert(old)) => (state.get(k).copied() == *old).then(|| {
            let mut s = state.clone();
            s.insert(*k, *v);
            s
        }),
        (OrdOp::Remove(k), OrdRet::Remove(old)) => (state.get(k).copied() == *old).then(|| {
            let mut s = state.clone();
            s.remove(k);
            s
        }),
        (OrdOp::PrevAtOrBefore(k), OrdRet::Found(got)) => {
            (state.range(..=*k).next_back().map(|(a, b)| (*a, *b)) == *got).then(|| state.clone())
        }
        (OrdOp::NextAtOrAfter(k), OrdRet::Found(got)) => {
            (state.range(*k..).next().map(|(a, b)| (*a, *b)) == *got).then(|| state.clone())
        }
        _ => None,
    }
}

fn check_ordered_linearizability(events: &[OrdEvent], initial: &BTreeMap<u64, u64>) -> bool {
    assert!(
        events.len() <= 64,
        "the whole-map search memoises on a u64 mask: at most 64 events, got {}",
        events.len()
    );
    fn search(
        events: &[OrdEvent],
        used: u64,
        state: &BTreeMap<u64, u64>,
        seen: &mut HashSet<(u64, Vec<(u64, u64)>)>,
    ) -> bool {
        if used.count_ones() as usize == events.len() {
            return true;
        }
        if !seen.insert((used, state.iter().map(|(a, b)| (*a, *b)).collect())) {
            return false;
        }
        // An operation may go next only if no unapplied operation ended before it started.
        let min_end = events
            .iter()
            .enumerate()
            .filter(|(i, _)| used & (1u64 << i) == 0)
            .map(|(_, e)| e.end)
            .min();
        for (i, e) in events.iter().enumerate() {
            if used & (1u64 << i) != 0 {
                continue;
            }
            if let Some(me) = min_end
                && me < e.start
            {
                continue;
            }
            if let Some(next) = ord_apply(state, &e.op, &e.ret)
                && search(events, used | (1u64 << i), &next, seen)
            {
                return true;
            }
        }
        false
    }
    search(events, 0, initial, &mut HashSet::new())
}

/// `n` strictly increasing instants, for a hand-built history.
fn increasing_instants(n: usize) -> Vec<Instant> {
    let mut out = vec![Instant::now()];
    while out.len() < n {
        let t = Instant::now();
        if t > *out.last().unwrap() {
            out.push(t);
        }
    }
    out
}

/// The whole-map checker rejects the answer an optimistic predecessor search
/// can return if it drops a subtree's snapshot before backtracking (#900): a
/// read overlapping two inserts returns `m`, which is below `k_prime` in every
/// state that contains it. Each answer that was the predecessor at some instant
/// is accepted, and a read that starts after both inserts cannot see the state
/// before them.
#[test]
fn ordered_checker_rejects_a_key_that_was_never_the_predecessor() {
    const K: u64 = 0x0200;
    let (m0, m, k_prime) = (0x0010u64, 0x0020u64, 0x01F0u64);
    let initial: BTreeMap<u64, u64> = [(m0, 1), (K + 5, 2)].into_iter().collect();
    let t = increasing_instants(8);
    let writes = [
        OrdEvent {
            op: OrdOp::Insert(k_prime, 3),
            ret: OrdRet::Insert(None),
            start: t[1],
            end: t[2],
        },
        OrdEvent {
            op: OrdOp::Insert(m, 4),
            ret: OrdRet::Insert(None),
            start: t[3],
            end: t[4],
        },
    ];
    let read = |got, start, end| OrdEvent {
        op: OrdOp::PrevAtOrBefore(K),
        ret: OrdRet::Found(got),
        start,
        end,
    };

    let bad = [
        writes[0].clone(),
        writes[1].clone(),
        read(Some((m, 4)), t[0], t[5]),
    ];
    assert!(
        !check_ordered_linearizability(&bad, &initial),
        "a key that was never the predecessor must be rejected"
    );
    for got in [Some((m0, 1)), Some((k_prime, 3))] {
        let good = [writes[0].clone(), writes[1].clone(), read(got, t[0], t[5])];
        assert!(
            check_ordered_linearizability(&good, &initial),
            "{got:?} was the predecessor at some instant of the read"
        );
    }
    let stale = [
        writes[0].clone(),
        writes[1].clone(),
        read(Some((m0, 1)), t[6], t[7]),
    ];
    assert!(
        !check_ordered_linearizability(&stale, &initial),
        "a read that starts after both inserts must not see the state before them"
    );
}

/// A live ordered history on a tree-rooted map, with predecessor and successor
/// reads taken through `with_locked` -- the only ordered route before #900, and
/// linearizable by construction. It exercises the whole-map checker on real
/// interleavings; the optimistic ordered reads replace `with_locked` here once
/// they exist. The keys straddle byte boundaries at two levels, so a search
/// from one of them leaves its terminal for a neighbour.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_ordered_linearizability_through_with_locked() {
    let map = Arc::new(SyncExpanseMap::new());
    const PREPOP: u64 = 64;
    let mut initial = BTreeMap::new();
    for b in 1..=PREPOP {
        map.insert(b << 56, b);
        initial.insert(b << 56, b);
    }
    let keys: [u64; 8] = [
        0x00FF, 0x0100, 0x01FF, 0x0200, 0xFFFF, 0x1_0000, 0x1_00FF, 0x1_0100,
    ];
    let history = Arc::new(Mutex::new(Vec::new()));
    let (threads, per_thread) = (3usize, 16usize);
    let mut handles = vec![];
    for t_id in 0..threads {
        let (map, history) = (Arc::clone(&map), Arc::clone(&history));
        handles.push(thread::spawn(move || {
            let mut local = Vec::with_capacity(per_thread);
            for i in 0..per_thread {
                let key = keys[(t_id * 3 + i) % keys.len()];
                let op = match (t_id + i) % 4 {
                    0 => OrdOp::Insert(key, (t_id * 1000 + i) as u64),
                    1 => OrdOp::Remove(key),
                    2 => OrdOp::PrevAtOrBefore(key),
                    _ => OrdOp::NextAtOrAfter(key),
                };
                let start = Instant::now();
                let ret = match &op {
                    OrdOp::Insert(k, v) => OrdRet::Insert(map.insert(*k, *v)),
                    OrdOp::Remove(k) => OrdRet::Remove(map.remove(*k)),
                    OrdOp::PrevAtOrBefore(k) => {
                        OrdRet::Found(map.with_locked(|m| m.prev_at_or_before(*k)))
                    }
                    OrdOp::NextAtOrAfter(k) => {
                        OrdRet::Found(map.with_locked(|m| m.next_at_or_after(*k)))
                    }
                };
                let end = Instant::now();
                local.push(OrdEvent {
                    op,
                    ret,
                    start,
                    end,
                });
            }
            history.lock().unwrap().extend(local);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let history = history.lock().unwrap().clone();
    assert_eq!(history.len(), threads * per_thread);
    assert!(
        check_ordered_linearizability(&history, &initial),
        "an ordered history taken through with_locked is not linearizable"
    );
}

/// One live ordered history on a fresh map: `threads` threads each run
/// `per_thread` operations over `keys`, taking ordered reads optimistically
/// through their own reader handle (#900).
fn optimistic_ordered_history(
    prepop: &[u64],
    keys: &[u64],
    threads: usize,
    per_thread: usize,
) -> (Vec<OrdEvent>, BTreeMap<u64, u64>) {
    let map = Arc::new(SyncExpanseMap::new());
    let mut initial = BTreeMap::new();
    for &k in prepop {
        map.insert(k, !k);
        initial.insert(k, !k);
    }
    let keys = Arc::new(keys.to_vec());
    let history = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];
    for t_id in 0..threads {
        let (map, keys, history) = (Arc::clone(&map), Arc::clone(&keys), Arc::clone(&history));
        handles.push(thread::spawn(move || {
            let rd = map.reader();
            let mut local = Vec::with_capacity(per_thread);
            for i in 0..per_thread {
                let key = keys[(t_id * 3 + i) % keys.len()];
                let op = match (t_id + i) % 4 {
                    0 => OrdOp::Insert(key, (t_id * 1000 + i) as u64),
                    1 => OrdOp::Remove(key),
                    2 => OrdOp::PrevAtOrBefore(key),
                    _ => OrdOp::NextAtOrAfter(key),
                };
                let start = Instant::now();
                let ret = match &op {
                    OrdOp::Insert(k, v) => OrdRet::Insert(map.insert(*k, *v)),
                    OrdOp::Remove(k) => OrdRet::Remove(map.remove(*k)),
                    OrdOp::PrevAtOrBefore(k) => OrdRet::Found(rd.prev_at_or_before(*k)),
                    OrdOp::NextAtOrAfter(k) => OrdRet::Found(rd.next_at_or_after(*k)),
                };
                let end = Instant::now();
                local.push(OrdEvent {
                    op,
                    ret,
                    start,
                    end,
                });
            }
            drop(rd);
            history.lock().unwrap().extend(local);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let history = history.lock().unwrap().clone();
    assert_eq!(history.len(), threads * per_thread);
    (history, initial)
}

/// G12.2 (`docs/benchmarks/concurrency/METHODOLOGY.md` §12.2): the tree-rooted
/// history above, with the ordered reads taken optimistically instead of
/// through `with_locked`, over repeated rounds.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_ordered_linearizability_optimistic_tree_rooted() {
    let prepop: Vec<u64> = (1..=64u64).map(|b| b << 56).collect();
    let keys = [
        0x00FF, 0x0100, 0x01FF, 0x0200, 0xFFFF, 0x1_0000, 0x1_00FF, 0x1_0100,
    ];
    for round in 0..24 {
        let (history, initial) = optimistic_ordered_history(&prepop, &keys, 3, 16);
        assert!(
            check_ordered_linearizability(&history, &initial),
            "round {round}: an optimistic ordered history is not linearizable"
        );
    }
}

/// G12.2, hot-spot: every key sits in one 2^16-wide expanse, so the writes
/// land in the subtrees the searches pass over and backtrack through. The
/// prefilled keys fill the expanse's level-2 digits on both sides of each
/// probe.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_ordered_linearizability_optimistic_hotspot() {
    const BASE: u64 = 0x5_0000;
    let prepop: Vec<u64> = (0..64u64).map(|d| BASE | (d << 10) | 0x80).collect();
    let keys = [
        BASE | 0x00FF,
        BASE | 0x0100,
        BASE | 0x0401,
        BASE | 0x07FF,
        BASE | 0x0800,
        BASE | 0x0C7F,
        BASE | 0x0C81,
        BASE | 0x1000,
    ];
    for round in 0..24 {
        let (history, initial) = optimistic_ordered_history(&prepop, &keys, 4, 12);
        assert!(
            check_ordered_linearizability(&history, &initial),
            "round {round}: an optimistic hot-spot ordered history is not linearizable"
        );
    }
}

// ---------------------------------------------------------------------------
// The string wrapper (Refs #929, METHODOLOGY §17.5)
// ---------------------------------------------------------------------------

/// The string keys of the #929 histories, by index, so the map checker's
/// `Op`/`Ret`/`Event` shapes serve unchanged: the checker needs only the
/// key's identity. Three of them share a first chunk and two of those a
/// second, so the writers contend on continuation entries — T2 against the
/// insert-if-absent race, T3, T4, T8 and T9 — and not only on terminal ones.
fn str_key(idx: u64) -> &'static NulFreeStr {
    const KEYS: [&[u8]; 6] = [
        b"k1",
        b"shared/prefix/aaaa",
        b"shared/prefix/aaab",
        b"shared/prefix/bbbb/deeper",
        b"shared/prefix/bbbb/deeper-still",
        b"zz",
    ];
    NulFreeStr::new(KEYS[idx as usize]).expect("literal keys are NUL-free")
}

/// #929: per-key linearizability of `SyncExpanseStrMap` under four writers
/// mixing inserts, removes and reads on six keys that share continuation
/// entries.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_strmap_linearizability() {
    let map = Arc::new(SyncExpanseStrMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 4;
    let ops_per_thread = 120;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let reader = map_clone.reader();

            for i in 0..ops_per_thread {
                let key = ((t_id * 7 + i * 5) % 6) as u64;

                let op = match (t_id + i) % 3 {
                    0 => Op::Insert(key, (t_id * 1000 + i) as u64),
                    1 => Op::Remove(key),
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(str_key(*k), *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(str_key(*k))),
                    Op::Get(k) => Ret::Get(reader.get(str_key(*k))),
                    Op::CompareExchange(..) => unreachable!(),
                    Op::Update(..) | Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        println!("Verifying string key {} with {} events", key, events.len());
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for string key {}",
            key
        );
    }
}

/// One operation of the compare-exchange mixes below.
///
/// Values come from `1..=3`, so an exchange's `expected` is a value some
/// thread does store: with a value unique to each operation no exchange
/// could ever succeed, and the history would hold failures only. Both
/// `Some -> Some` and the remove and insert-if-absent forms appear.
fn cas_mix_op(t_id: usize, i: usize, key: u64) -> Op {
    let v = ((t_id + i) % 3 + 1) as u64;
    match (t_id * 3 + i) % 5 {
        0 => Op::Insert(key, v),
        1 => Op::Remove(key),
        2 => Op::CompareExchange(key, Some(((t_id + 2 * i) % 3 + 1) as u64), Some(v)),
        3 => Op::CompareExchange(
            key,
            (!i.is_multiple_of(3)).then_some(v),
            (!i.is_multiple_of(2)).then_some(((i / 2) % 3 + 1) as u64),
        ),
        _ => Op::Get(key),
    }
}

/// A compare-exchange mix that recorded no success, or no failure, checked
/// one side of the operation only.
fn assert_cas_mix_exercised(history: &[Event]) {
    let (mut ok, mut err) = (0, 0);
    for e in history {
        match e.ret {
            Ret::CompareExchange(Ok(_)) => ok += 1,
            Ret::CompareExchange(Err(_)) => err += 1,
            _ => {}
        }
    }
    assert!(
        ok > 0 && err > 0,
        "compare-exchange outcomes: {ok} ok, {err} err"
    );
}

/// Linearizability verification of `SyncExpanseStrMap::compare_exchange`
/// across concurrent writers and readers mixing inserts, removes, gets, and CAS.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_strmap_compare_exchange_linearizability() {
    let map = Arc::new(SyncExpanseStrMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 4;
    let ops_per_thread = 50;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let reader = map_clone.reader();

            for i in 0..ops_per_thread {
                let key = ((t_id * 7 + i * 5) % 6) as u64;

                let op = cas_mix_op(t_id, i, key);

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(str_key(*k), *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(str_key(*k))),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(str_key(*k), *exp, *new))
                    }
                    Op::Get(k) => Ret::Get(reader.get(str_key(*k))),
                    Op::Update(..) | Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();
    assert_cas_mix_exercised(&history);

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for string key {}",
            key
        );
    }
}

fn bytes_key(idx: u64) -> &'static [u8] {
    const KEYS: [&[u8]; 6] = [
        b"b_short",
        b"shared/prefix/aaaa",
        b"shared/prefix/bbbb/first",
        b"shared/prefix/bbbb/second",
        b"shared/prefix/bbbb/deeper-still",
        b"b_zz",
    ];
    KEYS[idx as usize]
}

/// Linearizability verification of `SyncExpanseBytesMap::compare_exchange`
/// across concurrent writers and readers mixing inserts, removes, gets, and CAS.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_bytesmap_compare_exchange_linearizability() {
    let map = Arc::new(SyncExpanseBytesMap::new());
    // More hashes than a root leaf holds, so the mix runs on the optimistic
    // tree paths and not on the serialised root-leaf one.
    for i in 0..64u64 {
        map.insert(format!("filler/{i}").as_bytes(), i);
    }
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 4;
    let ops_per_thread = 50;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let reader = map_clone.reader();

            for i in 0..ops_per_thread {
                let key = ((t_id * 7 + i * 5) % 6) as u64;

                let op = cas_mix_op(t_id, i, key);

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(bytes_key(*k), *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(bytes_key(*k))),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(bytes_key(*k), *exp, *new))
                    }
                    Op::Get(k) => Ret::Get(reader.get(bytes_key(*k))),
                    Op::Update(..) | Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();
    assert_cas_mix_exercised(&history);

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for bytes key {}",
            key
        );
    }
}

/// Linearizability verification of `SyncExpanseMap::update`
/// across concurrent writers and readers mixing inserts, removes, gets, CAS, and update.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_update_linearizability() {
    let map = Arc::new(SyncExpanseMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 8;
    let ops_per_thread = 200;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let keys = [1u64, 2];

            for i in 0..ops_per_thread {
                let key = keys[(t_id + i) % keys.len()];

                let op = match (t_id + i) % 5 {
                    0 => Op::Insert(key, (t_id * 1000 + i) as u64),
                    1 => Op::Remove(key),
                    2 => {
                        let expected = if (t_id + i) % 2 == 0 {
                            Some((t_id * 1000 + i.saturating_sub(1)) as u64)
                        } else {
                            None
                        };
                        let new = Some((t_id * 1000 + i) as u64);
                        Op::CompareExchange(key, expected, new)
                    }
                    3 => {
                        let delta = match (t_id + i) % 3 {
                            0 => 10,
                            1 => -5,
                            _ => 0,
                        };
                        Op::Update(key, delta)
                    }
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(*k, *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(*k)),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(*k, *exp, *new))
                    }
                    Op::Update(k, delta) => {
                        let d = *delta;
                        Ret::Update(map_clone.update(*k, |cur| update_state(cur, d)))
                    }
                    Op::Get(k) => Ret::Get(map_clone.get(*k)),
                    Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for map key {}",
            key
        );
    }
}

/// Linearizability verification of `SyncExpanseStrMap::update`
/// across concurrent writers and readers mixing inserts, removes, gets, CAS, and update.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_strmap_update_linearizability() {
    let map = Arc::new(SyncExpanseStrMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 8;
    let ops_per_thread = 200;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let reader = map_clone.reader();
            let keys = [1u64, 2];

            for i in 0..ops_per_thread {
                let key = keys[(t_id + i) % keys.len()];

                let op = match (t_id + i) % 5 {
                    0 => Op::Insert(key, (t_id * 1000 + i) as u64),
                    1 => Op::Remove(key),
                    2 => {
                        let expected = if (t_id + i) % 2 == 0 {
                            Some((t_id * 1000 + i.saturating_sub(1)) as u64)
                        } else {
                            None
                        };
                        let new = Some((t_id * 1000 + i) as u64);
                        Op::CompareExchange(key, expected, new)
                    }
                    3 => {
                        let delta = match (t_id + i) % 3 {
                            0 => 10,
                            1 => -5,
                            _ => 0,
                        };
                        Op::Update(key, delta)
                    }
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(str_key(*k), *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(str_key(*k))),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(str_key(*k), *exp, *new))
                    }
                    Op::Update(k, delta) => {
                        let d = *delta;
                        Ret::Update(map_clone.update(str_key(*k), |cur| update_state(cur, d)))
                    }
                    Op::Get(k) => Ret::Get(reader.get(str_key(*k))),
                    Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for string key {}",
            key
        );
    }
}

/// Linearizability verification of `SyncExpanseBytesMap::update`
/// across concurrent writers and readers mixing inserts, removes, gets, CAS, and update.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_bytesmap_update_linearizability() {
    let map = Arc::new(SyncExpanseBytesMap::new());
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 8;
    let ops_per_thread = 200;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let reader = map_clone.reader();
            let keys = [1u64, 2];

            for i in 0..ops_per_thread {
                let key = keys[(t_id + i) % keys.len()];

                let op = match (t_id + i) % 5 {
                    0 => Op::Insert(key, (t_id * 1000 + i) as u64),
                    1 => Op::Remove(key),
                    2 => {
                        let expected = if (t_id + i) % 2 == 0 {
                            Some((t_id * 1000 + i.saturating_sub(1)) as u64)
                        } else {
                            None
                        };
                        let new = Some((t_id * 1000 + i) as u64);
                        Op::CompareExchange(key, expected, new)
                    }
                    3 => {
                        let delta = match (t_id + i) % 3 {
                            0 => 10,
                            1 => -5,
                            _ => 0,
                        };
                        Op::Update(key, delta)
                    }
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(bytes_key(*k), *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(bytes_key(*k))),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(bytes_key(*k), *exp, *new))
                    }
                    Op::Update(k, delta) => {
                        let d = *delta;
                        Ret::Update(map_clone.update(bytes_key(*k), |cur| update_state(cur, d)))
                    }
                    Op::Get(k) => Ret::Get(reader.get(bytes_key(*k))),
                    Op::Contains(_) => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for bytes key {}",
            key
        );
    }
}

/// Linearizability verification of `SyncExpanseBlobMap::compare_exchange`
/// across concurrent writers and readers mixing CAS, remove-if-equal, and gets.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_blobmap_compare_exchange_linearizability() {
    let map = Arc::new(SyncExpanseBlobMap::new());
    // More keys than a root leaf holds, so the mix runs on the optimistic
    // tree paths and not on the serialised root-leaf one.
    for k in 0..64u64 {
        map.insert(1_000 + k, &k.to_le_bytes(), 0).unwrap();
    }
    let history = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 4;
    let ops_per_thread = 50;

    let mut handles = vec![];

    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);

        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let mut reader = map_clone.reader();

            for i in 0..ops_per_thread {
                let key = ((t_id * 7 + i * 5) % 6) as u64;

                // Values from `1..=3`, so an exchange's `expected` is a value
                // some thread stores; see `cas_mix_op`.
                let v = ((t_id + i) % 3 + 1) as u64;
                let op = match (t_id * 3 + i) % 4 {
                    0 => Op::CompareExchange(key, None, Some(v)),
                    1 => Op::CompareExchange(key, Some(((t_id + 2 * i) % 3 + 1) as u64), Some(v)),
                    2 => Op::CompareExchange(key, Some(v), None),
                    _ => Op::Get(key),
                };

                let start = Instant::now();
                let ret = match &op {
                    Op::CompareExchange(k, exp, new) => {
                        let exp_buf = exp.map(|v| (v.to_le_bytes(), 0u32));
                        let new_buf = new.map(|v| (v.to_le_bytes(), 0u32));
                        let res = map_clone.compare_exchange(
                            *k,
                            exp_buf.as_ref().map(|(b, m)| (&b[..], *m)),
                            new_buf.as_ref().map(|(b, m)| (&b[..], *m)),
                        );
                        let conv =
                            match res {
                                Ok(prev) => Ok(prev
                                    .map(|(b, _)| u64::from_le_bytes(b[..8].try_into().unwrap()))),
                                Err(seen) => Err(seen
                                    .map(|(b, _)| u64::from_le_bytes(b[..8].try_into().unwrap()))),
                            };
                        Ret::CompareExchange(conv)
                    }
                    Op::Get(k) => {
                        let got = reader
                            .get(*k)
                            .map(|(b, _)| u64::from_le_bytes(b[..8].try_into().unwrap()));
                        Ret::Get(got)
                    }
                    _ => unreachable!(),
                };
                let end = Instant::now();

                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }

            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let history = history.lock().unwrap().clone();
    assert_cas_mix_exercised(&history);

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for blob key {}",
            key
        );
    }
}

/// #929: the disjoint-writer census for the string wrapper, as
/// [`test_multi_writer_parallel_disjoint_and_census`] performs it for the
/// integer wrappers. Each thread owns one first chunk; pairs of its keys
/// share a second chunk and diverge after it, so its own inserts split
/// suffixes and its removals prune the grandchildren they empty.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_multi_writer_str_parallel_disjoint_and_census() {
    let map = Arc::new(SyncExpanseStrMap::new());

    let prefill: Vec<Vec<u8>> = (0..48u64)
        .map(|i| format!("pre{i:05}").into_bytes())
        .collect();
    for (i, k) in prefill.iter().enumerate() {
        map.insert(NulFreeStr::new(k).unwrap(), i as u64);
    }

    let num_threads = 4;
    let keys_per_thread = 400;
    let key_of = |t: usize, i: usize| -> Vec<u8> {
        let tail = if i.is_multiple_of(2) {
            "left"
        } else {
            "right-and-longer"
        };
        format!("t{t}/item{:04}/{tail}", i / 2).into_bytes()
    };

    let mut handles: Vec<thread::JoinHandle<()>> = vec![];
    for t_id in 0..num_threads {
        let m = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 0..keys_per_thread {
                let k = key_of(t_id, i);
                let nk = NulFreeStr::new(&k).unwrap();
                assert_eq!(m.insert(nk, (t_id * 10_000 + i) as u64), None);
            }
            for i in 0..keys_per_thread {
                let k = key_of(t_id, i);
                let nk = NulFreeStr::new(&k).unwrap();
                assert_eq!(
                    m.insert(nk, (t_id * 10_000 + i + 1) as u64),
                    Some((t_id * 10_000 + i) as u64)
                );
            }
            for i in (0..keys_per_thread).step_by(3) {
                let k = key_of(t_id, i);
                let nk = NulFreeStr::new(&k).unwrap();
                assert_eq!(m.remove(nk), Some((t_id * 10_000 + i + 1) as u64));
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let removed_per_thread = keys_per_thread.div_ceil(3);
    let expected = prefill.len() + num_threads * (keys_per_thread - removed_per_thread);
    assert_eq!(map.len(), expected as u64);
    map.with_locked(|inner| {
        assert_eq!(inner.len(), expected as u64);
        for (i, k) in prefill.iter().enumerate() {
            assert_eq!(inner.get(NulFreeStr::new(k).unwrap()), Some(i as u64));
        }
        for t_id in 0..num_threads {
            for i in 0..keys_per_thread {
                let k = key_of(t_id, i);
                let want = if i.is_multiple_of(3) {
                    None
                } else {
                    Some((t_id * 10_000 + i + 1) as u64)
                };
                assert_eq!(inner.get(NulFreeStr::new(&k).unwrap()), want, "{k:?}");
            }
        }
    });
}

/// Linearizability of `SyncExpanseMap::contains_key` and `MapReader::contains`
/// against writers mixing inserts, removes, compare-exchanges and gets.
///
/// Values come from `1..=3` and the key domain is small, so a key is present
/// and absent in turn. `tree_rooted` pre-populates past `ROOT_LEAF_CAP` so the
/// mix runs on the optimistic tree paths; otherwise the root stays a leaf.
fn contains_key_mix_run(tree_rooted: bool) {
    let map = Arc::new(SyncExpanseMap::new());
    if tree_rooted {
        for b in 1..=64u64 {
            map.insert(b << 56, b);
        }
    }
    let history = Arc::new(Mutex::new(Vec::new()));
    let num_threads = 4;
    let ops_per_thread = 60;
    let mut handles = vec![];
    for t_id in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let history_clone = Arc::clone(&history);
        handles.push(thread::spawn(move || {
            let mut local_events = Vec::with_capacity(ops_per_thread);
            let reader = map_clone.reader();
            for i in 0..ops_per_thread {
                let key = ((t_id * 7 + i * 5) % 6) as u64;
                let v = ((t_id + i) % 3 + 1) as u64;
                let op = match (t_id * 3 + i) % 6 {
                    0 => Op::Insert(key, v),
                    1 => Op::Remove(key),
                    2 => Op::CompareExchange(
                        key,
                        (!i.is_multiple_of(3)).then_some(v),
                        (!i.is_multiple_of(2)).then_some(((i / 2) % 3 + 1) as u64),
                    ),
                    3 => Op::Get(key),
                    _ => Op::Contains(key),
                };
                let start = Instant::now();
                let ret = match &op {
                    Op::Insert(k, v) => Ret::Insert(map_clone.insert(*k, *v)),
                    Op::Remove(k) => Ret::Remove(map_clone.remove(*k)),
                    Op::CompareExchange(k, exp, new) => {
                        Ret::CompareExchange(map_clone.compare_exchange(*k, *exp, *new))
                    }
                    Op::Get(k) => Ret::Get(reader.get(*k)),
                    // Both entry points are exercised, alternating by step.
                    Op::Contains(k) if i % 2 == 0 => Ret::Contains(reader.contains(*k)),
                    Op::Contains(k) => Ret::Contains(map_clone.contains_key(*k)),
                    Op::Update(..) => unreachable!(),
                };
                let end = Instant::now();
                local_events.push(Event {
                    op,
                    ret,
                    start,
                    end,
                });
            }
            let mut h = history_clone.lock().unwrap();
            h.extend(local_events);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let history = history.lock().unwrap().clone();
    let (mut hit, mut miss) = (0, 0);
    for e in &history {
        match e.ret {
            Ret::Contains(true) => hit += 1,
            Ret::Contains(false) => miss += 1,
            _ => {}
        }
    }
    assert!(
        hit > 0 && miss > 0,
        "contains outcomes: {hit} present, {miss} absent"
    );
    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }
    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for contains key {}",
            key
        );
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_contains_key_linearizability_tree_rooted() {
    contains_key_mix_run(true);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_contains_key_linearizability_root_leaf() {
    contains_key_mix_run(false);
}

// ---------------------------------------------------------------------------
// `SyncExpanseMap::compare_exchange`: the lost-update detectors.
// ---------------------------------------------------------------------------

/// Retries after which a detector calls a stall a failure. Far above what
/// contention costs: a correct run retries a few times per operation.
const STALL: u64 = 2_000_000;

/// Counters built from `get` + `compare_exchange` alone, incremented by
/// several threads while other threads insert and remove neighbouring keys so
/// the counters' leaves grow, shrink, reallocate and change form under them.
/// Every counter must end at exactly the number of compares that reported a
/// store: one more is a store reported twice, one fewer is a lost update.
fn compare_exchange_counter_run(prepopulate: bool) {
    let map = Arc::new(SyncExpanseMap::new());
    if prepopulate {
        // Past the root leaf's capacity, so the root is a tree and the
        // compares run on the optimistic path.
        for b in 1..=64u64 {
            map.insert(b << 56, b);
        }
    }
    // Counters in different places: alone under the root branch, inside a
    // dense run, and inside two small clusters.
    let counters: [u64; 4] = [
        0x7700_0000_0000_0000,
        1_000,
        (3 << 56) | 5,
        (5 << 56) | (9 << 40),
    ];
    for &c in &counters {
        assert_eq!(map.compare_exchange(c, None, Some(0)), Ok(None));
    }

    const THREADS: u64 = 6;
    const INCREMENTS: u64 = 4_000;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let churners: Vec<_> = (0..2u64)
        .map(|t| {
            let (map, stop) = (Arc::clone(&map), Arc::clone(&stop));
            thread::spawn(move || {
                let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ t;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    // Neighbours of each counter, never a counter itself.
                    let k = match x % 4 {
                        0 => 1_001 + (x >> 8) % 600,
                        1 => (3 << 56) | (6 + (x >> 8) % 40),
                        2 => (5 << 56) | (9 << 40) | (1 + (x >> 8) % 40),
                        _ => ((x >> 8) % 250 + 1) << 48,
                    };
                    // Without the prepopulation the churn stays on twenty
                    // keys, so the root never leaves leaf state.
                    let k = if prepopulate {
                        k
                    } else {
                        2_000 + (x >> 8) % 20
                    };
                    if (x >> 3) & 1 == 0 {
                        map.insert(k, x);
                    } else {
                        map.remove(k);
                    }
                }
            })
        })
        .collect();

    let workers: Vec<_> = (0..THREADS)
        .map(|t| {
            let map = Arc::clone(&map);
            thread::spawn(move || {
                let mut stored = [0u64; 4];
                for i in 0..INCREMENTS {
                    let which = ((i + t) % 4) as usize;
                    let key = counters[which];
                    let mut cur = map.get(key);
                    for attempt in 0.. {
                        // A broken compare can livelock the counters rather
                        // than miscount them; that is a failure, not a hang.
                        assert!(attempt < STALL, "counter {key:#x} never takes a store");
                        let v = cur.expect("a counter is never removed");
                        match map.compare_exchange(key, cur, Some(v + 1)) {
                            Ok(prev) => {
                                assert_eq!(prev, cur, "Ok must carry the expected word");
                                stored[which] += 1;
                                break;
                            }
                            Err(seen) => {
                                assert_ne!(seen, cur, "Err must carry a word that differs");
                                cur = seen;
                            }
                        }
                    }
                }
                stored
            })
        })
        .collect();

    let mut stored = [0u64; 4];
    for w in workers {
        let s = w.join().expect("worker panicked");
        for i in 0..4 {
            stored[i] += s[i];
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    for c in churners {
        c.join().expect("churner panicked");
    }

    assert_eq!(stored.iter().sum::<u64>(), THREADS * INCREMENTS);
    for (i, &c) in counters.iter().enumerate() {
        assert_eq!(
            map.get(c),
            Some(stored[i]),
            "counter {c:#x}: {} stores were reported",
            stored[i]
        );
    }
    map.with_locked(expanse_trie::map::ExpanseMap::validate);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_compare_exchange_counter_tree_rooted() {
    compare_exchange_counter_run(true);
}

/// The same counters with the root held in leaf state (24 keys at most), so
/// every compare goes through the exclusive form.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_compare_exchange_counter_root_leaf() {
    compare_exchange_counter_run(false);
}

/// Mutual exclusion built from insert-if-absent and remove-if-equals: a key's
/// presence is the lock and its word names the holder. The protected counter
/// is read and written non-atomically, so two holders at once lose an update;
/// a release that fails means another thread's word replaced the holder's.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_compare_exchange_token_exclusion() {
    use std::sync::atomic::{AtomicU64, Ordering};
    let map = Arc::new(SyncExpanseMap::new());
    for b in 1..=64u64 {
        map.insert(b << 56, b);
    }
    // Two tokens: one alone in its branch slot (its removal empties the
    // slot), one in a populated leaf.
    let tokens: [u64; 2] = [0x7700_0000_0000_0000, (3 << 56) | 5];
    for n in 0..20u64 {
        map.insert((3 << 56) | (100 + n), n);
    }
    let guarded = Arc::new([AtomicU64::new(0), AtomicU64::new(0)]);

    const THREADS: u64 = 6;
    const ROUNDS: u64 = 3_000;
    let workers: Vec<_> = (1..=THREADS)
        .map(|id| {
            let (map, guarded) = (Arc::clone(&map), Arc::clone(&guarded));
            thread::spawn(move || {
                for i in 0..ROUNDS {
                    let which = ((i + id) % 2) as usize;
                    let key = tokens[which];
                    let mut attempts = 0u64;
                    while map.compare_exchange(key, None, Some(id)).is_err() {
                        attempts += 1;
                        assert!(attempts < STALL, "token {key:#x} is never released");
                        std::hint::spin_loop();
                    }
                    let seen = guarded[which].load(Ordering::Relaxed);
                    std::hint::spin_loop();
                    guarded[which].store(seen + 1, Ordering::Relaxed);
                    assert_eq!(
                        map.compare_exchange(key, Some(id), None),
                        Ok(Some(id)),
                        "the holder's word was replaced while it held the token"
                    );
                }
            })
        })
        .collect();
    for w in workers {
        w.join().expect("worker panicked");
    }
    let total: u64 = guarded.iter().map(|g| g.load(Ordering::Relaxed)).sum();
    assert_eq!(
        total,
        THREADS * ROUNDS,
        "two holders at once lost an update"
    );
    for &t in &tokens {
        assert_eq!(map.get(t), None);
    }
    assert_eq!(map.len(), 64 + 20);
    map.with_locked(expanse_trie::map::ExpanseMap::validate);
}

/// Remove-if-equals under contention: threads race to take the word out of a
/// key with `compare_exchange(key, Some(v), None)` and the one that wins puts
/// `v + 1` back. A removal that ignores its compare takes out a word its
/// caller never read: the winner's put-back then fails, or the key stays
/// absent and the run stalls.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_compare_exchange_claim_by_removal() {
    let map = Arc::new(SyncExpanseMap::new());
    for b in 1..=64u64 {
        map.insert(b << 56, b);
    }
    // One key alone in its branch slot, one inside a populated leaf, one
    // inside a dense run.
    let keys: [u64; 3] = [0x7700_0000_0000_0000, (3 << 56) | 5, 1_000];
    for n in 0..20u64 {
        map.insert((3 << 56) | (100 + n), n);
    }
    for n in 0..600u64 {
        map.insert(1_001 + n, n);
    }
    for &k in &keys {
        map.insert(k, 0);
    }

    const THREADS: u64 = 6;
    const CLAIMS: u64 = 2_000;
    let workers: Vec<_> = (0..THREADS)
        .map(|t| {
            let map = Arc::clone(&map);
            thread::spawn(move || {
                let mut claimed = [0u64; 3];
                for i in 0..CLAIMS {
                    let which = ((i + t) % 3) as usize;
                    let key = keys[which];
                    for attempt in 0.. {
                        assert!(attempt < STALL, "key {key:#x} stays absent");
                        let Some(v) = map.get(key) else {
                            std::hint::spin_loop();
                            continue;
                        };
                        if map.compare_exchange(key, Some(v), None) == Ok(Some(v)) {
                            assert_eq!(
                                map.compare_exchange(key, None, Some(v + 1)),
                                Ok(None),
                                "another thread took or replaced a claimed key"
                            );
                            claimed[which] += 1;
                            break;
                        }
                    }
                }
                claimed
            })
        })
        .collect();
    let mut claimed = [0u64; 3];
    for w in workers {
        let c = w.join().expect("worker panicked");
        for i in 0..3 {
            claimed[i] += c[i];
        }
    }
    for (i, &k) in keys.iter().enumerate() {
        assert_eq!(map.get(k), Some(claimed[i]), "key {k:#x}");
    }
    assert_eq!(claimed.iter().sum::<u64>(), THREADS * CLAIMS);
    assert_eq!(map.len(), 64 + 20 + 600 + 3);
    map.with_locked(expanse_trie::map::ExpanseMap::validate);
}

/// Root digits that never move in the floor-crossing tests below: one key
/// per top digit `0..FLOOR_FIXED`, so the root is a branch keyed on byte 7.
const FLOOR_FIXED: u64 = BRANCHU_TO_B_DOWN as u64 - 3;
/// Root digits the writers insert and remove: the root's digit count moves
/// through `FLOOR_FIXED..=FLOOR_FIXED + FLOOR_CHURN`, which spans both the
/// `BranchU` demotion floor and the bitmap-to-uncompressed threshold, so the
/// root is promoted to a `BranchU` and demoted back while readers run (Refs
/// #1079). The band between the two is 32 digits wide, so the schedule
/// alternates phases that insert with phases that remove
/// ([`floor_schedule`]); a uniform mix would hold the count mid-band.
const FLOOR_CHURN: u64 = BITMAP_TO_UNCOMPRESSED_THRESHOLD as u64 + 4 - FLOOR_FIXED;
const _: () = assert!(
    FLOOR_FIXED <= BRANCHU_TO_B_DOWN as u64
        && FLOOR_FIXED + FLOOR_CHURN > BITMAP_TO_UNCOMPRESSED_THRESHOLD as u64 + 1
);
/// Steps of one phase of [`floor_schedule`]; phases alternate between
/// inserting and removing.
const FLOOR_PHASE: usize = 40;

fn floor_key(d: u64) -> u64 {
    (d << 56) | 0x0001
}

/// A fixed per-thread operation schedule (xorshift64), so a failure replays
/// the same program order. A writer inserts during an even-numbered phase
/// of [`FLOOR_PHASE`] steps and removes during an odd-numbered one, drawing
/// the key uniformly from the churn range, so the writers together carry
/// the root's digit count from one end of the range towards the other
/// within each phase.
fn floor_schedule(t_id: u64, n: usize) -> Vec<(u64, bool)> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ (t_id + 1).wrapping_mul(0xD1B5_4A32_D192_ED03);
    (0..n)
        .map(|i| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (
                FLOOR_FIXED + x % FLOOR_CHURN,
                (i / FLOOR_PHASE).is_multiple_of(2),
            )
        })
        .collect()
}

/// Removes and re-inserts carry a `BranchU` root back and forth across its
/// demotion floor under W = 4 writers and 2 readers. Each key's history is
/// checked for linearizability, the untouched keys must stay readable
/// throughout, and the tree must validate once the threads join: before
/// #1079 an optimistic removal could leave the root uncompressed below the
/// floor. The crossings fall back (`DemoteU` down, `Upgrade` up), so this is
/// also the concurrent check of the fallback itself.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_linearizability_branch_u_floor_crossings() {
    const WRITERS: u64 = 4;
    const READERS: u64 = 2;
    const OPS: usize = 120;
    for round in 0..4u64 {
        let map = Arc::new(SyncExpanseMap::new());
        for d in 0..FLOOR_FIXED {
            map.insert(floor_key(d), d);
        }
        let history = Arc::new(Mutex::new(Vec::new()));
        let mut handles = vec![];
        for t_id in 0..WRITERS + READERS {
            let map = Arc::clone(&map);
            let history = Arc::clone(&history);
            handles.push(thread::spawn(move || {
                let mut local = Vec::with_capacity(OPS);
                for (i, (d, ins)) in floor_schedule(round * 16 + t_id, OPS)
                    .into_iter()
                    .enumerate()
                {
                    let key = floor_key(d);
                    let op = if t_id >= WRITERS {
                        // A reader also checks one untouched key per step.
                        let fixed = (i as u64 * 7 + t_id) % FLOOR_FIXED;
                        assert_eq!(map.get(floor_key(fixed)), Some(fixed));
                        Op::Get(key)
                    } else if ins {
                        Op::Insert(key, t_id * 1000 + i as u64)
                    } else {
                        Op::Remove(key)
                    };
                    let start = Instant::now();
                    let ret = match &op {
                        Op::Insert(k, v) => Ret::Insert(map.insert(*k, *v)),
                        Op::Remove(k) => Ret::Remove(map.remove(*k)),
                        Op::Get(k) => Ret::Get(map.get(*k)),
                        Op::CompareExchange(..) => unreachable!(),
                        Op::Update(..) | Op::Contains(_) => unreachable!(),
                    };
                    let end = Instant::now();
                    // The first writer validates the tree every tenth step
                    // from inside an exclusive section, so a branch left
                    // below its floor mid-run is caught at the step, not
                    // only if the run happens to end there.
                    if t_id == 0
                        && i % 10 == 9
                        && let Err(e) = map.with_locked(|m| m.validate_defensive())
                    {
                        panic!("round {round}, step {i}: {e}");
                    }
                    local.push(Event {
                        op,
                        ret,
                        start,
                        end,
                    });
                }
                history.lock().unwrap().extend(local);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let history = history.lock().unwrap().clone();
        let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
        for e in history {
            by_key.entry(e.op.key()).or_default().push(e);
        }
        for (key, events) in by_key {
            assert!(
                check_linearizability_for_key(&events),
                "round {round}: linearizability violation for key {key:#x}"
            );
        }
        for d in 0..FLOOR_FIXED {
            assert_eq!(map.get(floor_key(d)), Some(d), "round {round}");
        }
        // A deterministic tail: every churn digit in (the root is then a
        // `BranchU`), then every one out, across the floor on one thread.
        for d in FLOOR_FIXED..FLOOR_FIXED + FLOOR_CHURN {
            map.insert(floor_key(d), d);
        }
        for d in FLOOR_FIXED..FLOOR_FIXED + FLOOR_CHURN {
            assert_eq!(map.remove(floor_key(d)), Some(d));
        }
        if let Err(e) = map.with_locked(|m| m.validate_defensive()) {
            panic!("round {round}: {e}");
        }
    }
}

/// The set twin of the floor-crossing test.
#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_set_linearizability_branch_u_floor_crossings() {
    const WRITERS: u64 = 4;
    const READERS: u64 = 2;
    const OPS: usize = 120;
    for round in 0..4u64 {
        let set = Arc::new(SyncExpanseSet::new());
        for d in 0..FLOOR_FIXED {
            set.insert(floor_key(d));
        }
        let history = Arc::new(Mutex::new(Vec::new()));
        let mut handles = vec![];
        for t_id in 0..WRITERS + READERS {
            let set = Arc::clone(&set);
            let history = Arc::clone(&history);
            handles.push(thread::spawn(move || {
                let mut local = Vec::with_capacity(OPS);
                for (i, (d, ins)) in floor_schedule(round * 16 + t_id + 8, OPS)
                    .into_iter()
                    .enumerate()
                {
                    let key = floor_key(d);
                    let op = if t_id >= WRITERS {
                        let fixed = (i as u64 * 7 + t_id) % FLOOR_FIXED;
                        assert!(set.contains(floor_key(fixed)));
                        SetOp::Contains(key)
                    } else if ins {
                        SetOp::Insert(key)
                    } else {
                        SetOp::Remove(key)
                    };
                    let start = Instant::now();
                    let ret = match &op {
                        SetOp::Insert(k) => SetRet::Insert(set.insert(*k)),
                        SetOp::Remove(k) => SetRet::Remove(set.remove(*k)),
                        SetOp::Contains(k) => SetRet::Contains(set.contains(*k)),
                    };
                    let end = Instant::now();
                    // The first writer validates the tree every tenth step
                    // from inside an exclusive section, so a branch left
                    // below its floor mid-run is caught at the step, not
                    // only if the run happens to end there.
                    if t_id == 0
                        && i % 10 == 9
                        && let Err(e) = set.with_locked(|s| s.validate_defensive())
                    {
                        panic!("round {round}, step {i}: {e}");
                    }
                    local.push(SetEvent {
                        op,
                        ret,
                        start,
                        end,
                    });
                }
                history.lock().unwrap().extend(local);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let history = history.lock().unwrap().clone();
        let mut by_key: HashMap<u64, Vec<SetEvent>> = HashMap::new();
        for e in history {
            by_key.entry(e.op.key()).or_default().push(e);
        }
        for (key, events) in by_key {
            assert!(
                check_set_linearizability_for_key(&events),
                "round {round}: linearizability violation for key {key:#x}"
            );
        }
        for d in 0..FLOOR_FIXED {
            assert!(set.contains(floor_key(d)), "round {round}");
        }
        for d in FLOOR_FIXED..FLOOR_FIXED + FLOOR_CHURN {
            set.insert(floor_key(d));
        }
        for d in FLOOR_FIXED..FLOOR_FIXED + FLOOR_CHURN {
            assert!(set.remove(floor_key(d)));
        }
        if let Err(e) = set.with_locked(|s| s.validate_defensive()) {
            panic!("round {round}: {e}");
        }
    }
}

// --- Batch cursor scan linearizability verification (#1142) --------------

#[derive(Clone, Debug)]
struct ScanEvent {
    start: Instant,
    end: Instant,
    results: Vec<(u64, u64)>,
}

#[derive(Clone, Debug)]
enum MapWriteOp {
    Insert(u64, u64),
    Remove(u64),
}

#[derive(Clone, Debug)]
struct MapWriteEvent {
    op: MapWriteOp,
    start: Instant,
    end: Instant,
}

fn check_scan_linearizability(
    scan: &ScanEvent,
    writes: &[MapWriteEvent],
    initial: &BTreeMap<u64, u64>,
) -> bool {
    // 1. Strictly ascending order:
    for window in scan.results.windows(2) {
        if window[0].0 >= window[1].0 {
            return false;
        }
    }

    let scan_map: HashMap<u64, u64> = scan.results.iter().copied().collect();
    if scan_map.len() != scan.results.len() {
        return false;
    }

    // 2. Any key present in `initial` that had NO remove before `scan.end` must be seen:
    for &k in initial.keys() {
        let removed = writes.iter().any(|w| {
            if let MapWriteOp::Remove(rk) = w.op {
                rk == k && w.start <= scan.end
            } else {
                false
            }
        });
        if !removed && !scan_map.contains_key(&k) {
            return false;
        }
    }

    // 3. Any key observed by scan must have existed (in initial or inserted before scan.end):
    for (&k, &v) in &scan_map {
        let in_initial = initial.get(&k) == Some(&v);
        let inserted = writes.iter().any(|w| {
            if let MapWriteOp::Insert(ik, iv) = w.op {
                ik == k && iv == v && w.start <= scan.end
            } else {
                false
            }
        });
        if !in_initial && !inserted {
            return false;
        }
    }

    // 4. Any insert that completed before scan started, and was not removed before scan.end, MUST be seen:
    for w in writes {
        if let MapWriteOp::Insert(ik, iv) = w.op
            && w.end <= scan.start
        {
            let removed = writes.iter().any(|other| {
                if let MapWriteOp::Remove(rk) = other.op {
                    rk == ik && other.start >= w.end && other.start <= scan.end
                } else {
                    false
                }
            });
            if !removed && scan_map.get(&ik) != Some(&iv) {
                return false;
            }
        }
    }

    true
}

#[test]
#[cfg_attr(
    miri,
    ignore = "deliberate seqlock racy-read design; see sync.rs module docs"
)]
fn test_sync_map_batch_cursor_scan_linearizability() {
    use std::sync::atomic::{AtomicBool, Ordering};

    const INITIAL_KEYS: u64 = 64;
    // The writer runs until the reader has finished. The reader scans until
    // it has `SCAN_ROUNDS` scans of which `MIN_OVERLAPPED` had a write
    // complete during them, so the history is concurrent on any schedule.
    const SCAN_ROUNDS: usize = 20;
    const MIN_OVERLAPPED: u64 = 5;
    const SCAN_ROUNDS_CAP: usize = 200_000;

    let map = Arc::new(SyncExpanseMap::new());
    let mut initial = BTreeMap::new();
    for i in 0..INITIAL_KEYS {
        let k = i * 100;
        let v = i * 1000;
        map.insert(k, v);
        initial.insert(k, v);
    }

    let writes = Arc::new(Mutex::new(Vec::new()));
    let scans = Arc::new(Mutex::new(Vec::new()));
    let scans_done = Arc::new(AtomicBool::new(false));
    let write_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let overlapped = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let start_line = Arc::new(std::sync::Barrier::new(2));

    let mut handles = vec![];

    // Writer: inserts/removes in range 0..1000
    {
        let map = Arc::clone(&map);
        let writes = Arc::clone(&writes);
        let scans_done = Arc::clone(&scans_done);
        let write_count = Arc::clone(&write_count);
        let start_line = Arc::clone(&start_line);
        handles.push(thread::spawn(move || {
            let mut local = Vec::new();
            let mut prng = 0x1234_5678u64;
            start_line.wait();
            let mut i = 0usize;
            while !scans_done.load(Ordering::Acquire) {
                i += 1;
                prng = prng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let key = 50 + (prng % 50) * 100;
                let is_ins = i.is_multiple_of(2);
                let start = Instant::now();
                let op = if is_ins {
                    map.insert(key, key * 10);
                    MapWriteOp::Insert(key, key * 10)
                } else {
                    map.remove(key);
                    MapWriteOp::Remove(key)
                };
                let end = Instant::now();
                local.push(MapWriteEvent { op, start, end });
                write_count.fetch_add(1, Ordering::Release);
            }
            writes.lock().unwrap().extend(local);
        }));
    }

    // Reader: performs batch scans with cursor
    {
        let map = Arc::clone(&map);
        let scans = Arc::clone(&scans);
        let scans_done = Arc::clone(&scans_done);
        let write_count = Arc::clone(&write_count);
        let overlapped = Arc::clone(&overlapped);
        let start_line = Arc::clone(&start_line);
        handles.push(thread::spawn(move || {
            let rd = map.reader();
            let mut local = Vec::with_capacity(SCAN_ROUNDS);
            start_line.wait();
            let mut seen_overlap = 0u64;
            for round in 0..SCAN_ROUNDS_CAP {
                if round >= SCAN_ROUNDS && seen_overlap >= MIN_OVERLAPPED {
                    break;
                }
                let writes_before = write_count.load(Ordering::Acquire);
                let start = Instant::now();
                let cur = rd.cursor();
                let mut results = Vec::new();
                for e in cur {
                    results.push(e);
                }
                let end = Instant::now();
                local.push(ScanEvent {
                    start,
                    end,
                    results,
                });
                if write_count.load(Ordering::Acquire) != writes_before {
                    seen_overlap += 1;
                    overlapped.fetch_add(1, Ordering::Relaxed);
                }
            }
            scans_done.store(true, Ordering::Release);
            scans.lock().unwrap().extend(local);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let all_writes = writes.lock().unwrap().clone();
    let all_scans = scans.lock().unwrap().clone();

    assert!(all_scans.len() >= SCAN_ROUNDS);
    // A history in which no write completed during any scan checks nothing
    // about concurrency.
    assert!(
        overlapped.load(Ordering::Relaxed) >= MIN_OVERLAPPED,
        "{} of {} scans had a write complete during them ({} writes in all)",
        overlapped.load(Ordering::Relaxed),
        all_scans.len(),
        all_writes.len()
    );

    for (i, scan) in all_scans.iter().enumerate() {
        assert!(
            check_scan_linearizability(scan, &all_writes, &initial),
            "scan {i} must satisfy linearizability invariants"
        );
    }
}
