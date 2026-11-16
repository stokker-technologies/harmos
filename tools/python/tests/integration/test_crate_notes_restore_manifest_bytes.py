# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Retain the Selected Release Package Inputs."""

# === Imports === #

# Standard Library Imports
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.package import changelog

# Type Checking Imports


# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #


class ReleasePackage(unittest.TestCase):
    def test_crate_notes_stage_and_restore_manifest_bytes(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        temporary = temporary_directory.name
        try:
            directory = Path(temporary)
            manifests = []
            originals = []
            packages = {}
            for name, newline in (("first", "\n"), ("second", "\r\n")):
                parent = directory / name
                parent.mkdir()
                manifest = parent / "Cargo.toml"
                text = f'[package]\nname = "{name}"\nversion = "1.0.0"\ninclude = ["src/**"]\n'
                original = text.replace("\n", newline).encode()
                manifest.write_bytes(original)
                manifests.append(manifest)
                originals.append(original)
                packages[name] = {"manifest_path": str(manifest), "name": name, "version": "1.0.0"}
            retained = manifests[0].parent / "CHANGELOG.md"
            retained.write_bytes(b"retained\n")
            with patch.object(changelog, "notes", return_value="generated\n"):
                with self.assertRaises(RuntimeError):
                    staged_notes = changelog.StagedNotes(directory, packages)
                    try:
                        for manifest in manifests:
                            package = tomllib.loads(manifest.read_bytes().decode())["package"]
                            self.assertEqual(set(package["include"]), {"src/**", "CHANGELOG.md"})
                            self.assertTrue((manifest.parent / "CHANGELOG.md").is_file())
                        raise RuntimeError("abort staging")
                    finally:
                        staged_notes.close()
            self.assertEqual([manifest.read_bytes() for manifest in manifests], originals)
            self.assertEqual(retained.read_bytes(), b"retained\n")
            self.assertFalse((manifests[1].parent / "CHANGELOG.md").exists())
        finally:
            temporary_directory.cleanup()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
