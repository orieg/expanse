//! The live ordered histories of G36.1 on a `SyncExpanseSet` (#1194;
//! `docs/benchmarks/concurrency/METHODOLOGY.md` §36.4), shared by two test
//! binaries:
//!
//! - `tests/linearizability.rs` checks each history against one sequential
//!   set with the whole-map checker;
//! - `tests/test_occ_stats_set_ordered_histories.rs` reads, over the same
//!   histories, which node forms the ordered walk visited.
//!
//! A history is a prefill and one script per thread. Thread `t` reads
//! through the handle form `t % 3` (borrowed, owned, detached), so the
//! three forms run under the same interleavings. A script may hold `Sync`
//! steps: the threads that have them meet at a barrier there, which fixes
//! the tree's form between two phases without ordering the operations
//! inside a phase. Threads without `Sync` steps run free, across every
//! phase.
// Each binary uses its own part of this module.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Instant;

use expanse_trie::sync::{DetachedSetReader, OwnedSetReader, SetReader, SyncExpanseSet};
use expanse_trie::types::{LEAF1_CAP, LEAFB1_DOWN, ROOT_LEAF_CAP};

/// Rounds per history (§36.4, G36.1).
pub const ROUNDS: usize = 24;

/// The most events a history may hold: the checker memoises on a `u64` mask.
pub const MAX_EVENTS: usize = 64;

/// One operation of a set history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOp {
    Insert(u64),
    Remove(u64),
    First,
    Last,
    NextAtOrAfter(u64),
    NextAfter(u64),
    PrevAtOrBefore(u64),
    PrevBefore(u64),
    Clear,
    Compact,
}

impl SetOp {
    /// True for the six ordered reads.
    pub fn is_read(self) -> bool {
        !matches!(
            self,
            SetOp::Insert(_) | SetOp::Remove(_) | SetOp::Clear | SetOp::Compact
        )
    }
}

/// What an operation returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetRet {
    /// `insert` or `remove`: whether the set changed.
    Changed(bool),
    /// An ordered read's answer.
    Found(Option<u64>),
    /// `clear` or `compact`.
    Done,
}

/// One completed operation, with the instants around its call.
#[derive(Clone, Debug)]
pub struct SetEvent {
    pub op: SetOp,
    pub ret: SetRet,
    pub start: Instant,
    pub end: Instant,
}

/// One step of a thread's script.
#[derive(Clone, Copy, Debug)]
pub enum Step {
    Op(SetOp),
    /// Meet the other scripted threads at the barrier.
    Sync,
}

/// The node counts a history's prefill must produce, checked before any
/// thread starts so a history cannot pass on another shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// At most `ROOT_LEAF_CAP` keys, under no branch.
    RootLeaf,
    /// A tree with no bitmap leaf and no full expanse.
    Tree,
    /// A tree holding exactly one bitmap leaf and no full expanse.
    OneBitmapLeaf,
}

/// A history: a name, a prefill, its shape, and one script per thread.
pub struct History {
    pub name: &'static str,
    pub prepop: Vec<u64>,
    pub shape: Shape,
    pub scripts: Vec<Vec<Step>>,
}

impl History {
    /// The number of operations the scripts hold.
    pub fn events(&self) -> usize {
        self.scripts
            .iter()
            .flatten()
            .filter(|s| matches!(s, Step::Op(_)))
            .count()
    }

    /// The number of ordered reads the scripts hold.
    pub fn reads(&self) -> usize {
        self.scripts
            .iter()
            .flatten()
            .filter(|s| matches!(s, Step::Op(op) if op.is_read()))
            .count()
    }
}

/// A reader handle of one of the three forms.
enum Handle<'a> {
    Borrowed(SetReader<'a>),
    Owned(OwnedSetReader),
    Detached(DetachedSetReader),
}

