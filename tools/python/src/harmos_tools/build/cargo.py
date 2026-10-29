# Rendered by stkr project update from templates/python/build/cargo.py; sha256:4c56dfaf08d51627877cae6593507f7ee12e2548441d4c0ac677c5f8f37733a5
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
from ..process import spawn_flags
from . import paths, storage

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def clippy(directory: Path, arguments: Sequence[str], *, environment: Mapping[str, str] | None = None) -> None:
    storage.preflight(directory)
    # CI Product And Tests Producers Leave Clippy To Check / Rust, Whose lint:rust Covers Their Selections.
    if os.environ.get("CHECK_CLIPPY") == "1":
        return
    selected = dict(os.environ if environment is None else environment, CARGO_BUILD_WARNINGS="deny")
    with summary.phase("Clippy"):
        subprocess.run(
            ["cargo", "clippy", *arguments],
            cwd=directory,
            env=selected,
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=spawn_flags(),
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
            creationflags=spawn_flags(),
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


def focused(directory: Path, operation: str, arguments: Sequence[str]) -> int:
    if operation not in ("build", "check", "test", "clippy", "run"):
        raise ValueError("focused compilation requires build, check, test, clippy or run")
    profile = None
    compiler_arguments = list(arguments[: arguments.index("--")]) if "--" in arguments else list(arguments)
    if any(argument == "--target-dir" or argument.startswith("--target-dir=") for argument in compiler_arguments):
        raise ValueError("focused tasks use the repository Cargo target")
    for index, argument in enumerate(compiler_arguments):
        if argument == "--profile" and index + 1 < len(compiler_arguments):
            profile = compiler_arguments[index + 1]
        elif argument.startswith("--profile="):
            profile = argument.partition("=")[2]
    if profile is not None and profile not in ("dev", "release", "test", "bench"):
        raise ValueError("focused compilation requires a canonical Cargo profile")
    if operation == "test" and ("--release" in compiler_arguments or profile not in (None, "test")):
        raise ValueError("focused tests use Cargo's default test profile")
    if operation in ("build", "test", "run"):
        clippy(directory, [argument for argument in compiler_arguments if argument not in ("--no-run", "--no-fail-fast")])
    else:
        storage.preflight(directory)
    return subprocess.run(
        ["cargo", operation, *arguments],
        cwd=directory,
        env=dict(os.environ, CARGO_BUILD_WARNINGS="deny"),
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
        creationflags=spawn_flags(),
        check=False,
    ).returncode


def record_target(directory: Path, messages: Path, profile: str, native: str | None = None) -> None:
    """Record The Actual Rust Target For Generic And Authored Product Builders."""
    compiler = subprocess.run(
        ["rustc", "-vV"],
        creationflags=spawn_flags(),
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
    if sys.argv[1:2] == ["focused"]:
        parser = argparse.ArgumentParser(description="Compile An Explicit Local Rust Selection")
        parser.add_argument("operation", choices=("build", "check", "test", "clippy"))
        parser.add_argument("arguments", nargs=argparse.REMAINDER)
        options = parser.parse_args(sys.argv[2:])
        raise SystemExit(focused(Path.cwd(), options.operation, options.arguments))
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
