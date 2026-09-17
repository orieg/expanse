//! Concurrent OCC linearizability verification test harness.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};

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
