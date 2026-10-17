# Rendered by stkr project update from templates/python/test/archive.py; sha256:f61fe2f5f88beab5b576eeb55548267a1792e631ef2befd0f4e5df9fe9c9e267
# ***
#
# __PRODUCT_NAME__
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build The Workspace Nextest Archive And Bind It To The Source Tree Hash.

    python -m <package>.test.archive [--profile dev|release] [-- CARGO_ARGUMENTS...]

Writes `target/nextest/<profile>/archive.tar.zst` and `archive.json` with the tree hash, the
revision, the profile, the toolchain and the archive digest. Test lanes run from this archive
and refuse it when the tree has moved since it was built.

The tree hash is `git write-tree` over a copy of the index with every working-tree change
added (`git add -A`, then `git rm -r --cached --ignore-unmatch .stkr`), so it names exactly
the tracked and unignored content on disk outside the Hub state directory and matches the hash
`stkr` computes.
"""

# === Imports === #

# Standard Library Imports
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports

# Local Imports
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ARCHIVE = "archive.tar.zst"
RECEIPT = "archive.json"
FIXTURES = "target/fixtures/stamp.json"
CHUNK = 1 << 20

# === Types === #

# === Classes === #

# === Public Functions === #


def tree(directory: Path) -> str:
    """The hash of the working tree as `git add -A` would stage it, without touching the index."""
    index = Path(_git(directory, "rev-parse", "--git-path", "index"))
    index = index if index.is_absolute() else directory / index

    with tempfile.TemporaryDirectory(prefix="stkr-tree-") as scratch:
        copy = Path(scratch) / "index"
        if index.is_file():
            shutil.copyfile(index, copy)
        environment = dict(os.environ, GIT_INDEX_FILE=str(copy))
        _git(directory, "add", "-A", environment=environment)
        _git(
            directory,
            "rm",
            "-r",
            "-q",
            "--cached",
            "--ignore-unmatch",
            ".stkr",
            environment=environment,
        )
        return _git(directory, "write-tree", environment=environment)


def revision(directory: Path) -> str | None:
    """The checked-out commit, or None outside a repository with history."""
    try:
        return _git(directory, "rev-parse", "--verify", "HEAD")
    except policy.TestError:
        return None


def published(directory: Path, current: str) -> str | None:
    """The commit evidence may cite: HEAD when the tree hash is exactly its tree."""
    head = revision(directory)
    if head is None or _git(directory, "rev-parse", "HEAD^{tree}") != current:
        return None
    return head


def location(directory: Path, profile: str) -> Path:
    return directory / "target" / "nextest" / profile / ARCHIVE


def build(directory: Path, profile: str, extra: Sequence[str] = ()) -> dict:
    """Archive every workspace target and record what it was built from."""
    before = tree(directory)
    path = location(directory, profile)
    receipt = path.with_name(RECEIPT)
    path.parent.mkdir(parents=True, exist_ok=True)
    receipt.unlink(missing_ok=True)

    command = [
        "cargo",
        "nextest",
        "archive",
        "--locked",
        "--workspace",
        "--all-targets",
        "--cargo-profile",
        profile,
        "--archive-file",
        str(path),
        *extra,
    ]
    if _run(command, directory) != 0:
        raise policy.TestError("nextest archive build failed")
    names = listed(directory, profile, extra)

    # Edits During Compilation Make The Archive Unattributable.
    if tree(directory) != before:
        raise policy.TestError("the source tree changed while the archive was built")
    record = {
        "tree": before,
        "revision": published(directory, before),
        "profile": profile,
        "arguments": list(extra),
        "binaries": names,
        "toolchain": _toolchain(directory),
        "sha256": _digest(path),
    }
    receipt.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return record


def listed(directory: Path, profile: str, extra: Sequence[str] = ()) -> list[str]:
    """The test binary names of the workspace, from nextest's listing of the already built targets."""
    command = ["cargo", "nextest", "list", "--workspace", "--all-targets", "--cargo-profile", profile]
    command += ["--list-type", "binaries-only", "--message-format", "json", *extra]
    try:
        result = subprocess.run(command, cwd=directory, capture_output=True, text=True, check=False)
    except OSError as error:
        raise policy.TestError(f"cargo nextest is unavailable: {error}") from error
    if result.returncode != 0:
        raise policy.TestError(f"nextest could not list the test binaries: {result.stderr.strip()}")
    binaries = json.loads(result.stdout).get("rust-binaries", {}).values()
    return sorted({binary["binary-name"] for binary in binaries if binary.get("kind") == "test"})


