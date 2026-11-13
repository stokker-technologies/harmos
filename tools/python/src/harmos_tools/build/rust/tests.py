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
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from ... import temporary
from ...ci import summary
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

    compiler_profile = "test"
    binaries = {}
    messages = archive.location(ROOT, profile).with_name(archive.MESSAGES)
    artifacts = list(cargo.artifacts(ROOT, messages))
    for message in artifacts:
        fixture = Path(message["manifest_path"]) == ROOT / "tools/conformance/Cargo.toml"
        if not fixture or message["target"]["kind"] != ["bin"] or not message.get("executable"):
            continue
        source = Path(message["executable"])
        destination = fixtures / "bin" / source.name
        staging.copy(source, destination, files)
        binaries[message["target"]["name"]] = destination.relative_to(fixtures).as_posix()

    # Build and Retain the Two Native-Language SDK Programs Used by Installation Stories.
    subprocess.run(
        [sys.executable, "-m", "harmos_tools.ci.tasks", "build", "--language", "csharp", "--profile", profile], cwd=Path.cwd(), check=True
    )
    subprocess.run(
        [sys.executable, "-m", "harmos_tools.ci.tasks", "build", "--language", "cpp", "--profile", profile], cwd=Path.cwd(), check=True
    )
    configuration = "Debug" if profile == "dev" else "Release"
    csharp = ROOT / "sdk/csharp/Conformance/bin" / configuration / "net10.0"
    for source in csharp.iterdir():
        if source.is_file():
            staging.copy(source, fixtures / "csharp" / source.name, files)
    suffix = ".exe" if os.name == "nt" else ""
    binaries["harmos-csharp-conformance-plugin"] = f"csharp/harmos-csharp-conformance-plugin{suffix}"
    cpp = ROOT / "target/cpp" / profile
    source = cpp / configuration / f"harmos-cpp-conformance{suffix}" if os.name == "nt" else cpp / "harmos-cpp-conformance"
    staging.copy(source, fixtures / "bin" / source.name, files)
    binaries["harmos-cpp-conformance"] = f"bin/{source.name}"

    with summary.phase("Fixture Driver"):
        tool = staging.executable(artifacts, "fixture-driver")

    sandbox = "harmos-sandbox-guest"
    manifest = GUESTS[sandbox]
    with summary.phase("Guest Fixture"):
        capture = staging.capture(
            sandbox, manifest.parent, compiler_profile, ["-p", sandbox, "--target", "wasm32-unknown-unknown"], scratch
        )
    built, receipt = staging.collect(tool, manifest, sandbox, "guest", compiler_profile, capture, scratch)
    artifact, build = staging.stage_build(built, receipt, sandbox, files, fixtures)
    with summary.phase("Offline Inputs"):
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
            "artifacts": {sandbox: artifact, **binaries},
            "inspections": {sandbox: artifact},
            "libraries": {},
            "builds": {sandbox: build},
            "build_roots": {sandbox: str(ROOT)},
            "vendor": "inputs/vendor",
            "wheels": "inputs/wheels",
            "requirements": "inputs/requirements.txt",
            "executables": sorted(binaries.values()),
            "input_executables": executables,
        },
    )


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
