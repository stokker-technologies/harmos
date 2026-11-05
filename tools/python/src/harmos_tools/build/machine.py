# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Serialize Workstation Native Tasks Across Repositories And Worktrees."""

# === Imports === #

# Standard Library Imports
import json
import os
import subprocess
import sys
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import BinaryIO

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

__all__ = ["lock"]

# === Constants === #

INHERITED = "STKR_NATIVE_BUILD_LOCK"

# === Types === #

# === Classes === #

# === Public Functions === #


@contextmanager
def lock() -> Iterator[None]:
    cache = Path(os.environ.get("LOCALAPPDATA", "")) if os.name == "nt" else Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
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
            holder = {"pid": os.getpid(), "workspace": str(Path.cwd()), "started": datetime.now(UTC).isoformat(), "command": sys.argv}
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


@contextmanager
def _operating_system_lock(file: BinaryIO) -> Iterator[None]:
    if os.name != "nt":
        import fcntl

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
    if not kernel.LockFileEx(handle, 2, 0, 1, 0, ctypes.byref(overlapped)):
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        yield
    finally:
        if not kernel.UnlockFileEx(handle, 0, 1, 0, ctypes.byref(overlapped)):
            raise ctypes.WinError(ctypes.get_last_error())


# === Entry Point === #

if __name__ == "__main__":
    arguments = sys.argv[1:]
    if arguments[:1] == ["--"]:
        arguments = arguments[1:]
    with lock():
        raise SystemExit(subprocess.run(arguments, check=False).returncode)
