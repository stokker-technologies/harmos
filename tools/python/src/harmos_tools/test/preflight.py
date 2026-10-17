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
import hashlib
import os
import stat
import json
import sys

# Third Party Imports

# Local Imports
from ..build.fixtures import FIXTURES, STAMP
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
    if stamp["tree"] != archive.tree(FIXTURES.parents[1]):
        sys.exit("fixture stamp is stale: sources changed since the last build; run mise run build")
    if stamp["profile"] != profile:
        sys.exit(f"fixture profile is {stamp["profile"]}, expected {profile}; rebuild the fixtures")
    hashes = stamp["hashes"]
    for relative in (*stamp["artifacts"].values(), *stamp["libraries"].values()):
        if relative not in hashes:
            sys.exit(f"fixture {relative} has no build digest; run mise run build")
    for relative, expected in hashes.items():
        path = FIXTURES / relative
        if not path.is_file():
            sys.exit(f"stamped fixture {relative} is missing; run mise run build")
        with path.open("rb") as stream:
            actual = hashlib.file_digest(stream, "sha256").hexdigest()
        if actual != expected:
            sys.exit(f"stamped fixture {relative} differs from the build; run mise run build")

    if os.name != "nt":
        for relative in stamp["executables"]:
            path = FIXTURES / relative
            path.chmod(path.stat().st_mode | stat.S_IXUSR)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
