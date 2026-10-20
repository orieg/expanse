#!/usr/bin/env python3
"""Bounds for `SyncExpanseStrMap::mem_used_estimate` (#1586, AGENTS.md §8.8 commit 1).

The change `docs/benchmarks/concurrency/METHODOLOGY.md` §43 pre-registers
counts the bytes of the string map's node shells (`Box<StrNode>`) and suffix
leaves (one global allocation each) at their allocation and free sites on a
shared map, into a per-writer-slot counter, so the estimate is a shard sum
and does not walk the tree. Each counted site is one *event*: a shell
allocated or freed, a suffix leaf allocated or freed. The Callgrind cost of
the change on an arm is then bounded by

    events(arm) * C_EVENT_IR + constructions(arm) * C_CONSTRUCT_IR

instructions, which `bound_pct` divides by the arm's base count.

What is computed here, and from what
------------------------------------
1. `events(arm)`: an exact count from a model of the string map's meta-trie
   (`StrModel`), run over the same keys, in the same order, that the arm's
   setup and measured region use. The model mirrors, in
   `crates/expanse/src/strmap.rs` at `9a27dd3e`: `chunk_at` (8-byte chunks,
   a chunk is terminal when fewer than 8 bytes remain), `insert_shared` and
   the optimistic T2 / T4 transitions (a new chunk publishes one suffix leaf
   holding the key's remainder; a diverging suffix is split by
   `build_split_child`, which allocates one shell and, unless the old
   remainder's first chunk is terminal, one shorter suffix, and then frees the
   old suffix), and `StrNode::remove_shared` with `prune_child_shared` (a
   removal frees a matched suffix leaf; an emptied child shell is freed, up to
   the first non-empty ancestor; an emptied root is freed). It counts only the
   measured region: setup events are replayed and then discarded.
   The key generators mirror `crates/expanse/benches/instructions.rs`
   (`str_keys`, `short_keys`, `uuid_keys_sorted`, `shuffled_bytes`) and
   `crates/expanse/benches/rmw_instructions.rs` (`str_keys`,
   `absent_str_keys`, `shuffled`).
2. `C_EVENT_IR`, the per-event instruction cost, is DERIVED, not measured
   (see the constant's comment). It is fixed here before any measurement.
3. `BASE_IR`: each arm's instruction count on `main`, from the PR-vs-main
   pass of run https://github.com/orieg/expanse/actions/runs/39394551086
   (job "Perf / Callgrind Deterministic Instructions", head `5b7ae9bf`, whose
   engine code is `main` at `9a27dd3e`; measured: GitHub-hosted x86_64
   runner, 5b7ae9bf).

What this does not do: predict the count of any arm that has no event (those
are gated at No change), model contention retries (one thread, so none), or
model index-node bytes, which `NodeAlloc` already counts.

Usage:
    python3 scripts/strmap_estimate_bounds.py             # the table, then the self-test
    python3 scripts/strmap_estimate_bounds.py --self-test # the self-test only
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass, field

CHUNK = 8
MASK64 = (1 << 64) - 1
POP = 50_000
UUID_POP = 20_000

# Per-event instruction cost, DERIVED (an upper bound with margin, not a
# measurement). One counted event is an out-of-line, non-generic call:
#   call site: two argument moves and the call                          3
#   helper: the deferred cell's `OnceLock` state load, compare, branch,
#           and the cell pointer load                                   4
#   the writer slot: one thread-local load, compare, branch              3
#   the shard address: shift and add                                    2
#   one `lock xadd` on the shard counter                                1
#   return                                                              1
#   a suffix's byte count (`SUFFIX_BYTES + len`)                        1
# Nominal 15. C_EVENT_IR allows 9 more for register moves around the call
# in the caller and a prologue in the helper.
C_EVENT_IR = 24
# A map constructed inside an arm's measured region also takes the
# accounting-cell handle once (`NodeAlloc::acct_cell`: a `OnceLock` load,
# compare, branch, store) and initialises one more counter in each of the 16
# accounting shards. DERIVED upper bound.
C_CONSTRUCT_IR = 64


class XorShift:
    """`XorShift` of both bench files: x ^= x << 13; x ^= x >> 7; x ^= x << 17."""

    def __init__(self, seed: int) -> None:
        if not 0 < seed <= MASK64:
            raise ValueError("seed must be a non-zero u64")
        self.x = seed

    def next(self) -> int:
        x = self.x
        x ^= (x << 13) & MASK64
        x ^= x >> 7
        x ^= (x << 17) & MASK64
        self.x = x
        return x


def splitmix64(z: int) -> int:
    """`uuid_keys_sorted`'s SplitMix64 step (Steele, Lea and Flood, OOPSLA 2014)."""
    z = (z + 0x9E3779B97F4A7C15) & MASK64
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
    return z ^ (z >> 31)


