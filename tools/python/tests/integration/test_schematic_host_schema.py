# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Consume Captured Host Schemas After Native Plugins Stop Embedding Catalogs."""

# === Imports === #

# Standard Library Imports
import ast
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.build import fixtures

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class SchematicHostSchemaTests(unittest.TestCase):
    def test_python_generation_consumes_staged_schema_and_records_sorted_imports(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        producer = root / "retired-producer"
        export = root / "export"
        receipt = export / "receipts/plugin.json"
        receipt.parent.mkdir(parents=True)
        staged = export / "builds/plugin/target/schema"
        staged.mkdir(parents=True)
        descriptor = staged / "descriptors.bin"
        descriptor.write_bytes(b"descriptor")
        receipt.write_text(
            json.dumps(
                {
                    "schemas": [
                        {
                            "producer": "schematic-cli",
                            "catalog": "operations",
                            "directory": str(producer / "target/schema"),
                            "files": {"descriptors.bin": fixtures.digest(descriptor)},
                        }
                    ]
                }
            )
        )
        tool = root / "driver"
        protoc = root / "protoc"
        tool.touch()
        protoc.touch()
        generated = root / "generated/python"
        files: dict[str, str] = {}

        def execute(command: list[str], **options: object) -> subprocess.CompletedProcess[str]:
            if command[0] == str(tool):
                payload = options["input"]
                assert isinstance(payload, str)
                request = json.loads(payload)
                self.assertEqual(Path(request["export"]), staged)
                output = generated / "src/client.py"
                output.parent.mkdir(parents=True)
                output.write_text("import sys\nimport os\n")
            else:
                subprocess.check_call(command)
            return subprocess.CompletedProcess(command, 0)

        with (
            patch.object(fixtures, "ROOT", root),
            patch.object(fixtures, "GENERATED", root / "generated"),
            patch.object(fixtures.subprocess, "run", side_effect=execute),
        ):
            # The Real Ruff Config Is an Input, Even When Generation Uses a Disposable Export.
            config = root / "tools/python/ruff.toml"
            config.parent.mkdir(parents=True)
            config.write_text('target-version = "py313"\n')
            fixtures.generate_python(tool, receipt, protoc, files, source_root=producer)
            source = generated / "src/client.py"
            imports = [alias.name for node in ast.parse(source.read_text()).body if isinstance(node, ast.Import) for alias in node.names]
            self.assertEqual(imports, ["os", "sys"])
            self.assertEqual(files[source.relative_to(root).as_posix()], fixtures.digest(source))
            descriptor.write_bytes(b"replaced")
            with self.assertRaises(RuntimeError):
                fixtures.generate_python(tool, receipt, protoc, files, source_root=producer)

    def test_collects_host_schema_and_refuses_changed_capture(self) -> None:
        # Capture a Host Schema Beside a Native Plugin Without Generated Schemas
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            schema = root / "host-out/grpc/operations"
            schema.mkdir(parents=True)
            descriptor = schema / "descriptors.bin"
            descriptor.write_bytes(b"captured descriptor")
            manifest = schema / "manifest.txt"
            manifest.write_text("descriptors.bin\n", encoding="utf-8")
            metadata = root / "metadata.json"
            metadata.write_text(json.dumps({"packages": [{"id": "host", "name": "schematic-cli"}]}), encoding="utf-8")
            events = root / "messages.json"
            events.write_text(
                json.dumps({"reason": "build-script-executed", "package_id": "host", "out_dir": str(schema.parent.parent)}),
                encoding="utf-8",
            )
            capture = {
                "target": "native",
                "messages": {"path": str(events)},
                "metadata": {"path": str(metadata)},
                "receipt": str(root / "capture.json"),
                "generated": {str(path): fixtures.digest(path) for path in (descriptor, manifest)},
            }

            # Collect the Plugin's Bytes and Retain the Host's Stamped Client Schema
            with patch.object(fixtures.subprocess, "check_output", return_value='{"artifacts": [], "schemas": []}'):
                built, receipt = fixtures.collect(Path("driver"), Path("Cargo.toml"), "schematic-sidecar", "sidecar", "dev", capture, root)
            self.assertEqual(built, json.loads(receipt.read_text(encoding="utf-8")))
            self.assertEqual(built["schemas"][0]["producer"], "schematic-cli")
            self.assertEqual(built["schemas"][0]["directory"], str(schema))
            self.assertEqual(built["schemas"][0]["files"]["descriptors.bin"], fixtures.digest(descriptor))

            # Reject Schema Bytes Replaced After the Compiler Capture
            descriptor.write_bytes(b"changed descriptor")
            with patch.object(fixtures.subprocess, "check_output", return_value='{"artifacts": [], "schemas": []}'):
                with self.assertRaises(RuntimeError):
                    fixtures.collect(Path("driver"), Path("Cargo.toml"), "schematic-sidecar", "sidecar", "dev", capture, root)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
