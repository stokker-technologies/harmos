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
from .v1.storage_settings_pb2 import StorageTransferSettings, TRANSFER_FRAMING_BYTES

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def message_bytes(settings: StorageTransferSettings | None = None, ceiling: int | None = None) -> int:
    settings = settings if settings is not None else StorageTransferSettings()
    required = settings.chunk_bytes + TRANSFER_FRAMING_BYTES
    if settings.chunk_bytes == 0 or required > 2**31 - 1:
        raise ValueError("Configure Transfer: StorageSettings.transfer.chunk_bytes exceeds transport capacity")
    if ceiling is not None and ceiling < required:
        raise ValueError("Configure Transfer: message_bytes cannot carry StorageSettings.transfer.chunk_bytes plus framing")
    return required if ceiling is None else ceiling


# === Private Functions === #

# === Entry Point === #
