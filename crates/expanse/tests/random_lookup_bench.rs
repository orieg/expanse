use std::time::Instant;
use expanse_trie::map::ExpanseMap;
use std::collections::HashMap;

// Seeded RNG
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Rng(seed) }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[test]
fn bench_random_lookup() {
    let mut rng = Rng(12345);
    let n = 1_000_000;
    
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        keys.push(rng.next());
    }

    let mut map = ExpanseMap::new();
    let mut hash_map = HashMap::new();
    for &k in &keys {
        map.insert(k, k);
        hash_map.insert(k, k);
    }

    // Benchmark ExpanseMap
    let mut sum1 = 0;
    let start = Instant::now();
    for &k in &keys {
        if let Some(v) = map.get(k) {
            sum1 ^= v;
        }
    }
    let dur1 = start.elapsed();
    
    // Benchmark HashMap
    let mut sum2 = 0;
    let start2 = Instant::now();
    for &k in &keys {
        if let Some(&v) = hash_map.get(&k) {
            sum2 ^= v;
        }
    }
    let dur2 = start2.elapsed();
    
    println!("ExpanseMap lookup 1M random keys: {:?}", dur1);
    println!("HashMap lookup 1M random keys: {:?}", dur2);
    assert_eq!(sum1, sum2);
}