impl Handle<'_> {
    fn read(&self, set: &SyncExpanseSet, op: SetOp) -> Option<u64> {
        match (self, op) {
            (Handle::Borrowed(r), SetOp::First) => r.first(),
            (Handle::Borrowed(r), SetOp::Last) => r.last(),
            (Handle::Borrowed(r), SetOp::NextAtOrAfter(k)) => r.next_at_or_after(k),
            (Handle::Borrowed(r), SetOp::NextAfter(k)) => r.next_after(k),
            (Handle::Borrowed(r), SetOp::PrevAtOrBefore(k)) => r.prev_at_or_before(k),
            (Handle::Borrowed(r), SetOp::PrevBefore(k)) => r.prev_before(k),
            (Handle::Owned(r), SetOp::First) => r.first(),
            (Handle::Owned(r), SetOp::Last) => r.last(),
            (Handle::Owned(r), SetOp::NextAtOrAfter(k)) => r.next_at_or_after(k),
            (Handle::Owned(r), SetOp::NextAfter(k)) => r.next_after(k),
            (Handle::Owned(r), SetOp::PrevAtOrBefore(k)) => r.prev_at_or_before(k),
            (Handle::Owned(r), SetOp::PrevBefore(k)) => r.prev_before(k),
            (Handle::Detached(r), SetOp::First) => r.first(set),
            (Handle::Detached(r), SetOp::Last) => r.last(set),
            (Handle::Detached(r), SetOp::NextAtOrAfter(k)) => r.next_at_or_after(set, k),
            (Handle::Detached(r), SetOp::NextAfter(k)) => r.next_after(set, k),
            (Handle::Detached(r), SetOp::PrevAtOrBefore(k)) => r.prev_at_or_before(set, k),
            (Handle::Detached(r), SetOp::PrevBefore(k)) => r.prev_before(set, k),
            (_, other) => panic!("{other:?} is not a read"),
        }
    }
}

/// Runs `h` once on a fresh set. Returns the events and the prefilled model.
pub fn run(h: &History) -> (Vec<SetEvent>, BTreeSet<u64>) {
    assert!(
        h.events() <= MAX_EVENTS,
        "{}: {} events, at most {MAX_EVENTS}",
        h.name,
        h.events()
    );
    let set = Arc::new(SyncExpanseSet::new());
    let mut initial = BTreeSet::new();
    let mut prepop = h.prepop.clone();
    prepop.sort_unstable();
    for &k in &prepop {
        assert!(set.insert(k), "{}: prefill key {k:#x} twice", h.name);
        initial.insert(k);
    }
    let counts = set.with_locked(|s| s.stats().node_counts);
    // A root leaf is counted as one linear leaf under no branch.
    let branches = counts.branch_l3 + counts.branch_l7 + counts.branch_b + counts.branch_u;
    match h.shape {
        Shape::RootLeaf => assert!(
            prepop.len() <= ROOT_LEAF_CAP && branches == 0,
            "{}: the prefill must stay a root leaf: {counts:?}",
            h.name
        ),
        Shape::Tree => assert!(
            branches > 0 && counts.leaf_bitmap == 0 && counts.full_expanse == 0,
            "{}: the prefill must be a tree without bitmap leaves: {counts:?}",
            h.name
        ),
        Shape::OneBitmapLeaf => assert!(
            counts.leaf_bitmap == 1 && counts.full_expanse == 0,
            "{}: the prefill must hold one bitmap leaf: {counts:?}",
            h.name
        ),
    }

    let syncs = |s: &Vec<Step>| s.iter().filter(|x| matches!(x, Step::Sync)).count();
    let scripted: Vec<usize> = h.scripts.iter().map(syncs).filter(|&n| n > 0).collect();
    assert!(
        scripted.windows(2).all(|w| w[0] == w[1]),
        "{}: scripted threads must meet the same number of times",
        h.name
    );
    let barrier = Barrier::new(scripted.len().max(1));
    let history = Mutex::new(Vec::with_capacity(h.events()));
    thread::scope(|s| {
        for (t, script) in h.scripts.iter().enumerate() {
            let (set, barrier, history) = (&set, &barrier, &history);
            s.spawn(move || {
                let handle = match t % 3 {
                    0 => Handle::Borrowed(set.reader()),
                    1 => Handle::Owned(set.owned_reader()),
                    _ => Handle::Detached(set.detached_reader()),
                };
                let mut local = Vec::with_capacity(script.len());
                for step in script {
                    let op = match *step {
                        Step::Sync => {
                            barrier.wait();
                            continue;
                        }
                        Step::Op(op) => op,
                    };
                    let start = Instant::now();
                    let ret = match op {
                        SetOp::Insert(k) => SetRet::Changed(set.insert(k)),
                        SetOp::Remove(k) => SetRet::Changed(set.remove(k)),
                        SetOp::Clear => {
                            set.clear();
                            SetRet::Done
                        }
                        SetOp::Compact => {
                            set.compact();
                            SetRet::Done
                        }
                        read => SetRet::Found(handle.read(set, read)),
                    };
                    let end = Instant::now();
                    local.push(SetEvent {
                        op,
                        ret,
                        start,
                        end,
                    });
                }
                drop(handle);
                history.lock().unwrap().extend(local);
            });
        }
    });
    let history = history.into_inner().unwrap();
    assert_eq!(history.len(), h.events(), "{}", h.name);
    (history, initial)
}

