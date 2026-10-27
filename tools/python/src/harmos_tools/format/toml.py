# Rendered by stkr project update from templates/python/format/toml.py; sha256:21e071f065677d8cc45c4b83841bc5219adbd5ac5b896eeba5d2df9f18a22f4a
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Format Maintained TOML Files"""

# === Imports === #

# Standard Library Imports
import argparse
import subprocess
import sys
from pathlib import Path

# Third Party Imports
# Local Imports
from ..source import files

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    check = parser.parse_args().check
    paths = files(Path.cwd(), "*.toml")
    if not paths:
        return
    command = ["taplo", "fmt", "--option", "column_width=140"]
    if check:
        command.append("--check")
    subprocess.run(
        [*command, "--", *paths],
        check=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
    )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
