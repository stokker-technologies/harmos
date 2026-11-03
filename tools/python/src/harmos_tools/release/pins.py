# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Reject Git Dependencies For Releases And Report Them In Nightly."""

# === Imports === #

# Standard Library Imports
import json
import os
from pathlib import Path

# Third Party Imports
# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def check(directory: Path, *, warn: bool = False) -> bool:
    """Check The Resolved Graph, Including Transitive Git Sources."""
    metadata = policy.execute(directory, ["cargo", "metadata", "--locked", "--format-version", "1"], capture=True)
    packages = json.loads(metadata)["packages"]
    names = sorted({package["name"] for package in packages if (package.get("source") or "").startswith("git+")})
    names = sorted(
        set(names)
        | {
            dependency["name"]
            for package in packages
            for dependency in package.get("dependencies", [])
            if (dependency.get("source") or "").startswith("git+")
        }
    )
    if not names:
        return True
    message = "release requires registry dependencies; git dependencies: " + ", ".join(names)
    if not warn:
        raise policy.ToolError(message)
    print("::warning::" + message + "; skipping crate publication dry run")
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(summary).open("a", encoding="utf-8") as stream:
            stream.write("\n### Release Pin Warning\n\n" + message + "\n")
    return False


# === Private Functions === #

# === Entry Point === #
