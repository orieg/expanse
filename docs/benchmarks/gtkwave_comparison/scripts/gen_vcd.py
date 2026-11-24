#!/usr/bin/env python3
"""Deterministic synthetic VCD for the gtkwave drop-in evaluation.

The file exercises the three Judy uses on gtkwave's conversion path:

* ``vcd2fst`` keys every scope name in a JudySL array (one insert per
  ``$scope``, one lookup per ``$var`` re-scoped by an alias);
* ``fstapi.c``'s writer hashes every signal's compressed value-change block
  in a JudyHS array to find dynamic aliases, so many signals carry an
  identical waveform on purpose (``--dup-fraction``);
* the same identifier code declared under two scopes is a static alias.

Usage: gen_vcd.py SCOPES SIGNALS_PER_SCOPE STEPS SEED > out.vcd

The generator is xorshift64, so the output depends only on the arguments.
"""

import sys

MASK = (1 << 64) - 1


class XorShift64:
    def __init__(self, seed: int) -> None:
        if seed == 0:
            raise ValueError("seed must be non-zero")
        self.s = seed & MASK

    def next(self) -> int:
        x = self.s
        x ^= (x << 13) & MASK
        x ^= x >> 7
        x ^= (x << 17) & MASK
        self.s = x
        return x


def ident(n: int) -> str:
    """VCD identifier code from printable ASCII 33..126."""
    out = []
    n += 1
    while n:
        n -= 1
        out.append(chr(33 + n % 94))
        n //= 94
    return "".join(out)


def main() -> None:
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    scopes, per_scope, steps, seed = (int(a, 0) for a in sys.argv[1:])
    rng = XorShift64(seed)
    w = sys.stdout.write
    w("$date synthetic $end\n$version gen_vcd.py $end\n$timescale 1ns $end\n")
    w("$scope module top $end\n")
    sigs = []  # (ident, width, source index or None)
    n = 0
    for s in range(scopes):
        # three-level hierarchy: top.cluster<a>.core<b>.unit<s>
        a, b = s // 64, (s // 8) % 8
        w(f"$scope module cluster{a} $end\n$scope module core{b} $end\n")
        w(f"$scope module unit{s} $end\n")
        for k in range(per_scope):
            width = 1 if rng.next() % 4 else 1 + rng.next() % 32
            src = None
            if sigs and rng.next() % 10 < 3:
                # 30% of signals copy an earlier signal's waveform: the
                # writer's dynamic-alias hash should find them.
                cand = sigs[rng.next() % len(sigs)]
                if cand[1] == width:
                    src = cand[0] if cand[2] is None else cand[2]
            code = ident(n)
            n += 1
            kind = "wire" if width == 1 else "reg"
            w(f"$var {kind} {width} {code} sig{k} $end\n")
            sigs.append((code, width, src))
        if s and s % 16 == 0:
            # static alias: re-declare an earlier identifier in this scope
            code, width, _ = sigs[rng.next() % len(sigs)]
            kind = "wire" if width == 1 else "reg"
            w(f"$var {kind} {width} {code} alias $end\n")
        w("$upscope $end\n$upscope $end\n$upscope $end\n")
    w("$upscope $end\n$enddefinitions $end\n")

    state = {}

    def emit(code: str, width: int, val: int) -> None:
        if width == 1:
            w(f"{val & 1}{code}\n")
        else:
            w(f"b{val:b} {code}\n")

    primaries = [x for x in sigs if x[2] is None]
    copies = [x for x in sigs if x[2] is not None]
    w("#0\n$dumpvars\n")
    for code, width, _ in primaries:
        state[code] = 0
        emit(code, width, 0)
    for code, width, src in copies:
        emit(code, width, 0)
    w("$end\n")
    by_src = {}
    for code, width, src in copies:
        by_src.setdefault(src, []).append((code, width))
    for t in range(1, steps + 1):
        w(f"#{t * 10}\n")
        for code, width, _ in primaries:
            if rng.next() % 8 == 0:
                v = rng.next() & ((1 << width) - 1)
                state[code] = v
                emit(code, width, v)
                for c2, w2 in by_src.get(code, ()):
                    emit(c2, w2, v)


if __name__ == "__main__":
    main()
