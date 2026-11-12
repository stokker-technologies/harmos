# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Select Changed Stories And Product Consumers Without Building Other Targets."""

# === Imports === #

# Standard Library Imports
import json
import tempfile
import tomllib
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

# Third Party Imports

# Local Imports
from harmos_tools.test import focused, policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class TestSelection(unittest.TestCase):
    def test_package_runs_system_serially_and_propagates_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            targets = [{"kind": ["test"], "name": name} for name in ("integration", "system")]
            metadata = {"packages": [{"name": "model", "targets": targets}]}
            rules = policy.Policy({}, dict(policy.DEADLINES), ())
            results = [
                SimpleNamespace(returncode=0, stdout=json.dumps(metadata), stderr=""),
                SimpleNamespace(returncode=0),
                SimpleNamespace(returncode=7),
            ]

            def run(command, **options):
                result = results.pop(0)
                if "--config-file" in command:
                    config = tomllib.loads(Path(command[command.index("--config-file") + 1]).read_text())
                    junit = Path(config["store"]["dir"]) / "project/junit.xml"
                    junit.parent.mkdir(parents=True, exist_ok=True)
                    junit.write_text('<testsuites><testsuite><testcase name="story"/></testsuite></testsuites>')
                return result

            with (
                patch.object(focused.policy, "load", return_value=rules),
                patch.object(focused.subprocess, "run", side_effect=run) as execute,
            ):
                result = focused.run(directory, package="model", exact="case")
            self.assertEqual(result, 7)
            commands = [call.args[0] for call in execute.call_args_list]
            self.assertIn("--no-deps", commands[0])
            self.assertNotIn("--test-threads", commands[1])
            self.assertEqual(commands[2][-5:], ["--test-threads", "1", "--", "case", "--exact"])

            # Empty Intermediate Binaries Cannot Turn an Empty Selection Into Proof.
            def empty(command, **options):
                if "metadata" in command:
                    return SimpleNamespace(returncode=0, stdout=json.dumps(metadata), stderr="")
                config = tomllib.loads(Path(command[command.index("--config-file") + 1]).read_text())
                junit = Path(config["store"]["dir"]) / "project/junit.xml"
                junit.parent.mkdir(parents=True, exist_ok=True)
                junit.write_text("<testsuites/>")
                return SimpleNamespace(returncode=0)

            with patch.object(focused.policy, "load", return_value=rules), patch.object(focused.subprocess, "run", side_effect=empty):
                with self.assertRaises(policy.TestError):
                    focused.run(directory, package="model")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
