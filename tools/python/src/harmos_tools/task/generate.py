# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run the Generate Selection."""

# === Imports === #

# Standard Library Imports
import argparse

# Third Party Imports
# Local Imports
from . import execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    arguments = [argument for language in options.language or () for argument in ("--language", language)]
    if options.check:
        arguments.append("--check")
    return execute.module("build.codegen", *arguments)


# === Private Functions === #

# === Entry Point === #
