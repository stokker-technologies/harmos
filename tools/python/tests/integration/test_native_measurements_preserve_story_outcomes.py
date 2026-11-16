# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Retain Native Story Results Independently of Reported Measurements."""

# === Imports === #

# Standard Library Imports
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.test import coverage, evidence, execution, policy, report

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class NativeMeasurements(unittest.TestCase):
    def test_measurements_preserve_results_and_native_evidence(self) -> None:
        for language in ("python", "typescript", "javascript"):
            # Prepare the Lane
            temporary = tempfile.TemporaryDirectory()
            self.addCleanup(temporary.cleanup)
            directory = Path(temporary.name).resolve()
            (directory / "tools").mkdir()
            (directory / "tools/project.toml").write_text("", encoding="utf-8")
            rules = policy.load(directory)
            seconds = rules.deadline("unit", coverage=True) + 1
            key = f"unit-{language}"

            # Consume Native Evidence
            scratch_directory = evidence.scratch(directory, key, "dev")
            scratch = scratch_directory.path
            try:
                junit = scratch / "junit.xml"
                document = (
                    f'<testsuite name="fixture"><testcase name="passes" time="{seconds}"/>'
                    '<testcase name="skips"><skipped/></testcase></testsuite>'
                )
                junit.write_text(document, encoding="utf-8")
                environment = {coverage.VARIABLE: "1"}
                with patch.object(execution, "execute", return_value=0):
                    result = evidence.collect(
                        directory, rules, key, "unit", language, ".", "dev", [], directory, environment, junit, scratch
                    )
            finally:
                scratch_directory.close()

            # Check the Retained Results
            self.assertEqual((result["outcome"], result["passed"], result["failed"], result["skipped"]), ("passed", 1, 0, 1))
            self.assertEqual(result["durations"], [("fixture", "passes", directory.name, seconds)])
            destination = report.location(directory, "dev", key)
            self.assertEqual((destination / "junit.xml").read_text(), document)
            expected = {"fixture": {"passes": {"seconds": seconds}}}
            self.assertEqual(json.loads((destination / "durations.json").read_text()), expected)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
