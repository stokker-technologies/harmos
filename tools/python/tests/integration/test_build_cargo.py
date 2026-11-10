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
import json
import os
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.build import cargo
from harmos_tools.ci import artifacts, tasks, workflow
from harmos_tools.release import review

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
            (
                ["build", "--package", "example", "--features", "grpc,simulation", "--check"],
                "check",
                ["--locked", "--profile", "dev", "-p", "example", "--features", "grpc,simulation"],
            ),
            (
                ["build", "--package", "example", "--features", "grpc simulation"],
                "build",
                ["--locked", "--profile", "dev", "-p", "example", "--features", "grpc simulation"],
            ),
            (
                ["test", "--package", "example", "--features", "simulation", "--target", "lib", "--exact", "case::name"],
                "test",
                ["--locked", "-p", "example", "--lib", "--features", "simulation", "case::name", "--", "--exact"],
            ),
            (
                ["test", "--package", "example", "--features", "grpc,simulation", "--target", "messages"],
                "test",
                ["--locked", "-p", "example", "--test", "messages", "--features", "grpc,simulation"],
            ),
            (
                ["build", "--package", "example", "--example", "fixture", "--features", "guest", "--check"],
                "check",
                ["--locked", "--profile", "dev", "-p", "example", "--features", "guest", "--example", "fixture"],
            ),
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
            ["build", "--features", "grpc"],
            ["test", "--features", "simulation", "--target", "lib"],
            ["test", "--package", "example"],
            ["test", "--package", "example", "--target", "lib", "--coverage"],
            ["test", "--package", "example", "--target", "lib", "--profile", "release"],
            ["build", "--package", "example", "--artifact", "tests"],
            ["build", "--example", "fixture"],
        ]
        for arguments in selections:
            with self.subTest(arguments=arguments), patch.object(cargo, "focused") as focused, contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    tasks.main(arguments)
                self.assertEqual(error.exception.code, 2)
                focused.assert_not_called()

    def test_release_mode_task_exports_selected_squash(self):
        for mode, sha in (("tag", "a" * 40), ("prepare", ""), ("skip", "")):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "output"
                with (
                    patch.dict(os.environ, GITHUB_OUTPUT=str(output)),
                    patch.object(review, "github_release_mode", return_value={"mode": mode, "sha": sha}) as decision,
                ):
                    self.assertEqual(tasks.main(["ci:release-mode"]), 0)
                decision.assert_called_once_with()
                fields = dict(line.split("=", 1) for line in output.read_text().splitlines())
                self.assertEqual(fields, {"mode": mode, "sha": sha})

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


