# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Native gRPC Call Boundary for Generated Resources."""

# === Imports === #

from collections.abc import Sequence
from typing import cast

from google.protobuf.message import Message
from grpc import CallCredentials
from grpc.aio import AioRpcError, Channel, UnaryStreamCall, UnaryUnaryCall

# === Public Exports === #

# === Constants === #

# === Types === #

type Metadata = Sequence[tuple[str, str | bytes]]

# === Classes === #

# === Public Functions === #


async def unary[Q: Message, R: Message](
    channel: Channel,
    path: str,
    request: Q,
    response: type[R],
    *,
    action: str,
    metadata: Metadata | None = None,
    timeout: float | None = None,
    credentials: CallCredentials | None = None,
) -> R:
    rpc = channel.unary_unary(path, request_serializer=type(request).SerializeToString, response_deserializer=response.FromString)
    call = cast(UnaryUnaryCall[Q, R], rpc(request, metadata=metadata, timeout=timeout, credentials=credentials))
    try:
        return await call
    except AioRpcError as error:
        raise call_error(action, error) from error


def stream[Q: Message, R: Message](
    channel: Channel,
    path: str,
    request: Q,
    response: type[R],
    *,
    metadata: Metadata | None = None,
    timeout: float | None = None,
    credentials: CallCredentials | None = None,
) -> UnaryStreamCall[Q, R]:
    rpc = channel.unary_stream(path, request_serializer=type(request).SerializeToString, response_deserializer=response.FromString)
    return cast(UnaryStreamCall[Q, R], rpc(request, metadata=metadata, timeout=timeout, credentials=credentials))


def call_error(action: str, error: AioRpcError) -> AioRpcError:
    return AioRpcError(
        error.code(), error.initial_metadata(), error.trailing_metadata(), f"Call {action}: {error.details()}", error.debug_error_string()
    )


# === Private Functions === #

# === Entry Point === #
