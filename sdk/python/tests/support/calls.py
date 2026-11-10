# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Frame Mechanics for an Author's Host."""

# === Imports === #

# Standard Library Imports

# Third Party Imports
from google.protobuf.message import Message

# Local Imports
from harmos.v1 import plugin_pb2 as wire

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


async def invoke(client, operation: str, request: Message):
    call = client.Invoke()
    await call.write(
        wire.HostFrame(request=wire.Request(operation=operation, payload=request.SerializeToString(), invocation="story", attempt=1))
    )
    return call


async def reply(call, child: wire.ChildRequest, response: Message) -> None:
    await call.write(wire.HostFrame(child=wire.ChildResponse(call=child.call, payload=response.SerializeToString())))


# === Private Functions === #

# === Entry Point === #