/// The operation mix of the map's G12.2 histories, over the set's six
/// reads: a quarter inserts, a quarter removes, a quarter backward reads
/// and a quarter forward reads.
fn mix(t: usize, i: usize, key: u64) -> SetOp {
    match (t + i) % 4 {
        0 => SetOp::Insert(key),
        1 => SetOp::Remove(key),
        2 => match i % 6 {
            0..=2 => SetOp::PrevAtOrBefore(key),
            3 | 4 => SetOp::PrevBefore(key),
            _ => SetOp::Last,
        },
        _ => match i % 6 {
            0..=2 => SetOp::NextAtOrAfter(key),
            3 | 4 => SetOp::NextAfter(key),
            _ => SetOp::First,
        },
    }
}

/// `threads` scripts of `per_thread` mixed operations over `keys`.
fn mixed_scripts(keys: &[u64], threads: usize, per_thread: usize) -> Vec<Vec<Step>> {
    (0..threads)
        .map(|t| {
            (0..per_thread)
                .map(|i| Step::Op(mix(t, i, keys[(t * 3 + i) % keys.len()])))
                .collect()
        })
        .collect()
}

/// `n` reads cycling the six kinds over `probes`, for a free reader thread.
fn free_reads(probes: &[u64], n: usize, salt: usize) -> Vec<Step> {
    (0..n)
        .map(|i| {
            let k = probes[(i + salt) % probes.len()];
            Step::Op(match (i + salt) % 6 {
                0 => SetOp::PrevAtOrBefore(k),
                1 => SetOp::NextAtOrAfter(k),
                2 => SetOp::PrevBefore(k),
                3 => SetOp::NextAfter(k),
                4 => SetOp::Last,
                _ => SetOp::First,
            })
        })
        .collect()
}

/// The twin of the map's tree-rooted history (G12.2): one key under each of
/// 64 top digits, and keys that straddle byte boundaries at two levels.
pub fn tree_rooted() -> History {
    let keys = [
        0x00FF, 0x0100, 0x01FF, 0x0200, 0xFFFF, 0x1_0000, 0x1_00FF, 0x1_0100,
    ];
    History {
        name: "tree-rooted",
        prepop: (1..=64u64).map(|b| b << 56).collect(),
        shape: Shape::Tree,
        scripts: mixed_scripts(&keys, 3, 16),
    }
}

/// The twin of the map's hot-spot history (G12.2): every key in one
/// 2^16-wide expanse, so the writes land in the subtrees the searches pass
/// over and backtrack through.
pub fn hotspot() -> History {
    const BASE: u64 = 0x5_0000;
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
    History {
        name: "hot-spot",
        prepop: (0..64u64).map(|d| BASE | (d << 10) | 0x80).collect(),
        shape: Shape::Tree,
        scripts: mixed_scripts(&keys, 4, 12),
    }
}

/// Concatenates per-thread phases, with a `Sync` after each phase.
fn phased(phases: &[Vec<SetOp>]) -> Vec<Step> {
    let mut out = Vec::new();
    for p in phases {
        out.extend(p.iter().copied().map(Step::Op));
        out.push(Step::Sync);
    }
    out
}

