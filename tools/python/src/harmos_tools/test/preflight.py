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
from ..build.fixtures import ROOT
from ..build import paths
from . import archive, policy

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
    fixtures = paths.output(ROOT, "build", profile) / "fixtures"
    stamp_path = fixtures / "stamp.json"
    if not stamp_path.is_file():
        sys.exit(f"fixture stamp {stamp_path} is absent; run mise run build:tests")
    stamp = json.loads(stamp_path.read_text(encoding="utf-8"))
    selectors = stamp["metadata"]
    selected = [
        *selectors["artifacts"].values(),
        *selectors["libraries"].values(),
        *selectors["builds"].values(),
    ]
    hosts = [(fixtures / relative).resolve().relative_to(ROOT).as_posix() for relative in selected]
    try:
        archive.fixtures(ROOT, profile, hosts)
    except policy.TestError as error:
        sys.exit(f"preflight: {error}")
    # Offline Inputs Are Too Many Files To Hash Per Lane; Their Locked Export Only Has To Be Present.
    for relative in (selectors["vendor"], selectors["wheels"], selectors["requirements"]):
        if not (fixtures / relative).exists():
            sys.exit(f"offline input {relative} is absent; run mise run build:tests")
    if not set(selectors["executables"]).issubset(selectors["artifacts"].values()):
        sys.exit("executable fixture selectors must name stamped artifacts")
    if os.name != "nt":
        for relative in [*selectors["executables"], *selectors["input_executables"]]:
            path = fixtures / relative
            path.chmod(path.stat().st_mode | stat.S_IXUSR)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
