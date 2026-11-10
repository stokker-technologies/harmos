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
import contextlib
import io
import os
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.build import cargo
from harmos_tools.ci import tasks, workflow

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class FocusedCargoTests(unittest.TestCase):
    def test_dispatches_only_requested_operation(self):
        directory = Path.cwd()
        operations = ("build", "check", "test", "clippy", "run")
        for operation in operations:
            with self.subTest(operation=operation):
                arguments = ["--locked", "-p", "example"]
                if operation == "test":
                    arguments += ["--no-run", "--no-fail-fast"]
                events = []
                with (
                    patch.object(
                        cargo.subprocess,
                        "run",
                        side_effect=lambda *args, events=events, **kwargs: events.append((args, kwargs)) or SimpleNamespace(returncode=7),
                    ),
                ):
                    status = cargo.focused(directory, operation, arguments)
                self.assertEqual(status, 7)
                self.assertEqual(len(events), 1)
                command, options = events[0]
                self.assertEqual(command, (["cargo", operation, *arguments],))
                self.assertEqual(options["cwd"], directory)
                self.assertEqual(options["env"]["CARGO_BUILD_WARNINGS"], "deny")

    def test_rejects_target_redirection_before_dispatch(self):
        with patch.object(cargo.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                cargo.focused(Path.cwd(), "build", ["--target-dir", "elsewhere"])
        run.assert_not_called()

    def test_entry_points_dispatch_focused_cargo_directly(self):
        selections = [
            (["build", "--package", "example", "--check"], "check", ["--locked", "--profile", "dev", "-p", "example"]),
            (["build", "--package", "example", "--profile", "release"], "build", ["--locked", "--profile", "release", "-p", "example"]),
            (["test", "--package", "example", "--target", "lib"], "test", ["--locked", "-p", "example", "--lib"]),
            (
                ["test", "--package", "example", "--target", "messages", "--exact", "case::name"],
                "test",
                ["--locked", "-p", "example", "--test", "messages", "case::name", "--", "--exact"],
            ),
        ]
        for arguments, operation, expected in selections:
            with self.subTest(arguments=arguments), patch.object(cargo, "focused", return_value=7) as focused:
                self.assertEqual(tasks.main(arguments), 7)
                focused.assert_called_once_with(Path.cwd(), operation, expected)

    def test_invalid_focused_selections_do_not_start_cargo(self):
        selections = [
            ["test", "--package", "example"],
            ["test", "--package", "example", "--target", "lib", "--coverage"],
            ["test", "--package", "example", "--target", "lib", "--profile", "release"],
            ["build", "--package", "example", "--artifact", "tests"],
        ]
        for arguments in selections:
            with self.subTest(arguments=arguments), patch.object(cargo, "focused") as focused, contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    tasks.main(arguments)
                self.assertEqual(error.exception.code, 2)
                focused.assert_not_called()

    def test_nightly_gate_and_report_preserve_run_ownership(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            environment = {
                "GITHUB_EVENT_NAME": "schedule",
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_SHA": "abc",
                "GITHUB_OUTPUT": str(output),
            }
            with (
                patch.dict(os.environ, environment),
                patch.object(workflow.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout="abc\n")),
            ):
                self.assertEqual(workflow.run("nightly-gate", [], "release"), 0)
            self.assertEqual(output.read_text(), "build=false\n")
            environment.update(RUN_URL="https://example.test/run/1", SUITE="failed")
            for body, updated in [("[1](https://example.test/run/1)", True), ("[2](https://example.test/run/2)", False)]:
                with (
                    patch.dict(os.environ, environment),
                    patch.object(workflow.policy, "root", return_value=Path(temporary)),
                    patch.object(workflow.policy, "policy", return_value={}),
                    patch.object(workflow.registry, "document", return_value={"id": 9, "body": body}) as document,
                ):
                    self.assertEqual(workflow.run("nightly-report", [], "release"), 0)
                    self.assertEqual(document.call_count, 2 if updated else 1)
                    if updated:
                        self.assertIn("failed", document.call_args.args[3]["body"])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
