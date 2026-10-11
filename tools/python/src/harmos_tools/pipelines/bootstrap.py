# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Verify native runner prerequisites and prepare temporary storage."""

# === Imports === #

# Standard Library Imports
import argparse
import os
import shutil
import sys
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main(argv: Sequence[str] | None = None) -> int:
    """Create temporary storage and verify native compiler prerequisites."""
    parse_arguments(argv)
    temporary = os.environ.get("TMPDIR")
    if temporary:
        path = Path(temporary)
        _step(f"creating {path}")
        path.mkdir(parents=True, exist_ok=True)
    else:
        _step("skipping temporary directory: TMPDIR is unset")

    required = ("cc", "pkg-config") if os.name != "nt" else ("cl.exe",)
    for command in required:
        if shutil.which(command) is None:
            raise RuntimeError(f"native prerequisite {command} is unavailable")
    _step("bootstrap complete")
    return 0


def parse_arguments(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="bootstrap.py",
        description="Verify native runner prerequisites.",
    )
    return parser.parse_args(argv)


# === Private Functions === #


def _step(message: str) -> None:
    print(f"[ci:bootstrap] {message}", flush=True)


# === Entry Point === #

if __name__ == "__main__":
    sys.exit(main())
