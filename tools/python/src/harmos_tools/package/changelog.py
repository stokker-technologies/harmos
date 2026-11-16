# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Stage Generated Release Notes For Distribution"""

# === Imports === #

# Standard Library Imports
import json
import re
import tomllib
from pathlib import Path

# Third Party Imports
# Local Imports
from ..release import policy

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class StagedNotes:
    """Stage Generated Crate Notes and Restore Every Input on close()."""

    def __init__(self, directory: Path, packages: dict) -> None:
        self.originals: dict[Path, bytes | None] = {}
        try:
            for package in packages.values():
                manifest = policy.inside(directory, Path(package["manifest_path"]).relative_to(directory).as_posix())
                original = manifest.read_bytes()
                # Read Structured Input
                text = original.decode("utf-8")
                document = tomllib.loads(text)
                changelog = manifest.parent / "CHANGELOG.md"
                if changelog.is_symlink():
                    raise policy.ToolError("refusing linked crate changelog")
                self.originals[changelog] = changelog.read_bytes() if changelog.exists() else None
                changelog.write_text(notes(directory, "v" + package["version"]), encoding="utf-8", newline="\n")

                # Preserve Cargo's selected source inventory, including excludes and ignore rules.
                # Only the generated changelog is added to that inventory.
                section = document.get("package", {})
                include = section.get("include")
                if include is None:
                    command = ["cargo", "package", "--list", "--allow-dirty", "--package", package["name"]]
                    listing = policy.execute(directory, command, capture=True)
                    include = [name.replace("\\", "/") for name in listing.splitlines() if name != "Cargo.toml.orig"]
                include = [*include, "CHANGELOG.md"] if "CHANGELOG.md" not in include else include
                text = original.decode("utf-8")
                match = re.search(r"(?ms)^\[package\]\s*\n(.*?)(?=^\[|\Z)", text)
                if match is None:
                    raise policy.ToolError("crate has no package table")
                body = match.group(1)
                body = re.sub(r"(?ms)^include\s*=\s*\[.*?\][^\n]*\n?", "", body)
                body = re.sub(r"(?ms)^exclude\s*=\s*\[.*?\][^\n]*\n?", "", body)
                body = "include = " + json.dumps(include) + "\n" + body
                self.originals[manifest] = original
                manifest.write_text(text[: match.start(1)] + body + text[match.end(1) :], encoding="utf-8", newline="")
        except BaseException:
            self.close()
            raise

    def close(self) -> None:
        for path, data in reversed(list(self.originals.items())):
            if data is None:
                path.unlink(missing_ok=True)
            else:
                path.write_bytes(data)


# === Public Functions === #


def notes(directory: Path, tag: str, *, tagged: bool | None = None) -> str:
    """Render Only This Release's Changes, Never A Cumulative Tracked Changelog."""
    if tagged is None:
        tags = policy.execute(directory, ["git", "tag", "--points-at", "HEAD"], capture=True).splitlines()
        tagged = tag in tags
    command = ["git-cliff", "--config", "tools/config/cliff.toml", "--strip", "header"]
    command += ["--current"] if tagged else ["--unreleased", "--tag", tag]
    return policy.execute(directory, command, capture=True).strip() + "\n"


# === Private Functions === #

# === Entry Point === #
