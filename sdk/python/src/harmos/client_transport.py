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

import grpc
from google.protobuf.message import Message

# === Public Exports === #

# === Constants === #

# === Types === #

type Metadata = Sequence[tuple[str, str | bytes]]

# === Classes === #

# === Public Functions === #


def unary[Q: Message, R: Message](
    channel: grpc.aio.Channel,
    path: str,
    request: Q,
    response: type[R],
    *,
    metadata: Metadata | None = None,
    timeout: float | None = None,
    credentials: grpc.CallCredentials | None = None,
) -> grpc.aio.UnaryUnaryCall[Q, R]:
    rpc = channel.unary_unary(path, request_serializer=type(request).SerializeToString, response_deserializer=response.FromString)
    return cast(grpc.aio.UnaryUnaryCall[Q, R], rpc(request, metadata=metadata, timeout=timeout, credentials=credentials))


def stream[Q: Message, R: Message](
    channel: grpc.aio.Channel,
    path: str,
    request: Q,
    response: type[R],
    *,
    metadata: Metadata | None = None,
    timeout: float | None = None,
    credentials: grpc.CallCredentials | None = None,
) -> grpc.aio.UnaryStreamCall[Q, R]:
    rpc = channel.unary_stream(path, request_serializer=type(request).SerializeToString, response_deserializer=response.FromString)
    return cast(grpc.aio.UnaryStreamCall[Q, R], rpc(request, metadata=metadata, timeout=timeout, credentials=credentials))


# === Private Functions === #

# === Entry Point === #