class WorkflowTaskTests(unittest.TestCase):
    def test_fixture_join_selects_existing_inputs_without_compiling(self):
        for profile in ("dev", "release"):
            with (
                self.subTest(profile=profile),
                patch.object(tasks, "_task", return_value=0) as task,
                patch.object(cargo, "focused") as focused,
            ):
                self.assertEqual(tasks.dispatch(["build", "--artifact", "fixtures", "--profile", profile]), 0)
                task.assert_called_once_with("prepare:tests:fixtures", "--profile", profile)
                focused.assert_not_called()
        with patch.object(tasks, "_task") as task:
            self.assertEqual(tasks.dispatch(["prepare"]), 0)
        task.assert_not_called()

    def test_publication_selectors_preserve_guards_and_default_order(self):
        selections = [
            (["--dry"], [("release:publish:dry",)], []),
            (["--stage", "assets"], [("release:publish:assets",)], [("release.checks",), ("release.run", "identity")]),
            (["--stage", "rust"], [("release:publish:rust",)], []),
            (["--stage", "finalize"], [("release:publish:finalize",)], []),
            (["--stage", "nightly"], [], [("release.nightly",)]),
            ([], [("release:publish:assets",), ("release:publish:finalize",)], [("release.checks",), ("release.run", "identity")]),
        ]
        for arguments, expected_tasks, expected_modules in selections:
            events = []
            with (
                self.subTest(arguments=arguments),
                patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}),
                patch.object(tasks, "_task", side_effect=lambda *args, events=events: events.append(args) or 0) as task,
                patch.object(tasks, "_module", side_effect=lambda *args, events=events: events.append(args) or 0) as module,
            ):
                self.assertEqual(tasks.dispatch(["release:publish", *arguments]), 0)
                self.assertEqual([call.args for call in task.call_args_list], expected_tasks)
                self.assertEqual([call.args for call in module.call_args_list], expected_modules)
                self.assertEqual(events, [*expected_modules, *expected_tasks])

    def test_invalid_task_selectors_do_not_dispatch(self):
        selections = [
            ["prepare", "--fixtures"],
            ["prepare", "--profile", "release"],
            ["release:publish", "--step", "dry"],
            ["release:publish", "--dry", "--stage", "assets"],
            ["release:publish", "--stage", "dry"],
            ["build", "--artifact", "fixtures", "--package", "example"],
            ["build", "--artifact", "fixtures", "--extended"],
            ["build", "--artifact", "fixtures", "--cpp"],
            ["build", "--artifact", "fixtures", "--compatibility"],
            ["build", "--artifact", "fixtures", "--check"],
        ]
        for arguments in selections:
            with (
                self.subTest(arguments=arguments),
                patch.object(tasks, "_task") as task,
                patch.object(tasks, "_module") as module,
                contextlib.redirect_stderr(io.StringIO()),
            ):
                with self.assertRaises(SystemExit) as error:
                    tasks.dispatch(arguments)
                self.assertEqual(error.exception.code, 2)
                task.assert_not_called()
                module.assert_not_called()

    def test_transport_surrounds_successful_task_and_does_not_recur(self):
        events = []
        environment = {
            "CI_INPUT_ARCHIVES": json.dumps(["target/ci/input.tar.zst"]),
            "CI_OUTPUT_ARCHIVES": json.dumps(
                [{"archive": "target/ci/output.tar.zst", "paths": ["target/build/{profile}/{target}/bin"], "profile": "release"}]
            ),
        }

        def dispatch(arguments):
            self.assertNotIn("CI_INPUT_ARCHIVES", os.environ)
            self.assertNotIn("CI_OUTPUT_ARCHIVES", os.environ)
            events.append("task")
            return 0

        with (
            patch.dict(os.environ, environment),
            patch.object(tasks, "dispatch", side_effect=dispatch),
            patch.object(artifacts, "extract", side_effect=lambda *args: events.append("extract")),
            patch.object(artifacts, "pack", side_effect=lambda *args: events.append("pack")) as pack,
            patch("harmos_tools.build.paths.target", return_value="linux-x86_64"),
        ):
            self.assertEqual(tasks.main(["build", "--profile", "release"]), 0)
        self.assertEqual(events, ["extract", "task", "pack"])
        self.assertEqual(pack.call_args.args[3:], ("release", "linux-x86_64"))

    def test_failed_task_does_not_export_output(self):
        with (
            patch.dict(os.environ, {"CI_OUTPUT_ARCHIVES": '[{"archive":"target/ci/output.tar.zst","paths":[]}]'}),
            patch.object(tasks, "dispatch", return_value=1),
            patch.object(artifacts, "pack") as pack,
        ):
            self.assertEqual(tasks.main(["build"]), 1)
        pack.assert_not_called()

    def test_help_never_transports_artifacts(self):
        with (
            patch.dict(os.environ, {"CI_INPUT_ARCHIVES": '["target/ci/missing.tar.zst"]'}),
            patch.object(artifacts, "extract") as extract,
            contextlib.redirect_stdout(io.StringIO()),
        ):
            with self.assertRaises(SystemExit) as error:
                tasks.main(["build", "--help"])
            self.assertEqual(error.exception.code, 0)
        extract.assert_not_called()

    def test_required_gate_refuses_unexpected_skips(self):
        for result in ("failure", "cancelled", "skipped"):
            with self.subTest(result=result), patch.dict(os.environ, {"RESULTS": json.dumps({"test": {"result": result}})}):
                with self.assertRaises(ValueError):
                    tasks.main(["ci:required"])
        with patch.dict(os.environ, {"RESULTS": '{"release_rehearsal":{"result":"skipped"}}', "RELEASE_REVIEW": "false"}):
            self.assertEqual(tasks.main(["ci:required"]), 0)
        with patch.dict(os.environ, {"RESULTS": '{"release_rehearsal":{"result":"skipped"}}', "RELEASE_REVIEW": "true"}):
            with self.assertRaises(ValueError):
                tasks.main(["ci:required"])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
