# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Release Scratch Contents after Use and Failed Ownership Acquisition."""

# === Imports === #

# Standard Library Imports
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports
# Local Imports
from harmos_tools import temporary

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class TemporaryDirectories(unittest.TestCase):
    def test_close_and_failed_acquisition_release_owned_contents(self) -> None:
        # Close a Populated Directory
        directory = temporary.Directory()
        root = directory.root
        try:
            (directory.path / "content").write_bytes(b"retained until close")
        finally:
            directory.close()
        self.assertFalse(root.exists())

        # Refuse Ownership and Reclaim the Empty Directory
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        refused = Path(scratch.name) / "refused"
        refused.mkdir()
        with (
            patch.object(temporary.tempfile, "mkdtemp", return_value=str(refused)),
            patch.object(Path, "open", side_effect=PermissionError("ownership refused")),
            self.assertRaises(PermissionError),
        ):
            temporary.Directory()
        self.assertFalse(refused.exists())


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
