# Rendered by stkr project update from templates/python/source/__init__.py; sha256:2962b7ad40ebbe5ee937d668b4c9daa3f0862a2a27d22f9e3889f3b6598c3236
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

# Seeded Templates Contain Jinja Until Instantiated
TEMPLATE_ROOT = Path(".stkr/templates")

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
    return not path.is_relative_to(TEMPLATE_ROOT)


# === Private Functions === #

# === Entry Point === #
