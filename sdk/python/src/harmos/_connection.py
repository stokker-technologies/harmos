# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Typed Host Client and Explicit Connection Ownership."""

# === Imports === #

from typing import Self

import grpc
from grpc.aio import Channel

from ._generated.client import ClientRoot
from ._generated.shared import StorageTransferSettings
from ._transfer import message_bytes

# === Public Exports === #


# === Constants === #

# === Types === #

# === Classes === #


class Connection:
    """Connection Ownership Shared by Library and Composed Resource Roots."""

    def __init__(self, channel: Channel) -> None:
        self._channel = channel
        self._owned_channel: Channel | None = None

    @classmethod
    def connect(cls, address: str, *, transfer: StorageTransferSettings | None = None, message_ceiling: int | None = None) -> Self:
        maximum = message_bytes(transfer, message_ceiling)
        channel = grpc.aio.insecure_channel(
            address.removeprefix("http://"),
            options=[("grpc.max_send_message_length", maximum), ("grpc.max_receive_message_length", maximum)],
        )
        client = cls(channel)
        client._owned_channel = channel
        return client

    async def close(self) -> None:
        if self._owned_channel is not None:
            await self._owned_channel.close()


class Client(Connection, ClientRoot):
    """Library Resources, With an Optional Owned Connection."""

    def __init__(self, channel: Channel) -> None:
        ClientRoot.__init__(self, channel)
        Connection.__init__(self, channel)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
