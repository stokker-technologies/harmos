# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Prepare Only The Project Fixture Inputs Consumed By The Selected Rust Lane"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import stat
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from ..build.fixtures import ROOT
from ..build import paths

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main(arguments: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--category", choices=("integration", "acceptance", "system"), required=True)
    options = parser.parse_args(arguments)
    profile, category = options.profile, options.category
    fixtures = paths.output(ROOT, "build", profile) / "fixtures"
    stamp_path = fixtures / "stamp.json"
    if not stamp_path.is_file():
        sys.exit(f"fixture stamp {stamp_path} is absent; run mise run build:tests")
    stamp = json.loads(stamp_path.read_text(encoding="utf-8"))
    if stamp.get("profile") != profile or stamp.get("target") != paths.target():
        sys.exit("fixture stamp belongs to another profile or native target")
    selectors = stamp["metadata"]
    if category == "integration":
        selected = [selectors["inspections"][name] for name in ("schematic-sidecar", "schematic-guest", "harmos-sandbox-guest")]
    else:
        executable = "cargo-harmos" if category == "acceptance" else "schematic-cli"
        selected = [selectors["artifacts"][name] for name in (executable, "schematic-sidecar", "schematic-guest")]
        if category == "system":
            selected.append(selectors["artifacts"]["harmos-sandbox-guest"])
    if category == "acceptance":
        selected.extend(selectors["libraries"].values())
        selected.extend(selectors[name] for name in ("vendor", "wheels", "requirements"))
    for relative in selected:
        if not _input(fixtures, relative).exists():
            sys.exit(f"fixture input {relative} is absent; run mise run build:tests")

    if category == "acceptance":
        # Starter Stories Share Mise's Runtime Links; Install Their Full Toolset Before Nextest Starts Them.
        environment = dict(os.environ)
        environment.pop("MISE_ENABLE_TOOLS", None)
        environment.pop("UV_PROJECT_ENVIRONMENT", None)
        subprocess.run(["mise", "install"], cwd=ROOT, env=environment, check=True)

    # The Native Runner Verifies Selected Stamp Hashes And Archive Ownership.
    # Only Hosts Used Here And Acceptance Compiler Inputs Need Executable Modes.
    if os.name == "nt" or category == "integration":
        return
    executables = [relative for relative in selectors["executables"] if relative in selected]
    if category == "acceptance":
        executables.extend(selectors["input_executables"])
    for relative in executables:
        path = _input(fixtures, relative)
        if not path.is_file():
            sys.exit(f"executable input {relative} is absent; run mise run build:tests")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)


# === Private Functions === #


def _input(fixtures: Path, relative: str) -> Path:
    path = (fixtures / relative).resolve()
    if not path.is_relative_to(fixtures.parent.resolve()):
        sys.exit(f"fixture input escapes its profile and native target: {relative}")
    return path


# === Entry Point === #

if __name__ == "__main__":
    main()
