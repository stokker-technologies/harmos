# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Prepare."""

# === Imports === #

# Standard Library Imports
import re
from pathlib import Path

# Third Party Imports
# Local Imports
import harmos_tools.release.requirements
import harmos_tools.release.source

from . import policy, screenshots, version

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def planned_version(directory: Path, current: str) -> str:
    """The Current Version Before Its First Tag, Else git-cliff's Configured Automatic Version."""
    if not policy.execute(directory, ["git", "tag", "--list", "v" + current], capture=True).strip():
        return current
    # Breaking changes bump the minor and everything else the patch while the version is 0.x, as tools/config/cliff.toml [bump] declares.
    command = ["git-cliff", "--config", "tools/config/cliff.toml", "--bumped-version"]
    planned = policy.execute(directory, command, capture=True).strip().removeprefix("v")
    if planned == current:
        since = policy.execute(directory, ["git", "log", "--format=%h %s", f"v{current}..HEAD"], capture=True).strip()
        listing = "\n".join("  " + line for line in since.splitlines()) or "  (no commits)"
        raise policy.ToolError(f"v{current} is released and no Conventional Commit since warrants a new version; commits since:\n{listing}")
    return planned


def run(directory: Path, settings: dict) -> None:
    """Bump Every Version Carrier And Regenerate Release Inputs In The Working Tree."""
    if policy.execute(directory, ["git", "status", "--porcelain"], capture=True).strip():
        raise policy.ToolError("prepare requires clean committed source")
    harmos_tools.release.requirements.require_publication(directory, settings)
    current = harmos_tools.release.source.release_tag(directory)[1:]
    planned = planned_version(directory, current)
    if re.fullmatch(harmos_tools.release.source.STABLE, planned) is None:
        raise policy.ToolError(f"the next version is not a stable version: {planned}")
    version.run(directory, "sync", value=planned)
    sync_policy_version(directory, planned)
    commands = settings.get("lock_commands")
    if commands is None:
        commands = []
        if (directory / "Cargo.toml").is_file():
            # Cold runners start with an empty Cargo home, so the workspace-only update may fetch the index.
            commands.append(["cargo", "update", "--workspace"])
        for lock in harmos_tools.release.source.tracked(directory, "*package-lock.json"):
            commands.append(["npm", "install", "--package-lock-only", "--ignore-scripts", "--prefix", str(directory / Path(lock).parent)])
        for lock in harmos_tools.release.source.tracked(directory, "*pnpm-lock.yaml"):
            commands.append(["pnpm", "install", "--lockfile-only", "--ignore-scripts", "--dir", str(directory / Path(lock).parent)])
        for lock in harmos_tools.release.source.tracked(directory, "*uv.lock"):
            commands.append(["uv", "lock", "--offline", "--project", str(directory / Path(lock).parent)])
    for command in commands:
        policy.execute(directory, policy.command(command, "release.lock_commands"))
    if policy.policy(directory, "screenshots").get("scenes"):
        screenshots.capture(directory)
    print(f"Prepared v{planned}; review the changes, or let Release / Prepare open the release review")


def sync_policy_version(directory: Path, value: str) -> None:
    """Write `[version] value` In The Project Policy, Keeping Every Other Line."""
    path = directory / "tools/project.toml"
    text = path.read_text(encoding="utf-8")
    match = re.search(r"(?ms)(^\[version\]\s*\n)(.*?)(?=^\[|\Z)", text)
    if match is None:
        raise policy.ToolError("prepare requires [version] policy")
    body = re.sub(r"(?m)^value\s*=.*$", f'value = "{value}"', match.group(2), count=1)
    path.write_text(text[: match.start()] + match.group(1) + body + text[match.end() :], encoding="utf-8")


# === Private Functions === #

# === Entry Point === #
