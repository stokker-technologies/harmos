# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""A Python Author Serves the Declarations to the Reference Host."""

# === Imports === #

# Standard Library Imports

# Third Party Imports
# Local Imports
from unittest import IsolatedAsyncioTestCase

from tests.fixtures.conformance import plugin
from tests.support import host

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class RemotePlugin(IsolatedAsyncioTestCase):
    async def test_remote_plugin_passes_every_rule(self) -> None:
        # Serve the Author's Instance at an Explicit Remote Address.
        endpoint = await plugin().start("127.0.0.1:0")
        self.addAsyncCleanup(endpoint.close)

        # Run Every Protocol Rule Through the Language-Neutral Reference Host.
        status, stdout, stderr = await host.run(endpoint.address)
        self.assertEqual(status, 0, stdout + stderr)
        rows = [line for line in stdout.splitlines() if line.endswith("| PASS |")]
        self.assertTrue(rows, stdout)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
