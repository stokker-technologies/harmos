# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Serve One Public Echo Operation."""

# === Imports === #

import asyncio
from argparse import ArgumentParser

from google.protobuf.wrappers_pb2 import BytesValue

from harmos import VISIBILITY_PUBLIC, SettingDeclaration, SettingValue
from harmos.plugin import Context, HostOperation, Plugin, operation

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Echo:
    @operation("example.echo", request=BytesValue, response=BytesValue, visibility=VISIBILITY_PUBLIC)
    async def echo(self, context: Context, request: BytesValue) -> BytesValue:
        settings = await context.host.settings.list()
        prefix = settings.value.values["prefix"]
        if prefix.WhichOneof("value") != "text":
            raise ValueError("Echo: Prefix Setting Must Be Text")
        return BytesValue(value=prefix.text.encode() + request.value)


# === Public Functions === #


async def run(address: str) -> None:
    plugin = Plugin(
        Echo(),
        id="example",
        version="1.0.0",
        requires=[HostOperation.SETTINGS_LIST],
        settings=[SettingDeclaration(name="prefix", title="Echo Prefix", default=SettingValue(text=""))],
    )
    endpoint = await plugin.start(address)
    print(endpoint.address, flush=True)
    try:
        await endpoint.server.wait_for_termination()
    finally:
        await endpoint.close()


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--address", required=True)
    options = parser.parse_args()
    asyncio.run(run(options.address))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
