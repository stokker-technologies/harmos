# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Report Nested Generated Import Failures from a Native Child."""

# === Imports === #

# Standard Library Imports
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# Third Party Imports
# Local Imports
from harmos_tools.process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class GeneratedImports(unittest.TestCase):
    def test_nested_import_failure_preserves_status_and_diagnostic(self) -> None:
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        generated = Path(scratch.name)
        package = generated / "fixture_generated"
        package.mkdir()
        (package / "__init__.py").write_text("from . import child\n", encoding="utf-8")
        (package / "child.py").write_text("VALUE = 17\n", encoding="utf-8")
        command = [sys.executable, "-m", "harmos_tools.source.imports", "--directory", str(generated)]

        # Import the Complete Generated Package.
        result = subprocess.run(command, capture_output=True, text=True, creationflags=spawn_flags(), check=False)
        self.assertEqual(result.returncode, 0, result.stderr)

        # Preserve the Nested Import's Failure and Diagnostic.
        (package / "broken.py").write_text("import fixture_dependency_that_does_not_exist\n", encoding="utf-8")
        result = subprocess.run(command, capture_output=True, text=True, creationflags=spawn_flags(), check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("fixture_dependency_that_does_not_exist", result.stderr)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    unittest.main()
