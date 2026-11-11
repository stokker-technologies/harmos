# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prove Focused Cargo Dispatch Without Invoking A Compiler."""

# === Imports === #

# Standard Library Imports
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.build import cargo

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class FocusedCargoTests(unittest.TestCase):
    def test_dispatches_only_requested_operation_after_preflight(self):
        directory = Path.cwd()
        operations = ("build", "check", "test", "clippy", "run")
        for operation in operations:
            with self.subTest(operation=operation):
                arguments = ["--locked", "-p", "example"]
                if operation == "test":
                    arguments += ["--no-run", "--no-fail-fast"]
                events = []
                with (
                    patch.object(cargo.storage, "preflight", side_effect=lambda root, events=events: events.append(("preflight", root))),
                    patch.object(
                        cargo.subprocess,
                        "run",
                        side_effect=lambda *args, events=events, **kwargs: events.append((args, kwargs)) or SimpleNamespace(returncode=7),
                    ),
                ):
                    status = cargo.focused(directory, operation, arguments)
                self.assertEqual(status, 7)
                self.assertEqual(len(events), 2)
                self.assertEqual(events[0], ("preflight", directory))
                command, options = events[1]
                self.assertEqual(command, (["cargo", operation, *arguments],))
                self.assertEqual(options["cwd"], directory)
                self.assertEqual(options["env"]["CARGO_BUILD_WARNINGS"], "deny")

    def test_rejects_target_redirection_before_dispatch(self):
        with patch.object(cargo.storage, "preflight") as preflight, patch.object(cargo.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                cargo.focused(Path.cwd(), "build", ["--target-dir", "elsewhere"])
        preflight.assert_not_called()
        run.assert_not_called()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
