//! Concurrent OCC linearizability verification test harness.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use expanse_trie::sync::SyncExpanseMap;

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
                if let Some(me) = min_end {
                    if me < e.start {
                        continue;
                    }
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
fn test_sync_map_sequential_linearizability() {
    let map = Arc::new(SyncExpanseMap::new());
    let mut history = Vec::new();
    let ops = 20; // smaller for miri
    let keys = [1, 2, 3];

    for i in 0..ops {
        let key = keys[i % keys.len()];
        let op = match i % 3 {
            0 => Op::Insert(key, i as u64),
            1 => Op::Remove(key),
            _ => Op::Get(key),
        };

        let start = Instant::now();
        let ret = match &op {
            Op::Insert(k, v) => Ret::Insert(map.insert(*k, *v)),
            Op::Remove(k) => Ret::Remove(map.remove(*k)),
            Op::Get(k) => Ret::Get(map.get(*k)),
        };
        let end = Instant::now();

        history.push(Event {
            op,
            ret,
            start,
            end,
        });
    }

    let mut by_key: HashMap<u64, Vec<Event>> = HashMap::new();
    for e in history {
        by_key.entry(e.op.key()).or_default().push(e);
    }

    for (key, events) in by_key {
        assert!(
            check_linearizability_for_key(&events),
            "Linearizability violation for key {}",
            key
        );
    }
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
