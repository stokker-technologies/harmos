# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Refuse A Test Lane Whose Fixture Stamp Is Absent, Stale Or Incomplete"""

# === Imports === #

# Standard Library Imports
import argparse
import os
import stat
import json
import sys

# Third Party Imports

# Local Imports
from ..build.fixtures import FIXTURES, ROOT, STAMP
from . import archive

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main(arguments: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release", "bench"), default="dev")
    profile = parser.parse_args(arguments).profile
    if not STAMP.is_file():
        sys.exit(f"fixture stamp {STAMP} is absent; run mise run build")
    stamp = json.loads(STAMP.read_text(encoding="utf-8"))
    selectors = stamp["metadata"]
    selected = [
        *selectors["artifacts"].values(),
        *selectors["libraries"].values(),
        *selectors["builds"].values(),
        selectors["vendor"],
        selectors["wheels"],
        selectors["requirements"],
    ]
    hosts = [(FIXTURES / relative).relative_to(ROOT).as_posix() for relative in selected]
    archive.fixtures(ROOT, archive.tree(ROOT), profile, hosts)
    if not set(selectors["executables"]).issubset(selectors["artifacts"].values()):
        sys.exit("executable fixture selectors must name verified artifacts")
    if os.name != "nt":
        for relative in selectors["executables"]:
            path = FIXTURES / relative
            path.chmod(path.stat().st_mode | stat.S_IXUSR)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
