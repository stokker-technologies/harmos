# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Discover Operations, Read Status and Observe One Host Update."""

# === Imports === #

import asyncio
from argparse import ArgumentParser

from harmos import DiscoveredOperation
from harmos.client import StatusGetResponse, StatusWatchResponse
from tests.fixtures.client._generated.client import Client

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


async def run(address: str) -> tuple[list[DiscoveredOperation], StatusGetResponse, StatusWatchResponse]:
    client = Client.connect(address)
    try:
        # Discover Operations
        declarations = await client.operations.list()
        print(declarations)

        # Read Typed Host Status
        status = await client.status.get()
        print(status)

        # Observe One Typed Update, Then Cancel Demand
        updates = client.status.watch()
        try:
            async for update in updates:
                print(update)
                return declarations, status, update
        finally:
            await updates.aclose()
        raise RuntimeError("Watch Status: Host Closed Without an Update")
    finally:
        await client.close()


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    options = parser.parse_args()
    asyncio.run(run(options.address))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
