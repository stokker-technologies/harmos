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
from importlib import import_module
from pathlib import Path
import sys

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures/client"
sys.path.insert(0, str(FIXTURES))
plugins = import_module("library.v1.plugins_pb2")
settings = import_module("library.v1.plugins.settings_pb2")
jobs = import_module("application.v1.jobs_pb2")
progress = import_module("application.v1.jobs.progress_pb2")


# === Types === #

# === Classes === #

# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
