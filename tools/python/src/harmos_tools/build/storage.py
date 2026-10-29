# Rendered by stkr project update from templates/python/build/storage.py; sha256:05015423565f6c899d8182b19f67abd447d64c05fe2e776606dbf3dd4ccbd2e4
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Reclaim Idle Disposable Cargo Compiler Output Before Builds."""

# === Imports === #

# Standard Library Imports
import os
import re
import stat
from collections.abc import Generator
from contextlib import ExitStack, contextmanager
from pathlib import Path

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

LIMIT = 64 * 1024**3
PRESERVED = frozenset(
    ("build", "ci", "tests", "generated", "package", "packages", "reports", "documentation", "release-assets", "cache", "caches")
)

# === Types === #

# === Classes === #

# === Public Functions === #


def preflight(directory: Path, *, target: Path | None = None) -> None:
    print("Build: Storage Preflight", flush=True)
    root = directory.resolve()
    selected = Path(os.environ.get("CARGO_TARGET_DIR", "target")) if target is None else target
    target = selected if selected.is_absolute() else root / selected
    if not target.resolve().is_relative_to(root / "target"):
        raise ValueError("Cargo compiler target must remain inside the repository target")
    for ancestor in (target, *target.parents):
        if ancestor == root:
            break
        if ancestor.exists() or ancestor.is_symlink() or ancestor.is_junction():
            _metadata(ancestor)
    for name in ("CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
        if name in os.environ:
            selected = Path(os.environ[name])
            actual = selected if selected.is_absolute() else root / selected
            if actual.resolve() != target.resolve():
                raise ValueError(f"{name} must match the preflighted Cargo target")
    if not target.exists():
        return
    profiles = _profiles(target)
    # Live Compilers May Retire Scratch Entries During The Read-Only Size Pass.
    if sum(_scan(profile, allow_missing=True)[0] for profile in profiles) <= LIMIT:
        return
    with ExitStack() as ownership:
        for profile in profiles:
            ownership.enter_context(_lock(profile / ".cargo-lock"))
        snapshots = [_scan(profile) for profile in profiles]
        size = sum(snapshot[0] for snapshot in snapshots)
        if size <= LIMIT:
            return
        for _, entries in snapshots:
            for path in sorted(entries, key=lambda path: len(path.parts), reverse=True):
                mode = _metadata(path).st_mode
                if stat.S_ISDIR(mode):
                    path.rmdir()
                else:
                    path.unlink()
        print(f"Build preflight cleared {size} disposable compiler bytes", flush=True)


# === Private Functions === #


def _metadata(path: Path) -> os.stat_result:
    metadata = path.lstat()
    reparse = getattr(metadata, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)
    if stat.S_ISLNK(metadata.st_mode) or reparse:
        raise ValueError(f"Cargo compiler output must not contain links: {path}")
    return metadata


def _profiles(target: Path) -> list[Path]:
    _metadata(target)
    profiles = []
    for path in sorted(target.iterdir()):
        if path.name in PRESERVED:
            continue
        if not stat.S_ISDIR(_metadata(path).st_mode):
            continue
        if _profile(path):
            profiles.append(path)
        elif re.fullmatch(r"[a-z0-9_]+(?:-[a-z0-9_]+){2,}", path.name):
            for nested in sorted(path.iterdir()):
                if stat.S_ISDIR(_metadata(nested).st_mode) and _profile(nested):
                    profiles.append(nested)
    return profiles


def _profile(path: Path) -> bool:
    try:
        metadata = _metadata(path / ".cargo-lock")
    except FileNotFoundError:
        return False
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError(f"Cargo profile lock must be a regular file: {path}")
    return True


def _scan(profile: Path, *, allow_missing: bool = False) -> tuple[int, list[Path]]:
    size = 0
    entries = []
    pending = [profile]
    while pending:
        directory = pending.pop()
        try:
            _metadata(directory)
            children = sorted(directory.iterdir())
        except FileNotFoundError:
            if not allow_missing or directory == profile:
                raise
            continue
        for path in children:
            if path == profile / ".cargo-lock":
                _metadata(path)
                continue
            try:
                metadata = _metadata(path)
            except FileNotFoundError:
                if not allow_missing:
                    raise
                continue
            if stat.S_ISDIR(metadata.st_mode):
                pending.append(path)
            elif stat.S_ISREG(metadata.st_mode):
                size += metadata.st_size
            else:
                raise ValueError(f"Cargo compiler output must contain regular files or directories: {path}")
            entries.append(path)
    return size, entries


@contextmanager
def _lock(path: Path) -> Generator[None]:
    _metadata(path)
    with path.open("r+b") as handle:
        try:
            if os.name == "nt":
                import msvcrt

                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise ValueError(f"Cargo compiler output is busy: {path.parent}") from error
        yield


# === Entry Point === #
