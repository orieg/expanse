import pytest
from amdahl import ratio, max_gain, share_needed

def test_reference_values():
    assert ratio(0.05, 2.0) == pytest.approx(1.05)      # 5% share, calls twice as slow
    assert ratio(0.05, 0.5) == pytest.approx(0.975)     # 5% share, calls twice as fast
    assert max_gain(0.03) == pytest.approx(0.03)
    assert share_needed(0.03, 2.0) == pytest.approx(0.03)
    assert share_needed(0.03, 0.5) == pytest.approx(0.06)  # halving the calls needs a 6% share to move 3%

def test_rejects_bad_inputs():
    for bad in (lambda: ratio(1.5, 1.0), lambda: ratio(0.1, -1.0), lambda: share_needed(0.1, 1.0)):
        with pytest.raises(ValueError):
            bad()
