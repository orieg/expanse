//! Which call frees the arena chunks a `SyncExpanseBlobMap` compaction
//! retires, how many it frees at once, and on which later operation (Refs
//! #1481 item 1).
//!
//! The instrument is the collector's cumulative counters (feature
//! `collector-census`, #1310). `UnclassedCounters::retired_*` count the
//! blocks no size class serves (an arena chunk is one; so is a large chunk
//! table) as they are retired, and `released_*` as an epoch advance takes
//! them from their bin; the free follows in the same call. The one caller of
//! that take is `Collector::try_advance` (`take::<false>` in `occ.rs`); the
//! drain on drop takes with `take::<true>`, which counts nothing. A delta in
//! `released_*` across one operation is therefore what `try_advance` freed
//! during it, and a release seen before the map drops was not freed by the
//! drop.
//!
//! The census counts a release at the take, not at the free. Today the
//! free follows in the same loop of the same call, so the two coincide; a
//! change that takes a block in one call and frees it in another (a bounded
//! free) needs an instrument at the free site instead.
//!
//! The counters are per collector, so the tests in this binary do not see
//! each other. Every operation runs on one thread with no reader registered,
//! so every `try_advance` advances and the schedule is deterministic. With
//! several writers each has its own advance countdown, and a pinned reader
//! delays an advance; neither case is covered here.
//!
//! Run by `scripts/test_collector_census.sh`, which fails a binary that runs
//! zero tests. Excluded from Miri: thousands of records.
#![cfg(not(miri))]
#![cfg(feature = "collector-census")]

use expanse_trie::occ::UnclassedCounters;
use expanse_trie::sync::SyncExpanseBlobMap;

const PAYLOAD: [u8; 128] = [0xA5; 128];

/// The epoch-advance interval of the write paths (`sync::ADVANCE_EVERY`,
/// crate-private). With no reader pinned, a block is freed by the second
/// advance after its retirement, so within two intervals of the operation
/// that retired it.
const ADVANCE_EVERY: usize = 32;

/// The smallest arena chunk (`BlobArena::new` clamps to it), so that a
/// 512-chunk arena — the `reclaim_stall` cap, there of 2 MiB chunks — fits a
/// test.
const CHUNK: usize = 4096;

/// One operation's change in the unclassed counters, and the cumulative
/// totals before it.
#[derive(Clone, Copy, Debug)]
struct Op {
    before: UnclassedCounters,
    after: UnclassedCounters,
}

impl Op {
    fn retired_blocks(&self) -> u64 {
        self.after.retired_blocks - self.before.retired_blocks
    }
    fn retired_bytes(&self) -> u64 {
        self.after.retired_bytes - self.before.retired_bytes
    }
    fn released_blocks(&self) -> u64 {
        self.after.released_blocks - self.before.released_blocks
    }
    fn released_bytes(&self) -> u64 {
        self.after.released_bytes - self.before.released_bytes
    }
}

/// Runs `f` and records what it moved through the unclassed counters.
fn measured<R>(map: &SyncExpanseBlobMap, f: impl FnOnce() -> R) -> (R, Op) {
    let before = map.collector_counters().unclassed;
    let r = f();
    let after = map.collector_counters().unclassed;
    (r, Op { before, after })
}

/// Where a compaction's retired chunk set was freed.
#[derive(Debug)]
struct Release {
    /// Operations after the compaction, 1 for the next one.
    after: usize,
    blocks: u64,
    bytes: u64,
}

