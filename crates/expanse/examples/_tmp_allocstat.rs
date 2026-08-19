//! Throwaway: allocation counts per insert for the vs_stock key streams.
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;

struct X(u64);
impl X { fn next(&mut self) -> u64 { let mut x=self.0; x^=x<<13; x^=x>>7; x^=x<<17; self.0=x; x } }

const POP: usize = 30_000;

fn keys(dist: &str) -> Vec<u64> {
    let mut rng = X(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(POP);
    match dist {
        "sequential" => out.extend((0..POP as u64).map(|k| k)),
        "random" => out.extend((0..POP).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0u64;
            for i in 0..POP as u64 {
                if i % 256 == 0 { base = rng.next() & !0xFF; }
                out.push(base + (i % 256));
            }
        }
        _ => unreachable!(),
    }
    out
}

fn main() {
    for dist in ["sequential", "random", "clustered"] {
        let ks = keys(dist);
        let mut m = ExpanseMap::new();
        for &k in &ks { m.insert(k, k); }
        let mut s = ExpanseSet::new();
        for &k in &ks { s.insert(k); }
        println!(
            "{dist:>10}: map allocs={:>8} ({:.2}/ins, {:.2} B/key)  set allocs={:>8} ({:.2}/ins)",
            m.total_node_allocs(), m.total_node_allocs() as f64 / POP as f64,
            m.mem_used() as f64 / POP as f64,
            s.total_node_allocs(), s.total_node_allocs() as f64 / POP as f64,
        );
    }
}
