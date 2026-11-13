# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Discover Operations, Read Status and Observe One Host Update."""

# === Imports === #

import argparse
import asyncio

from harmos import Client
from harmos.v1.catalog_entries_pb2 import CatalogEntriesListRequest, CatalogEntriesListResponse
from harmos.v1.status_pb2 import StatusGetRequest, StatusGetResponse, StatusWatchRequest, StatusWatchResponse
from harmos.v1.types_pb2 import CATALOG_KIND_OPERATIONS

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


async def run(address: str) -> tuple[CatalogEntriesListResponse, StatusGetResponse, StatusWatchResponse]:
    client = Client.connect(address)
    try:
        # Discover Operations
        catalog = await client.catalog_entries.list(CatalogEntriesListRequest(kind=CATALOG_KIND_OPERATIONS))
        print(catalog)

        # Read Typed Host Status
        status = await client.status.get(StatusGetRequest())
        print(status)

        # Observe One Typed Update, Then Cancel Demand
        updates = client.status.watch(StatusWatchRequest())
        try:
            async for update in updates:
                print(update)
                return catalog, status, update
        finally:
            updates.cancel()
        raise RuntimeError("Watch Status: Host Closed Without an Update")
    finally:
        await client.close()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    options = parser.parse_args()
    asyncio.run(run(options.address))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
