# Rendered by stkr project update from templates/python/process/__init__.py; sha256:9792f7143814143687e6bd3082fbd4ee8d67a8d930cf4fde6e3f8ee202bc12c7
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Preserve Attached Consoles And Keep Headless Child Processes Hidden."""

# === Imports === #

# Standard Library Imports
import ctypes
import os
import subprocess

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def spawn_flags(flags: int = 0) -> int:
    if os.name != "nt" or flags & subprocess.CREATE_NEW_CONSOLE:
        return flags

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetConsoleWindow.argtypes = []
    kernel32.GetConsoleWindow.restype = ctypes.c_void_p
    if kernel32.GetConsoleWindow():
        return flags
    return flags | subprocess.CREATE_NO_WINDOW


# === Private Functions === #

# === Entry Point === #
