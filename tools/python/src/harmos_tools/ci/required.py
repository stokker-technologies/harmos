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
from collections.abc import Mapping
from typing import Any

# Third Party Imports
# Local Imports
from .impact import JOBS

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def failures(results: Mapping[str, Any]) -> list[str]:
    """Accept Success or an Explicit Planned Skip; Require the Complete Job Inventory."""
    planner = results.get("impact", {})
    if planner.get("result") != "success":
        return ["impact"]
    try:
        outputs = planner["outputs"]
        selected = json.loads(outputs["selected"])
        skipped = json.loads(outputs["skipped"])
        valid = (
            isinstance(selected, list)
            and isinstance(skipped, list)
            and all(isinstance(job, str) for job in [*selected, *skipped])
            and len(selected) == len(set(selected))
            and len(skipped) == len(set(skipped))
            and not set(selected) & set(skipped)
            and set(selected) | set(skipped) == set(JOBS)
        )
    except (KeyError, TypeError, ValueError):
        return ["impact"]
    if not valid:
        return ["impact"]
    failed = []
    for name in JOBS:
        result = results.get(name, {}).get("result")
        if result == "success" or result == "skipped" and name in skipped:
            continue
        failed.append(name)
    failed.extend(name for name in results if name not in {*JOBS, "impact"})
    return failed


def run() -> int:
    results = json.loads(os.environ["RESULTS"])
    failed = failures(results)
    if failed:
        raise ValueError("Complete Required CI Jobs: Repair or Rerun Failed, Missing or Unexpectedly Skipped Jobs: " + ", ".join(failed))
    return 0


# === Private Functions === #

# === Entry Point === #
