# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Format Selected Languages."""

# === Imports === #

# Standard Library Imports
import argparse

# Third Party Imports
# Local Imports
from . import arguments, csharp, execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    if options.language == "csharp":
        return csharp.run(options, parser)
    languages = [options.language] if options.language else list(arguments.LANGUAGES)
    for language in languages:
        # Format the Selected Language
        extra = ["--check"] if options.check else []
        execute.task(f"format:{language}", *extra)
    return 0


# === Private Functions === #

# === Entry Point === #
