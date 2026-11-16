# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Select Repository Task Steps and Transport Their Artifacts."""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess
import sys
from pathlib import Path

# Third Party Imports
# Local Imports
from ..task import arguments, bench, build, check, format, generate, lint, package, prepare, release, test
from . import workflow

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def main(arguments: list[str] | None = None) -> int:
    from . import artifacts

    # Nested Task Leaves Must Not Repeat Transport Or See The Outer Job's Plan.
    inputs = os.environ.pop("CI_INPUT_ARCHIVES", "[]")
    outputs = os.environ.pop("CI_OUTPUT_ARCHIVES", "[]")
    selected = arguments if arguments is not None else sys.argv[1:]
    if "--help" in selected or "-h" in selected:
        return dispatch(arguments)
    root = Path.cwd()
    archives = [Path(path) for path in json.loads(inputs)]
    for archive in archives:
        artifacts.extract(root, archive)
    status = dispatch(arguments)
    if status:
        return status
    from ..build.paths import target

    for output in json.loads(outputs):
        artifacts.pack(root, Path(output["archive"]), output["paths"], output.get("profile", os.environ.get("PROFILE", "dev")), target())
    return status


def dispatch(selected: list[str] | None = None) -> int:
    parser, options = arguments.parse(selected)
    if options.command.startswith("ci:"):
        return workflow.run(options.command.partition(":")[2], options.arguments, options.profile)
    match options.command:
        case "prepare":
            return prepare.run(options, parser)
        case "generate":
            return generate.run(options, parser)
        case "format":
            return format.run(options, parser)
        case "lint":
            return lint.run(options, parser)
        case "check":
            return check.run(options, parser)
        case "build":
            return build.run(options, parser)
        case "test":
            return test.run(options, parser)
        case "bench":
            return bench.run(options, parser)
        case "package":
            return package.run(options, parser)
        case _:
            return release.run(options, parser)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode) from None
