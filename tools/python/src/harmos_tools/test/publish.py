# Rendered by stkr project update from templates/python/test/publish.py; sha256:22420346939b3b1442b172057e08b6f5d30a45be4fe365fed2c4eed7e85bf1af
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Dry-Run Registry Publication Directly From The Checkout."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import subprocess
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy
from .. import temporary

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def commands(directory: Path, rust: bool, wheels: list[str], scratch: Path) -> list[list[str]]:
    commands = []
    if rust:
        metadata = json.loads(
            subprocess.check_output(
                ["cargo", "metadata", "--no-deps", "--format-version", "1"],
                cwd=directory,
            )
        )
        members = set(metadata["workspace_members"])
        command = ["cargo", "publish", "--dry-run", "--locked"]
        for package in metadata["packages"]:
            if package["id"] in members and package.get("publish") != []:
                command.extend(["--package", package["name"]])
        if "--package" in command:
            commands.append(command)
    commands.extend(
        [
            "uv",
            "build",
            "--wheel",
            "--project",
            project,
            "--out-dir",
            str(scratch / "wheels"),
        ]
        for project in wheels
    )
    return commands


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rust", action="store_true")
    parser.add_argument("--python", action="append", default=[])
    options = parser.parse_args()
    directory = policy.root()
    with temporary.directory() as scratch:
        for command in commands(directory, options.rust, options.python, scratch):
            environment = dict(os.environ)
            target = str(scratch / "cargo")
            for name in (
                "CARGO_TARGET_DIR",
                "CARGO_BUILD_TARGET_DIR",
                "CARGO_BUILD_BUILD_DIR",
            ):
                environment[name] = target
            subprocess.run(command, cwd=directory, check=True, env=environment)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
