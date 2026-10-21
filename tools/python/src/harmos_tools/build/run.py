# Rendered by stkr project update from templates/python/build/run.py; sha256:3aa214ffe27709147bd82ed95a8cd91e33ce8681361f9154a00675f67704972a
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build Packageable Products Without Test Harnesses Or Fixtures"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import shutil
import subprocess
from collections.abc import Iterator
from pathlib import Path
from typing import Any

# Third Party Imports
# Local Imports
from .. import temporary
from . import paths

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def rust(directory: Path, profile: str, native: str | None = None) -> None:
    destination = paths.output(directory, "build", profile, native)
    messages = destination / "product/compiler.json"
    messages.parent.mkdir(parents=True, exist_ok=True)
    command = ["cargo", "build", "--locked", "--workspace", "--profile", profile, "--message-format=json-render-diagnostics"]
    with messages.open("w", encoding="utf-8") as output:
        subprocess.run(command, cwd=directory, stdout=output, check=True)
    with temporary.directory() as staging:
        for name in ("bin", "lib"):
            (staging / name).mkdir()
        for event in _artifacts(directory, messages):
            executable = event.get("executable")
            if executable and event.get("target", {}).get("kind") == ["bin"]:
                source = Path(executable)
                shutil.copy2(source, staging / "bin" / source.name)
            elif any(
                kind in event.get("target", {}).get("kind", []) for kind in ("lib", "rlib", "cdylib", "dylib", "staticlib", "proc-macro")
            ):
                for filename in event.get("filenames", []):
                    source = Path(filename)
                    shutil.copy2(source, staging / "lib" / source.name)
        for name in ("bin", "lib"):
            paths.replace(directory, staging / name, destination / name, origin=staging)
    record_target(directory, messages, profile, native)


def record_target(directory: Path, messages: Path, profile: str, native: str | None = None) -> None:
    """Record The Actual Rust Target For Generic And Authored Product Builders."""
    compiler = subprocess.run(["rustc", "-vV"], cwd=directory, capture_output=True, text=True, check=True).stdout
    host = next(line.removeprefix("host: ") for line in compiler.splitlines() if line.startswith("host: "))
    triple = os.environ.get("CARGO_BUILD_TARGET", host)
    compiled_targets = set()
    for event in _artifacts(directory, messages):
        for filename in event.get("filenames", []):
            parent = Path(filename).parent
            if parent.name == "deps":
                parent = parent.parent
            candidate = parent.parent.name
            if "-linux-" in candidate or "-windows-" in candidate or candidate.endswith("-apple-darwin"):
                compiled_targets.add(candidate)
    if len(compiled_targets) > 1:
        raise ValueError("Product Build exported more than one Rust target")
    actual = next(iter(compiled_targets), triple)
    if "CARGO_BUILD_TARGET" in os.environ and actual != triple:
        raise ValueError("Product Build target differs from CARGO_BUILD_TARGET")
    destination = paths.output(directory, "build", profile, native) / "product/target.txt"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(actual + "\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Produce Canonical Build Outputs")
    parser.add_argument("language", choices=("rust", "python", "typescript"))
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--project", default=".")
    parser.add_argument("--target")
    options = parser.parse_args()
    directory = Path.cwd()
    project = directory / options.project
    if not project.resolve().is_relative_to(directory.resolve()):
        raise ValueError("build project escapes the repository")
    destination = paths.output(directory, "build", options.profile, options.target)
    if options.language == "rust":
        rust(directory, options.profile, options.target)
    elif options.language == "python":
        subprocess.run(
            ["uv", "build", "--project", str(project), "--wheel", "--out-dir", str(destination / "python")],
            cwd=directory,
            check=True,
        )
        if options.target == "any" and any(not wheel.name.endswith("-none-any.whl") for wheel in (destination / "python").glob("*.whl")):
            raise ValueError("the any Product producer requires platform-independent Python wheels")
    else:
        paths.replace(directory, project / "dist", destination / "web")


# === Private Functions === #


def _artifacts(directory: Path, messages: Path) -> Iterator[dict[str, Any]]:
    for line in messages.read_text(encoding="utf-8").splitlines():
        event = json.loads(line)
        if event.get("reason") != "compiler-artifact" or event.get("profile", {}).get("test"):
            continue
        manifest = Path(event["manifest_path"])
        if manifest.resolve().is_relative_to(directory.resolve()) and "target" not in manifest.relative_to(directory).parts:
            yield event


# === Entry Point === #

if __name__ == "__main__":
    main()
