# Rendered by stkr project update from templates/python/build/cargo.py; sha256:47d8b0298a1606d401cea2988303e46e8a3602afb2346035b3d8ba7b5d0c8aef
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Validate The Exact Cargo Configuration Before Its Compilation"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import subprocess
import sys
from collections.abc import Iterator, Mapping, Sequence
from pathlib import Path
from typing import Any

# Third Party Imports
# Local Imports
from ..ci import summary
from . import paths

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def clippy(directory: Path, arguments: Sequence[str], *, environment: Mapping[str, str] | None = None) -> None:
    selected = dict(os.environ if environment is None else environment, CARGO_BUILD_WARNINGS="deny")
    with summary.phase("Clippy"):
        subprocess.run(
            ["cargo", "clippy", *arguments],
            cwd=directory,
            env=selected,
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            check=True,
        )


def archive(directory: Path, profile: str, extra: Sequence[str] = ()) -> Path:
    from ..test import archive as reader
    from ..test import policy

    if profile not in policy.PROFILES:
        raise policy.TestError("unknown build profile")
    path = reader.location(directory, profile)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.unlink(missing_ok=True)
    compiler_profile = "test"
    clippy(
        directory,
        ["--locked", "--workspace", "--tests", "--profile", compiler_profile, *extra],
    )
    command = [
        "cargo",
        "nextest",
        "archive",
        "--locked",
        "--workspace",
        "--tests",
        "--cargo-profile",
        compiler_profile,
        "--cargo-message-format",
        "json",
        "--archive-file",
        str(path),
        *extra,
    ]
    with summary.phase("Compilation And Archive"), path.with_name(reader.MESSAGES).open("w", encoding="utf-8") as output:
        result = subprocess.run(
            command,
            stdin=sys.stdin,
            stderr=sys.stderr,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            cwd=directory,
            stdout=output,
            check=False,
        )
    if result.returncode:
        path.unlink(missing_ok=True)
        for line in path.with_name(reader.MESSAGES).read_text(encoding="utf-8").splitlines():
            message = json.loads(line)
            diagnostic = message.get("message", {}).get("rendered")
            if diagnostic:
                print(diagnostic, file=sys.stderr, end="")
        raise policy.TestError(f"nextest archive build failed; compiler messages: {path.with_name(reader.MESSAGES)}")
    reader.require(directory, profile)
    reader.prune(path.parent, {reader.ARCHIVE, reader.MESSAGES}, archives=True)
    return path


def record_target(directory: Path, messages: Path, profile: str, native: str | None = None) -> None:
    """Record The Actual Rust Target For Generic And Authored Product Builders."""
    compiler = subprocess.run(
        ["rustc", "-vV"],
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        cwd=directory,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    host = next(line.removeprefix("host: ") for line in compiler.splitlines() if line.startswith("host: "))
    triple = os.environ.get("CARGO_BUILD_TARGET", host)
    compiled_targets = set()
    for event in artifacts(directory, messages):
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


def artifacts(directory: Path, messages: Path) -> Iterator[dict[str, Any]]:
    for line in messages.read_text(encoding="utf-8").splitlines():
        event = json.loads(line)
        if event.get("reason") != "compiler-artifact" or event.get("profile", {}).get("test"):
            continue
        manifest = Path(event["manifest_path"])
        if manifest.resolve().is_relative_to(directory.resolve()) and "target" not in manifest.relative_to(directory).parts:
            yield event


def main() -> None:
    if sys.argv[1:2] == ["archive"]:
        parser = argparse.ArgumentParser(description="Build A Nextest Archive")
        parser.add_argument("--profile", choices=("dev", "release"), default="dev")
        options, extra = parser.parse_known_args(sys.argv[2:])
        extra = extra[1:] if extra[:1] == ["--"] else extra
        archive(Path.cwd(), options.profile, extra)
        return
    parser = argparse.ArgumentParser(description="Validate A Declared Cargo Target In Build")
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    arguments = options.arguments[1:] if options.arguments[:1] == ["--"] else options.arguments
    clippy(Path.cwd(), arguments)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
