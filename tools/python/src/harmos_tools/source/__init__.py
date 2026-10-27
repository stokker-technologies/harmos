# Rendered by stkr project update from templates/python/source/__init__.py; sha256:38559478dd89b73ade004043a28900ae420f5932a2e3300ab60e81ee420d0cad
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
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
    ).stdout
    return sorted({name for name in listing.split("\0") if name and maintained(Path(name)) and (directory / name).is_file()})


def maintained(path: Path) -> bool:
    return not path.is_relative_to(TEMPLATE_ROOT)


# === Private Functions === #

# === Entry Point === #
