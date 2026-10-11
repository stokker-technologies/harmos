# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Verify Native CI Prerequisites And Temporary Storage"""

# === Imports === #

# Standard Library Imports
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

# Third Party Imports

# Local Imports
from harmos_tools.pipelines import bootstrap

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class BootstrapTests(unittest.TestCase):
    def test_prerequisite_setup(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory) / "tmp"
            with (
                patch.dict("os.environ", {"TMPDIR": str(temporary)}, clear=True),
                patch(
                    "harmos_tools.pipelines.bootstrap.shutil.which",
                    return_value="compiler",
                ),
            ):
                self.assertEqual(bootstrap.main([]), 0)
                self.assertTrue(temporary.is_dir())
                with self.assertRaises(SystemExit):
                    bootstrap.main(["sync"])
            with patch(
                "harmos_tools.pipelines.bootstrap.shutil.which", return_value=None
            ):
                with self.assertRaisesRegex(RuntimeError, "prerequisite"):
                    bootstrap.main([])


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
