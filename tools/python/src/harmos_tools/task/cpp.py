# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Select the C++ SDK Task."""

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
    if options.command == "build":
        if (
            options.package
            or options.features
            or options.example
            or options.check
            or options.artifact != "product"
            or options.compatibility
            or options.extended
        ):
            parser.error("C++ builds select only --language cpp and optional --profile")
        return execute.command(["mise", "-C", "sdk/cpp", "run", "build", "--", "--profile", options.profile])
    if options.command == "check":
        if options.package or options.repository or options.generated or options.extended:
            parser.error("C++ checks select only --language cpp")
        return execute.command(["mise", "-C", "sdk/cpp", "run", "check"])
    if (
        options.package
        or options.features
        or options.target
        or options.coverage
        or options.extended
        or options.filter
        or options.stress_count
        or options.layer not in (None, "unit", "integration")
    ):
        parser.error("C++ conformance selects --language cpp, optional --profile and --exact")
    selected = ["--profile", options.profile]
    if options.layer:
        selected += ["--layer", options.layer]
    if options.exact:
        selected += ["--exact", options.exact]
    return execute.command(["mise", "-C", "sdk/cpp", "run", "test", "--", *selected])


# === Private Functions === #

# === Entry Point === #