def shuffled(items: list) -> list:
    """`shuffled` / `shuffled_bytes`: Fisher–Yates with `XorShift(0x9E37_79B9)`."""
    out = list(items)
    rng = XorShift(0x9E37_79B9)
    for i in range(len(out) - 1, 0, -1):
        j = rng.next() % (i + 1)
        out[i], out[j] = out[j], out[i]
    return out


def route_keys(lo: int, hi: int) -> list[bytes]:
    """`str_keys` (indices 0..POP) and `absent_str_keys` (POP..2*POP)."""
    if not 0 <= lo <= hi:
        raise ValueError("need 0 <= lo <= hi")
    return [f"/api/v2/tenants/{i // 16:06}/resources/{i % 16:04}".encode() for i in range(lo, hi)]


def short_keys(pop: int = POP) -> list[bytes]:
    """`short_keys`: 8..=16 random alphanumerics, duplicates dropped in draw order."""
    alnum = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
    rng = XorShift(0x0DDB_1A5E_5EED_0730)
    seen: set[bytes] = set()
    out: list[bytes] = []
    while len(out) < pop:
        n = 8 + rng.next() % 9
        k = bytes(alnum[rng.next() % 62] for _ in range(n))
        if k not in seen:
            seen.add(k)
            out.append(k)
    return out


def uuid_keys_sorted(pop: int = UUID_POP) -> list[bytes]:
    """`uuid_keys_sorted`: canonical UUIDv4 strings, sorted ascending."""
    out = []
    for i in range(pop):
        hi = (splitmix64((i << 1) & MASK64) & 0xFFFF_FFFF_FFFF_0FFF) | 0x4000
        lo = (splitmix64(((i << 1) | 1) & MASK64) & 0x3FFF_FFFF_FFFF_FFFF) | (1 << 63)
        out.append(
            f"{hi >> 32:08x}-{(hi >> 16) & 0xFFFF:04x}-{hi & 0xFFFF:04x}-"
            f"{lo >> 48:04x}-{lo & 0xFFFF_FFFF_FFFF:012x}".encode()
        )
    out = sorted(set(out))
    if len(out) != pop:
        raise AssertionError("uuid_keys_sorted drew a duplicate")
    return out


def chunk_at(key: bytes, off: int) -> tuple[bytes, bool]:
    """`strmap::chunk_at`: the zero-padded chunk at `off`, and whether it is terminal."""
    rest = key[min(off, len(key)):]
    return rest[:CHUNK].ljust(CHUNK, b"\0"), len(rest) < CHUNK


@dataclass
class Events:
    shell_alloc: int = 0
    shell_free: int = 0
    sfx_alloc: int = 0
    sfx_free: int = 0

    @property
    def total(self) -> int:
        return self.shell_alloc + self.shell_free + self.sfx_alloc + self.sfx_free


@dataclass
class Node:
    # chunk -> ("t",) | ("s", remainder) | ("c", Node)
    map: dict = field(default_factory=dict)


class StrModel:
    """The meta-trie's shells and suffix leaves, counting events (see module doc)."""

    def __init__(self) -> None:
        self.root: Node | None = None
        self.ev = Events()

    def insert(self, key: bytes) -> None:
        if b"\0" in key:
            raise ValueError("keys are NUL-free")
        if self.root is None:
            self.root = Node()
            self.ev.shell_alloc += 1
        node, off = self.root, 0
        while True:
            chunk, terminal = chunk_at(key, off)
            if terminal:
                node.map.setdefault(chunk, ("t",))
                return
            e = node.map.get(chunk)
            rem = key[off + CHUNK:]
            if e is None:
                node.map[chunk] = ("s", rem)
                self.ev.sfx_alloc += 1
                return
            if e[0] == "s":
                if e[1] == rem:
                    return
                old = e[1]
                child = Node()
                self.ev.shell_alloc += 1
                c1, t1 = chunk_at(old, 0)
                if t1:
                    child.map[c1] = ("t",)
                else:
                    child.map[c1] = ("s", old[CHUNK:])
                    self.ev.sfx_alloc += 1
                node.map[chunk] = ("c", child)
                self.ev.sfx_free += 1
                node = child
            else:
                node = e[1]
            off += CHUNK

    def remove(self, key: bytes) -> bool:
        if self.root is None:
            return False
        node, off, path = self.root, 0, []
        while True:
            chunk, terminal = chunk_at(key, off)
            e = node.map.get(chunk)
            if terminal:
                if e is None:
                    return False
                del node.map[chunk]
                break
            if e is None:
                return False
            if e[0] == "s":
                if e[1] != key[off + CHUNK:]:
                    return False
                del node.map[chunk]
                self.ev.sfx_free += 1
                break
            path.append((node, chunk))
            node = e[1]
            off += CHUNK
        while path:
            parent, chunk = path.pop()
            child = parent.map[chunk][1]
            if child.map:
                break
            del parent.map[chunk]
            self.ev.shell_free += 1
        if not self.root.map:
            self.root = None
            self.ev.shell_free += 1
        return True