/// Checks the attribution for the compaction at `ops[c]`, which retired
/// `chunks` arena chunks of [`CHUNK`] bytes, and returns where they were
/// freed. `ops[c + 1..]` are the plain inserts that followed it.
fn attribute(ops: &[Op], c: usize, chunks: u64) -> Release {
    let compaction = &ops[c];
    assert_eq!(
        compaction.released_blocks(),
        0,
        "the compacting operation freed unclassed blocks itself: {compaction:?}"
    );
    assert!(
        compaction.retired_blocks() >= chunks
            && compaction.retired_bytes() >= chunks * CHUNK as u64,
        "the compaction retired fewer than its {chunks} old chunks: {compaction:?}"
    );
    // The first later operation that frees anything the compaction retired.
    // Bins drain oldest epoch first, so that is the first one whose
    // releases pass everything retired before the compaction.
    let owed_before = compaction.before.retired_blocks;
    let owed = compaction.after.retired_blocks;
    let f = (c + 1..ops.len())
        .find(|&i| ops[i].after.released_blocks > owed_before)
        .unwrap_or_else(|| {
            panic!(
                "none of the chunk set was freed within the {} operations recorded",
                ops.len() - c - 1
            )
        });
    let freeing = &ops[f];
    // That operation freed the whole set.
    assert!(
        freeing.after.released_blocks >= owed,
        "operation {f} freed only part of the chunk set ({} of {} blocks): {freeing:?}",
        freeing.after.released_blocks.saturating_sub(owed_before),
        owed - owed_before
    );
    let release = Release {
        after: f - c,
        blocks: freeing.released_blocks(),
        bytes: freeing.released_bytes(),
    };
    // Within two advance intervals of the compaction.
    assert!(
        (1..=2 * ADVANCE_EVERY).contains(&release.after),
        "freed {} operations after the compaction",
        release.after
    );
    release
}

/// An explicit `compact()` of an arena of more than `chunks` chunks, most
/// of each dead, followed by plain inserts.
fn explicit_compaction(chunks: usize) -> (u64, Release) {
    let map = SyncExpanseBlobMap::with_chunk_size(CHUNK);
    let mut key = 0u64;
    while map.with_locked(|m| m.arena().chunks().len()) <= chunks {
        map.insert(key, &PAYLOAD, 1).expect("insert");
        key += 1;
    }
    for k in 0..key {
        if k % 64 != 0 {
            assert!(map.remove(k));
        }
    }
    let (stats, compaction) = measured(&map, || map.compact().expect("compact"));
    let old = stats.chunks_before as u64;
    let mut ops = vec![compaction];
    // Recorded past the two-interval window, so a release outside it is
    // seen and reported as such.
    for _ in 0..4 * ADVANCE_EVERY {
        let ((), op) = measured(&map, || map.insert(key, &PAYLOAD, 1).expect("insert"));
        key += 1;
        ops.push(op);
    }
    let release = attribute(&ops, 0, old);
    eprintln!("explicit compact of {old} chunks: {compaction:?}, freed {release:?}");
    (old, release)
}

#[test]
fn explicit_compaction_frees_the_chunk_set_on_one_later_insert() {
    let (old, release) = explicit_compaction(512);
    assert!(old > 512);
    // The old chunks, the chunk tables retired with them, and whatever else
    // shared their epoch bin.
    assert!(release.blocks >= old && release.bytes >= old * CHUNK as u64);
}

/// The reclaim at the capacity cap (#1290), as the `reclaim_stall`
/// overwrite cells run it: prefill `live` keys, then overwrite them in turn
/// until the cap-refused insert has compacted `compactions` times.
#[test]
fn reclaim_at_cap_frees_each_chunk_set_on_one_later_insert() {
    const CAP_CHUNKS: usize = 512;
    const LIVE: u64 = 256;
    const COMPACTIONS: usize = 3;
    let map = SyncExpanseBlobMap::with_chunk_size_and_max_capacity(CHUNK, CHUNK * CAP_CHUNKS);
    for k in 0..LIVE {
        map.insert(k, &PAYLOAD, 1).expect("prefill");
    }
    let mut ops = Vec::new();
    let mut compactions = Vec::new();
    let mut k = 0u64;
    // Recorded four advance intervals past the last compaction, as above.
    while compactions.len() < COMPACTIONS
        || ops.len() <= compactions.last().copied().unwrap_or(0) + 4 * ADVANCE_EVERY
    {
        let (r, op) = measured(&map, || map.insert(k % LIVE, &PAYLOAD, 1));
        r.expect("overwrite");
        k += 1;
        // An insert that retires half the cap's bytes or more compacted.
        if op.retired_bytes() >= (CHUNK * CAP_CHUNKS / 2) as u64 {
            compactions.push(ops.len());
        }
        ops.push(op);
        assert!(
            ops.len() < 100_000,
            "{} compactions in {k} overwrites",
            compactions.len()
        );
    }
    for &c in &compactions {
        // At the cap the arena holds `CAP_CHUNKS` chunks, all retired.
        let release = attribute(&ops, c, CAP_CHUNKS as u64);
        eprintln!(
            "cap compaction at overwrite {c}: retired {} blocks / {} B, freed {release:?}",
            ops[c].retired_blocks(),
            ops[c].retired_bytes()
        );
    }
}
