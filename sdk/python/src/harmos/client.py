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

from .generated.client import ClientRoot
from .transfer import message_bytes
from .v1.storage_settings_pb2 import StorageTransferSettings

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Client(ClientRoot):
    """Library Resources, With an Optional Owned Connection."""

    def __init__(self, channel: grpc.aio.Channel) -> None:
        super().__init__(channel)
        self._owned_channel: grpc.aio.Channel | None = None

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


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
