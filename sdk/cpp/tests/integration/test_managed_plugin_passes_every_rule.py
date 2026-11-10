# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""A C++ Author Serves Every Protocol Rule as a Managed Plugin."""

# === Imports === #

# Standard Library Imports
import unittest

# Third Party Imports
# Local Imports
from tests.support import host

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ManagedPlugin(unittest.TestCase):
    def test_managed_plugin_passes_every_rule(self):
        # Start the Managed Plugin and Run Every Reference-Host Case Through Stop.
        result = host.run_managed()

        # Check Every Rule and the Managed Process's Successful Cleanup and Exit.
        self.assertEqual(result.returncode, 0, result.stdout)
        rows = [
            line for line in result.stdout.splitlines() if line.endswith("| PASS |")
        ]
        self.assertEqual(len(rows), 18, result.stdout)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
