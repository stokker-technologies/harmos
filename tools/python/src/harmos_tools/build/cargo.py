# Rendered by stkr project update from templates/python/build/cargo.py; sha256:4be9e48000a744fbf2bddfa7daa4279cc2ee1a5e71bce6028157872b7366224b
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Validate The Exact Cargo Configuration Before Its Compilation"""

# === Imports === #

# Standard Library Imports
import argparse
import os
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def clippy(directory: Path, arguments: Sequence[str], *, environment: Mapping[str, str] | None = None) -> None:
    selected = dict(os.environ if environment is None else environment, CARGO_BUILD_WARNINGS="deny")
    subprocess.run(
        ["cargo", "clippy", *arguments],
        cwd=directory,
        env=selected,
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate A Declared Cargo Target In Build")
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    arguments = options.arguments[1:] if options.arguments[:1] == ["--"] else options.arguments
    clippy(Path.cwd(), arguments)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
