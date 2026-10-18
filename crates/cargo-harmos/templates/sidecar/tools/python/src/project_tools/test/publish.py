# Rendered by stkr project update from templates/python/test/publish.py; sha256:17f45586e89b465595e2b52520a987ad9b3718e5b91df9499bc5113a7cf90aeb
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Dry-Run Registry Publication Directly From The Checkout."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import subprocess
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def commands(directory: Path, rust: bool, wheels: list[str]) -> list[list[str]]:
    commands = []
    if rust:
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=directory))
        members = set(metadata["workspace_members"])
        for package in metadata["packages"]:
            if package["id"] in members and package.get("publish") != []:
                commands.append(["cargo", "publish", "--dry-run", "--locked", "--package", package["name"]])
    commands.extend(["uv", "build", "--wheel", "--project", project, "--out-dir", "target/publish-dry"] for project in wheels)
    return commands


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rust", action="store_true")
    parser.add_argument("--python", action="append", default=[])
    options = parser.parse_args()
    directory = policy.root()
    for command in commands(directory, options.rust, options.python):
        subprocess.run(command, cwd=directory, check=True)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
