# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Serialize Workstation Native Tasks Across Repositories And Worktrees"""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess
import sys
from collections.abc import Generator
from contextlib import contextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import BinaryIO

# Third Party Imports
# Local Imports
from ..process import spawn_flags

# Type Checking Imports


# === Public Exports === #

__all__ = ["lock"]

# === Constants === #

INHERITED = "STKR_NATIVE_BUILD_LOCK"

# === Types === #

# === Classes === #

# === Public Functions === #


@contextmanager
def lock() -> Generator[None]:
    cache = _cache()
    path = cache / "native-build.lock"
    if os.environ.get(INHERITED) == str(path):
        yield
        return
    cache.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_RDWR | os.O_CREAT, 0o600)
    with os.fdopen(descriptor, "r+b") as file:
        print(f"Waiting For Machine Build Lock: {path}", flush=True)
        with _operating_system_lock(file):
            file.seek(1)
            previous = file.read().decode("utf-8", errors="replace")
            if previous.strip():
                print(f"Recovered Stale Build Holder Metadata: {previous}", flush=True)
            holder = {
                "pid": os.getpid(),
                "project": Path(__file__).parents[1].name.removesuffix("_tools"),
                "workspace": str(Path.cwd()),
                "task": os.environ.get("MISE_TASK_NAME", " ".join(sys.argv)),
                "started": datetime.now(UTC).isoformat(),
                "command": sys.argv,
            }
            file.seek(0)
            file.write(b"\0")
            file.seek(1)
            file.write(json.dumps(holder).encode("utf-8"))
            file.truncate()
            file.flush()
            prior = os.environ.get(INHERITED)
            os.environ[INHERITED] = str(path)
            print(f"Acquired Machine Build Lock: PID {os.getpid()}", flush=True)
            try:
                yield
            finally:
                if prior is None:
                    os.environ.pop(INHERITED, None)
                else:
                    os.environ[INHERITED] = prior
                file.seek(1)
                file.truncate()
                file.flush()


# === Private Functions === #


def _cache() -> Path:
    if os.name == "nt":
        return Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData/Local")
    return Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))


@contextmanager
def _operating_system_lock(file: BinaryIO) -> Generator[None]:
    if os.name != "nt":
        import fcntl

        try:
            fcntl.flock(file, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            _waiting(file)
            fcntl.flock(file, fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(file, fcntl.LOCK_UN)
        return

    import ctypes
    import msvcrt
    from ctypes import wintypes

    class Overlapped(ctypes.Structure):
        _fields_ = [
            ("Internal", ctypes.c_size_t),
            ("InternalHigh", ctypes.c_size_t),
            ("Offset", wintypes.DWORD),
            ("OffsetHigh", wintypes.DWORD),
            ("hEvent", wintypes.HANDLE),
        ]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LockFileEx.argtypes = [
        wintypes.HANDLE,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.DWORD,
        ctypes.POINTER(Overlapped),
    ]
    kernel.LockFileEx.restype = wintypes.BOOL
    kernel.UnlockFileEx.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.DWORD, wintypes.DWORD, ctypes.POINTER(Overlapped)]
    kernel.UnlockFileEx.restype = wintypes.BOOL
    handle = msvcrt.get_osfhandle(file.fileno())
    overlapped = Overlapped()
    if not kernel.LockFileEx(handle, 3, 0, 1, 0, ctypes.byref(overlapped)):
        if ctypes.get_last_error() != 33:  # ERROR_LOCK_VIOLATION
            raise ctypes.WinError(ctypes.get_last_error())
        _waiting(file)
        if not kernel.LockFileEx(handle, 2, 0, 1, 0, ctypes.byref(overlapped)):
            raise ctypes.WinError(ctypes.get_last_error())
    try:
        yield
    finally:
        if not kernel.UnlockFileEx(handle, 0, 1, 0, ctypes.byref(overlapped)):
            raise ctypes.WinError(ctypes.get_last_error())


def _waiting(file: BinaryIO) -> None:
    file.seek(1)
    previous = file.read().decode("utf-8", errors="replace")
    try:
        owner = json.loads(previous) if previous.strip() else {}
    except json.JSONDecodeError:
        owner = {}
    pid = owner.get("pid", "Unknown")
    project = owner.get("project", owner.get("workspace", "Unknown"))
    task = owner.get("task", owner.get("command", "Unknown"))
    print(f"Waiting For Build Lock Held By {pid} {project} {task}", flush=True)


# === Entry Point === #

if __name__ == "__main__":
    arguments = sys.argv[1:]
    if arguments[:1] == ["--"]:
        arguments = arguments[1:]
    if not arguments:
        raise SystemExit("pass an existing mise task invocation")
    # Reuse The Tooling Interpreter In Nested Python Commands.
    if arguments[0] == "python":
        arguments[0] = sys.executable
    with lock():
        environment = dict(os.environ)
        environment.pop("UV_PROJECT_ENVIRONMENT", None)
        if not environment.get("CI") and environment.get("RUSTC_WRAPPER") == "sccache" and not environment.get("STKR_LOCAL_COMPILER_CACHE"):
            from ..ci import compiler

            os.environ.update(environment, COMPILER_CACHE_BACKEND="disk", COMPILER_CACHE_MODE="READ_WRITE", STKR_LOCAL_COMPILER_CACHE="1")
            statistics = {}
            try:
                status = compiler.run(arguments, statistics=statistics)
            except compiler.CacheError:
                raise SystemExit(1) from None
            print(f"Local Compiler Cache: {statistics.get('cache_hits')} Hits, {statistics.get('cache_misses')} Misses", flush=True)
            raise SystemExit(status)
        result = subprocess.run(
            arguments,
            env=environment,
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
            check=False,
            creationflags=spawn_flags(),
        )
        raise SystemExit(result.returncode)
