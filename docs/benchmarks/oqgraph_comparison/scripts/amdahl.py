"""Amdahl bound for replacing OQGraph's bitset library.

If the bitset (library + wrapper) is a share `s` of a query's cost and the
replacement makes those calls cost `k` times as much, the query costs
R = 1 + s * (k - 1) times as much (Amdahl 1967, "Validity of the single
processor approach to achieving large scale computing capabilities",
AFIPS SJCC 30:483-485). The best case (k -> 0) bounds the gain at s.
"""

def ratio(share: float, k: float) -> float:
    if not 0.0 <= share <= 1.0:
        raise ValueError("share must be in [0, 1]")
    if k < 0.0:
        raise ValueError("k must be >= 0")
    return 1.0 + share * (k - 1.0)

def max_gain(share: float) -> float:
    """Largest fractional saving any replacement can deliver (k = 0)."""
    return 1.0 - ratio(share, 0.0)

def share_needed(delta: float, k: float) -> float:
    """Share at which a per-call ratio k moves the query by `delta` (|R-1|)."""
    if k == 1.0:
        raise ValueError("k == 1 never moves the query")
    return abs(delta / (k - 1.0))
