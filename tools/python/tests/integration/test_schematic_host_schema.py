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
import json
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
    def test_collects_host_schema_and_refuses_changed_capture(self):
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
