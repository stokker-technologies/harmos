# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run CI Nightly Report."""

# === Imports === #

# Standard Library Imports
import json
import os
from pathlib import Path

# Third Party Imports
# Local Imports
from ..release import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run() -> int:
    from ..release import nightly

    published = nightly.report(policy.root(), json.loads(os.environ["NEEDS"]), os.environ["RUN_URL"])
    output = Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8")
    try:
        output.write(f"published={str(published).lower()}\n")
    finally:
        output.close()
    return 0


# === Private Functions === #

# === Entry Point === #
