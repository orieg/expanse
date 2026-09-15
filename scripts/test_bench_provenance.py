#!/usr/bin/env python3
"""Unit tests for `bench_provenance.py`, pinning the jiffy arithmetic (#732).

The busy-CPU figure is the one number in the provenance block that is computed
rather than read, and it is the one a reader uses to decide whether a run was
contaminated. `python3 -m unittest test_bench_provenance -v`, from `scripts/`.
"""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bench_provenance as bp  # noqa: E402


def snap(busy, total):
    return {"stat_busy_jiffies": busy, "stat_total_jiffies": total}


class BusyCpus(unittest.TestCase):
    """`(busy - prev.busy) / (total - prev.total) * ncpu`, in core-equivalents."""

    def test_fully_idle_host_reads_zero_cores(self):
        # 1000 jiffies passed on a 24-CPU host, none of them busy.
        self.assertEqual(bp.busy_cpus(snap(0, 0), 0, 24_000, ncpu=24), 0.0)

    def test_one_busy_core_of_twenty_four_reads_one(self):
        # A single-threaded phase on a quiet host: one CPU's worth of jiffies.
        # Over 1000 ticks of wall time a 24-CPU host accrues 24_000 total.
        self.assertEqual(bp.busy_cpus(snap(0, 0), 1_000, 24_000, ncpu=24), 1.0)

    def test_sixteen_busy_cores_reads_its_own_thread_count(self):
        # The concurrent sweep's 16 threads, which is what makes the figure
        # readable: it reports the sweep, not a mysterious load.
        self.assertEqual(bp.busy_cpus(snap(0, 0), 16_000, 24_000, ncpu=24), 16.0)

    def test_fully_saturated_host_reads_the_cpu_count(self):
        self.assertEqual(bp.busy_cpus(snap(0, 0), 24_000, 24_000, ncpu=24), 24.0)

    def test_deltas_are_differenced_against_the_previous_snapshot(self):
        # Absolute counters, not rates: only the interval between two snapshots
        # is reported, so a long-idle uptime does not dilute a busy phase.
        self.assertEqual(
            bp.busy_cpus(snap(500_000, 5_000_000), 502_000, 5_048_000, ncpu=24), 1.0
        )

    def test_rounding_is_to_two_places(self):
        self.assertEqual(bp.busy_cpus(snap(0, 0), 1_234, 24_000, ncpu=24), 1.23)

    def test_no_previous_snapshot_is_none_not_zero(self):
        # The first snapshot of a run has nothing to difference against. Zero
        # would read as "the host was idle", which is a claim, not an absence.
        self.assertIsNone(bp.busy_cpus(None, 1_000, 24_000, ncpu=24))
        self.assertIsNone(bp.busy_cpus({}, 1_000, 24_000, ncpu=24))

    def test_off_linux_is_none(self):
        self.assertIsNone(bp.busy_cpus(snap(0, 0), None, None, ncpu=24))
        self.assertIsNone(bp.busy_cpus(snap(None, None), 1_000, 24_000, ncpu=24))

    def test_no_time_passed_is_none_not_a_division_by_zero(self):
        self.assertIsNone(bp.busy_cpus(snap(0, 24_000), 0, 24_000, ncpu=24))
        self.assertIsNone(bp.busy_cpus(snap(0, 25_000), 0, 24_000, ncpu=24))


