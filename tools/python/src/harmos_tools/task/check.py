# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Check Selected Languages and Repository Inputs."""

# === Imports === #

# Standard Library Imports
import argparse
from pathlib import Path

# Third Party Imports
# Local Imports
from . import arguments, cpp, csharp, execute

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def run(options: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    root = Path.cwd()
    if options.language == "csharp":
        return csharp.run(options, parser)
    if options.language == "cpp":
        return cpp.run(options, parser)
    if options.package:
        if options.language != "python" or options.repository or options.generated or options.extended:
            parser.error("SDK checks require --language python --package harmos-sdk")
        return execute.task("check:python:sdk")
    if options.repository:
        return execute.task("check:repository")
    if options.generated or options.language in (None, "python"):
        if not (root / "target/generated/python/src").is_dir():
            parser.error(
                "generated Python clients are absent; run `mise run build` before the complete Python check (hosted CI supplies Product outputs)"
            )
    if options.generated:
        return execute.task("check:python:generated")
    languages = [options.language] if options.language else list(arguments.LANGUAGES)
    for language in languages:
        # Check Formatting
        execute.task(f"format:{language}", "--check")
        if language == "python":
            # Check Python
            command = ["mise", "-C", "sdk/python", "run", "prepare"]
            execute.command(command)
            execute.task("lint:python")
            execute.module("build.codegen", "--language", "python", "--check")
        if language == "rust":
            # Check Rust
            extra = ["--extended"] if options.extended else []
            execute.task("lint:rust", *extra)
            execute.task("check:rust")
        if language == "typescript":
            # Check the Documentation Site
            execute.task("lint:typescript")
    # Check Repository Inputs
    execute.task("check:repository")
    if options.language in (None, "python"):
        execute.task("check:python:generated")
    return 0


# === Private Functions === #

# === Entry Point === #
