# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Exercise repository workflows without building or publishing artifacts."""

# === Imports === #

# Standard Library Imports
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports

# Local Imports
from harmos_tools import artifacts, examples

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class WorkflowTests(unittest.TestCase):
    def test_example_build_outputs(self) -> None:
        for backend, filename in [
            ("guest", "/custom/guest.component.wasm"),
            ("sidecar", "/custom/sidecar.exe"),
        ]:
            with (
                self.subTest(backend=backend),
                patch("sys.argv", ["examples", backend]),
                patch(
                    "harmos_tools.examples.subprocess.run",
                    return_value=subprocess.CompletedProcess(
                        [],
                        0,
                        json.dumps(
                            {
                                "artifacts": [
                                    {
                                        "producer": f"schematic-{backend}",
                                        "path": filename,
                                    }
                                ]
                            }
                        ),
                        "",
                    ),
                ) as run,
            ):
                examples.main()
                commands = [call.args[0] for call in run.call_args_list]
                self.assertEqual(len(commands), 2)
                self.assertIn("cargo-harmos", commands[0])
                self.assertEqual(
                    commands[0][-5:],
                    [backend, "build", "-p", f"schematic-{backend}", "--json"],
                )
                self.assertEqual(commands[1][-2:], [f"--{backend}", filename])

    def test_tool_feature_availability(self) -> None:
        prepared = artifacts.artifacts()
        dependencies = json.loads(
            prepared["cargo-harmos-dependencies"].read_text(encoding="utf-8")
        )
        for variant, groups in (
            ("none", set()),
            ("sidecar", {"sidecar"}),
            ("guest", {"guest"}),
            ("both", {"sidecar", "guest"}),
        ):
            with self.subTest(variant=variant):
                executable = prepared[f"cargo-harmos-{variant}"]
                help_result = subprocess.run(
                    [executable, "--help"],
                    check=True,
                    capture_output=True,
                    text=True,
                    timeout=10,
                )
                self.assertIn("inspect", help_result.stdout)
                self.assertIn("bindings", help_result.stdout)
                bindings_help = subprocess.run(
                    [executable, "bindings", "python", "--help"],
                    check=True,
                    capture_output=True,
                    text=True,
                    timeout=10,
                )
                for option in (
                    "--export",
                    "--build-json",
                    "--producer",
                    "--catalog",
                    "--package-version",
                    "--protoc",
                ):
                    self.assertIn(option, bindings_help.stdout)
                self.assertNotIn("componentize", help_result.stdout)
                for group in ("sidecar", "guest"):
                    if group in groups:
                        self.assertIn(group, help_result.stdout)
                        group_help = subprocess.run(
                            [executable, group, "--help"],
                            check=True,
                            capture_output=True,
                            text=True,
                            timeout=10,
                        )
                        self.assertIn("build", group_help.stdout)
                        self.assertIn("dev", group_help.stdout)
                    else:
                        self.assertNotIn(group, help_result.stdout)
                        rejected = subprocess.run(
                            [executable, group, "build"],
                            capture_output=True,
                            text=True,
                            timeout=10,
                        )
                        self.assertEqual(rejected.returncode, 2)
                        self.assertIn("unrecognized subcommand", rejected.stderr)
                normal = set(dependencies[variant])
                self.assertFalse(
                    normal & {"wasmtime", "wasmtime-wasi", "harmos-sidecar"}
                )
                self.assertEqual("harmos-guest" in normal, "guest" in groups)
                self.assertEqual("wit-component" in normal, "guest" in groups)
                self.assertEqual("ctrlc" in normal, bool(groups))

    def test_fixture_deadlines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "consumer.json"
            manifest.write_text('{"artifacts": {"old": "stale"}}')
            failure = subprocess.TimeoutExpired("cargo build", 1)
            with patch(
                "harmos_tools.artifacts.subprocess.run",
                side_effect=[
                    subprocess.CompletedProcess(
                        [], 0, "host: x86_64-pc-windows-msvc\n"
                    ),
                    failure,
                ],
            ) as run:
                with self.assertRaises(subprocess.TimeoutExpired):
                    artifacts.prepare(root)
                self.assertFalse(manifest.exists())
                self.assertEqual(run.call_count, 2)
                compiler = run.call_args_list[0].kwargs
                build = run.call_args_list[1].kwargs
                self.assertGreater(build["timeout"], compiler["timeout"])
                self.assertTrue(build["check"])
                run.reset_mock()
                run.side_effect = subprocess.TimeoutExpired("cargo metadata", 1)
                with self.assertRaises(subprocess.TimeoutExpired):
                    artifacts._run(root, ["metadata", "--locked", "--no-deps"])
                self.assertEqual(run.call_args.kwargs["timeout"], compiler["timeout"])
                self.assertEqual(run.call_count, 1)

    def test_artifact_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "consumer.json"
            artifact = root / "module.wasm"
            with patch.dict("os.environ", {"HARMOS_FIXTURES_MANIFEST": str(manifest)}):
                with self.assertRaises(FileNotFoundError):
                    artifacts.artifacts()
                manifest.write_text(json.dumps({"artifacts": {"guest": artifact.name}}))
                with self.assertRaises(FileNotFoundError):
                    artifacts.artifacts()
                artifact.write_bytes(b"prepared")
                self.assertEqual(artifacts.artifacts(), {"guest": artifact})
                second = root / "second.wasm"
                manifest.write_text(
                    json.dumps(
                        {"artifacts": {"guest": artifact.name, "second": second.name}}
                    )
                )
                with self.assertRaisesRegex(FileNotFoundError, "second"):
                    artifacts.artifacts()
                second.write_bytes(b"prepared")
                self.assertEqual(
                    artifacts.artifacts(), {"guest": artifact, "second": second}
                )


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
