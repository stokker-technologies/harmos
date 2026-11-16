# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Check Generated Imports from Their Native Working Directory."""

# === Imports === #

# Standard Library Imports
import argparse
import importlib
import pkgutil
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path)
    options = parser.parse_args()
    if options.directory is not None:
        if not options.directory.is_dir():
            parser.error("Generate the Python sources before checking their imports")
        # Run from the Generated Package Root
        command = [sys.executable, "-m", "harmos_tools.source.imports"]
        result = subprocess.run(
            command,
            cwd=options.directory,
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=spawn_flags(),
            check=False,
        )
        return result.returncode

    # Import Every Generated Module
    modules = pkgutil.walk_packages([str(Path.cwd())])
    for module in modules:
        importlib.import_module(module.name)
    return 0


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
