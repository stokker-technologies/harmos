# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Descriptor-Derived Client Generation and Composition."""

# === Imports === #

import ast
import tempfile
import unittest
from pathlib import Path

from google.protobuf.descriptor_pb2 import FileDescriptorSet

from harmos_tools.build.resource_roots import compose, python, table

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]
LIBRARY = ROOT / "crates/harmos-protocol/src/descriptors.bin"
APPLICATION = ROOT / "sdk/rust/tests/fixtures/client/client-descriptors.bin"

# === Types === #

# === Classes === #


class PythonClients(unittest.TestCase):
    def test_committed_roots_match_fresh_descriptor_generation(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        output = Path(scratch.name) / "client.py"
        library = FileDescriptorSet.FromString(LIBRARY.read_bytes())
        python(output, [method for method in table(library) if method["face"] == "client"], library)
        self.assertEqual(output.read_bytes(), (ROOT / "sdk/python/src/harmos/generated/client.py").read_bytes())

        compose(ROOT, [APPLICATION], output, "application", language="python")
        composed = FileDescriptorSet.FromString(LIBRARY.read_bytes())
        composed.file.extend(FileDescriptorSet.FromString(APPLICATION.read_bytes()).file)
        expected = Path(scratch.name) / "fixture.py"
        python(expected, [method for method in table(composed) if method["face"] == "client"], composed)
        self.assertEqual(output.read_bytes(), expected.read_bytes())

    def test_generation_refuses_collisions_faces_and_client_streams(self) -> None:
        descriptors = FileDescriptorSet.FromString(APPLICATION.read_bytes())
        methods = table(descriptors)
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        output = Path(scratch.name) / "client.py"
        with self.assertRaises(ValueError):
            python(output, [methods[0], methods[0]], descriptors)
        with self.assertRaises(ValueError):
            python(output, [{**methods[0], "face": "context"}], descriptors)
        with self.assertRaises(ValueError):
            python(output, [{**methods[0], "client_streaming": True}], descriptors)
        with self.assertRaises(ValueError):
            python(output, [methods[0], {**methods[0], "path": methods[0]["path"][:-1]}], descriptors)
        self.assertFalse(output.exists())

    def test_composition_refuses_conflicting_descriptor_definitions(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        descriptor = FileDescriptorSet.FromString(LIBRARY.read_bytes())
        descriptor.file[0].package = "conflicting.v1"
        conflicting = Path(scratch.name) / "conflicting.bin"
        conflicting.write_bytes(descriptor.SerializeToString())
        with self.assertRaises(ValueError):
            compose(ROOT, [conflicting], Path(scratch.name) / "client.py", "application", language="python")

    def test_cross_file_and_nested_messages_keep_their_defining_module(self) -> None:
        descriptors = FileDescriptorSet()
        messages = descriptors.file.add(name="example/v1/messages.proto", package="example.v1")
        outer = messages.message_type.add(name="Outer")
        outer.nested_type.add(name="Input")
        outer.nested_type.add(name="Output")
        service = descriptors.file.add(name="example/v1/jobs.proto", package="example.v1").service.add(name="Jobs")
        service.method.add(name="Start", input_type=".example.v1.Outer.Input", output_type=".example.v1.Outer.Output")
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        output = Path(scratch.name) / "client.py"
        python(output, table(descriptors), descriptors)
        source = ast.parse(output.read_text())
        imports = [alias.name for node in ast.walk(source) if isinstance(node, ast.Import) for alias in node.names]
        self.assertIn("example.v1.messages_pb2", imports)
        self.assertNotIn("example.v1.jobs_pb2", imports)
        compile(source, str(output), "exec")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
