# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Format Every Maintained TOML File Without Traversing Build Outputs"""

# === Imports === #

# Standard Library Imports
import argparse
import subprocess
from pathlib import Path

# Third Party Imports

# Local Imports

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
    listing = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "*.toml"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    paths = sorted({name for name in listing.split("\0") if name and Path(name).is_file() and not _rendered(Path(name))})
    command = ["taplo", "fmt", "--option", "column_width=140"]
    if check:
        command.append("--check")
    subprocess.run([*command, *paths], check=True)


# === Private Functions === #

# === Entry Point === #


def _rendered(path: Path) -> bool:
    """Hub-rendered files keep the bytes `stkr project update` wrote."""
    with path.open(encoding="utf-8") as handle:
        head = [handle.readline() for _ in range(2)]
    return any("Rendered by stkr project update" in line for line in head)


if __name__ == "__main__":
    main()
