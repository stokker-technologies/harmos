# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Collect Maintained Source Files For Repository Tools"""

# === Imports === #

# Standard Library Imports
import subprocess
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


def files(directory: Path, *patterns: str) -> list[str]:
    listing = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", *patterns],
        cwd=directory,
        check=True,
        capture_output=True,
        text=True,
        creationflags=spawn_flags(),
    ).stdout
    return sorted({name for name in listing.split("\0") if name and maintained(Path(name)) and (directory / name).is_file()})


def maintained(path: Path) -> bool:
    """Exclude Private Dot-Directories Without Loading Their Templates."""
    return not any(part.startswith(".") for part in path.parts[:-1])


# === Private Functions === #

# === Entry Point === #