def binaries(directory: Path, profile: str) -> list[str]:
    """The test binary names recorded with the archive of this profile."""
    receipt = location(directory, profile).with_name(RECEIPT)
    if not receipt.is_file():
        return []
    return list(json.loads(receipt.read_text(encoding="utf-8")).get("binaries", []))


def require(directory: Path, profile: str, current: str) -> Path:
    """The archive of this profile, refused when absent or built from another tree."""
    path = location(directory, profile)
    receipt = path.with_name(RECEIPT)
    if not path.is_file() or not receipt.is_file():
        raise policy.TestError(f"no {profile} nextest archive; run the build that produces {path.relative_to(directory)}")
    record = json.loads(receipt.read_text(encoding="utf-8"))
    if record.get("tree") != current or record.get("profile") != profile:
        raise policy.TestError(f"the {profile} nextest archive was built from another tree; rebuild it")
    if record.get("sha256") != _digest(path):
        raise policy.TestError(f"the {profile} nextest archive digest does not match its build receipt")
    if record.get("toolchain") != _toolchain(directory):
        raise policy.TestError(f"the {profile} nextest archive uses another Rust toolchain")
    return path


def fixtures(directory: Path, current: str) -> None:
    """Refuse a fixture stamp from another tree; projects without fixtures have no stamp."""
    stamp = directory / FIXTURES
    if not stamp.is_file():
        return
    record = json.loads(stamp.read_text(encoding="utf-8"))
    if record.get("tree") != current:
        raise policy.TestError(f"{FIXTURES} is stale; rebuild the fixtures")


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build The Workspace Nextest Archive")
    parser.add_argument("--profile", choices=policy.PROFILES, default="dev")
    parser.add_argument("--verify", action="store_true", help="Require The Existing Archive Without Compiling")
    parser.add_argument(
        "extra",
        nargs=argparse.REMAINDER,
        help="Cargo arguments after --, such as --all-features",
    )
    options = parser.parse_args(arguments)
    extra = options.extra[1:] if options.extra[:1] == ["--"] else options.extra
    try:
        directory = policy.root()
        if options.verify:
            if extra:
                raise policy.TestError("archive verification takes no Cargo build arguments")
            path = require(directory, options.profile, tree(directory))
            record = json.loads(path.with_name(RECEIPT).read_text(encoding="utf-8"))
        else:
            record = build(directory, options.profile, extra)
    except (policy.TestError, OSError, ValueError) as error:
        print(f"archive: {error}", file=sys.stderr)
        return 1
    print(json.dumps(record, indent=2))
    return 0


# === Private Functions === #


def _git(directory: Path, *arguments: str, environment: dict[str, str] | None = None) -> str:
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=directory,
            env=environment,
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=False,
        )
    except OSError as error:
        raise policy.TestError(f"git is unavailable: {error}") from error
    if result.returncode != 0:
        raise policy.TestError(f"git {arguments[0]} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def _run(command: Sequence[str], directory: Path) -> int:
    try:
        return subprocess.run(list(command), cwd=directory, check=False).returncode
    except OSError as error:
        raise policy.TestError(f"{command[0]} is unavailable: {error}") from error


def _toolchain(directory: Path) -> str:
    result = subprocess.run(
        ["rustc", "-V"],
        cwd=directory,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    return result.stdout.strip()


def _digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(CHUNK), b""):
            digest.update(chunk)
    return digest.hexdigest()


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
