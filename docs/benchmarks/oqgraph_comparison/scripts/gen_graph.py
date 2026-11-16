#!/usr/bin/env python3
"""Seeded synthetic graphs for the OQGraph evaluation.

Writes <out>/<shape>_<n>.tsv (origid, destid, weight) and <out>/<shape>_<n>.queries.json
(origins and reachable destinations for the path queries).

Shapes (each ~4 directed edges per vertex):
  er      uniform random digraph, dense ids 1..N
  pl      Chung-Lu power law (gamma = 2.2): hubs, dense ids 1..N
  grid    sqrt(N) x sqrt(N) 4-neighbour lattice, both directions: long BFS depth
  er_sid  `er` with vertex ids scattered over 63 bits (sparse Judy1 keys)
"""
import bisect, json, math, random, sys
from collections import deque

M64 = (1 << 64) - 1

def scatter(i):
    return (((i + 1) * 0x9E3779B97F4A7C15) & M64) >> 1

def edges_for(shape, n, rng):
    E = set()
    if shape in ("er", "er_sid"):
        while len(E) < 4 * n:
            a, b = rng.randrange(n), rng.randrange(n)
            if a != b:
                E.add((a, b))
    elif shape == "pl":
        g = 2.2
        w = [(i + 1) ** (-1.0 / (g - 1)) for i in range(n)]
        cum, s = [], 0.0
        for x in w:
            s += x; cum.append(s)
        perm = list(range(n)); rng.shuffle(perm)       # hubs are not the low ids
        pick = lambda: perm[min(bisect.bisect_left(cum, rng.random() * s), n - 1)]
        tries = 0
        while len(E) < 4 * n and tries < 40 * n:
            a, b = pick(), pick(); tries += 1
            if a != b:
                E.add((a, b))
    elif shape == "grid":
        side = int(math.isqrt(n))
        for r in range(side):
            for c in range(side):
                v = r * side + c
                if c + 1 < side: E.add((v, v + 1)); E.add((v + 1, v))
                if r + 1 < side: E.add((v, v + side)); E.add((v + side, v))
    else:
        raise SystemExit("unknown shape " + shape)
    return sorted(E)

def main():
    shape, n, seed, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    rng = random.Random(f"{shape}-{n}-{seed}")
    E = edges_for(shape, n, rng)
    vid = scatter if shape == "er_sid" else (lambda i: i + 1)
    ids = {vid(i) for a, b in E for i in (a, b)}
    assert len(ids) == len({i for a, b in E for i in (a, b)}), "id collision"
    adj = {}
    with open(f"{out}/{shape}_{n}.tsv", "w") as f:
        for a, b in E:
            wgt = rng.randrange(100, 1001) / 100.0
            f.write(f"{vid(a)}\t{vid(b)}\t{wgt:.2f}\n")
            adj.setdefault(a, []).append(b)
    # Origins: K vertices with out-degree >= 1; destinations: reachable, chosen
    # uniformly from the origin's reachable set (so path queries terminate early
    # on average rather than always traversing everything).
    qrng = random.Random(f"q-{shape}-{n}-{seed}")
    srcs = sorted(adj)
    origins, dests = [], []
    while len(origins) < 16:
        o = qrng.choice(srcs)
        seen, dq = {o}, deque([o])
        while dq:
            u = dq.popleft()
            for v in adj.get(u, ()):
                if v not in seen:
                    seen.add(v); dq.append(v)
        if len(seen) < 2:
            continue
        d = qrng.choice(sorted(seen - {o}))
        origins.append(vid(o)); dests.append(vid(d))
    meta = dict(shape=shape, n=n, seed=seed, edges=len(E), vertices=len(ids),
                origins=origins, dests=dests)
    json.dump(meta, open(f"{out}/{shape}_{n}.queries.json", "w"))
    print(json.dumps({k: meta[k] for k in ("shape", "n", "edges", "vertices")}))

if __name__ == "__main__":
    main()
