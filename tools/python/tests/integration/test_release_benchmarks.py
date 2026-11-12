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
from harmos_tools import storage_bench

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class BenchmarkRelease(unittest.TestCase):
    def test_ordinary_bench_runner_skips_large_storage_and_runs_other_integration_cases(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            messages = run.paths.output(directory, "build", "bench") / "bench/compiler.json"
            messages.parent.mkdir(parents=True)
            messages.write_text(
                "\n".join(
                    json.dumps(
                        {
                            "reason": "compiler-artifact",
                            "target": {"kind": ["bench"], "name": name},
                            "manifest_path": "crates/harmos-storage/Cargo.toml",
                            "executable": f"target/{name}",
                        }
                    )
                    for name in ("integration-multi-day", "integration-query")
                )
            )
            with patch.object(run.archive, "fixtures"), patch.object(run, "_measured", return_value=(0, None)) as execute:
                rows, status = run._bench_rust(directory, "integration", False, True, [], [], {})
            self.assertEqual((rows, status), ([], 0))
            self.assertEqual(execute.call_count, 1)
            self.assertEqual(Path(execute.call_args.args[0][0]).name, "integration-query")

    def test_storage_events_keep_units_platforms_and_repeated_contention_in_release_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            events = [
                "STORAGE_MEASUREMENT "
                + json.dumps(
                    {
                        "name": "storage.object.contend.8",
                        "seconds": seconds,
                        "allocations": allocations,
                        "allocated_bytes": allocations * 8,
                        "peak_heap_bytes": allocations * 2,
                    }
                )
                for seconds, allocations in [(0.006, 40), (0.002, 20), (0.004, 30)]
            ]
            events += [
                "STORAGE_COUNTER "
                + json.dumps(
                    {
                        "name": "storage.window.read.2m",
                        "bytes_read": 512,
                        "rows": 4096,
                        "payload_rows": 0,
                    }
                )
            ]
            for system in ("windows", "linux"):
                document = storage_bench.measurements(events, "revision", system)
                path = directory / "target/nightly-reports" / system / f"benchmarks-storage-{system}.json"
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps(document))
            shared = directory / "shared"
            shared.mkdir()
            reports.stage(directory, shared, "nightly")
            rows = json.loads((shared / "benchmarks.json").read_text())["benchmarks"]
            for system in ("windows", "linux"):
                cases = {row["name"]: row for row in rows if row["suite"] == f"storage-sqlite-{system}"}
                self.assertEqual(cases["storage.object.contend.8"]["value"], 0.004)
                self.assertEqual(cases["storage.object.contend.8"]["unit"], "s")
                self.assertEqual(cases["storage.object.contend.8.allocations"]["value"], 30)
                self.assertEqual(cases["storage.object.contend.8.throughput"]["value"], 2000)
                self.assertEqual(cases["storage.window.read.2m.read.bytes"]["value"], 512)
                self.assertEqual(cases["storage.window.read.2m.payload.rows"]["value"], 0)
            events.append(
                "STORAGE_MEASUREMENT "
                + json.dumps(
                    {
                        "name": "invalid",
                        "seconds": float("nan"),
                        "allocations": 1,
                        "allocated_bytes": 8,
                        "peak_heap_bytes": 2,
                    }
                )
            )
            with self.assertRaises(reports.policy.ToolError):
                storage_bench.measurements(events, "revision", "windows")

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
