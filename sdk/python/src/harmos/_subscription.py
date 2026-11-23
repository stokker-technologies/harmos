# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Live Rows and Explicit Demand Closure."""

# === Imports === #

# Standard Library Imports
from dataclasses import dataclass
from typing import TYPE_CHECKING

# Third Party Imports
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

# Local Imports
from ._generated.proto.plugin.v1.subscriptions_pb2 import SubscriptionsSubscribeResponse

# Type Checking Imports
if TYPE_CHECKING:
    from ._invocation import _Watching

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


@dataclass(frozen=True)
class StreamRow[R: Message]:
    value: R
    source: Timestamp
    received: Timestamp


@dataclass(frozen=True)
class Gap:
    missed: int


class Subscription[R: Message]:
    def __init__(self, incoming: "_Watching[SubscriptionsSubscribeResponse]", row: type[R]) -> None:
        self._incoming = incoming
        self._row = row
        self._closed = False

    def __aiter__(self) -> "Subscription[R]":
        return self

    async def __anext__(self) -> StreamRow[R] | Gap:
        if self._closed:
            raise StopAsyncIteration
        response = await anext(self._incoming)
        kind = response.WhichOneof("item")
        if kind == "missed":
            return Gap(response.missed)
        if kind != "row" or not response.row.HasField("source") or not response.row.HasField("received"):
            raise ValueError("Read Subscription: Supply a Row With Both Timestamps or a Gap")
        row = response.row
        return StreamRow(self._row.FromString(row.payload), row.source, row.received)

    async def close(self) -> None:
        self._closed = True
        await self._incoming.aclose()


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
