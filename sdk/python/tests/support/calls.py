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
from grpc.aio import StreamStreamCall

from harmos._wire.plugin.v1.instance_pb2 import InstanceInvokeRequest, InstanceInvokeResponse
from harmos._wire.plugin.v1.instance_pb2_grpc import InstanceStub
from harmos._wire.v1.invocation_pb2 import ChildRequest, ChildResponse, Request

# Local Imports


# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


async def invoke(client: InstanceStub, operation: str, request: Message) -> StreamStreamCall[InstanceInvokeRequest, InstanceInvokeResponse]:
    call = client.Invoke()
    await call.write(
        InstanceInvokeRequest(request=Request(operation=operation, payload=request.SerializeToString(), invocation="story", attempt=1))
    )
    return call


async def reply(
    call: StreamStreamCall[InstanceInvokeRequest, InstanceInvokeResponse],
    child: ChildRequest,
    response: Message,
) -> None:
    await call.write(InstanceInvokeRequest(child=ChildResponse(call=child.call, payload=response.SerializeToString())))


# === Private Functions === #

# === Entry Point === #
