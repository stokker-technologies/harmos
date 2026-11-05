# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Own Disposable Simulation Workspaces Across Parent Termination."""

# === Imports === #

# Standard Library Imports
import ctypes
import hashlib
import json
import os
import re
import shutil
import sys
import tempfile
import uuid
from collections.abc import Callable, Generator
from contextlib import contextmanager
from pathlib import Path

# Third Party Imports
# Local Imports
from . import containers

# Type Checking Imports

# === Public Exports === #

# === Constants === #

MARKER = ".project-ci-owner.json"

# === Types === #

# === Classes === #


class Owner:
    def __init__(self, path: Path, record: dict):
        self.path = path
        self.record = record

    @property
    def root(self) -> Path:
        return Path(self.record["root"])

    def save(self) -> None:
        if linked(self.root):
            raise ValueError("workspace ownership must not be linked")
        if self.record["version"] == 2 and self.root.exists():
            write(self.root / MARKER, self.record)
        write(self.path, self.record)

    def starting(self) -> str:
        self.record["command"] = {"platform": sys.platform, "job": "project-ci-" + uuid.uuid4().hex, "state": "launching"}
        self.save()
        return self.record["command"]["job"]

    def attached(self, pid: int) -> None:
        self.record["command"].update(pid=pid, state="running")
        self.save()

    def settled(self) -> None:
        self.record.pop("command", None)
        self.save()

    def cleanup(self, preserve: Callable[[Path], None]) -> None:
        if linked(self.root) or linked(self.root / MARKER):
            raise ValueError("workspace ownership must not be linked")
        if not self.root.exists():
            self.record["phase"] = "clean"
            self.save()
            return
        marker = json.loads((self.root / MARKER).read_text(encoding="utf-8"))
        expected = (
            self.record if self.record["version"] == 2 else {key: self.record[key] for key in ("repository", "run", "job", "nonce", "root")}
        )
        if marker != expected:
            raise ValueError("workspace ownership marker does not match")
        if "command" in self.record and command_live(self.record["command"]):
            raise containers.Busy("workspace command is live or its ownership is unknown")
        verify_tree(self.root)
        preserve(self.root / "workspace")
        for path in self.root.iterdir():
            if path.name == MARKER:
                continue
            if path.is_symlink():
                path.unlink()
            elif path.is_junction():
                path.rmdir()
            elif path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink()
        try:
            (self.root / MARKER).unlink()
            self.root.rmdir()
        except OSError:
            if self.root.is_dir() and not linked(self.root) and not (self.root / MARKER).exists():
                (self.root / MARKER).write_text(json.dumps(marker), encoding="utf-8")
            raise
        self.record["phase"] = "clean"
        self.save()


# === Public Functions === #


def linked(path: Path) -> bool:
    return path.is_symlink() or path.is_junction()


def command_live(command: dict) -> bool:
    if not isinstance(command, dict) or not isinstance(command.get("job"), str) or command.get("platform") != sys.platform:
        return True
    if command.get("state") not in ("launching", "running") or re.fullmatch(r"project-ci-[a-f0-9]{32}", command["job"]) is None:
        return True
    if os.name == "nt":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenJobObjectW.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_wchar_p]
        kernel.OpenJobObjectW.restype = ctypes.c_void_p
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        handle = kernel.OpenJobObjectW(4, False, command["job"])
        if not handle:
            return ctypes.get_last_error() != 2
        try:
            return active_processes(kernel, handle) != 0
        finally:
            kernel.CloseHandle(handle)
    pid = command.get("pid")
    if not isinstance(pid, int) or pid <= 0 or not Path("/proc").is_dir():
        return True
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) == pid and fields[0] not in ("Z", "X"):
                return True
        except FileNotFoundError:
            continue
    return False


