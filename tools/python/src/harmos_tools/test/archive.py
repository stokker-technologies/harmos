# Rendered by stkr project update from templates/python/test/archive.py; sha256:2a054255f0417b778ec47365695532cad01da87d85c367d0b5c016b4ae252eb8
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build One Nextest Archive And Check The Fixture Files Consumed By Lanes."""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

# Third Party Imports

# Local Imports
from . import cache, policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ARCHIVE = "archive.tar.zst"
MESSAGES = "compiler.json"
FIXTURES = Path("target/fixtures/stamp.json")

# === Types === #

# === Classes === #

# === Public Functions === #


def revision(directory: Path) -> str | None:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=directory,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else None


def location(directory: Path, profile: str) -> Path:
    return directory / "target/nextest" / profile / ARCHIVE


def build(directory: Path, profile: str, extra: Sequence[str] = ()) -> Path:
    if profile not in (*policy.PROFILES, "bench"):
        raise policy.TestError("unknown build profile")
    cache.prepare()
    path = location(directory, profile)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.unlink(missing_ok=True)
    command = [
        "cargo",
        "nextest",
        "archive",
        "--locked",
        "--workspace",
        "--all-targets",
        "--cargo-profile",
        profile,
        "--cargo-message-format",
        "json",
        "--archive-file",
        str(path),
        *extra,
    ]
    with path.with_name(MESSAGES).open("w", encoding="utf-8") as output:
        result = subprocess.run(command, cwd=directory, stdout=output, check=False)
    if result.returncode:
        path.unlink(missing_ok=True)
        raise policy.TestError("nextest archive build failed")
    require(directory, profile)
    files = []
    for line in path.with_name(MESSAGES).read_text(encoding="utf-8").splitlines():
        message = json.loads(line)
        executable = message.get("executable")
        if executable and message.get("target", {}).get("kind") == ["bin"] and not message.get("profile", {}).get("test"):
            source = Path(executable)
            destination = directory / "target/fixtures/bin" / source.name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
            files.append(destination.relative_to(directory).as_posix())
    if files:
        stamp(directory, profile, files)
    return path


def binaries(directory: Path, profile: str) -> list[str]:
    path = require(directory, profile)
    listing = path.parent / "listing"
    listing.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "nextest",
        "list",
        "--archive-file",
        str(path),
        "--workspace-remap",
        str(directory),
        "--extract-to",
        str(listing),
        "--extract-overwrite",
        "--list-type",
        "binaries-only",
        "--message-format",
        "json",
    ]
    result = subprocess.run(command, cwd=directory, capture_output=True, text=True, check=False)
    if result.returncode:
        raise policy.TestError(f"nextest could not list test binaries: {result.stderr.strip()}")
    return sorted(
        {entry["binary-name"] for entry in json.loads(result.stdout).get("rust-binaries", {}).values() if entry.get("kind") == "test"}
    )


def require(directory: Path, profile: str) -> Path:
    path = location(directory, profile)
    if not path.is_file() or path.stat().st_size == 0:
        raise policy.TestError(f"missing {profile} nextest archive; run the build task first")
    return path


def install_wheels(directory: Path) -> None:
    wheels = sorted((directory / "target/wheels").glob("*.whl"))
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
            cwd=directory,
            check=True,
        )


def stamp(directory: Path, profile: str, files: Sequence[str], metadata: Mapping | None = None) -> None:
    """Name And Hash The Built Fixtures, Without Binding Source Or Revision."""
    record = {
        "profile": profile,
        "files": {name: _digest(_inside(directory, name)) for name in files},
        "metadata": dict(metadata or {}),
    }
    target = directory / FIXTURES
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def fixtures(directory: Path, profile: str = "dev", hosts: Sequence[str] = ()) -> None:
    path = directory / FIXTURES
    if not path.is_file():
        if hosts:
            raise policy.TestError("missing fixture stamp")
        return
    record = json.loads(path.read_text(encoding="utf-8"))
    if record.get("profile") != profile:
        raise policy.TestError("fixture stamp belongs to another profile")
    files = record.get("files", {})
    for name, digest in files.items():
        target = _inside(directory, name)
        if not target.is_file() or _digest(target) != digest:
            raise policy.TestError(f"missing or stale fixture: {name}")
    for host in hosts:
        name = host.format(
            profile=profile,
            platform=sys.platform,
            architecture=platform.machine(),
            target=record.get("metadata", {}).get("target", ""),
            exe=".exe" if os.name == "nt" else "",
        )
        if name not in files or not _inside(directory, name).is_file():
            raise policy.TestError(f"missing host fixture: {name}")


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build The Workspace Nextest Archive")
    parser.add_argument("--profile", choices=(*policy.PROFILES, "bench"), default="dev")
    parser.add_argument("--install-wheels", action="store_true")
    parser.add_argument("extra", nargs=argparse.REMAINDER)
    options = parser.parse_args(arguments)
    extra = options.extra[1:] if options.extra[:1] == ["--"] else options.extra
    try:
        if options.install_wheels:
            install_wheels(policy.root())
            return 0
        print(build(policy.root(), options.profile, extra))
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
    return subprocess.check_output(["rustc", "-Vv"], cwd=directory, text=True).strip()


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
