# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run the Bench Selection."""

# === Imports === #

# Standard Library Imports
import argparse
from pathlib import Path

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
    root = Path.cwd()
    if options.storage:
        from .. import storage_bench

        return storage_bench.run(root, options.filter)
    if options.filter:
        parser.error("--filter requires --storage")
    layers = [options.layer] if options.layer else ["unit", "integration", "system"]
    extra = ["--extended"] if options.extended else []
    if options.refresh_baseline:
        extra.append("--refresh-baseline")
    for layer in layers:
        execute.task(f"bench:rust:{layer}", *extra)
    return 0


# === Private Functions === #

# === Entry Point === #
