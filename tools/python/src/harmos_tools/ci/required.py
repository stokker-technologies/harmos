# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run CI Required."""

# === Imports === #

# Standard Library Imports
import json
import os

# Third Party Imports


# Local Imports


# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run() -> int:
    results = json.loads(os.environ["RESULTS"])
    optional = set(json.loads(os.environ.get("OPTIONAL_JOBS", "[]")))
    if os.environ.get("RELEASE_REVIEW") != "true":
        optional.add("release_rehearsal")
    if any(job["result"] != "success" and not (name in optional and job["result"] == "skipped") for name, job in results.items()):
        raise ValueError("required CI jobs failed or were skipped; Inspect the required jobs and rerun failed or incorrectly skipped jobs")
    return 0


# === Private Functions === #

# === Entry Point === #
