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
import json
import sys

# Third Party Imports

# Local Imports
from ..build.fixtures import FIXTURES, STAMP, digest

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    if not STAMP.is_file():
        sys.exit(f"fixture stamp {STAMP} is absent; run mise run build")
    stamp = json.loads(STAMP.read_text(encoding="utf-8"))
    if stamp["inputs"] != digest():
        sys.exit("fixture stamp is stale: sources changed since the last build; run mise run build")
    missing = [path for path in (*stamp["artifacts"].values(), *stamp["libraries"].values()) if not (FIXTURES / path).is_file()]
    if missing:
        sys.exit(f"stamped fixtures are missing: {missing}; run mise run build")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