def measured(setup, body) -> Events:
    """Runs `setup` then `body` on one model; returns the events of `body` alone."""
    m = StrModel()
    setup(m)
    m.ev = Events()
    body(m)
    return m.ev


def _insert_all(keys):
    return lambda m: [m.insert(k) for k in keys]


def _churn(keys):
    def body(m):
        for k in keys:
            m.insert(k)
            m.remove(k)
            m.insert(k)
    return body


def _remove_all(keys):
    def body(m):
        for k in keys:
            if not m.remove(k):
                raise AssertionError("a probe of a remove arm was absent")
    return body


def nothing(m: StrModel) -> None:
    """An empty setup: the arm builds its map inside the measured region."""


def arm_events() -> dict[str, tuple[Events, int]]:
    """{arm: (events in the measured region, maps constructed in it)}."""
    routes = route_keys(0, POP)
    short = short_keys()
    return {
        # instructions.rs
        "sync_strmap_insert/routes": (measured(nothing, _insert_all(routes)), 1),
        "sync_strmap_insert_short/short": (measured(nothing, _insert_all(short)), 1),
        "sync_strmap_insert_sorted/uuid": (measured(nothing, _insert_all(uuid_keys_sorted())), 1),
        "sync_strmap_remove/routes": (measured(_insert_all(routes), _remove_all(shuffled(routes))), 0),
        "sync_strmap_churn/routes": (measured(_insert_all(routes), _churn(shuffled(routes))), 0),
        "sync_strmap_churn_short/short": (measured(_insert_all(short), _churn(shuffled(short))), 0),
        # overwrites of present keys only
        "sync_strmap_update/routes": (measured(_insert_all(routes), _insert_all(shuffled(routes))), 0),
        "sync_strmap_compare_exchange/routes": (
            measured(_insert_all(routes), _insert_all(shuffled(routes))), 0),
        # rmw_instructions.rs
        "sync_strmap_cas/routes": (measured(_insert_all(routes), _insert_all(shuffled(routes))), 0),
        "sync_strmap_update_rmw/routes": (
            measured(_insert_all(routes), _insert_all(shuffled(routes))), 0),
        "sync_strmap_cas_insert/routes": (
            measured(_insert_all(routes), _insert_all(shuffled(route_keys(POP, 2 * POP)))), 0),
    }


# Base instruction counts on `main` (see the module doc for the run).
BASE_IR = {
    "sync_strmap_insert/routes": 77_553_128,
    "sync_strmap_insert_short/short": 97_934_923,
    "sync_strmap_insert_sorted/uuid": 46_291_751,
    "sync_strmap_remove/routes": 89_033_733,
    "sync_strmap_churn/routes": 189_097_756,
    "sync_strmap_churn_short/short": 234_191_347,
    "sync_strmap_update/routes": 74_316_762,
    "sync_strmap_compare_exchange/routes": 123_749_450,
    "sync_strmap_cas/routes": 72_879_132,
    "sync_strmap_update_rmw/routes": 122_239_487,
    "sync_strmap_cas_insert/routes": 101_026_457,
}


def bound_ir(events: int, constructions: int, c_event: int = C_EVENT_IR,
             c_construct: int = C_CONSTRUCT_IR) -> int:
    """Upper bound on the instructions the change adds to an arm."""
    if events < 0 or constructions < 0 or c_event <= 0 or c_construct < 0:
        raise ValueError("counts must be non-negative and the event cost positive")
    return events * c_event + constructions * c_construct


def bound_pct(base_ir: int, events: int, constructions: int) -> float:
    """`bound_ir` as a percentage of the arm's base count."""
    if base_ir <= 0:
        raise ValueError("base count must be positive")
    return 100.0 * bound_ir(events, constructions) / base_ir


