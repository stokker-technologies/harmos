# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Discover and Invoke an Encoded Operation"""

# === Imports === #

# Standard Library Imports
import asyncio
from argparse import ArgumentParser

# Third Party Imports
# Local Imports
from harmos import Client

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


async def run(address: str, operation: str, payload: bytes) -> bytes:
    client = Client.connect(address)
    try:
        # Discover the Expected Contract
        selected = await client.operations.get(operation)

        # Create the Invocation with Its Encoded Request
        return await client.invocations.create(selected.contract.id, selected.contract.fingerprint, payload)
    finally:
        await client.close()


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    parser.add_argument("--operation", required=True)
    parser.add_argument("--payload", default="", help="Hex-Encoded Request Payload")
    options = parser.parse_args()
    payload = bytes.fromhex(options.payload)
    response = asyncio.run(run(options.address, options.operation, payload))
    print(response.hex())


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
