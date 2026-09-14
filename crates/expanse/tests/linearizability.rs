//! Concurrent OCC linearizability verification test harness.

use std::collections::{BTreeMap, HashMap, HashSet};
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
