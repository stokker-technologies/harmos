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
from ..fixtures import ROOT, TARGET, GUESTS

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--extended", action="store_true")
    options = parser.parse_args()
    with temporary.directory() as scratch:
        build(options.profile, scratch, extended=options.extended)


def build(profile: str, scratch: Path, *, extended: bool = False) -> None:
    fixtures = paths.output(ROOT, "build", profile) / "fixtures"
    os.environ["CARGO_TARGET_DIR"] = str(TARGET)
    cargo.archive(ROOT, profile, ["--all-features"] if extended else [])
    # Staging Starts From An Empty Fixture Directory, So The Stamp Names Only What This Build Produced.
    shutil.rmtree(fixtures, ignore_errors=True)
    fixtures.mkdir(parents=True)
    files: dict[str, str] = {}

    # Nextest Builds The CLI Test Harness; Sandbox Collection Needs Its Ordinary Executable.
    compiler_profile = "test"
    collector = staging.capture("collector", ROOT, compiler_profile, ["-p", "cargo-harmos"], scratch)
    tool = staging.executable(staging.messages(collector), "cargo-harmos")

    sandbox = "harmos-sandbox-guest"
    manifest = GUESTS[sandbox]
    capture = staging.capture(sandbox, manifest.parent, compiler_profile, ["-p", sandbox, "--target", "wasm32-unknown-unknown"], scratch)
    built, receipt = staging.collect(tool, manifest, sandbox, "guest", compiler_profile, capture, scratch)
    artifact, build = staging.stage_build(built, receipt, sandbox, files, fixtures)
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
            "artifacts": {sandbox: artifact},
            "inspections": {sandbox: artifact},
            "libraries": {},
            "builds": {sandbox: build},
            "build_roots": {sandbox: str(ROOT)},
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
