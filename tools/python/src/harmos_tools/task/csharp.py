# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Select the C# SDK Task."""

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
    incompatible = (
        "package",
        "features",
        "example",
        "compatibility",
        "extended",
        "target",
        "exact",
        "coverage",
        "filter",
        "stress_count",
        "repository",
        "generated",
    )
    if any(getattr(options, name, None) for name in incompatible):
        parser.error("C# selects only --language, --profile and --layer integration")
    if getattr(options, "artifact", "product") != "product" or getattr(options, "layer", None) not in (None, "unit", "integration"):
        parser.error("C# builds its SDK and tests its one integration story")
    if options.command == "build" and options.check:
        parser.error("C# formatting uses check --language csharp")
    selected = []
    if options.command in ("build", "test", "package"):
        selected += ["--profile", options.profile]
    if getattr(options, "layer", None):
        selected += ["--layer", options.layer]
    if getattr(options, "check", False):
        selected += ["--check"]
    return execute.command(["mise", "-C", "sdk/csharp", "run", options.command, "--", *selected])


# === Private Functions === #

# === Entry Point === #
