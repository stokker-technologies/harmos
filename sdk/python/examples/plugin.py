# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Serve One Public Echo Operation."""

# === Imports === #

import argparse
import asyncio

from google.protobuf.wrappers_pb2 import BytesValue

from harmos import Context, Plugin, operation
from harmos.v1.operation_pb2 import VISIBILITY_PUBLIC

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Echo:
    @operation("example.echo", request=BytesValue, response=BytesValue, visibility=VISIBILITY_PUBLIC)
    async def echo(self, context: Context, request: BytesValue) -> BytesValue:
        return request


# === Public Functions === #


async def run(address: str) -> None:
    plugin = Plugin(Echo(), id="example", version="1.0.0")
    endpoint = await plugin.start(address)
    print(endpoint.address, flush=True)
    try:
        await endpoint.server.wait_for_termination()
    finally:
        await endpoint.close()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    options = parser.parse_args()
    asyncio.run(run(options.address))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
