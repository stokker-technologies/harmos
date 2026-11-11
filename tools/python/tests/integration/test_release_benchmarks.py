# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Retain Native Benchmark Measurements For Nightly Even When The Runner Fails"""

# === Imports === #

# Standard Library Imports
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.test import run
from harmos_tools.release import reports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class BenchmarkRelease(unittest.TestCase):
    def test_native_measurements_survive_runner_failure_and_baseline_refresh(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            rules = run.policy.Policy(limits={}, deadlines={}, overrides=(), bench_regression_percent=10)
            with (
                patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}),
                patch.object(run.policy, "load", return_value=rules),
                patch.object(run, "_preflight"),
                patch.object(run.archive, "_toolchain", return_value={}),
                patch.object(run.archive, "revision", return_value="a" * 40),
                patch.object(run, "_bench_rust", return_value=([("read", 120.0, 100.0), ("new", 40.0, None)], 1)),
            ):
                self.assertEqual(run.bench(directory, "unit", "rust", ".", True, []), 1)
            document = json.loads((directory / "target/benchmarks/benchmarks-unit-rust.json").read_text())
            self.assertEqual(
                document,
                {
                    "benchmarks": [
                        {
                            "name": "read",
                            "suite": "unit-rust",
                            "unit": "ns",
                            "value": 120.0,
                            "baseline_value": 100.0,
                            "revision": "a" * 40,
                            "threshold_percent": 10,
                        },
                        {
                            "name": "new",
                            "suite": "unit-rust",
                            "unit": "ns",
                            "value": 40.0,
                            "baseline_value": None,
                            "revision": "a" * 40,
                            "threshold_percent": 10,
                        },
                    ]
                },
            )
            with self.assertRaises(run.policy.TestError):
                with (
                    patch.object(run.policy, "load", return_value=rules),
                    patch.object(run, "_preflight"),
                    patch.object(run.archive, "_toolchain", return_value={}),
                    patch.object(run, "_bench_rust", return_value=([], 1)),
                ):
                    run.bench(directory, "unit", "rust", ".", False, [])
            self.assertEqual(json.loads((directory / "target/benchmarks/benchmarks-unit-rust.json").read_text()), {"benchmarks": []})
            # Native Values Must Remain Finite In Published JSON.
            for value, baseline in ((float("nan"), 1.0), (1.0, float("inf"))):
                with self.assertRaises(reports.policy.ToolError):
                    reports.measurements([("invalid", value, baseline)], "unit-rust", "a" * 40, 10)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
