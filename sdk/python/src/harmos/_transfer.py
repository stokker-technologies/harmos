# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Storage Transfer Message Limits"""

# === Imports === #

# Standard Library Imports

# Third Party Imports

# Local Imports
from ._wire.v1.transfer_pb2 import TRANSFER_FRAMING_BYTES, MessageSettings, StorageTransferSettings

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def message_bytes(settings: StorageTransferSettings | None = None, ceiling: int | None = None) -> int:
    settings = settings if settings is not None else StorageTransferSettings()
    ceiling = MessageSettings().message_bytes if ceiling is None else ceiling
    required = settings.chunk_bytes + TRANSFER_FRAMING_BYTES
    if settings.chunk_bytes == 0 or required > 2**31 - 1:
        raise ValueError("Configure Transfer: StorageSettings.transfer.chunk_bytes exceeds transport capacity")
    if ceiling < required:
        raise ValueError("Configure Transfer: message_bytes cannot carry StorageSettings.transfer.chunk_bytes plus framing")
    return ceiling


# === Private Functions === #

# === Entry Point === #
