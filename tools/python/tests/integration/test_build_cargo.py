# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Preserve FocusedCargo Behavior."""

# === Imports === #

# Standard Library Imports
import contextlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock, patch

# Third Party Imports
# Local Imports
import harmos_tools.test.archive
from harmos_tools.build import cargo
from harmos_tools.ci import compiler, tasks
from harmos_tools.release import review

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class FocusedCargoTests(unittest.TestCase):
    def test_cache_write_failures_preserve_compile_status_and_statistics(self) -> None:
        for status, reason, category in ((0, "Cannot write to a read-only cache", "ReadOnly"), (7, "PermissionDenied", "PermissionDenied")):
            with self.subTest(status=status):
                # Start an Owned Cache That Cannot Write Its Compiled Miss.
                server = MagicMock()
                server.stderr = io.StringIO(f"server started, listening on 127.0.0.1:23456\nError executing cache write: {reason}\n")
                server.poll.return_value = None
                server.__enter__.return_value = server

                def terminate(server: MagicMock = server) -> None:
                    server.poll.return_value = 0

                server.terminate.side_effect = terminate
                child = MagicMock()
                child.poll.return_value = status
                child.returncode = status
                child.__enter__.return_value = child

                def inspect(command: list[str], **options: object) -> SimpleNamespace:
                    output = "sccache 0.19.0\n" if "--version" in command else json.dumps({"stats": {"cache_write_errors": 1}})
                    return SimpleNamespace(returncode=0, stdout=output)

                # Keep Both Successful and Failed Compiles Authoritative.
                statistics = {}
                diagnostic = io.StringIO()
                with (
                    patch.object(compiler.subprocess, "Popen", side_effect=[server, child]) as launch,
                    patch.object(compiler.subprocess, "run", side_effect=inspect),
                    contextlib.redirect_stderr(diagnostic),
                ):
                    result = compiler._execute(["compiler", "input"], None, {}, statistics=statistics)
                self.assertEqual(result, status)
                self.assertEqual(launch.call_count, 2)
                self.assertEqual(statistics["cache_write_errors"], 1)
                self.assertEqual(statistics[f"write_error_{category}"], 1)
                self.assertIn("cache write warning", diagnostic.getvalue())
                self.assertNotIn("cache write failed", diagnostic.getvalue())

    def test_archive_publishes_complete_compiler_messages(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
            directory = Path(temporary)
            archive = harmos_tools.test.archive.location(directory, "dev")
            archive.parent.mkdir(parents=True)
            messages = archive.with_name(harmos_tools.test.archive.MESSAGES)
            previous = {"reason": "build-finished", "success": False}
            messages.write_text(json.dumps(previous) + "\n", encoding="utf-8")
            complete = {"reason": "build-finished", "success": True}
            encoded = json.dumps(complete) + "\n"

            def compile_archive(command: list[str], **options: object) -> SimpleNamespace:
                output = options["stdout"]
                assert isinstance(output, io.TextIOBase)
                middle = len(encoded) // 2
                output.write(encoded[:middle])
                output.flush()
                self.assertEqual(json.loads(messages.read_text(encoding="utf-8")), previous)
                output.write(encoded[middle:])
                archive.write_bytes(b"archive")
                return SimpleNamespace(returncode=0)

            with (
                patch.object(cargo.subprocess, "run", side_effect=compile_archive),
                patch.object(cargo.summary, "Phase"),
            ):
                result = cargo.archive(directory, "dev")
            self.assertEqual(result, archive)
            self.assertEqual(json.loads(messages.read_text(encoding="utf-8")), complete)
            self.assertEqual(set(archive.parent.iterdir()), {archive, messages})
        finally:
            temporary_directory.cleanup()

    def test_dispatches_only_requested_operation(self) -> None:
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
                self.assertEqual(command, (["cargo", operation, "--timings", *arguments],))
                self.assertEqual(options["cwd"], directory)
                self.assertEqual(options["env"]["CARGO_BUILD_WARNINGS"], "deny")
                self.assertEqual(options["env"]["CARGO_TARGET_DIR"], str(directory / "target"))

    def test_nested_manifest_shares_repository_target(self) -> None:
        directory = Path.cwd()
        for operation in ("build", "clippy"):
            with (
                self.subTest(operation=operation),
                patch.dict(os.environ, CARGO_TARGET_DIR="elsewhere"),
                patch.object(cargo.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run,
            ):
                arguments = ["--manifest-path", "examples/schematic/Cargo.toml"]
                if operation == "clippy":
                    cargo.clippy(directory, arguments)
                else:
                    cargo.focused(directory, operation, arguments)
            self.assertEqual(run.call_args.kwargs["env"]["CARGO_TARGET_DIR"], str(directory / "target"))

    def test_rejects_target_redirection_before_dispatch(self) -> None:
        with patch.object(cargo.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                cargo.focused(Path.cwd(), "build", ["--target-dir", "elsewhere"])
        run.assert_not_called()

    def test_entry_points_dispatch_focused_cargo_directly(self) -> None:
        selections = [
            (
                ["build", "--package", "example", "--lib", "--check"],
                "check",
                ["--locked", "--profile", "dev", "-p", "example", "--lib"],
            ),
            (
                ["build", "--package", "example", "--manifest-path", "nested/Cargo.toml", "--check"],
                "check",
                ["--locked", "--profile", "dev", "-p", "example", "--manifest-path", str(Path("nested/Cargo.toml"))],
            ),
            (
                ["test", "--package", "example", "--manifest-path", "nested/Cargo.toml", "--target", "journey"],
                "test",
                ["--locked", "-p", "example", "--test", "journey", "--manifest-path", str(Path("nested/Cargo.toml"))],
            ),
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

    def test_invalid_focused_selections_do_not_start_cargo(self) -> None:
        selections = [
            ["build", "--lib"],
            ["build", "--package", "example", "--example", "fixture", "--lib"],
            ["build", "--package", "harmos-sdk", "--lib"],
            ["build", "--language", "cpp", "--package", "example", "--lib"],
            ["build", "--manifest-path", "nested/Cargo.toml"],
            ["test", "--manifest-path", "nested/Cargo.toml", "--target", "lib"],
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

    def test_release_mode_task_exports_selected_squash(self) -> None:
        for mode, sha in (("tag", "a" * 40), ("prepare", ""), ("skip", "")):
            with self.subTest(mode=mode):
                temporary_directory = tempfile.TemporaryDirectory()
                temporary = temporary_directory.name
                try:
                    output = Path(temporary) / "output"
                    with (
                        patch.dict(os.environ, GITHUB_OUTPUT=str(output)),
                        patch.object(review, "github_release_mode", return_value={"mode": mode, "sha": sha}) as decision,
                    ):
                        self.assertEqual(tasks.main(["ci:release-mode"]), 0)
                    decision.assert_called_once_with()
                    fields = dict(line.split("=", 1) for line in output.read_text().splitlines())
                    self.assertEqual(fields, {"mode": mode, "sha": sha})
                finally:
                    temporary_directory.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
