# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Lint Selected Languages."""

# === Imports === #

# Standard Library Imports
import argparse

# Third Party Imports
# Local Imports
from . import arguments, execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    languages = [options.language] if options.language else list(arguments.LANGUAGES)
    for language in languages:
        if language == "toml":
            continue
        if language == "python":
            # Prepare the Python SDK
            command = ["mise", "-C", "sdk/python", "run", "prepare"]
            execute.command(command)
        # Lint the Selected Language
        extra = ["--extended"] if language == "rust" and options.extended else []
        execute.task(f"lint:{language}", *extra)
    return 0


# === Private Functions === #

# === Entry Point === #
