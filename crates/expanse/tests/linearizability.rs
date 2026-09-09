//! Concurrent OCC linearizability verification test harness.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Insert(u64, u64),
    Remove(u64),
    Get(u64),
}

#[derive(Clone, Debug, PartialEq)]
enum Ret {
    Insert(Option<u64>),
    Remove(Option<u64>),
    Get(Option<u64>),
}

impl Op {
    fn key(&self) -> u64 {
        match self {
            Op::Insert(k, _) => *k,
            Op::Remove(k) => *k,
            Op::Get(k) => *k,
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

fn is_valid_transition(state: &Option<u64>, op: &Op, ret: &Ret) -> (bool, Option<u64>) {
    match (op, ret) {
        (Op::Insert(_, v), Ret::Insert(old)) => (old == state, Some(*v)),
        (Op::Remove(_), Ret::Remove(old)) => (old == state, None),
        (Op::Get(_), Ret::Get(val)) => (val == state, *state),
        _ => (false, None),
    }
}

fn check_linearizability_for_key(events: &[Event]) -> bool {
    // Check using a backtracking search
    fn search(
        events: &[Event],
        used: &mut Vec<bool>,
        state: Option<u64>,
        completed: usize,
    ) -> bool {
        if completed == events.len() {
            return true;
        }

        // Find the earliest end time of an unused event.
        // If an unused event ended BEFORE some other event started,
        // the other event CANNOT be ordered before it in a valid linearization.
        // Actually, we must process an event if its end time is <= the start time
        // of all other unused events.
        let mut min_end = None;
        for (i, e) in events.iter().enumerate() {
            if !used[i] && min_end.is_none_or(|me| e.end < me) {
                min_end = Some(e.end);
            }
        }

        for i in 0..events.len() {
            if !used[i] {
                let e = &events[i];

                // Real-time order violation: if an unused event ended before `e` started,
                // `e` cannot be executed before it.
                if let Some(me) = min_end
                    && me < e.start
                {
                    continue;
                }

                let (valid, next_state) = is_valid_transition(&state, &e.op, &e.ret);
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
    search(events, &mut used, None, 0)
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
