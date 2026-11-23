# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Build Canonical Shipping Products"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

# Third Party Imports
# Local Imports
from .. import temporary
from ..process import spawn_flags
from . import cargo, fixtures, paths
from .fixtures import ROOT, SCHEMATIC, TARGET

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    options = parser.parse_args()
    scratch_directory = temporary.Directory()
    scratch = scratch_directory.path
    try:
        product(options.profile, scratch)
    finally:
        scratch_directory.close()


def product(profile: str, scratch: Path) -> None:
    os.environ["CARGO_TARGET_DIR"] = str(TARGET)
    export = paths.output(ROOT, "build", profile)
    messages = export / "product/compiler.json"
    messages.parent.mkdir(parents=True, exist_ok=True)
    arguments = ["--locked", "--workspace", "--profile", profile]
    output = messages.open("w", encoding="utf-8")
    try:
        # Run the Native Command
        command = ["cargo", "build", "--timings", *arguments, "--message-format=json-render-diagnostics"]
        subprocess.run(
            command,
            cwd=ROOT,
            stdout=output,
            stderr=sys.stderr,
            stdin=sys.stdin,
            creationflags=spawn_flags(),
            check=True,
        )
    finally:
        output.close()
    staging_directory = temporary.Directory()
    staging = staging_directory.path
    try:
        for name in ("bin", "lib"):
            (staging / name).mkdir()
        for event in cargo.artifacts(ROOT, messages):
            executable = event.get("executable")
            kinds = event.get("target", {}).get("kind", [])
            if executable and kinds == ["bin"]:
                source = Path(executable)
                shutil.copy2(source, staging / "bin" / source.name)
            elif any(kind in kinds for kind in ("lib", "rlib", "cdylib", "dylib", "staticlib", "proc-macro")):
                for filename in event.get("filenames", []):
                    source = Path(filename)
                    shutil.copy2(source, staging / "lib" / source.name)
        for name in ("bin", "lib"):
            paths.replace(ROOT, staging / name, export / name, origin=staging)
    finally:
        staging_directory.close()
    cargo.record_target(ROOT, messages, profile)

    examples = export / "product/examples"
    shutil.rmtree(examples, ignore_errors=True)
    examples.mkdir(parents=True)
    source = fixtures.driver(profile, scratch)
    tool = export / "product/tools" / source.name
    tool.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, tool)
    native = fixtures.capture("schematic", SCHEMATIC.parent, profile, ["-p", "schematic-cli", "-p", "schematic-sidecar"], scratch)
    files: dict[str, str] = {}
    # Export The Actual Shipping Libraries While Producer-Local Compiler Paths Still Exist.
    libraries = fixtures.libraries(fixtures.messages(native), files, examples)
    artifacts = {}
    files[tool.relative_to(ROOT).as_posix()] = fixtures.digest(tool)
    cli = fixtures.executable(fixtures.messages(native), "schematic-cli")
    fixtures.copy(cli, export / "bin" / cli.name, files)
    artifacts["schematic-cli"] = f"bin/{cli.name}"

    protoc = _protoc()

    builds = {}
    for package in ("schematic-sidecar", "schematic-guest"):
        capture = (
            fixtures.capture(package, SCHEMATIC.parent, profile, ["-p", package, "--target", "wasm32-unknown-unknown"], scratch)
            if package == "schematic-guest"
            else native
        )
        backend = "sidecar" if package == "schematic-sidecar" else "guest"
        built, receipt = fixtures.collect(tool, SCHEMATIC, package, backend, profile, capture, scratch)
        artifact, staged = fixtures.stage_build(built, receipt, package, files, examples)
        source = examples / artifact
        filename = source.name if package == "schematic-sidecar" else "schematic-guest.wasm"
        destination = export / "bin/plugins" / filename
        fixtures.copy(source, destination, files)
        artifacts[package] = destination.relative_to(export).as_posix()
        builds[package] = f"product/examples/{staged}"
        if package == "schematic-sidecar":
            # Generate From the Staged Schema Before the Guest Build.
            fixtures.generate_python(tool, export / builds[package], protoc, files)

    catalog = {
        "artifacts": artifacts,
        "builds": builds,
        "build_roots": {name: str(ROOT) for name in builds},
        "libraries": {name: f"product/examples/{path}" for name, path in libraries.items()},
        "files": files,
    }
    (examples / "catalog.json").write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")


def _protoc() -> Path:
    # Read Structured Input
    # Run the Native Command
    command = ["cargo", "metadata", "--locked", "--format-version", "1"]
    text = subprocess.check_output(
        command,
        cwd=ROOT,
        creationflags=spawn_flags(),
        stdin=sys.stdin,
        stderr=sys.stderr,
    )
    metadata = json.loads(text)
    platform = {"linux": "linux-x86_64", "darwin": "macos-x86_64", "win32": "win32"}[sys.platform]
    package = next(package for package in metadata["packages"] if package["name"] == f"protoc-bin-vendored-{platform}")
    return Path(package["manifest_path"]).parent / "bin" / ("protoc.exe" if os.name == "nt" else "protoc")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
