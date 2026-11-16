# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run CI Cache Summary."""

# === Imports === #

# Standard Library Imports
import json
import os

# Third Party Imports
# Local Imports
from . import summary

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run() -> int:
    # Read Structured Input
    text = os.environ.get("CACHE_RESTORES", "{}")
    caches = json.loads(text)
    summary.render("Cache Summary", None, None, caches={name: {"Restore": value} for name, value in caches.items()})
    return 0


# === Private Functions === #

# === Entry Point === #