/// The root leaf crossing `ROOT_LEAF_CAP` in both directions (§36.3, W3 and
/// W4). The prefill is one key under the cap. Two scripted writers insert
/// three keys between them, which makes the root a tree; both read; they
/// remove the three, which condenses the tree to a root leaf; both read.
/// Twice, under two free readers.
pub fn root_leaf() -> History {
    let key = |i: u64| (i + 1) << 32;
    let prepop: Vec<u64> = (0..ROOT_LEAF_CAP as u64 - 1).map(key).collect();
    // Each written key is alone under its own digit of the top branch, so
    // its removal empties a child. That removal takes the exclusive path
    // (an optimistic one never condenses the root), and the exclusive path
    // is what returns a tree below the cap to a root leaf.
    let (a, b, c) = (key(35), key(36), key(37));
    let probes = [a, b, c, key(0), key(15), key(29), key(40)];
    let mut w0 = Vec::new();
    let mut w1 = Vec::new();
    for _ in 0..2 {
        w0.extend(phased(&[
            vec![SetOp::Insert(a), SetOp::Insert(b)],
            vec![SetOp::PrevBefore(b), SetOp::NextAfter(a)],
            vec![SetOp::Remove(a), SetOp::Remove(b)],
            vec![SetOp::Last, SetOp::NextAtOrAfter(a)],
        ]));
        w1.extend(phased(&[
            vec![SetOp::Insert(c)],
            vec![SetOp::PrevAtOrBefore(c), SetOp::First],
            vec![SetOp::Remove(c)],
            vec![SetOp::PrevAtOrBefore(c), SetOp::NextAfter(c)],
        ]));
    }
    History {
        name: "root leaf",
        prepop,
        shape: Shape::RootLeaf,
        scripts: vec![
            w0,
            w1,
            free_reads(&probes, 16, 0),
            free_reads(&probes, 16, 3),
        ],
    }
}

/// The 256-key block the bitmap and full-expanse histories write: one
/// terminal byte, in a level-1 slot.
const BLOCK: u64 = 0x0042_4242_4242_4200;

/// Keys outside the block: one under each of 16 other top digits, so the
/// root is a tree whatever the block holds, and twelve in each of the next
/// three blocks. Those 36 keep a branch at level 2 above the block: with
/// fewer, the block's keys and its neighbours' fit one wider linear leaf.
fn around_block() -> impl Iterator<Item = u64> {
    (0x80..0x90u64)
        .map(|b| b << 56)
        .chain((1..=3u64).flat_map(|d| (0..12u64).map(move |i| BLOCK + (d << 8) + 0x41 + i)))
}

const _: () = assert!(LEAF1_CAP + 1 - 7 < LEAFB1_DOWN);

/// One terminal byte prefilled one key above `LEAF1_CAP`, a `LeafBitmap1`,
/// and written across `LEAFB1_DOWN`. Two scripted writers remove seven keys
/// between them, which demotes the leaf to a linear one; both read; they
/// reinsert the seven, which promotes it again; both read. Twice, under two
/// free readers.
pub fn bitmap() -> History {
    let low = |i: u64| BLOCK | (0x10 + 9 * i);
    let n = LEAF1_CAP as u64 + 1;
    let prepop: Vec<u64> = (0..n).map(low).chain(around_block()).collect();
    let probes = [low(0), low(3), low(6), low(12), low(n - 1), BLOCK | 0xFF];
    let mut w0 = Vec::new();
    let mut w1 = Vec::new();
    for _ in 0..2 {
        w0.extend(phased(&[
            (0..4).map(|i| SetOp::Remove(low(i))).collect(),
            vec![SetOp::NextAtOrAfter(BLOCK), SetOp::PrevBefore(low(8))],
            (0..4).map(|i| SetOp::Insert(low(i))).collect(),
            vec![SetOp::NextAfter(low(1)), SetOp::PrevAtOrBefore(low(5))],
        ]));
        w1.extend(phased(&[
            (4..7).map(|i| SetOp::Remove(low(i))).collect(),
            vec![
                SetOp::PrevAtOrBefore(BLOCK | 0xFF),
                SetOp::NextAfter(low(7)),
            ],
            (4..7).map(|i| SetOp::Insert(low(i))).collect(),
            vec![SetOp::PrevBefore(low(4)), SetOp::NextAtOrAfter(low(2) + 1)],
        ]));
    }
    History {
        name: "bitmap leaf",
        prepop,
        shape: Shape::OneBitmapLeaf,
        scripts: vec![
            w0,
            w1,
            free_reads(&probes, 10, 0),
            free_reads(&probes, 10, 2),
        ],
    }
}

