# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Committed C# Roots and MSBuild Outputs Reach Their Consumers."""

# === Imports === #

# Standard Library Imports
import re
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from harmos_tools import csharp
from harmos_tools.build.rust import tests as producer
from harmos_tools.source.protocol import narrow_callers

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class CSharpTooling(unittest.TestCase):
    def test_narrowed_callers_change_csharp_surfaces_and_preserve_other_operations(self) -> None:
        root = Path(__file__).resolve().parents[4]
        for callers, client_get, plugin_get in (("ALL", True, True), ("CLIENTS", True, False), ("PLUGINS", False, True)):
            scratch = Path(tempfile.mkdtemp())
            try:
                # Copy Only the C# Generator Inputs.
                for relative in (
                    "crates/harmos-protocol/src/descriptors.bin",
                    "sdk/rust/tests/fixtures/client/client-descriptors.bin",
                ):
                    target = scratch / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(root / relative, target)
                descriptor = scratch / "crates/harmos-protocol/src/descriptors.bin"
                schema = FileDescriptorSet.FromString(descriptor.read_bytes())
                narrow_callers(schema, {"harmos.host.v1.Time/Get": callers})
                descriptor.write_bytes(schema.SerializeToString())

                # Generate the Actual Client and Context from the Restricted Input.
                csharp.clients(scratch)
                generated = scratch / "sdk/csharp/src/Harmos/Generated"
                for filename, resource, has_get in (
                    ("Client.g.cs", "TimeResource", client_get),
                    ("Host.g.cs", "HostTimeResource", plugin_get),
                ):
                    source = (generated / filename).read_text(encoding="utf-8")
                    body = source.split(f"public sealed class {resource}", 1)[1].split("public sealed class ", 1)[0]
                    methods = set(re.findall(r"public async [^\n]+? (\w+)\(", body))
                    self.assertEqual("Get" in methods, has_get)
                    self.assertIn("Sleep", methods)
                    self.assertIn("Task<SettingsListResponse> List(", source)
                client = (generated / "Client.g.cs").read_text(encoding="utf-8")
                context = (generated / "Host.g.cs").read_text(encoding="utf-8")
                self.assertIn("Task<ByteString> Create(string operation,", client)
                self.assertNotIn("InvocationsCreate", context)
                self.assertIn("PluginsSettingsList", context)
                self.assertNotIn("PluginsSettingsSet", context)
            finally:
                shutil.rmtree(scratch)

    def test_generated_root_checks_detect_drift_without_rewriting_sources(self) -> None:
        root = Path(__file__).resolve().parents[4]
        with tempfile.TemporaryDirectory() as name:
            scratch = Path(name)
            for relative in (
                "crates/harmos-protocol/src/descriptors.bin",
                "sdk/rust/tests/fixtures/client/client-descriptors.bin",
            ):
                target = scratch / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(root / relative, target)
            # Generate Both Roots and Check the Unmodified Output.
            csharp.clients(scratch)
            csharp.clients(scratch, check=True)
            roots = list((scratch / "sdk/csharp").rglob("*.g.cs"))
            # Detect Changed and Missing Output Without Repairing Either.
            for target in roots:
                original = target.read_bytes()
                target.write_bytes(original + b"// drift\n")
                with self.assertRaises(ValueError):
                    csharp.clients(scratch, check=True)
                self.assertEqual(target.read_bytes(), original + b"// drift\n")
                target.unlink()
                with self.assertRaises(ValueError):
                    csharp.clients(scratch, check=True)
                self.assertFalse(target.exists())
                target.write_bytes(original)

    def test_rust_producer_stages_the_msbuild_selected_runtime_closure(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            for profile, configuration in (("dev", "Debug"), ("release", "Release")):
                with self.subTest(profile=profile):
                    # MSBuild Owns an Arbitrary Executable Location and Its Dependencies.
                    runtime = root / "sdk/csharp/artifacts/custom" / configuration
                    runtime.mkdir(parents=True)
                    plugin = runtime / "fixture-program"
                    plugin.write_bytes(b"fixture executable")
                    (runtime / "dependency.dll").write_bytes(b"runtime dependency")
                    manifest = root / "sdk/csharp/artifacts/paths" / configuration / "Harmos.Conformance.txt"
                    manifest.parent.mkdir(parents=True)
                    manifest.write_text(plugin.relative_to(root / "sdk/csharp").as_posix(), encoding="utf-8-sig")
                    fixtures = root / "target" / profile
                    files = {}
                    binaries = {}
                    # Run the Actual Producer Staging Path Without Compiling in This Test.
                    with (
                        patch.object(producer, "ROOT", root),
                        patch.object(producer.staging, "ROOT", root),
                        patch.object(producer.subprocess, "run") as build,
                    ):
                        producer.csharp_fixture(profile, fixtures, files, binaries)
                    build.assert_called_once()
                    staged = fixtures / binaries["harmos-csharp-conformance-plugin"]
                    self.assertEqual(staged.read_bytes(), plugin.read_bytes())
                    self.assertEqual((staged.parent / "dependency.dll").read_bytes(), b"runtime dependency")
                    self.assertEqual(len(files), 2)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
