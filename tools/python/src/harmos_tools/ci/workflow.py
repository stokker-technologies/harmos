# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Select Hidden CI Operations."""

# === Imports === #

# Standard Library Imports


# Third Party Imports

# Local Imports
from ..task import execute
from . import cache_summary, mirror, nightly_freshness, nightly_report, release_mode, required

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run(operation: str, arguments: list[str], profile: str) -> int:
    arguments = arguments[1:] if arguments[:1] == ["--"] else arguments
    if operation == "review":
        return execute.module("release.review", *arguments)
    if operation in ("pack", "extract"):
        return execute.module("ci.artifacts", operation, "--profile", profile, *arguments)
    if arguments:
        raise ValueError(f"Run {operation} with inputs from the workflow environment")
    match operation:
        case "release-mode":
            return release_mode.run()
        case "required":
            return required.run()
        case "summary":
            return cache_summary.run()
        case "nightly-freshness":
            return nightly_freshness.run()
        case "nightly-report":
            return nightly_report.run()
        case "mirror":
            return mirror.run()
        case _:
            raise ValueError(f"Select a supported workflow operation: {operation}")


# === Private Functions === #

# === Entry Point === #
