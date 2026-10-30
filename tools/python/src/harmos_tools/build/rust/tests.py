# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Build The Independent Rust Test Archive And Fixtures"""

# === Imports === #

# Standard Library Imports
import argparse
import os
import shutil
import stat
from pathlib import Path

# Third Party Imports

# Local Imports
from ... import temporary
from ...test import archive
from .. import fixtures as staging
from .. import cargo, paths
from ..fixtures import ROOT, TARGET

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--features")
    parser.add_argument("--extended", action="store_true")
    options = parser.parse_args()
    with temporary.directory() as scratch:
        build(options.profile, scratch, features=options.features, extended=options.extended)


def build(profile: str, scratch: Path, *, features: str | None = None, extended: bool = False) -> None:
    fixtures = paths.output(ROOT, "build", profile) / "fixtures"
    os.environ["CARGO_TARGET_DIR"] = str(TARGET)
    arguments = ["--features", features] if features else []
    if extended:
        arguments = ["--all-features"]
    cargo.archive(ROOT, profile, arguments)
    # Staging Starts From An Empty Fixture Directory, So The Stamp Names Only What This Build Produced.
    shutil.rmtree(fixtures, ignore_errors=True)
    fixtures.mkdir(parents=True)
    files: dict[str, str] = {}

    inputs = staging.offline_inputs(fixtures / "inputs")
    # Artifact Transfer Drops File Modes, So The Stamp Names The Staged Inputs Consumers Must Mark Executable Again.
    selected_inputs = (path for path in inputs.rglob("*") if path.is_file() and path.stat().st_mode & stat.S_IXUSR)
    executables = [path.relative_to(fixtures).as_posix() for path in selected_inputs]
    executables.sort()
    archive.stamp(
        ROOT,
        profile,
        sorted(files),
        {
            "artifacts": {},
            "inspections": {},
            "libraries": {},
            "builds": {},
            "build_roots": {},
            "vendor": "inputs/vendor",
            "wheels": "inputs/wheels",
            "requirements": "inputs/requirements.txt",
            "executables": [],
            "input_executables": executables,
        },
    )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