def active_processes(kernel, handle) -> int:
    if os.name != "nt":
        raise OSError("Windows command-tree accounting is unavailable on this platform")

    class Accounting(ctypes.Structure):
        _fields_ = [
            ("user", ctypes.c_longlong),
            ("kernel", ctypes.c_longlong),
            ("period_user", ctypes.c_longlong),
            ("period_kernel", ctypes.c_longlong),
            ("faults", ctypes.c_ulong),
            ("total", ctypes.c_ulong),
            ("active", ctypes.c_ulong),
            ("terminated", ctypes.c_ulong),
        ]

    kernel.QueryInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_ulong, ctypes.c_void_p]
    information = Accounting()
    if not kernel.QueryInformationJobObject(handle, 1, ctypes.byref(information), ctypes.sizeof(information), None):
        raise OSError(ctypes.get_last_error(), "cannot inspect owned command tree")
    return information.active


@contextmanager
def job(output: Path, repository: str, run: str, key: str, preserve: Callable[[Path], None], *, stable: bool = False) -> Generator[Owner]:
    path = output / f"{key}.workspace-owner.json"
    lock = path.with_suffix(".lock")
    if path.exists() or linked(path) or linked(lock):
        raise ValueError("workspace ownership already exists or is linked")
    lock.touch(exist_ok=True)
    with containers.lease(lock):
        # Stable Per-Job Paths Preserve Rust Cache Keys Without Sharing Mutable Workspaces.
        if re.fullmatch(r"[a-zA-Z0-9_-]+", repository) is None or re.fullmatch(r"[a-zA-Z0-9_-]+", key) is None:
            raise ValueError("invalid workspace identity")
        if stable:
            root = Path(tempfile.gettempdir()).resolve() / f"project-ci-job-{repository}-{key}-v2"
        else:
            root = Path(tempfile.gettempdir()).resolve() / f"project-ci-job-{repository}-{uuid.uuid4().hex}-v2"
        record = {
            "version": 2,
            "repository": repository,
            "output": str(output.resolve()),
            "run": run,
            "job": key,
            "nonce": uuid.uuid4().hex,
            "root": str(root),
            "phase": "active",
        }
        with lease(root, repository):
            if root.exists() or linked(root):
                raise ValueError(f"scratch retained at {root}; inspect its ownership marker and rerun ci:simulate for recovery")
            # Publish The Directory And Its Self-Contained Ownership Together.
            with tempfile.TemporaryDirectory(prefix="project-ci-stage-") as staging:
                staged = Path(staging)
                write(staged / MARKER, record)
                publish(staged, root)
            owner = Owner(path, record)
            try:
                owner.save()
                yield owner
            finally:
                cleanup(owner, preserve, required=True)


def cleanup(owner: Owner, preserve: Callable[[Path], None], *, required: bool = False) -> None:
    try:
        owner.cleanup(preserve)
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Simulation retained workspace ownership {owner.path}: {error}", file=sys.stderr, flush=True)
        if required:
            raise


def reconcile(root: Path, repository: str, preserve: Callable[[Path, str, Path], None]) -> None:
    if not root.exists() or linked(root):
        return
    for path in sorted(root.glob("*/*.workspace-owner.json")):
        if any(linked(part) for part in (path.parent, path)):
            continue
        lock = path.with_suffix(".lock")
        if not lock.is_file() or linked(lock):
            continue
        try:
            with containers.lease(lock):
                record = json.loads(path.read_text(encoding="utf-8"))
                if not valid(path, record, repository) or record["phase"] == "clean":
                    continue
                with lease(Path(record["root"]), repository):
                    cleanup(Owner(path, record), lambda workspace, output=path.parent, key=record["job"]: preserve(output, key, workspace))
        except (OSError, ValueError, RuntimeError) as error:
            print(f"Simulation retained workspace ownership {path}: {error}", file=sys.stderr, flush=True)


