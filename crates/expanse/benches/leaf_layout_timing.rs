//! W3 timing for the #1257 id encodings: does the memory saving of an
//! encoded key set cost lookups? One process times one (key set, load order,
//! round) cell: the insert load into a `SyncExpanseStrMap`, then point
//! lookups from 16 reader threads under a skewed key choice, hits and misses.
//! The key sets are the census's own (`leaf_layout_census.rs --dump-keys`),
//! so the timed keys are the measured keys byte for byte. Driven, one process
//! per cell and round, by `scripts/leaf_layout_timing.py`, which pins, takes
//! the load windows and computes the paired intervals.
//!
//! Run: `leaf_layout_timing --keys <arm>.keys --miss <arm>.miss --order sorted|shuffled --round K [--threads 16] [--ops 2000000]`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `bench_leaf_layout_timing` |
//! | `group` | 5 |
//! | `population` | 10^7 keys per arm (the census's `orders` and uniform `composite`, text or 7-bit ids against aligned base-32) |
//! | `insertion_order` | both — `sorted` (the key file's order) or `shuffled` (a Fisher–Yates permutation from the suite PRNG, the same for every arm and round) |
//! | `probes_and_reuse` | per thread `--ops` probes (default 2,000,000) of a pre-generated index stream; 16 threads |
//! | `hit_rate` | 100% in the hit phase, 0% in the miss phase |
//! | `miss_gen_method` | drawn from each shape's own generator and absent by construction (`dump_keys` in `leaf_layout_census.rs`): `orders` a tenant and an id rejected when present, `composite` a name's ids continued past its population (§8.6) |
//! | `value_dereference` | every lookup's value is summed into a `black_box` sink |
//! | `measured_region` | Clean: the key file read, the shuffle and the probe streams are generated before each timed region; teardown follows the last one |
//! | `arm_symmetry` | every arm runs the same code on its own key file; the probe stream is a Zipfian (θ = 0.99) rank over a fixed permutation of the arm's keys, with the same seeds |
//! | `statistics` | per-process means; the driver pairs arms by round and reports BCa 95% intervals of the ratios, with an A/A control |
//! | `verdict` | **PENDING**: W3 instrument for #1257; results in docs/ARCHITECTURE.md §3.6 once two runs agree. |

use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::SyncExpanseStrMap;
use std::hint::black_box;
use std::sync::Barrier;
use std::time::Instant;

/// The suite PRNG (`examples/bytes_per_key.rs`).
struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}
const SEED: u64 = 0x0DDB_1A5E_5EED_0001;

/// YCSB's Zipfian generator (Gray et al., "Quickly generating billion-record
/// synthetic databases", SIGMOD 1994), over ranks `0..n`.
struct Zipf {
    n: f64,
    theta: f64,
    alpha: f64,
    zetan: f64,
    eta: f64,
}

impl Zipf {
    fn new(n: usize, theta: f64) -> Self {
        let zeta = |k: usize| (1..=k).map(|i| 1.0 / (i as f64).powf(theta)).sum::<f64>();
        let zetan = zeta(n);
        let zeta2 = zeta(2);
        Self {
            n: n as f64,
            theta,
            alpha: 1.0 / (1.0 - theta),
            zetan,
            eta: (1.0 - (2.0 / n as f64).powf(1.0 - theta)) / (1.0 - zeta2 / zetan),
        }
    }
    fn rank(&self, u: f64) -> usize {
        let uz = u * self.zetan;
        if uz < 1.0 {
            return 0;
        }
        if uz < 1.0 + 0.5f64.powf(self.theta) {
            return 1;
        }
        let r = (self.n * (self.eta * u - self.eta + 1.0).powf(self.alpha)) as usize;
        r.min(self.n as usize - 1)
    }
}

