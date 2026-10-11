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
            self.assertEqual(len(commands), 4)
            self.assertEqual(commands[0][:2], ["cargo", "metadata"])
            self.assertIn("wasm32-unknown-unknown", commands[1])
            self.assertIn("harmos-component", commands[2])
            self.assertEqual(
                commands[3][-2:], ["--guest", "custom-target/artifacts/autoroute.wasm"]
            )

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
                self.assertEqual(run.call_count, 4)
                self.assertIn("harmos-sidecar-export", run.call_args_list[2].args[0])
                self.assertEqual(
                    run.call_args.args[0][-1],
                    f"custom-target/debug/minibench-probe-sidecar{suffix}",
                )

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
