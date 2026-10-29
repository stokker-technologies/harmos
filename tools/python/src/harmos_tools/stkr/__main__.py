# Rendered by stkr project update from templates/python/stkr/__main__.py; sha256:53831cfc6c831efc8ab26165f0ab977e1bee39c2306be05c15fc7eccb15b6a5f
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Run The Hub CLI From PATH Or The Mise Shims, Or Report It Unavailable.

`python -m <tools>.stkr [--optional] <arguments>` runs `stkr <arguments>` with this process's standard streams and
returns its exit code. Without the CLI an `--optional` call prints "stkr unavailable" and succeeds, so hooks and
optional checks pass on machines without the Hub; any other call fails and names `stkr machine install`.
"""

# === Imports === #

# Standard Library Imports
import os
import shutil
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports

# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

UNAVAILABLE = "stkr unavailable"

# === Types === #

# === Classes === #

# === Public Functions === #


def locate() -> str | None:
    """The Hub CLI on PATH, else in the mise shims directory."""
    found = shutil.which("stkr")
    if found is not None:
        return found
    data = os.environ.get("MISE_DATA_DIR")
    shims = Path(data) / "shims" if data else Path.home() / ".local/share/mise/shims"
    return shutil.which("stkr", path=str(shims))


def main(arguments: Sequence[str] | None = None) -> int:
    arguments = list(sys.argv[1:] if arguments is None else arguments)
    optional = arguments[:1] == ["--optional"]
    if optional:
        arguments = arguments[1:]
    executable = locate()
    if executable is None:
        if optional:
            print(UNAVAILABLE, file=sys.stderr)
            return 0
        print(f"{UNAVAILABLE}; install it with stkr machine install", file=sys.stderr)
        return 127
    return subprocess.call(
        [executable, *arguments],
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        creationflags=spawn_flags(),
    )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
