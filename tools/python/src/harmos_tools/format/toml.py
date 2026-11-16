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
from ..process import spawn_flags
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
    command = ["taplo", "fmt", "--config", "tools/config/taplo.toml"]
    if check:
        command.append("--check")
    # Run the Native Command
    command = [*command, "--", *paths]
    subprocess.run(
        command,
        check=True,
        creationflags=spawn_flags(),
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
    )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
