# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Descriptor-Derived Client Root"""

# === Imports === #

# Standard Library Imports
import json
from importlib import import_module
from importlib.resources import files
from typing import Any

# Third Party Imports
from google.protobuf import descriptor_pool, message_factory
import grpc

# Local Imports
from .transfer import message_bytes

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Client:
    """Compose Client-Face Descriptors Into One Mechanical Resource Hierarchy."""

    def __getattr__(self, name: str) -> Any:
        raise AttributeError(name)

    @classmethod
    def connect(cls, address, *, transfer=None, message_ceiling=None):
        maximum = message_bytes(transfer, message_ceiling)
        channel = grpc.aio.insecure_channel(address.removeprefix("http://"), options=[
            ("grpc.max_send_message_length", maximum), ("grpc.max_receive_message_length", maximum)])
        client = cls(channel)
        client._channel = channel
        return client

    async def close(self):
        if self._channel is not None:
            await self._channel.close()

    def __init__(self, channel, *tables):
        self._channel = None
        builtin = json.loads(files("harmos").joinpath("resources.json").read_text(encoding="utf-8"))
        for table in tables or (builtin,):
            for method in table:
                if method["face"] != "client":
                    raise ValueError("Bind Client: Context and Instance Methods Are Not Client Capabilities")
                import_module(method["file"].removesuffix(".proto").replace("/", ".") + "_pb2")
                node = self
                for part in method["path"][:-1]:
                    if not hasattr(node, part):
                        setattr(node, part, _Resource())
                    node = getattr(node, part)
                leaf = method["path"][-1]
                if hasattr(node, leaf):
                    raise ValueError(f"Client Resource Collision: {'.'.join(method['path'])}")
                request = descriptor_pool.Default().FindMessageTypeByName(method["request"])
                response = descriptor_pool.Default().FindMessageTypeByName(method["response"])
                request_type = message_factory.GetMessageClass(request)
                response_type = message_factory.GetMessageClass(response)
                if method["client_streaming"]:
                    raise ValueError("Bind Client: Managed Methods Require Their Explicit Invocation API")
                factory = channel.unary_stream if method["server_streaming"] else channel.unary_unary
                rpc = factory(
                    f"/{method['package']}.{method['service']}/{method['method']}",
                    request_serializer=request_type.SerializeToString,
                    response_deserializer=response_type.FromString,
                )
                setattr(node, leaf, rpc)


class _Resource:
    pass


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
