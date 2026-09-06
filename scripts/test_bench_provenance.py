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
