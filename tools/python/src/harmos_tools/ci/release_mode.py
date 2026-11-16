# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run CI Release Mode."""

# === Imports === #

# Standard Library Imports
import os
from pathlib import Path

# Third Party Imports


# Local Imports


# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def run() -> int:
    from ..release import review

    result = review.github_release_mode()
    output = Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8")
    try:
        output.write("".join(f"{key}={value}\n" for key, value in result.items()))
    finally:
        output.close()
    return 0


# === Private Functions === #

# === Entry Point === #
