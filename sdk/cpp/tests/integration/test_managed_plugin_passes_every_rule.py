# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""A C++ Author Serves Every Protocol Rule as a Managed Plugin."""

# === Imports === #

# Standard Library Imports
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from tests.support import host

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ManagedPlugin(unittest.TestCase):
    def test_managed_plugin_passes_every_rule(self) -> None:
        # Start the Managed Plugin and Run Every Reference-Host Case Through Stop.
        result = host.run_managed()

        # Check Every Rule and the Managed Process's Successful Cleanup and Exit.
        self.assertEqual(result.returncode, 0, result.stdout)
        rows = [line for line in result.stdout.splitlines() if line.endswith("| PASS |")]
        summary = re.search(r"(\d+)/(\d+) Cases Passed", result.stdout)
        self.assertIsNotNone(summary, result.stdout)
        assert summary is not None
        self.assertEqual(int(summary[1]), int(summary[2]), result.stdout)
        self.assertEqual(len(rows), int(summary[2]), result.stdout)


class ManagedHostLauncher(unittest.TestCase):
    def test_explicit_artifacts_use_the_shared_launcher(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        executable = root / "host"
        plugin = root / "plugin"
        executable.touch()
        plugin.touch()
        environment = patch.dict(os.environ, {"HARMOS_ADDRESS": "inherited-address"})
        environment.start()
        self.addCleanup(environment.stop)
        launcher = patch(
            "tests.support.host.subprocess.run",
            return_value=subprocess.CompletedProcess([], 0, ""),
        )
        run = launcher.start()
        self.addCleanup(launcher.stop)

        result = host.run_managed(host=executable, plugin=plugin, timeout_s=73)

        self.assertEqual(result.returncode, 0)
        self.assertEqual(run.call_args.args[0], [str(executable), "--plugin", str(plugin)])
        self.assertNotIn("HARMOS_ADDRESS", run.call_args.kwargs["env"])
        self.assertEqual(run.call_args.kwargs["timeout"], 73)

    def test_missing_explicit_plugin_does_not_launch(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        executable = root / "host"
        executable.touch()
        launcher = patch("tests.support.host.subprocess.run")
        run = launcher.start()
        self.addCleanup(launcher.stop)

        self.assertRaises(
            FileNotFoundError,
            host.run_managed,
            host=executable,
            plugin=root / "missing",
        )

        run.assert_not_called()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
