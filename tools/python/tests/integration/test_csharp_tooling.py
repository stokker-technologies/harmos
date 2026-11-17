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
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
import harmos_tools.build.fixtures
import harmos_tools.build.rust.tests
from harmos_tools import csharp

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class CSharpTooling(unittest.TestCase):
    def test_generated_root_checks_detect_drift_without_rewriting_sources(self) -> None:
        root = Path(__file__).resolve().parents[4]
        name_directory = tempfile.TemporaryDirectory()
        name = name_directory.name
        try:
            scratch = Path(name)
            for relative in (
                "crates/harmos-protocol/src/descriptors.bin",
                "sdk/rust/tests/fixtures/client/client-descriptors.bin",
                "sdk/csharp/Directory.Packages.props",
            ):
                target = scratch / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(root / relative, target)
            # Generate Both Roots and Check the Unmodified Output.
            csharp.clients(scratch)
            csharp.clients(scratch, check=True)
            roots = list((scratch / "sdk/csharp").rglob("*.g.cs"))
            # Detect Changed and Missing Output Without Repairing Either.
            # Reuse the Already Verified Protocol Inputs
            with patch.object(csharp, "protocols"):
                for target in roots:
                    original = target.read_bytes()
                    target.write_bytes(original + b"// drift\n")
                    self.assertRaises(ValueError, csharp.clients, scratch, check=True)
                    self.assertEqual(target.read_bytes(), original + b"// drift\n")
                    target.unlink()
                    self.assertRaises(ValueError, csharp.clients, scratch, check=True)
                    self.assertFalse(target.exists())
                    target.write_bytes(original)
        finally:
            name_directory.cleanup()

    def test_rust_producer_stages_the_msbuild_selected_runtime_closure(self) -> None:
        name_directory = tempfile.TemporaryDirectory()
        name = name_directory.name
        try:
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
                        patch.object(harmos_tools.build.rust.tests, "ROOT", root),
                        patch.object(harmos_tools.build.fixtures, "ROOT", root),
                        patch.object(harmos_tools.build.rust.tests.subprocess, "run") as build,
                    ):
                        harmos_tools.build.rust.tests.csharp_fixture(profile, fixtures, files, binaries)
                    build.assert_called_once()
                    staged = fixtures / binaries["harmos-csharp-conformance-plugin"]
                    self.assertEqual(staged.read_bytes(), plugin.read_bytes())
                    self.assertEqual((staged.parent / "dependency.dll").read_bytes(), b"runtime dependency")
                    self.assertEqual(len(files), 2)
        finally:
            name_directory.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
