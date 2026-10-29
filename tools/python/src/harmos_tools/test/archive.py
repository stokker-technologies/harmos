# Rendered by stkr project update from templates/python/test/archive.py; sha256:d3e1f5b3f98fd3037d05dc6760aa815033800c0963a1b5b7c50daa48738874b4
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Read Nextest Archives And Check The Fixture Files Consumed By Lanes."""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from .. import temporary
from ..build import paths
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ARCHIVE = "archive.tar.zst"
MESSAGES = "compiler.json"

# === Types === #

# === Classes === #

# === Public Functions === #


def revision(directory: Path) -> str | None:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        creationflags=spawn_flags(),
        cwd=directory,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else None


def location(directory: Path, profile: str) -> Path:
    return paths.output(directory, "build", profile) / "tests" / ARCHIVE


def prune(directory: Path, keep: set[str], *, archives: bool = False) -> None:
    """Discard Superseded Roots Without Following Links Or Touching Flat Reports."""
    for path in directory.iterdir():
        if path.name in keep or path.is_symlink():
            continue
        if archives and path.name.startswith("listing-"):
            continue
        if path.is_dir():
            shutil.rmtree(path)
        elif archives and path.name.endswith(".tar.zst"):
            path.unlink()


def binaries(directory: Path, profile: str) -> dict[str, str]:
    """Map Archive Binary IDs To Names, Retaining Package Identity For Measured Overrides."""
    path = require(directory, profile)
    with temporary.directory() as listing:
        extracted = listing / "extract"
        extracted.mkdir()
        command = [
            "cargo",
            "nextest",
            "list",
            "--archive-file",
            str(path),
            "--workspace-remap",
            str(directory),
            "--extract-to",
            str(extracted),
            "--list-type",
            "binaries-only",
            "--message-format",
            "json",
        ]
        result = subprocess.run(
            command,
            creationflags=spawn_flags(),
            cwd=directory,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode:
            raise policy.TestError(f"nextest could not list test binaries: {result.stderr.strip()}")
        return {identity: entry["binary-name"] for identity, entry in json.loads(result.stdout).get("rust-binaries", {}).items()}


def require(directory: Path, profile: str) -> Path:
    path = location(directory, profile)
    if not path.is_file() or path.stat().st_size == 0:
        raise policy.TestError(f"missing {profile} nextest archive; run the build task first")
    return path


def install_wheels(directory: Path, profile: str, native: str | None = None) -> None:
    targets = [native] if native is not None else [paths.target(), "any"]
    roots = [paths.output(directory, "build", profile, target) / "python" for target in targets]
    wheels = sorted(path for root in roots for path in root.glob("*.whl"))
    names = [path.name.split("-", 1)[0].replace("_", "-").lower() for path in wheels]
    if len(names) != len(set(names)):
        raise policy.TestError("ambiguous built wheels across selected native/any outputs")
    if any(root.is_dir() for root in roots) and not wheels:
        raise policy.TestError("missing built wheels in the selected profile/target")
    if wheels:
        subprocess.run(
            [
                "uv",
                "pip",
                "install",
                "--python",
                sys.executable,
                "--no-deps",
                "--reinstall",
                *map(str, wheels),
            ],
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=spawn_flags(),
            cwd=directory,
            check=True,
        )


def stamp(directory: Path, profile: str, files: Sequence[str], metadata: Mapping | None = None) -> None:
    """Name And Hash The Built Fixtures, Without Binding Source Or Revision."""
    record = {
        "profile": profile,
        "target": paths.target(),
        "files": {Path(name).as_posix(): _digest(_inside(directory, name)) for name in files},
        "metadata": dict(metadata or {}),
    }
    target = paths.output(directory, "build", profile) / "fixtures/stamp.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def fixtures(
    directory: Path,
    profile: str = "dev",
    required: Sequence[str] = (),
    *,
    roots: Sequence[str] | None = None,
) -> None:
    """Refuse stamped fixtures that changed, and required files the stamp does not cover."""
    required = [Path(name).as_posix() for name in required]
    roots = None if roots is None else [Path(name).as_posix() for name in roots]
    path = paths.output(directory, "build", profile) / "fixtures/stamp.json"
    if not path.is_file():
        if required:
            raise policy.TestError("missing fixture stamp")
        return
    record = json.loads(path.read_text(encoding="utf-8"))
    if record.get("profile") != profile:
        raise policy.TestError("fixture stamp belongs to another profile")
    if record.get("target") != paths.target():
        raise policy.TestError("fixture stamp belongs to another native target")
    files = record.get("files", {})
    selected = None if roots is None else [*roots, *required]
    for name, digest in files.items():
        if selected is not None and not any(name == scope.rstrip("/") or name.startswith(scope.rstrip("/") + "/") for scope in selected):
            continue
        target = _inside(directory, name)
        if not target.is_file() or _digest(target) != digest:
            raise policy.TestError(f"missing or stale fixture: {name}")
    for name in required:
        if name not in files or not _inside(directory, name).is_file():
            raise policy.TestError(f"missing stamped fixture: {name}")


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate Existing Test Inputs")
    parser.add_argument("--profile", choices=policy.PROFILES, default="dev")
    parser.add_argument("--install-wheels", action="store_true")
    parser.add_argument("--target")
    options = parser.parse_args(arguments)
    try:
        if options.install_wheels:
            install_wheels(policy.root(), options.profile, options.target)
            return 0
        print(require(policy.root(), options.profile))
        return 0
    except (policy.TestError, OSError, ValueError) as error:
        print(f"archive: {error}", file=sys.stderr)
        return 1


# === Private Functions === #


def _inside(directory: Path, name: str) -> Path:
    path = directory / name
    if Path(name).is_absolute() or not path.resolve().is_relative_to(directory.resolve()):
        raise policy.TestError(f"fixture path escapes the workspace: {name}")
    return path


def _digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _toolchain(directory: Path) -> str:
    return subprocess.check_output(
        ["rustc", "-Vv"],
        creationflags=spawn_flags(),
        cwd=directory,
        text=True,
    ).strip()


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
