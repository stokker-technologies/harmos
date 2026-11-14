# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Load the Generated Client Fixture Messages"""

# === Imports === #

# Standard Library Imports
from harmos._wire.client.v1 import status_pb2 as status
from harmos._wire.client.v1.status import diagnostics_pb2 as diagnostics
from tests.fixtures.client._wire.application.v1 import jobs_pb2 as jobs
from tests.fixtures.client._wire.application.v1.jobs import progress_pb2 as progress
from tests.fixtures.client._wire.application.v1.plugins import annotations_pb2 as annotations

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

__all__ = ["annotations", "diagnostics", "jobs", "progress", "status"]


# === Types === #

# === Classes === #

# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