/// One terminal byte prefilled with 250 keys. Two scripted writers insert
/// the last six between them, which converts the leaf to a full expanse;
/// both read; they remove the six, the first removal materialising a
/// 256-bit `LeafBitmap1` again (§36.3, W5); both read. Twice, under two
/// free readers.
pub fn full_expanse() -> History {
    let prepop: Vec<u64> = (0..250u64)
        .map(|i| BLOCK | i)
        .chain(around_block())
        .collect();
    let k = |i: u64| BLOCK | (250 + i);
    let probes = [BLOCK, BLOCK | 0x7F, k(0), k(2), k(5), BLOCK + 0x100];
    let mut w0 = Vec::new();
    let mut w1 = Vec::new();
    for _ in 0..2 {
        w0.extend(phased(&[
            (0..3).map(|i| SetOp::Insert(k(i))).collect(),
            vec![SetOp::PrevBefore(k(4)), SetOp::NextAtOrAfter(k(1))],
            (0..3).map(|i| SetOp::Remove(k(i))).collect(),
            vec![SetOp::PrevAtOrBefore(k(5)), SetOp::NextAfter(BLOCK | 0x20)],
        ]));
        w1.extend(phased(&[
            (3..6).map(|i| SetOp::Insert(k(i))).collect(),
            vec![SetOp::NextAfter(k(2)), SetOp::PrevAtOrBefore(BLOCK | 0x7F)],
            (3..6).map(|i| SetOp::Remove(k(i))).collect(),
            vec![SetOp::NextAtOrAfter(k(0)), SetOp::PrevBefore(BLOCK | 0xF0)],
        ]));
    }
    History {
        name: "full expanse",
        prepop,
        shape: Shape::OneBitmapLeaf,
        scripts: vec![
            w0,
            w1,
            free_reads(&probes, 12, 0),
            free_reads(&probes, 12, 1),
        ],
    }
}

/// One thread interleaves `compact` and `clear` with its own inserts while
/// three others run the mixed operations of the tree-rooted history
/// (§36.3, W1 and W2).
pub fn compact_clear() -> History {
    let keys = [
        0x00FF, 0x0100, 0x01FF, 0x0200, 0xFFFF, 0x1_0000, 0x1_00FF, 0x1_0100,
    ];
    let own = |i: u64| (0x70 + i) << 56;
    let rebuild = vec![
        SetOp::Compact,
        SetOp::Insert(own(0)),
        SetOp::Last,
        SetOp::Compact,
        SetOp::Clear,
        SetOp::Insert(own(1)),
        SetOp::Insert(keys[1]),
        SetOp::Compact,
        SetOp::First,
        SetOp::Insert(own(2)),
        SetOp::Clear,
        SetOp::Compact,
        SetOp::Insert(own(3)),
        SetOp::PrevAtOrBefore(u64::MAX),
        SetOp::Compact,
        SetOp::Clear,
    ];
    let mut scripts = vec![rebuild.into_iter().map(Step::Op).collect()];
    scripts.extend(mixed_scripts(&keys, 3, 16));
    History {
        name: "compact and clear",
        prepop: (1..=64u64).map(|b| b << 56).collect(),
        shape: Shape::Tree,
        scripts,
    }
}

/// The six histories G36.1 registers.
pub fn all() -> Vec<History> {
    vec![
        tree_rooted(),
        hotspot(),
        root_leaf(),
        bitmap(),
        full_expanse(),
        compact_clear(),
    ]
}
