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
import os
from pathlib import Path

# Third Party Imports
# Local Imports
from ..build import cargo
from . import execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    root = Path.cwd()
    import harmos_tools.build.resource_roots

    if options.descriptors:
        import harmos_tools.build.resource_roots

        if options.output is None:
            parser.error("composed client roots require --output")
        harmos_tools.build.resource_roots.compose(
            root, options.descriptors, options.output, options.namespace, language=options.sdk, composition=options.composition
        )
        return 0
    if options.sdk == "python":
        return execute.command(["mise", "-C", "sdk/python", "run", "generate"])
    if options.sdk == "csharp":
        return execute.command(["mise", "-C", "sdk/csharp", "run", "generate"])
    if options.sdk == "cpp":
        return execute.command(["mise", "-C", "sdk/cpp", "run", "generate", "--", "--profile", options.profile])
    if options.sdk == "rust":
        result = cargo.focused(root, "build", ["--locked", "--profile", options.profile, "-p", "harmos-protocol", "--example", "generate"])
        if result:
            return result
        binary = (
            root
            / "target"
            / ("debug" if options.profile == "dev" else "release")
            / "examples"
            / ("generate.exe" if os.name == "nt" else "generate")
        )
        result = execute.command([str(binary)])
        if result == 0:
            harmos_tools.build.resource_roots.generate(root)
        return result
    from ..build.__main__ import generate

    generate(options.profile)
    return 0


# === Private Functions === #

# === Entry Point === #
