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
from types import SimpleNamespace
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
    def test_guest_example(self) -> None:
        with (
            patch("sys.argv", ["examples", "guest"]),
            patch("harmos_tools.examples.Path.mkdir"),
            patch(
                "harmos_tools.examples.subprocess.run",
                return_value=subprocess.CompletedProcess(
                    [], 0, '{"target_directory": "custom-target"}'
                ),
            ) as run,
        ):
            examples.main()
            commands = [call.args[0] for call in run.call_args_list]
            guest = Path("custom-target") / "artifacts" / "schematic-tools.wasm"
            self.assertEqual(len(commands), 4)
            self.assertEqual(commands[0][:2], ["cargo", "metadata"])
            self.assertIn("wasm32-unknown-unknown", commands[1])
            self.assertIn("harmos-component", commands[2])
            self.assertEqual(commands[3][-2:], ["--guest", str(guest)])

    def test_sidecar_example(self) -> None:
        for platform, suffix in [("nt", ".exe"), ("posix", "")]:
            with (
                self.subTest(platform=platform),
                patch("harmos_tools.examples.os", SimpleNamespace(name=platform)),
                patch("sys.argv", ["examples", "sidecar"]),
                patch(
                    "harmos_tools.examples.subprocess.run",
                    return_value=subprocess.CompletedProcess(
                        [], 0, '{"target_directory": "custom-target"}'
                    ),
                ) as run,
            ):
                examples.main()
                sidecar = Path("custom-target") / "debug" / f"schematic-sidecar{suffix}"
                self.assertEqual(run.call_count, 4)
                self.assertIn("harmos-sidecar-export", run.call_args_list[2].args[0])
                self.assertEqual(
                    run.call_args.args[0][-1],
                    str(sidecar),
                )

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
