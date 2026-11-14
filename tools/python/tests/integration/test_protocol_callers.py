# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Protocol Caller Contract Checks"""

# === Imports === #

# Standard Library Imports
import ast
import tempfile
import unittest
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from harmos_tools.build.resource_roots import compose, table
from harmos_tools.source.protocol import caller_options, narrow_callers

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


class ProtocolCallers(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).resolve().parents[4]
        self.descriptors = FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())

    def test_missing_callers_refuses_host_operations(self) -> None:
        descriptors = FileDescriptorSet.FromString(self.descriptors.SerializeToString())
        declared = caller_options(descriptors)
        plugin = next(file for file in descriptors.file if "/plugin/" in file.name and file.service)
        for service in plugin.service:
            for method in service.method:
                method.ClearField("options")
        self.assertEqual(caller_options(descriptors), declared)
        host = next(file for file in descriptors.file if "/host/" in file.name and file.service)
        host.service[0].method[0].ClearField("options")
        with self.assertRaises(ValueError):
            caller_options(descriptors)

    def test_shared_operation_narrowing_changes_only_its_surfaces(self) -> None:
        original = table(self.descriptors)
        shared = next(method for method in original if method["face"] == "context")
        identity = f"{shared['package']}.{shared['service']}/{shared['method']}"
        narrow_callers(self.descriptors, {identity: "PLUGINS"})
        changed = table(self.descriptors)
        matching = [method["face"] for method in changed if f"{method['package']}.{method['service']}/{method['method']}" == identity]
        self.assertEqual(matching, ["context"])
        self.assertEqual(
            [method for method in changed if f"{method['package']}.{method['service']}/{method['method']}" != identity],
            [method for method in original if f"{method['package']}.{method['service']}/{method['method']}" != identity],
        )

    def test_narrowing_cannot_widen_or_change_exclusive_callers(self) -> None:
        identity = next(identity for identity, callers in caller_options(self.descriptors).items() if callers == "CLIENTS")
        for selected in ["ALL", "PLUGINS", "CALLERS_UNSPECIFIED"]:
            with self.subTest(selected=selected), self.assertRaises(ValueError):
                narrow_callers(self.descriptors, {identity: selected})

    def test_unknown_narrowing_refuses(self) -> None:
        with self.assertRaises(ValueError):
            narrow_callers(self.descriptors, {"missing.host.v1.Resource/Get": "CLIENTS"})

    def test_shared_settings_has_nested_paths_and_optional_selector(self) -> None:
        methods = [method for method in table(self.descriptors) if method["path"] == ["plugins", "settings", "list"]]
        self.assertEqual({method["face"] for method in methods}, {"client", "context"})
        self.assertEqual(len({method["request"] for method in methods}), 1)
        self.assertEqual(len({method["response"] for method in methods}), 1)
        self.assertTrue(all(next(field for field in method["fields"] if field["name"] == "name")["optional"] for method in methods))
        self.assertFalse(
            any(method["face"] == "context" and method["path"] == ["plugins", "settings", "set"] for method in table(self.descriptors))
        )

    def test_composition_file_narrows_generated_client(self) -> None:
        root = Path(__file__).resolve().parents[4]
        with tempfile.TemporaryDirectory() as scratch:
            folder = Path(scratch)
            manifest = folder / "composition.toml"
            descriptor = (root / "crates/harmos-protocol/src/descriptors.bin").as_posix()
            manifest.write_text(
                f'[[libraries]]\ndescriptors = "{descriptor}"\npackages = ["harmos.model.v1", "harmos.host.v1", "harmos.plugin.v1"]\n'
                '[callers]\n"harmos.host.v1.Time/Get" = "PLUGINS"\n',
                encoding="utf-8",
            )
            output = folder / "client.py"
            compose(root, [], output, "application", language="python", composition=manifest)
            tree = ast.parse(output.read_text(encoding="utf-8"))
            resource = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == "TimeResource")
            methods = {node.name for node in resource.body if isinstance(node, ast.AsyncFunctionDef)}
            self.assertNotIn("get", methods)
            self.assertIn("sleep", methods)


# === Private Functions === #

# === Entry Point === #
