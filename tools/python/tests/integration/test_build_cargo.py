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
from harmos_tools.ci import artifacts, tasks
from harmos_tools.release import review
from harmos_tools.test import archive as archives

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class FocusedCargoTests(unittest.TestCase):
    def test_archive_publishes_complete_compiler_messages(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            archive = archives.location(directory, "dev")
            archive.parent.mkdir(parents=True)
            messages = archive.with_name(archives.MESSAGES)
            previous = {"reason": "build-finished", "success": False}
            messages.write_text(json.dumps(previous) + "\n", encoding="utf-8")
            complete = {"reason": "build-finished", "success": True}
            encoded = json.dumps(complete) + "\n"

            def compile_archive(command, **options):
                output = options["stdout"]
                middle = len(encoded) // 2
                output.write(encoded[:middle])
                output.flush()
                self.assertEqual(json.loads(messages.read_text(encoding="utf-8")), previous)
                output.write(encoded[middle:])
                archive.write_bytes(b"archive")
                return SimpleNamespace(returncode=0)

            with (
                patch.object(cargo.subprocess, "run", side_effect=compile_archive),
                patch.object(cargo.summary, "phase", return_value=contextlib.nullcontext()),
            ):
                result = cargo.archive(directory, "dev")
            self.assertEqual(result, archive)
            self.assertEqual(json.loads(messages.read_text(encoding="utf-8")), complete)
            self.assertEqual(set(archive.parent.iterdir()), {archive, messages})

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
                self.assertEqual(options["env"]["CARGO_TARGET_DIR"], str(directory / "target"))

    def test_nested_manifest_shares_repository_target(self):
        directory = Path.cwd()
        with (
            patch.dict(os.environ, CARGO_TARGET_DIR="elsewhere"),
            patch.object(cargo.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run,
        ):
            cargo.focused(directory, "build", ["--manifest-path", "examples/schematic/Cargo.toml"])
        self.assertEqual(run.call_args.kwargs["env"]["CARGO_TARGET_DIR"], str(directory / "target"))

    def test_rejects_target_redirection_before_dispatch(self):
        with patch.object(cargo.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                cargo.focused(Path.cwd(), "build", ["--target-dir", "elsewhere"])
        run.assert_not_called()

    def test_entry_points_dispatch_focused_cargo_directly(self):
        selections = [
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

    def test_invalid_focused_selections_do_not_start_cargo(self):
        selections = [
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

    def test_focused_layers_preserve_the_requested_filter(self):
        expression = "package(=harmos-runtime) & test(guest_contract::)"
        for layer in ("integration", "system"):
            with self.subTest(layer=layer), patch.object(tasks, "_task", return_value=0) as task:
                self.assertEqual(tasks.dispatch(["test", "--language", "rust", "--layer", layer, "--filter", expression]), 0)
                task.assert_called_once_with(f"test:rust:{layer}", "--profile", "dev", "--filter", expression)

    def test_prepare_syncs_sdk_without_inheriting_tooling_environment(self):
        toolchain = '[toolchain]\nchannel = "custom"\nprofile = "minimal"\ncomponents = ["rustfmt"]\ntargets = ["wasm32-wasip2"]\n'
        with (
            patch.dict(os.environ, VIRTUAL_ENV=str(Path.cwd() / ".venv")),
            patch.object(Path, "read_text", return_value=toolchain),
            patch.object(tasks, "_execute", return_value=0) as execute,
            patch.object(cargo, "focused") as focused,
        ):
            self.assertEqual(tasks.dispatch(["prepare"]), 0)
        self.assertEqual(execute.call_count, 2)
        self.assertEqual(
            execute.call_args_list[0].args[0],
            [
                "rustup",
                "toolchain",
                "install",
                "custom",
                "--profile",
                "minimal",
                "--component",
                "rustfmt",
                "--target",
                "wasm32-wasip2",
            ],
        )
        self.assertEqual(execute.call_args.args[0], ["uv", "sync", "--locked", "--all-groups", "--project", str(Path.cwd() / "sdk/python")])
        self.assertNotIn("VIRTUAL_ENV", execute.call_args.kwargs["environment"])
        focused.assert_not_called()

    def test_sdk_tests_use_their_project_and_preserve_selection(self):
        for exact in (None, "tests.integration.example.Story.test_behavior"):
            arguments = ["test", "--language", "python", "--package", "harmos-plugin", "--target", "integration", "--profile", "release"]
            selected = ["discover", "-s", "tests/integration", "-v"]
            if exact:
                arguments += ["--exact", exact]
                selected = [exact]
            with (
                self.subTest(exact=exact),
                patch.dict(os.environ, VIRTUAL_ENV=str(Path.cwd() / ".venv")),
                patch.object(tasks, "_execute", return_value=0) as execute,
            ):
                self.assertEqual(tasks.dispatch(arguments), 0)
            sdk = Path.cwd() / "sdk/python"
            execute.assert_called_once()
            self.assertEqual(
                execute.call_args.args[0],
                ["uv", "run", "--locked", "--no-sync", "--project", str(sdk), "python", "-m", "unittest", *selected],
            )
            self.assertEqual(execute.call_args.kwargs["directory"], sdk)
            self.assertEqual(execute.call_args.kwargs["environment"]["TEST_PROFILE"], "release")
            self.assertNotIn("VIRTUAL_ENV", execute.call_args.kwargs["environment"])

    def test_publication_selectors_preserve_guards_and_default_order(self):
        selections = [
            (["--dry"], [("release:publish:dry",)], []),
            (["--stage", "assets"], [("release:publish:assets",)], [("release.checks",), ("release.run", "identity")]),
            (["--stage", "rust"], [("release:publish:rust",)], []),
            (["--stage", "finalize"], [("release:publish:finalize",)], []),
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

        environment = {
            "OPTIONAL_JOBS": '["test","package_rust"]',
            "RESULTS": '{"test":{"result":"skipped"},"package_rust":{"result":"skipped"},"build_product":{"result":"success"}}',
        }
        with patch.dict(os.environ, environment):
            self.assertEqual(tasks.main(["ci:required"]), 0)
            for name in ("test", "package_rust", "build_product"):
                os.environ["RESULTS"] = json.dumps({name: {"result": "failure"}})
                with self.assertRaises(ValueError):
                    tasks.main(["ci:required"])
            os.environ["RESULTS"] = '{"build_product":{"result":"skipped"}}'
            with self.assertRaises(ValueError):
                tasks.main(["ci:required"])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
