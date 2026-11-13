# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Frame Mechanics for an Author's Host."""

# === Imports === #

import grpc

# Standard Library Imports
# Third Party Imports
from google.protobuf.message import Message

from harmos.plugin.v1 import instance_pb2 as carrier
from harmos.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos.v1.invocation_pb2 import ChildRequest, ChildResponse, Request

# Local Imports


# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


async def invoke(
    client: InstanceStub, operation: str, request: Message
) -> grpc.aio.StreamStreamCall[carrier.InstanceInvokeRequest, carrier.InstanceInvokeResponse]:
    call = client.Invoke()
    await call.write(
        carrier.InstanceInvokeRequest(
            request=Request(operation=operation, payload=request.SerializeToString(), invocation="story", attempt=1)
        )
    )
    return call


async def reply(
    call: grpc.aio.StreamStreamCall[carrier.InstanceInvokeRequest, carrier.InstanceInvokeResponse],
    child: ChildRequest,
    response: Message,
) -> None:
    await call.write(carrier.InstanceInvokeRequest(child=ChildResponse(call=child.call, payload=response.SerializeToString())))


# === Private Functions === #

# === Entry Point === #
