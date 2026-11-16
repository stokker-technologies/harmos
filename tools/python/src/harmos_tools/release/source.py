# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Source."""

# === Imports === #

# Standard Library Imports
import os
import platform
import re
import subprocess
import sys
import tomllib
from functools import cache
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from ..test import archive
from . import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

STABLE = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
RELEASE_WORKFLOW = "release.yml"

# === Types === #

# === Classes === #

# === Public Functions === #


def continuous_integration() -> bool:
    """Whether A CI Runner Executes This Process."""
    return any(os.environ.get(name, "").lower() in {"1", "true"} for name in ("CI", "GITHUB_ACTIONS", "GITLAB_CI"))


def release_tag(directory: Path) -> str:
    """The Stable Tag Of The Prepared Version: `v` Plus `[version] value`."""
    value = policy.policy(directory, "version").get("value")
    if not isinstance(value, str) or re.fullmatch(STABLE, value) is None:
        raise policy.ToolError("version.value must declare a stable MAJOR.MINOR.PATCH version")
    return "v" + value


def native_target(settings: dict) -> str:
    selected = native()
    if selected not in settings.get("targets", []):
        raise policy.ToolError(f"native target {selected} is not one of release.targets")
    return selected


def native() -> str:
    system = {"Linux": "linux", "Windows": "windows", "Darwin": "macos"}.get(platform.system(), "")
    architecture = {"amd64": "x86_64", "arm64": "aarch64"}.get(platform.machine().lower(), platform.machine().lower())
    return f"{system}-{architecture}"


def revision(directory: Path) -> str:
    sha = archive.revision(directory)
    if not sha:
        raise policy.ToolError("release operations require a committed HEAD")
    return sha


def push_remote(settings: dict, token: str) -> str:
    """An Authenticated Push URL For The Release Identity, Which The main Ruleset Admits."""
    if settings["provider"] == "github":
        repository = settings.get("repository", os.environ.get("GITHUB_REPOSITORY"))
        return f"https://x-access-token:{token}@github.com/{repository}.git"
    return f"https://oauth2:{token}@{os.environ['CI_SERVER_HOST']}/{os.environ['CI_PROJECT_PATH']}.git"


def execute(directory: Path, command: list[str], secret: str) -> str:
    """Run A Command That Carries A Credential, Never Echoing It; Returns Its Scrubbed Standard Output."""
    result = subprocess.run(
        command,
        creationflags=spawn_flags(),
        cwd=directory,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    stderr = result.stderr or ""
    stdout = result.stdout or ""
    if secret:
        stderr = stderr.replace(secret, "***")
        stdout = stdout.replace(secret, "***")
    print(stderr, end="", file=sys.stderr)
    if result.returncode:
        raise policy.ToolError(f"{command[0]} {command[1]} failed with exit code {result.returncode}")
    return stdout


@cache
def tracked(directory: Path, *patterns: str) -> list[str]:
    """Tracked paths matching Git pathspec patterns, where `*` also crosses directories."""
    listed = policy.execute(directory, ["git", "ls-files", "-z", "--", *patterns], capture=True)
    return sorted(filter(None, listed.split("\0")))


def task_defined(directory: Path, name: str) -> bool:
    text = (directory / "mise.toml").read_text(encoding="utf-8")
    document = tomllib.loads(text)
    return name in document.get("tasks", {})


# === Private Functions === #

# === Entry Point === #
