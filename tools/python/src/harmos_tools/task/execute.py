# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Execute Task Children with Their Native Failure Status."""

# === Imports === #

# Standard Library Imports
import os
import subprocess
import sys
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def command(command: list[str], *, directory: Path | None = None, environment: dict[str, str] | None = None) -> int:
    result = subprocess.run(
        command,
        cwd=directory,
        env=environment,
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        creationflags=spawn_flags(),
        check=True,
    )
    return result.returncode


def task(name: str, *arguments: str) -> int:
    selected = ["mise", "run", name, "--", *arguments]
    compiling = name.startswith("build:") or name == "lint:rust" or name.startswith("release:publish:")
    if compiling and os.environ.get("COMPILER_CACHE_BACKEND"):
        from ..ci import build

        label = name.replace(":", " / ").replace("-", " ").title()
        status = build.run(selected, label)
        if status:
            raise SystemExit(status)
        return status
    return command(selected)


def module(name: str, *arguments: str) -> int:
    selected = [sys.executable, "-m", f"harmos_tools.{name}", *arguments]
    return command(selected)


# === Private Functions === #

# === Entry Point === #