fn read_keys(path: &str) -> Vec<Vec<u8>> {
    let buf = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let count = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
    let mut out = Vec::with_capacity(count);
    let mut at = 8;
    for _ in 0..count {
        let len = u32::from_le_bytes(buf[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        out.push(buf[at..at + len].to_vec());
        at += len;
    }
    assert_eq!(at, buf.len(), "{path}: trailing bytes");
    out
}

fn shuffle<T>(v: &mut [T], seed: u64) {
    let mut rng = XorShift(seed);
    for i in (1..v.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

/// Per-thread probe streams: Zipfian ranks mapped through a fixed permutation
/// of `0..n`, so the hot keys are spread over the key set, not its first
/// entries.
fn streams(n: usize, threads: usize, ops: usize, seed: u64) -> Vec<Vec<u32>> {
    let zipf = Zipf::new(n, 0.99);
    let mut perm: Vec<u32> = (0..n as u32).collect();
    shuffle(&mut perm, seed ^ 0x9E37);
    (0..threads)
        .map(|t| {
            let mut rng = XorShift(seed ^ (0x51ED_0000 + t as u64));
            (0..ops).map(|_| perm[zipf.rank(rng.unit())]).collect()
        })
        .collect()
}

struct Phase {
    mops: f64,
    p50_ns: f64,
    p99_ns: f64,
    found: u64,
}

/// Every thread probes its own stream once, all released by one barrier;
/// throughput is total probes over the wall time from release to the last
/// join. Every 64th probe is also timed alone, for the latency percentiles.
fn probe_phase(map: &SyncExpanseStrMap, keys: &[Vec<u8>], streams: &[Vec<u32>]) -> Phase {
    let barrier = Barrier::new(streams.len() + 1);
    let (wall, results) = std::thread::scope(|s| {
        let handles: Vec<_> = streams
            .iter()
            .map(|stream| {
                let barrier = &barrier;
                s.spawn(move || {
                    let mut sink = 0u64;
                    let mut found = 0u64;
                    let mut samples = Vec::with_capacity(stream.len() / 64 + 1);
                    barrier.wait();
                    for (i, &idx) in stream.iter().enumerate() {
                        let key = NulFreeStr::new(&keys[idx as usize]).unwrap();
                        if i % 64 == 0 {
                            let t0 = Instant::now();
                            let v = black_box(map.get(black_box(key)));
                            samples.push(t0.elapsed().as_nanos() as u64);
                            if let Some(v) = v {
                                sink = sink.wrapping_add(v);
                                found += 1;
                            }
                        } else if let Some(v) = map.get(key) {
                            sink = sink.wrapping_add(v);
                            found += 1;
                        }
                    }
                    black_box(sink);
                    (found, samples)
                })
            })
            .collect();
        barrier.wait();
        let start = Instant::now();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        (start.elapsed(), results)
    });
    let ops: usize = streams.iter().map(Vec::len).sum();
    let mut samples: Vec<u64> = results
        .iter()
        .flat_map(|(_, s)| s.iter().copied())
        .collect();
    samples.sort_unstable();
    let pct = |p: f64| samples[((samples.len() - 1) as f64 * p) as usize] as f64;
    Phase {
        mops: ops as f64 / wall.as_secs_f64() / 1e6,
        p50_ns: pct(0.50),
        p99_ns: pct(0.99),
        found: results.iter().map(|(f, _)| f).sum(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .map(|i| args[i + 1].clone())
    };
    let keys_path = arg("--keys").expect("--keys <file>");
    let miss_path = arg("--miss").expect("--miss <file>");
    let order = arg("--order").unwrap_or_else(|| "sorted".into());
    let round: u64 = arg("--round").map_or(0, |r| r.parse().expect("--round K"));
    let threads: usize = arg("--threads").map_or(16, |t| t.parse().expect("--threads N"));
    let ops: usize = arg("--ops").map_or(2_000_000, |o| o.parse().expect("--ops N"));

    let mut keys = read_keys(&keys_path);
    let misses = read_keys(&miss_path);
    let mut load: Vec<usize> = (0..keys.len()).collect();
    match order.as_str() {
        "sorted" => {}
        "shuffled" => shuffle(&mut load, SEED ^ 0x5F),
        _ => panic!("--order sorted|shuffled"),
    }
    // Probe streams before any timed region; seeded by round, so each round
    // draws its own probes and every arm draws the same ranks in a round.
    let hit_streams = streams(keys.len(), threads, ops, SEED ^ round);
    let miss_streams = streams(misses.len(), threads, ops, SEED ^ round ^ 0xAB);

    eprintln!("BENCH_WINDOW begin insert");
    let map = SyncExpanseStrMap::new();
    let t0 = Instant::now();
    for &i in &load {
        map.insert(NulFreeStr::new(&keys[i]).unwrap(), i as u64);
    }
    let insert = t0.elapsed();
    eprintln!("BENCH_WINDOW end insert");
    assert_eq!(map.len(), keys.len() as u64, "{keys_path}: duplicate keys");
    drop(load);

    eprintln!("BENCH_WINDOW begin hit");
    let hit = probe_phase(&map, &keys, &hit_streams);
    eprintln!("BENCH_WINDOW end hit");
    assert_eq!(
        hit.found as usize,
        threads * ops,
        "every hit probe must be present"
    );
    eprintln!("BENCH_WINDOW begin miss");
    let miss = probe_phase(&map, &misses, &miss_streams);
    eprintln!("BENCH_WINDOW end miss");
    assert_eq!(miss.found, 0, "a miss probe was present (§8.6)");

    let mem_used = map.with_locked(expanse_trie::strmap::ExpanseStrMap::mem_used);
    let n = keys.len();
    keys.clear();
    println!(
        "{{\"keys\": \"{keys_path}\", \"order\": \"{order}\", \"round\": {round}, \
         \"threads\": {threads}, \"ops_per_thread\": {ops}, \"n\": {n}, \
         \"mem_used\": {mem_used}, \"insert_ns_per_key\": {:.3}, \
         \"hit_mops\": {:.4}, \"hit_p50_ns\": {}, \"hit_p99_ns\": {}, \
         \"miss_mops\": {:.4}, \"miss_p50_ns\": {}, \"miss_p99_ns\": {}}}",
        insert.as_nanos() as f64 / n as f64,
        hit.mops,
        hit.p50_ns,
        hit.p99_ns,
        miss.mops,
        miss.p50_ns,
        miss.p99_ns
    );
}
