# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generated Binding Checks Report Drift Without Repairing It"""

# === Imports === #

# Standard Library Imports
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools.build import codegen
from harmos_tools.build.codegen import Placement, place

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class GeneratedBindingsReportDrift(unittest.TestCase):
    def test_check_reports_changed_missing_and_obsolete_files_then_generation_repairs_them(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        rendered = root / "rendered"
        destination = root / "sdk"
        for folder in (rendered / "proto", destination / "proto"):
            folder.mkdir(parents=True)
        placement = Placement("", destination, ("host.py", "client.py", "proto/*_pb2.py"))

        # Render Fresh Output Beside a Drifted Copy
        (rendered / "host.py").write_text("fresh\n", encoding="utf-8")
        (rendered / "proto/value_pb2.py").write_text("wire\n", encoding="utf-8")
        (destination / "host.py").write_text("drifted\n", encoding="utf-8")
        (destination / "client.py").write_text("obsolete\n", encoding="utf-8")
        (destination / "authored.py").write_text("authored\n", encoding="utf-8")

        # Check Without Writing
        with patch.object(codegen, "ROOT", root):
            differences = place(rendered, placement, check=True)
        self.assertEqual(differences, ["Missing: sdk/proto/value_pb2.py", "Obsolete: sdk/client.py", "Changed: sdk/host.py"])
        self.assertEqual((destination / "host.py").read_text(encoding="utf-8"), "drifted\n")

        # Generate, Then Check Clean
        with patch.object(codegen, "ROOT", root):
            self.assertEqual(place(rendered, placement, check=False), [])
            self.assertEqual(place(rendered, placement, check=True), [])
        self.assertFalse((destination / "client.py").exists())
        self.assertEqual((destination / "authored.py").read_text(encoding="utf-8"), "authored\n")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