class MinimumWindow(unittest.TestCase):
    """A window too short to resolve has no busy-CPU number (section 8.1).

    Two snapshots taken back to back differ by rounding over a wall time of
    almost nothing, and the quotient is then any value at all: an own figure
    below zero and a foreign remainder above the 1.0 void boundary.
    """

    def test_the_minimum_is_ten_jiffies(self):
        self.assertEqual(bp.MIN_WINDOW_JIFFIES, 10)
        self.assertAlmostEqual(bp.MIN_WINDOW_S, 10 / bp.USER_HZ)
        self.assertGreater(bp.USER_HZ, 0)

    def test_host_busy_below_the_minimum_is_none(self):
        # 24 CPUs accrue 24 total jiffies per jiffy of wall time.
        below = 24 * bp.MIN_WINDOW_JIFFIES - 1
        self.assertIsNone(bp.busy_cpus(snap(0, 0), 0, below, ncpu=24))
        self.assertIsNone(bp.busy_cpus(snap(0, 0), 1, 24, ncpu=24))  # one jiffy: read 24.0 before

    def test_host_busy_at_the_minimum_is_a_number(self):
        at = 24 * bp.MIN_WINDOW_JIFFIES
        self.assertEqual(bp.busy_cpus(snap(0, 0), bp.MIN_WINDOW_JIFFIES, at, ncpu=24), 1.0)

    def test_own_below_the_minimum_is_none_and_so_is_foreign(self):
        prev = {"child_cpu_s": 68.39, "monotonic_s": 100.0}
        # Sub-millisecond window with the stored operand's rounding in it.
        own = bp.own_busy_cpus(prev, 68.3895, 100.0006)
        self.assertIsNone(own)
        self.assertIsNone(bp.foreign_busy_cpus(0.0, own))
        self.assertIsNone(bp.own_busy_cpus(prev, 68.39, 100.0 + bp.MIN_WINDOW_S * 0.99))

    def test_own_at_and_above_the_minimum_is_a_number(self):
        prev = {"child_cpu_s": 10.0, "monotonic_s": 100.0}
        self.assertEqual(bp.own_busy_cpus(prev, 11.0, 101.0), 1.0)
        # Just above the minimum (1% over, clear of float round-off).
        just = bp.MIN_WINDOW_S * 1.01
        self.assertIsNotNone(bp.own_busy_cpus(prev, 10.0 + just, 100.0 + just))
        self.assertEqual(bp.foreign_busy_cpus(1.5, 1.0), 0.5)

    def test_the_published_shapes_carry_no_figures(self):
        # The two shapes phase snapshots published: stored fields rounded to
        # 1 ms, differenced against unrounded readings over a sub-millisecond
        # window. Stored child 68.39 is 0.4 ms below the reading it came from,
        # stored clock 100.0 is 0.4 ms below its own, and 0.15 ms later...
        prev = {"child_cpu_s": 68.39, "monotonic_s": 100.0,
                "stat_busy_jiffies": 5_000, "stat_total_jiffies": 50_000}
        child_now, mono_now = 68.3905, 100.00055
        # ... the grid offset alone is 0.5 ms / 0.55 ms = 0.91 core, and with a
        # jiffy total that did not move the host figure is 0.0 (foreign = -own)
        # or, with nothing to difference, None (a numeric own beside a null
        # foreign). Neither may carry a number.
        self.assertIsNone(bp.own_busy_cpus(prev, child_now, mono_now))
        self.assertIsNone(bp.busy_cpus(prev, 5_000, 50_001, ncpu=24))
        self.assertIsNone(bp.foreign_busy_cpus(0.0, bp.own_busy_cpus(prev, child_now, mono_now)))
        self.assertIsNone(bp.foreign_busy_cpus(None, bp.own_busy_cpus(prev, child_now, mono_now)))

    def test_a_snapshot_is_differenced_on_its_unrounded_readings(self):
        # Stored fields rounded in opposite directions: child 0.4 ms down, clock
        # 0.4 ms up. Raw window 0.11 s (clear of the minimum), raw child delta
        # 0.11 s: 1.0 core. Differenced on the stored fields it reads
        # 0.1104 / 0.1096 = 1.0073, published as 1.01.
        prev = bp.Snapshot({"child_cpu_s": 10.0, "monotonic_s": 101.0},
                           raw={"child_cpu_s": 10.0004, "monotonic_s": 100.9996})
        self.assertEqual(bp.own_busy_cpus(prev, 10.1104, 101.1096), 1.0)
        self.assertEqual(bp.own_busy_cpus(dict(prev), 10.1104, 101.1096), 1.01)
        # The raw readings never reach the artifact.
        self.assertEqual(json.loads(json.dumps(prev)), {"child_cpu_s": 10.0, "monotonic_s": 101.0})

    def test_load_snapshot_carries_its_raw_readings(self):
        s = bp.load_snapshot("start")
        self.assertIsInstance(s, bp.Snapshot)
        self.assertEqual(round(s.raw["monotonic_s"], 3), s["monotonic_s"])
        self.assertNotIn("raw", json.loads(json.dumps(s)))

    def test_back_to_back_snapshots_carry_no_figures(self):
        # The production call path: `begin_cell` twice with nothing between.
        prov = {"loads": []}
        bp.begin_cell(prov, "first")
        second = bp.begin_cell(prov, "second")
        for k in ("busy_cpus_since_prev", "own_busy_cpus_since_prev", "foreign_busy_cpus_since_prev"):
            self.assertIsNone(second[k], f"{k} = {second[k]} over a back-to-back window")
        load = bp.end_cell(second)
        for k in ("busy_cpus_since_prev", "own_busy_cpus", "foreign_busy_cpus"):
            self.assertIsNone(load[k], f"end_cell {k} = {load[k]} over a back-to-back window")

    def test_a_long_enough_window_still_attributes(self):
        # The same call path over a window above the minimum, with the start
        # snapshot's clock moved back rather than a sleep in a unit test. A
        # plain-dict copy, as read back from an artifact, so the stored fields
        # are the ones differenced.
        start = dict(bp.load_snapshot("start"))
        start["monotonic_s"] -= 1.0
        if start["stat_total_jiffies"] is not None:
            start["stat_total_jiffies"] -= (bp.os.cpu_count() or 1) * bp.USER_HZ
        load = bp.end_cell(start)
        if start["child_cpu_s"] is not None:
            self.assertIsNotNone(load["own_busy_cpus"])
        if start["stat_total_jiffies"] is not None and start["child_cpu_s"] is not None:
            self.assertIsNotNone(load["busy_cpus_since_prev"])
            self.assertIsNotNone(load["foreign_busy_cpus"])


