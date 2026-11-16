# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Test Execution Operations."""

# === Imports === #

# Standard Library Imports
import os
import signal
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def console() -> dict:
    """Keep Native Console Signaling Available Without A Visible Window."""
    if os.name != "nt":
        return {}
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = subprocess.SW_HIDE
    return {
        "startupinfo": startup,
        "creationflags": spawn_flags(subprocess.CREATE_NEW_CONSOLE),
        "stdin": sys.stdin,
        "stdout": sys.stdout,
        "stderr": sys.stderr,
    }


def execute(command: Sequence[str], cwd: Path, environment: dict[str, str] | None, *, timeout_s: float | None = None) -> int:
    try:
        process = subprocess.Popen(
            list(command), cwd=cwd, env=environment, start_new_session=os.name != "nt" and timeout_s is not None, **console()
        )
        try:
            try:
                return process.wait(timeout=timeout_s)
            except subprocess.TimeoutExpired:
                print(f"{command[0]}: script lane exceeded its {timeout_s}-second hang guard", file=sys.stderr)
                if os.name == "nt":
                    # Run the Native Command
                    command = ["taskkill.exe", "/PID", str(process.pid), "/T", "/F"]
                    subprocess.run(
                        command,
                        check=False,
                        capture_output=True,
                        creationflags=spawn_flags(),
                    )
                else:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                if process.poll() is None:
                    process.kill()
                process.wait()
                return 1
        finally:
            if process.stdout is not None:
                process.stdout.close()
            if process.stderr is not None:
                process.stderr.close()
            if process.stdin is not None:
                process.stdin.close()
            process.wait()
    except OSError as error:
        raise policy.TestError(f"{command[0]} is unavailable: {error}") from error


def measured(command: Sequence[str], cwd: Path, environment: dict[str, str]) -> tuple[int, int | None]:
    """Exit status and peak resident bytes of one process and the children it waited for."""
    try:
        process = subprocess.Popen(list(command), cwd=cwd, env=environment, **console())
    except OSError as error:
        raise policy.TestError(f"{command[0]} is unavailable: {error}") from error
    if not hasattr(os, "wait4"):
        return process.wait(), None
    _, status, usage = os.wait4(process.pid, 0)
    process.returncode = os.waitstatus_to_exitcode(status)
    scale = 1 if sys.platform == "darwin" else 1024
    return process.returncode, usage.ru_maxrss * scale


# === Private Functions === #

# === Entry Point === #
