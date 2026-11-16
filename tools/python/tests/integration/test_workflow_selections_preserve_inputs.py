# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Preserve WorkflowTask Behavior."""

# === Imports === #

# Standard Library Imports
import contextlib
import hashlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
import harmos_tools.build.fixtures
import harmos_tools.prepare.tests.fixtures
import harmos_tools.task.execute
import harmos_tools.test.archive
from harmos_tools.build import cargo, paths
from harmos_tools.ci import artifacts, tasks

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class WorkflowTaskTests(unittest.TestCase):
    def test_fixture_join_replaces_retired_product_files(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        directory = Path(scratch.name)
        export = paths.output(directory, "build", "dev")
        fixtures = export / "fixtures"
        fixtures.mkdir(parents=True)
        retained = fixtures / "retained.bin"
        retained.write_bytes(b"test fixture")
        harmos_tools.test.archive.stamp(
            directory,
            "dev",
            [retained.relative_to(directory).as_posix()],
            {
                "artifacts": {},
                "builds": {},
                "libraries": {},
                "build_roots": {},
                "executables": [],
                "inspections": {},
            },
        )
        product = export / "product/examples"
        product.mkdir(parents=True)
        binaries = {}
        files = {}
        for name in ("schematic-cli", "schematic-sidecar", "schematic-guest"):
            binary = export / "bin" / name
            binary.parent.mkdir(exist_ok=True)
            binary.write_bytes(name.encode())
            binaries[name] = binary.relative_to(export).as_posix()
            files[binary.relative_to(directory).as_posix()] = hashlib.sha256(binary.read_bytes()).hexdigest()
        old = product / "old.rmeta"
        old.write_bytes(b"old library")
        old_name = old.relative_to(directory).as_posix()
        files[old_name] = hashlib.sha256(old.read_bytes()).hexdigest()
        catalog = {
            "artifacts": binaries,
            "builds": {},
            "libraries": {"runtime": old.relative_to(export).as_posix()},
            "build_roots": {},
            "files": files,
        }
        manifest = product / "catalog.json"
        manifest.write_text(json.dumps(catalog), encoding="utf-8")
        override = patch.object(harmos_tools.prepare.tests.fixtures, "ROOT", directory)
        override.start()
        self.addCleanup(override.stop)
        staging_override = patch.object(harmos_tools.build.fixtures, "ROOT", directory)
        staging_override.start()
        self.addCleanup(staging_override.stop)
        harmos_tools.prepare.tests.fixtures.prepare("dev")

        old.unlink()
        replacement = product / "new.rmeta"
        replacement.write_bytes(b"new library")
        del files[old_name]
        files[replacement.relative_to(directory).as_posix()] = hashlib.sha256(replacement.read_bytes()).hexdigest()
        catalog["libraries"] = {"runtime": replacement.relative_to(export).as_posix()}
        manifest.write_text(json.dumps(catalog), encoding="utf-8")
        harmos_tools.prepare.tests.fixtures.prepare("dev")
        updated: dict[str, str] = json.loads((fixtures / "stamp.json").read_text(encoding="utf-8"))["files"]
        self.assertNotIn(old_name, updated)
        self.assertIn(retained.relative_to(directory).as_posix(), updated)
        self.assertIn(replacement.relative_to(directory).as_posix(), updated)

    def test_fixture_join_selects_existing_inputs_without_compiling(self) -> None:
        for profile in ("dev", "release"):
            with (
                self.subTest(profile=profile),
                patch.object(harmos_tools.task.execute, "task", return_value=0) as task,
                patch.object(cargo, "focused") as focused,
            ):
                self.assertEqual(tasks.dispatch(["build", "--artifact", "fixtures", "--profile", profile]), 0)
                task.assert_called_once_with("prepare:tests:fixtures", "--profile", profile)
                focused.assert_not_called()

        with patch.object(harmos_tools.task.execute, "task", return_value=0) as task, patch.object(cargo, "focused") as focused:
            self.assertEqual(
                tasks.dispatch(
                    [
                        "build",
                        "--artifact",
                        "fixtures",
                        "--focused-messages",
                        "native.json",
                        "--libraries",
                        "harmos",
                        "--inspection-messages",
                        "guest.json",
                        "--inspections",
                        "schematic-guest",
                        "--componentizer",
                        "driver",
                    ]
                ),
                0,
            )
            task.assert_called_once_with(
                "prepare:tests:fixtures",
                "--profile",
                "dev",
                "--focused-messages",
                "native.json",
                "--libraries",
                "harmos",
                "--inspection-messages",
                "guest.json",
                "--inspections",
                "schematic-guest",
                "--componentizer",
                "driver",
            )
            focused.assert_not_called()

    def test_focused_layers_preserve_the_requested_filter(self) -> None:
        expression = "package(=harmos-runtime) & test(guest_contract::)"
        for layer in ("integration", "system"):
            with self.subTest(layer=layer), patch.object(harmos_tools.task.execute, "task", return_value=0) as task:
                self.assertEqual(tasks.dispatch(["test", "--language", "rust", "--layer", layer, "--filter", expression]), 0)
                task.assert_called_once_with(f"test:rust:{layer}", "--profile", "dev", "--filter", expression)

    def test_prepare_installs_only_the_root_toolchain(self) -> None:
        toolchain = '[toolchain]\nchannel = "custom"\nprofile = "minimal"\ncomponents = ["rustfmt"]\ntargets = ["wasm32-wasip2"]\n'
        with (
            patch.dict(os.environ, VIRTUAL_ENV=str(Path.cwd() / ".venv")),
            patch.object(Path, "read_text", return_value=toolchain),
            patch.object(harmos_tools.task.execute, "command", return_value=0) as execute,
            patch.object(cargo, "focused") as focused,
        ):
            self.assertEqual(tasks.dispatch(["prepare"]), 0)
        self.assertEqual(execute.call_count, 1)
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
        focused.assert_not_called()

    def test_sdk_tests_use_their_project_and_preserve_selection(self) -> None:
        for exact in (None, "tests.integration.example.Story.test_behavior"):
            arguments = ["test", "--language", "python", "--package", "harmos-sdk", "--target", "integration", "--profile", "release"]
            selected = ["--profile", "release"]
            if exact:
                arguments += ["--exact", exact]
                selected += ["--exact", exact]
            with (
                self.subTest(exact=exact),
                patch.dict(os.environ, VIRTUAL_ENV=str(Path.cwd() / ".venv")),
                patch.object(harmos_tools.task.execute, "command", return_value=0) as execute,
            ):
                self.assertEqual(tasks.dispatch(arguments), 0)
            execute.assert_called_once()
            self.assertEqual(
                execute.call_args.args[0],
                ["mise", "-C", "sdk/python", "run", "test", "--", *selected],
            )
            self.assertNotIn("VIRTUAL_ENV", execute.call_args.kwargs["environment"])

    def test_csharp_solution_tasks_preserve_profile_and_test_layer(self) -> None:
        for verb in ("build", "test", "package"):
            arguments = [verb, "--language", "csharp", "--profile", "release"]
            selected = ["--profile", "release"]
            if verb == "test":
                arguments += ["--layer", "unit"]
                selected += ["--layer", "unit"]
            with self.subTest(verb=verb), patch.object(harmos_tools.task.execute, "command", return_value=0) as execute:
                self.assertEqual(tasks.dispatch(arguments), 0)
            execute.assert_called_once_with(["mise", "-C", "sdk/csharp", "run", verb, "--", *selected])

    def test_publication_selectors_preserve_guards_and_default_order(self) -> None:
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
                patch.object(harmos_tools.task.execute, "task", side_effect=lambda *args, events=events: events.append(args) or 0) as task,
                patch.object(
                    harmos_tools.task.execute, "module", side_effect=lambda *args, events=events: events.append(args) or 0
                ) as module,
            ):
                self.assertEqual(tasks.dispatch(["release:publish", *arguments]), 0)
                self.assertEqual([call.args for call in task.call_args_list], expected_tasks)
                self.assertEqual([call.args for call in module.call_args_list], expected_modules)
                self.assertEqual(events, [*expected_modules, *expected_tasks])

    def test_invalid_task_selectors_do_not_dispatch(self) -> None:
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
                patch.object(harmos_tools.task.execute, "task") as task,
                patch.object(harmos_tools.task.execute, "module") as module,
                contextlib.redirect_stderr(io.StringIO()),
            ):
                with self.assertRaises(SystemExit) as error:
                    tasks.dispatch(arguments)
                self.assertEqual(error.exception.code, 2)
                task.assert_not_called()
                module.assert_not_called()

    def test_transport_surrounds_successful_task_and_does_not_recur(self) -> None:
        events = []
        environment = {
            "CI_INPUT_ARCHIVES": json.dumps(["target/ci/input.tar.zst"]),
            "CI_OUTPUT_ARCHIVES": json.dumps(
                [{"archive": "target/ci/output.tar.zst", "paths": ["target/build/{profile}/{target}/bin"], "profile": "release"}]
            ),
        }

        def dispatch(arguments: list[str] | None) -> int:
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

    def test_failed_task_does_not_export_output(self) -> None:
        with (
            patch.dict(os.environ, {"CI_OUTPUT_ARCHIVES": '[{"archive":"target/ci/output.tar.zst","paths":[]}]'}),
            patch.object(tasks, "dispatch", return_value=1),
            patch.object(artifacts, "pack") as pack,
        ):
            self.assertEqual(tasks.main(["build"]), 1)
        pack.assert_not_called()

    def test_help_never_transports_artifacts(self) -> None:
        with (
            patch.dict(os.environ, {"CI_INPUT_ARCHIVES": '["target/ci/missing.tar.zst"]'}),
            patch.object(artifacts, "extract") as extract,
            contextlib.redirect_stdout(io.StringIO()),
        ):
            with self.assertRaises(SystemExit) as error:
                tasks.main(["build", "--help"])
            self.assertEqual(error.exception.code, 0)
        extract.assert_not_called()

    def test_sdk_delegation_rejects_rust_only_selections(self) -> None:
        selections = []
        for language in ("csharp", "cpp"):
            selections.extend(
                [
                    ["build", "--language", language, "--lib"],
                    ["build", "--language", language, "--componentizer", "fixture-driver"],
                    ["test", "--language", language, "--story", "save"],
                    ["test", "--language", language, "--paths", "model/src/lib.rs"],
                    ["test", "--language", language, "--no-run"],
                    ["test", "--language", language, "--timings"],
                    ["test", "--language", language, "--exact", ""],
                    ["test", "--language", language, "--target", ""],
                ]
            )
        for selection in selections:
            with (
                self.subTest(selection=selection),
                patch.object(harmos_tools.task.execute, "command") as execute,
                contextlib.redirect_stderr(io.StringIO()),
                self.assertRaises(SystemExit) as error,
            ):
                tasks.dispatch(selection)
            self.assertEqual(error.exception.code, 2)
            execute.assert_not_called()

    def test_rust_generation_builds_and_runs_the_selected_profile(self) -> None:
        root = Path.cwd()
        for profile in ("dev", "release"):
            with (
                self.subTest(profile=profile),
                patch("harmos_tools.build.cargo.focused", return_value=0) as build,
                patch.object(harmos_tools.task.execute, "command", return_value=0) as execute,
                patch("harmos_tools.build.resource_roots.generate") as generate,
            ):
                result = tasks.dispatch(["generate", "--sdk", "rust", "--profile", profile])
            self.assertEqual(result, 0)
            arguments = build.call_args.args[2]
            self.assertEqual(arguments[arguments.index("--profile") + 1], profile)
            directory = "debug" if profile == "dev" else profile
            executable = "generate.exe" if os.name == "nt" else "generate"
            binary = root / "target" / directory / "examples" / executable
            execute.assert_called_once_with([str(binary)])
            generate.assert_called_once_with(root)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
