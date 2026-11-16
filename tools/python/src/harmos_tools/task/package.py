# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run the Package Selection."""

# === Imports === #

# Standard Library Imports
import argparse

# Third Party Imports
# Local Imports
from . import csharp, execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    if options.language == "csharp":
        return csharp.run(options, parser)
    return execute.task("package:rust", "--profile", options.profile)


# === Private Functions === #

# === Entry Point === #