def render_table() -> str:
    rows = ["| arm | shell alloc | shell free | suffix alloc | suffix free | events | "
            "constructions | base Ir | bound Ir | bound |",
            "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for arm, (ev, cons) in arm_events().items():
        base = BASE_IR[arm]
        b = bound_ir(ev.total, cons)
        rows.append(
            f"| `{arm}` | {ev.shell_alloc:,} | {ev.shell_free:,} | {ev.sfx_alloc:,} | "
            f"{ev.sfx_free:,} | {ev.total:,} | {cons} | {base:,} | {b:,} | "
            f"+{bound_pct(base, ev.total, cons):.3f}% |")
    return "\n".join(rows)


def self_test() -> int:
    # XorShift(1), first output, by hand: 1 ^ 1<<13 = 8193; ^ 8193>>7 (64) = 8257;
    # ^ 8257<<17 (1,082,261,504) = 1,082,269,761.
    assert XorShift(1).next() == 1_082_269_761
    # SplitMix64's first output from state 0 (the reference implementation's
    # well-known value).
    assert splitmix64(0) == 0xE220A8397B1DCDAF
    assert chunk_at(b"12345678", 0) == (b"12345678", False)
    assert chunk_at(b"12345678", 8) == (b"\0" * 8, True)
    assert chunk_at(b"1234567", 0)[1]

    # Two keys of one tenant, by hand (METHODOLOGY §43.3): the first publishes
    # the root shell and one suffix; the second splits four times down the
    # chain "/api/v2/", "tenants/", "000000/r", "esources", allocating four
    # shells and three shorter suffixes and freeing four suffixes; the last
    # split's continuation "/0000" is terminal.
    m = StrModel()
    for k in route_keys(0, 2):
        m.insert(k)
    assert (m.ev.shell_alloc, m.ev.sfx_alloc, m.ev.sfx_free, m.ev.shell_free) == (5, 4, 4, 0)
    # Removing both empties every node: five shells freed, no suffix left.
    for k in route_keys(0, 2):
        assert m.remove(k)
    assert (m.ev.shell_free, m.ev.sfx_free) == (5, 4)
    assert m.root is None
    # A tenant after the first: its first key publishes one suffix under the
    # shared "tenants/" child, its second splits twice.
    m = StrModel()
    for k in route_keys(0, 16):
        m.insert(k)
    m.ev = Events()
    for k in route_keys(16, 32):
        m.insert(k)
    assert (m.ev.shell_alloc, m.ev.sfx_alloc, m.ev.sfx_free) == (2, 2, 2)
    # A removed and reinserted suffix leaf is two events.
    m = StrModel()
    m.insert(b"abcdefghij")
    m.insert(b"zbcdefghij")
    m.ev = Events()
    assert m.remove(b"abcdefghij")
    m.insert(b"abcdefghij")
    assert m.ev.total == 2
    # An 8-byte key's first chunk is not terminal: it is a zero-length suffix.
    m = StrModel()
    m.insert(b"ABCDEFGH")
    assert m.ev.sfx_alloc == 1

    ev = arm_events()
    # Hand-derived (§43.3): 3,125 tenants of 16 keys; tenant 0 costs 13 events
    # and every other tenant 6, so 13 + 3,124 * 6 = 18,757.
    assert ev["sync_strmap_insert/routes"][0].total == 18_757
    # The built routes tree holds root, "/api/v2/", "tenants/" and two shells
    # per tenant, 3 + 2 * 3,125 = 6,253; removing every key frees them all.
    assert ev["sync_strmap_remove/routes"][0].total == 6_253
    # A churn of present route keys leaves every child with 15 other keys.
    assert ev["sync_strmap_churn/routes"][0].total == 0
    # Overwrites only.
    for arm in ("sync_strmap_update/routes", "sync_strmap_compare_exchange/routes",
                "sync_strmap_cas/routes", "sync_strmap_update_rmw/routes"):
        assert ev[arm][0].total == 0, arm
    # The absent half: tenants 3,125..6,249 under the existing "tenants/" child.
    assert ev["sync_strmap_cas_insert/routes"][0].total == 3_125 * 6
    # Model outputs, pinned so a change to a generator or the model shows.
    assert ev["sync_strmap_insert_short/short"][0].total == 50_001
    assert ev["sync_strmap_churn_short/short"][0].total == 100_000
    assert ev["sync_strmap_insert_sorted/uuid"][0].total == 20_001

    assert bound_ir(10, 1) == 10 * C_EVENT_IR + C_CONSTRUCT_IR
    for bad in ((-1, 0), (0, -1)):
        try:
            bound_ir(*bad)
        except ValueError:
            pass
        else:
            raise AssertionError("bound_ir accepted a negative count")
    try:
        bound_pct(0, 1, 0)
    except ValueError:
        pass
    else:
        raise AssertionError("bound_pct accepted a zero base")
    assert set(BASE_IR) == set(ev)
    print("strmap_estimate_bounds.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if not args.self_test:
        print(render_table())
    return self_test()


if __name__ == "__main__":
    sys.exit(main())
