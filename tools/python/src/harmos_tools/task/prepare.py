# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Run the Prepare Selection."""

# === Imports === #

# Standard Library Imports
import argparse
import os
import tomllib
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
    enabled = os.environ.get("MISE_ENABLE_TOOLS")
    if enabled is None or "rust" in enabled.split(","):
        toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]
        execute.command(
            [
                "rustup",
                "toolchain",
                "install",
                toolchain["channel"],
                "--profile",
                toolchain["profile"],
                "--component",
                ",".join(toolchain["components"]),
                "--target",
                ",".join(toolchain["targets"]),
            ]
        )
    return 0


# === Private Functions === #

# === Entry Point === #
