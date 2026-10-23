# Rendered by stkr project update from templates/python/release/policy.py; sha256:8c4b50fe8ccc5cc449b67b795aa9d2daac00eb493b7ce2bfd9cc988919452c9f
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Policy And Native Execution For Managed Repository Tools"""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import subprocess
import sys
import tomllib
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Any

# Third Party Imports

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class ToolError(Exception):
    """Invalid policy or a failed native operation."""


# === Public Functions === #


def root() -> Path:
    current = Path.cwd().resolve()
    for candidate in (current, *current.parents):
        if (candidate / "stkr.toml").is_file():
            return candidate
    raise ToolError("no stkr.toml found")


def config(directory: Path) -> dict[str, Any]:
    return tomllib.loads((directory / "stkr.toml").read_text(encoding="utf-8"))


def policy(directory: Path, name: str) -> dict[str, Any]:
    value = config(directory).get(name, {})
    if not isinstance(value, dict):
        raise ToolError(f"[{name}] must be a table")
    return value


def profile(default: str = "release") -> str:
    """The Package Profile Passed By The Rendered Job Or A Local Invocation."""
    selected = os.environ.get("RELEASE_PROFILE") or default
    if selected not in ("dev", "release"):
        raise ToolError("RELEASE_PROFILE must be dev or release")
    return selected


def inside(directory: Path, relative: str) -> Path:
    candidate = directory / relative
    if Path(relative).is_absolute() or not candidate.resolve().is_relative_to(directory.resolve()):
        raise ToolError(f"path must stay inside the repository: {relative}")
    return candidate


def command(value: object, name: str) -> list[str]:
    if not isinstance(value, list) or not value or any(not isinstance(part, str) or not part for part in value):
        raise ToolError(f"{name} must be a nonempty argument array")
    return value


def execute(directory: Path, arguments: Sequence[str], *, capture: bool = False) -> str:
    result = subprocess.run(
        list(arguments),
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        cwd=directory,
        check=False,
        text=True,
        encoding="utf-8",
        stdin=sys.stdin,
        stdout=subprocess.PIPE if capture else sys.stdout,
        stderr=subprocess.PIPE if capture else sys.stderr,
    )
    if result.returncode:
        raise ToolError(f"{arguments[0]} failed with exit code {result.returncode}: {result.stderr or ''}")
    return result.stdout or ""


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_symlink():
        raise ToolError(f"refusing linked output: {path}")
    temporary = path.with_suffix(path.suffix + ".tmp")
    if temporary.is_symlink():
        raise ToolError(f"refusing linked staging file: {temporary}")
    temporary.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def main(
    name: str,
    verbs: Sequence[str],
    operation: Callable[[Path, str], None],
    arguments: Sequence[str] | None = None,
) -> int:
    parser = argparse.ArgumentParser(description=f"Managed {name.title()} Tooling")
    parser.add_argument("verb", choices=verbs)
    options = parser.parse_args(arguments)
    try:
        operation(root(), options.verb)
    except (ToolError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"{name}: {error}", file=sys.stderr)
        return 1
    return 0


# === Private Functions === #

# === Entry Point === #