def recover(output: Path, repository: str, preserve: Callable[[Path, str, Path], None]) -> None:
    """Recover Scratch Even When Its Original Checkout And Run Records Are Gone."""
    for root in sorted(Path(tempfile.gettempdir()).resolve().glob(f"project-ci-job-{repository}-*")):
        try:
            with lease(root, repository):
                if linked(root) or linked(root / MARKER):
                    raise ValueError("scratch or ownership marker is linked")
                record = json.loads((root / MARKER).read_text(encoding="utf-8"))
                if not isinstance(record, dict) or record.get("version") != 2:
                    raise ValueError("legacy marker cannot prove owner liveness")
                if not all(isinstance(record.get(name), str) for name in ("output", "job")):
                    raise ValueError("invalid scratch ownership marker")
                old_path = Path(record.get("output", "")) / f"{record.get('job', '')}.workspace-owner.json"
                if not valid(old_path, record, repository) or Path(record["root"]) != root:
                    raise ValueError("invalid scratch ownership marker")
                # Retain Recovered Diagnostics In This Run, Without Recreating A Retired Checkout.
                destination = output / "recovered" / record["run"]
                destination.mkdir(parents=True, exist_ok=True)
                owner = Owner(destination / old_path.name, record)
                owner.cleanup(lambda workspace, destination=destination, key=record["job"]: preserve(destination, key, workspace))
        except (OSError, ValueError, RuntimeError) as error:
            print(
                f"Simulation retained scratch {root}: {error}. Inspect the marker and stop its owner before manual removal; "
                "unverifiable legacy scratch is never deleted automatically.",
                file=sys.stderr,
                flush=True,
            )


# === Private Functions === #


def valid(path: Path, record: object, repository: str) -> bool:
    if not isinstance(record, dict) or record.get("version") not in (1, 2) or record.get("repository") != repository:
        return False
    if not all(isinstance(record.get(key), str) for key in ("output", "run", "job", "nonce", "root")):
        return False
    target = Path(record["root"])
    return (
        Path(record["output"]).is_absolute()
        and record["output"] == str(path.parent.resolve())
        and path.name == f"{record['job']}.workspace-owner.json"
        and re.fullmatch(r"[a-zA-Z0-9_-]+", record["run"]) is not None
        and re.fullmatch(r"[a-zA-Z0-9_-]+", record["job"]) is not None
        and re.fullmatch(r"[a-f0-9]{32}", record["nonce"]) is not None
        and target.is_absolute()
        and target.parent == Path(tempfile.gettempdir()).resolve()
        and target.name.startswith("project-ci-job-")
        and (
            record["version"] == 1
            or re.fullmatch(rf"project-ci-job-{re.escape(repository)}-(?:{re.escape(record['job'])}|[a-f0-9]{{32}})-v2", target.name)
            is not None
        )
        and record.get("phase") in ("active", "clean")
    )


def write(path: Path, record: dict) -> None:
    staging = path.with_suffix(".tmp")
    if linked(path) or linked(staging):
        raise ValueError("workspace ownership must not be linked")
    with staging.open("w", encoding="utf-8") as file:
        json.dump(record, file)
        file.flush()
        os.fsync(file.fileno())
    staging.replace(path)


def verify_tree(root: Path) -> None:
    device = root.stat().st_dev
    pending = [root]
    while pending:
        with os.scandir(pending.pop()) as entries:
            for entry in entries:
                if entry.is_symlink() or entry.is_junction() or not entry.is_dir(follow_symlinks=False):
                    continue
                if os.stat(entry.path, follow_symlinks=False).st_dev != device:
                    raise ValueError("scratch contains a foreign filesystem; retain it for inspection")
                pending.append(Path(entry.path))


def publish(staged: Path, root: Path) -> None:
    if os.name == "nt":
        staged.rename(root)
        return
    # Linux rename() Can Replace A Foreign Empty Directory Created After The Existence Check.
    library = ctypes.CDLL(None, use_errno=True)
    rename = getattr(library, "renameat2", None)
    if rename is None:
        raise OSError("atomic scratch publication requires Linux renameat2 or Windows rename")
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(-100, os.fsencode(staged), -100, os.fsencode(root), 1):
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), str(root))


@contextmanager
def lease(root: Path, repository: str) -> Generator[None]:
    cache = (
        Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData/Local")
        if os.name == "nt"
        else Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
    )
    directory = cache / "ci-simulation" / repository / "workspace-leases"
    if any(linked(part) for part in (directory, *directory.parents)):
        raise ValueError("scratch lease storage must not be linked")
    directory.mkdir(parents=True, exist_ok=True)
    identity = hashlib.sha256(os.path.normcase(str(root)).encode()).hexdigest()
    path = directory / f"{identity}.lock"
    if linked(path):
        raise ValueError("scratch lease must not be linked")
    path.touch(exist_ok=True)
    with containers.lease(path):
        yield


# === Entry Point === #