class CpuJiffies(unittest.TestCase):
    def test_reads_a_pair_or_a_pair_of_nones(self):
        busy, total = bp.cpu_jiffies()
        if busy is None:
            self.assertIsNone(total)
        else:
            self.assertIsInstance(busy, int)
            self.assertGreater(total, 0)
            self.assertLessEqual(busy, total)


class RawRounds(unittest.TestCase):
    def test_keeps_the_requested_keys_and_the_round_index(self):
        rows = [
            {"round": 0, "first_arm": "hot", "hot_ns_per_op": 1.5, "noise": "x"},
            {"round": 1, "first_arm": "expanse", "hot_ns_per_op": 1.6, "noise": "y"},
        ]
        out = bp.raw_rounds(rows, ("first_arm", "hot_ns_per_op"))
        self.assertEqual(out, [
            {"round": 0, "first_arm": "hot", "hot_ns_per_op": 1.5},
            {"round": 1, "first_arm": "expanse", "hot_ns_per_op": 1.6},
        ])

    def test_a_missing_key_is_none_never_dropped(self):
        # A dropped key would make a raw row look complete when it is not.
        out = bp.raw_rounds([{"round": 0}], ("first_arm",))
        self.assertEqual(out, [{"round": 0, "first_arm": None}])


class AttachAndBody(unittest.TestCase):
    def test_an_object_payload_gains_a_provenance_key(self):
        got = bp.attach({"cells": [1, 2]}, {"suite": "s"})
        self.assertEqual(got["cells"], [1, 2])
        self.assertEqual(got["provenance"], {"suite": "s"})

    def test_an_array_payload_is_wrapped(self):
        got = bp.attach([1, 2, 3], {"suite": "s"})
        self.assertEqual(got, {"provenance": {"suite": "s"}, "cells": [1, 2, 3]})

    def test_body_round_trips_both_shapes(self):
        self.assertEqual(bp.body(bp.attach([1, 2], {"suite": "s"})), [1, 2])
        self.assertEqual(bp.body([1, 2]), [1, 2])
        # An object payload that happens to have a `cells` key but no
        # provenance is returned whole, not unwrapped.
        self.assertEqual(bp.body({"cells": [1]}), {"cells": [1]})

    def test_attach_does_not_mutate_the_payload(self):
        payload = {"cells": [1]}
        bp.attach(payload, {"suite": "s"})
        self.assertNotIn("provenance", payload)


class Estimators(unittest.TestCase):
    def test_the_block_says_the_ratio_is_not_the_quotient_of_the_columns(self):
        e = bp.estimators("mean(A rounds) / mean(B rounds), two-sample BCa 95%")
        self.assertIn("mean(A rounds)", e["ratio"])
        self.assertIn("not the quotient", e["columns"])
        self.assertIn("rounds_raw", e["raw"])


class GitSha(unittest.TestCase):
    def test_the_environment_override_wins(self):
        import os
        old = os.environ.get("EXPANSE_BENCH_COMMIT")
        os.environ["EXPANSE_BENCH_COMMIT"] = "cafef00d"
        try:
            self.assertEqual(bp.git_sha(), "cafef00d")
        finally:
            if old is None:
                del os.environ["EXPANSE_BENCH_COMMIT"]
            else:
                os.environ["EXPANSE_BENCH_COMMIT"] = old

    def test_a_checkout_without_git_says_unknown_not_a_plausible_sha(self):
        import os
        old = os.environ.pop("EXPANSE_BENCH_COMMIT", None)
        try:
            with tempfile.TemporaryDirectory() as d:
                self.assertEqual(bp.git_sha(d), "unknown")
        finally:
            if old is not None:
                os.environ["EXPANSE_BENCH_COMMIT"] = old


class HostFacts(unittest.TestCase):
    def test_every_declared_field_is_present_even_off_linux(self):
        f = bp.host_facts()
        for k in ("cpu_model", "cpus_online", "cpu_core_cpus", "cpu_atom_cpus",
                  "cpu0_thread_siblings", "scaling_driver", "scaling_governor",
                  "transparent_hugepage", "platform"):
            self.assertIn(k, f, f"host_facts() dropped {k}")
        self.assertTrue(f["cpu_model"])

    def test_it_serialises(self):
        json.dumps(bp.host_facts())


if __name__ == "__main__":
    unittest.main()
